// Copyright (C) 2020-2021 Parity Technologies (UK) Ltd. and Centrality Investment Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use std::sync::Arc;

use codec::{Codec, Decode, Encode};
use futures::{future, FutureExt, StreamExt};
use log::{debug, error, trace, warn};
use parking_lot::Mutex;

use sc_client_api::{Backend, FinalityNotification, FinalityNotifications};
use sc_network_gossip::GossipEngine;

use sp_api::BlockId;
use sp_runtime::{
	generic::OpaqueDigestItemId,
	traits::{Block, Header, NumberFor},
};

use crate::{
	gossip::{topic, GossipValidator},
	keystore::{EthyEcdsaToEthereum, EthyKeystore},
	metric_inc, metric_set,
	metrics::Metrics,
	notification,
	witness_record::WitnessRecord,
	Client,
};
use cennznet_primitives::eth::{
	crypto::{AuthorityId as Public, AuthoritySignature as Signature},
	ConsensusLog, EthyApi, EventId, EventProof, Message, ValidatorSet, VersionedEventProof, Witness, ETHY_ENGINE_ID,
	GENESIS_AUTHORITY_SET_ID,
};
use crml_support::EthAbiCodec;

/// % signature to generate a proof
const PROOF_THRESHOLD: f32 = 0.6;

pub(crate) struct WorkerParams<B, BE, C>
where
	B: Block,
{
	pub client: Arc<C>,
	pub backend: Arc<BE>,
	pub key_store: EthyKeystore,
	pub event_proof_sender: notification::EthyEventProofSender,
	pub gossip_engine: GossipEngine<B>,
	pub gossip_validator: Arc<GossipValidator<B>>,
	pub metrics: Option<Metrics>,
}

/// A ETHY worker plays the ETHY protocol
pub(crate) struct EthyWorker<B, C, BE>
where
	B: Block,
	BE: Backend<B>,
	C: Client<B, BE>,
{
	client: Arc<C>,
	backend: Arc<BE>,
	key_store: EthyKeystore,
	event_proof_sender: notification::EthyEventProofSender,
	gossip_engine: Arc<Mutex<GossipEngine<B>>>,
	gossip_validator: Arc<GossipValidator<B>>,
	metrics: Option<Metrics>,
	finality_notifications: FinalityNotifications<B>,
	/// Tracks on-going witnesses
	witness_record: WitnessRecord,
	/// Best block we received a GRANDPA notification for
	best_grandpa_block: NumberFor<B>,
	/// Current validator set
	validator_set: ValidatorSet<Public>,
	/// Validator set id for the last signed witness
	last_signed_id: u64,
}

impl<B, C, BE> EthyWorker<B, C, BE>
where
	B: Block + Codec,
	BE: Backend<B>,
	C: Client<B, BE>,
	C::Api: EthyApi<B>,
{
	/// Return a new ETHY worker instance.
	///
	/// Note that a ETHY worker is only fully functional if a corresponding
	/// ETHY pallet has been deployed on-chain.
	///
	/// The ETHY pallet is needed in order to keep track of the ETHY authority set.
	pub(crate) fn new(worker_params: WorkerParams<B, BE, C>) -> Self {
		let WorkerParams {
			client,
			backend,
			key_store,
			event_proof_sender,
			gossip_engine,
			gossip_validator,
			metrics,
		} = worker_params;

		EthyWorker {
			client: client.clone(),
			backend,
			key_store,
			event_proof_sender,
			gossip_engine: Arc::new(Mutex::new(gossip_engine)),
			gossip_validator,
			metrics,
			finality_notifications: client.finality_notification_stream(),
			best_grandpa_block: client.info().finalized_number,
			last_signed_id: 0,
			validator_set: ValidatorSet {
				id: 0,
				validators: Default::default(),
			},
			witness_record: Default::default(),
		}
	}
}

impl<B, C, BE> EthyWorker<B, C, BE>
where
	B: Block,
	BE: Backend<B>,
	C: Client<B, BE>,
	C::Api: EthyApi<B>,
{
	/// Return the current active validator set at header `header`.
	///
	/// Note that the validator set could be `None`. This is the case if we don't find
	/// a ETHY authority set change and we can't fetch the authority set from the
	/// ETHY on-chain state.
	///
	/// Such a failure is usually an indication that the ETHY pallet has not been deployed (yet).
	fn validator_set(&self, header: &B::Header) -> Option<ValidatorSet<Public>> {
		let new = if let Some(new) = find_authorities_change::<B, Public>(header) {
			Some(new)
		} else {
			let at = BlockId::hash(header.hash());
			// queries the BEEFY pallet to get the active validator set public keys
			self.client.runtime_api().validator_set(&at).ok()
		};

		trace!(target: "ethy", "💎 active validator set: {:?}", new);

		new
	}

	// For Ethy this would be a notification from something polling Ethereum full nodes
	fn handle_finality_notification(&mut self, notification: FinalityNotification<B>) {
		// TODO: this will only be called when grandpa finalizes at a new block/checkpoint
		// grandpa does not finalize individual blocks.
		// we need to backtrack to find requests in all blocks since the last finalization and start signing them
		trace!(target: "ethy", "💎 finality notification for block #{:?}", &notification.header.number());

		if let Some(active) = self.validator_set(&notification.header) {
			// Authority set change or genesis set id triggers new voting rounds
			// this block has a different validator set id to the one we know about OR
			// it's the first block
			if active.id != self.validator_set.id
				|| (active.id == GENESIS_AUTHORITY_SET_ID && self.validator_set.validators.is_empty())
			{
				debug!(target: "ethy", "💎 new active validator set: {:?}", active);
				debug!(target: "ethy", "💎 old validator set: {:?}", self.validator_set);
				metric_set!(self, ethy_validator_set_id, active.id);
				self.gossip_validator.set_active_validators(active.validators.clone());
				self.validator_set = active;
			}
		}

		let authority_id = if let Some(id) = self.key_store.authority_id(self.validator_set.validators.as_slice()) {
			trace!(target: "ethy", "💎 Local authority id: {:?}", id);
			id
		} else {
			trace!(target: "ethy", "💎 Missing validator id - can't vote for: {:?}", notification.header.hash());
			return;
		};

		// Search from (self.best_grandpa_block - notification.block) to find all signing requests
		// Sign and broadcast a witness
		for (message, event_id) in extract_proof_requests::<B>(&notification.header).iter() {
			warn!(target: "ethy", "💎 got event proof request. event id: {:?}, message: {:?}", event_id, message);
			// `message = abi.encodePacked(param0, param1,.., paramN, nonce)`
			let signature = match self.key_store.sign(&authority_id, message.as_ref()) {
				Ok(sig) => sig,
				Err(err) => {
					error!(target: "ethy", "💎 error signing witness: {:?}", err);
					return;
				}
			};
			let witness = Witness {
				digest: sp_core::keccak_256(message.as_ref()),
				event_id: *event_id,
				authority_id: authority_id.clone(),
				signature,
			};
			let broadcast_witness = witness.encode();

			metric_inc!(self, ethy_votes_sent);
			debug!(target: "ethy", "💎 Sent witness: {:?}", witness);

			// process the witness
			self.handle_witness(witness.clone());

			// broadcast the witness
			self.gossip_engine
				.lock()
				.gossip_message(topic::<B>(), broadcast_witness, false);
			debug!(target: "ethy", "💎 gossiped witness for event: {:?}", witness.event_id);
		}

		self.best_grandpa_block = *notification.header.number();
	}

	/// Note an individual witness for a message
	/// If the witness means consensus is reached on a message then;
	/// 1) Assemble the aggregated witness
	/// 2) Add justification in DB
	/// 3) Broadcast the witness to listeners
	fn handle_witness(&mut self, witness: Witness) {
		// The aggregated signed witness here could be different to another validators.
		// As long as we have threshold of signatures the proof is valid.
		warn!(target: "ethy", "💎 got witness: {:?}", witness);
		self.witness_record.note(&witness);

		// metric_set!(self, ethy_round_concluded, round.1);
		// info!(target: "ethy", "💎 Round #{} concluded, committed: {:?}.", round.1, event_proof);

		let threshold = self.validator_set.validators.len() as f32 * PROOF_THRESHOLD;
		if self
			.witness_record
			.has_consensus(witness.event_id, &witness.digest, threshold as usize)
		{
			let signatures = self.witness_record.signatures_for(
				witness.event_id,
				&witness.digest,
				self.validator_set.validators.clone(),
			);
			warn!(target: "ethy", "💎 generating proof for event: {:?}, signatures: {:?}", witness.event_id, signatures);
			let event_proof = EventProof {
				digest: witness.digest,
				event_id: witness.event_id,
				validator_set_id: self.validator_set.id,
				signatures,
			};
			// We can add proof to the DB that this block has been finalized specifically by the
			// given threshold of validators
			if Backend::insert_aux(
				self.backend.as_ref(),
				&[
					// DB key is (engine_id + proof_id)
					(
						[&ETHY_ENGINE_ID[..], &event_proof.event_id.to_be_bytes()[..]]
							.concat()
							.as_ref(),
						VersionedEventProof::V1(event_proof.clone()).encode().as_ref(),
					),
				],
				&[],
			)
			.is_err()
			{
				// this is a warning for now, because until the round lifecycle is improved, we will
				// conclude certain rounds multiple times.
				warn!(target: "ethy", "💎 failed to store witness: {:?}", event_proof);
			}
			// Notify an subscribers that we've got a witness for a new message e.g. open RPC subscriptions
			self.event_proof_sender.notify(event_proof);
			// Remove from memory
			self.witness_record.clear(witness.event_id);
			self.gossip_validator.mark_complete(witness.event_id);
		} else {
			trace!(target: "ethy", "💎 no consensus yet for event: {:?}", witness.event_id);
		}
	}

	pub(crate) async fn run(mut self) {
		let mut witnesses = Box::pin(self.gossip_engine.lock().messages_for(topic::<B>()).filter_map(
			|notification| async move {
				trace!(target: "ethy", "💎 got witness: {:?}", notification);

				Witness::decode(&mut &notification.message[..]).ok()
			},
		));

		loop {
			let engine = self.gossip_engine.clone();
			let gossip_engine = future::poll_fn(|cx| engine.lock().poll_unpin(cx));

			futures::select! {
				notification = self.finality_notifications.next().fuse() => {
					if let Some(notification) = notification {
						self.handle_finality_notification(notification);
					} else {
						return;
					}
				},
				witness = witnesses.next().fuse() => {
					if let Some(witness) = witness {
						self.handle_witness(witness);
					} else {
						return;
					}
				},
				_ = gossip_engine.fuse() => {
					error!(target: "ethy", "💎 Gossip engine has terminated.");
					return;
				}
			}
		}
	}
}

/// Extract event proof requests from a digest in the given header, if any.
fn extract_proof_requests<B>(header: &B::Header) -> Vec<(Message, EventId)>
where
	B: Block,
{
	header
		.digest()
		.logs()
		.iter()
		.flat_map(|log| {
			let res: Option<(Vec<u8>, EventId)> =
				match log.try_to::<ConsensusLog<Public>>(OpaqueDigestItemId::Consensus(&ETHY_ENGINE_ID)) {
					Some(ConsensusLog::OpaqueSigningRequest(r)) => Some(r),
					// Note: we also handle this in `find_authorities_change` to update the validator set
					// here we want to convert it into an 'OpaqueSigningRequest` to create a proof of the validator set change
					// we must do this before the validators officially change next session (~10 minutes)
					Some(ConsensusLog::PendingAuthoritiesChange((validator_set, event_id))) => {
						use sp_runtime::traits::Convert;
						// Convert the validator ECDSA pub keys to addresses and `abi.encodePacked()` them
						// + the `event_id`
						let mut message = Vec::with_capacity(32 * (validator_set.validators.len() + 2));
						let extra_idx = 32 * (validator_set.validators.len());
						for (idx, ecda_pubkey) in validator_set.validators.into_iter().enumerate() {
							message[(idx * 32) + 12..].copy_from_slice(&EthyEcdsaToEthereum::convert(ecda_pubkey)[..]);
						}
						message[extra_idx..extra_idx + 32]
							.copy_from_slice(EthAbiCodec::encode(&validator_set.id).as_slice());
						message[extra_idx + 32..].copy_from_slice(EthAbiCodec::encode(&event_id).as_slice());

						Some((message, event_id))
					}
					_ => None,
				};
			res
		})
		.collect()
}

/// Scan the `header` digest log for a ETHY validator set change. Return either the new
/// validator set or `None` in case no validator set change has been signaled.
fn find_authorities_change<B, Id>(header: &B::Header) -> Option<ValidatorSet<Id>>
where
	B: Block,
	Id: Codec,
{
	let id = OpaqueDigestItemId::Consensus(&ETHY_ENGINE_ID);

	let filter = |log: ConsensusLog<Id>| match log {
		ConsensusLog::AuthoritiesChange(validator_set) => Some(validator_set),
		_ => None,
	};

	header.digest().convert_first(|l| l.try_to(id).and_then(filter))
}
