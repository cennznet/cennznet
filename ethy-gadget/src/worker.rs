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

use std::{convert::TryInto, sync::Arc};

use codec::{Codec, Decode, Encode};
use futures::{future, FutureExt, StreamExt};
use log::{debug, error, info, trace, warn};
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
	crypto::AuthorityId as Public, ConsensusLog, EthyApi, EventId, EventProof, ValidatorSet, ValidatorSetId,
	VersionedEventProof, Witness, ETHY_ENGINE_ID, GENESIS_AUTHORITY_SET_ID,
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
				self.witness_record.set_validators(active.validators.clone());
				self.validator_set = active;
			}
		}

		let authority_id = if let Some(id) = self.key_store.authority_id(self.validator_set.validators.as_slice()) {
			trace!(target: "ethy", "💎 Local authority id: {:?}", id);
			id
		} else {
			trace!(target: "ethy", "💎 No authority id - can't vote for events in: {:?}", notification.header.hash());
			for ProofRequest {
				message: _,
				event_id,
				tag,
				block,
			} in extract_proof_requests::<B>(&notification.header, self.validator_set.id).into_iter()
			{
				trace!(target: "ethy", "💎 noting event metadata: {:?}", event_id);
				// it's possible this event already has a proof stored due to differences in block
				// propagation times.
				// update the proof block hash and tag
				let proof_key = [&ETHY_ENGINE_ID[..], &event_id.to_be_bytes()[..]].concat();

				if let Ok(Some(encoded_proof)) = Backend::get_aux(self.backend.as_ref(), proof_key.as_ref()) {
					if let Ok(VersionedEventProof::V1 { 0: mut proof }) =
						VersionedEventProof::decode(&mut &encoded_proof[..])
					{
						proof.block = block;
						proof.tag = tag;

						if Backend::insert_aux(
							self.backend.as_ref(),
							&[
								// DB key is (engine_id + proof_id)
								(
									[&ETHY_ENGINE_ID[..], &event_id.to_be_bytes()[..]]
										.concat()
										.as_ref(),
										VersionedEventProof::V1(proof).encode().as_ref(),
								),
							],
							&[],
						)
						.is_err()
						{
							// this is a warning for now, because until the round lifecycle is improved, we will
							// conclude certain rounds multiple times.
							error!(target: "ethy", "💎 failed to store proof: {:?}", event_id);
						}
					} else {
						error!(target: "ethy", "💎 failed decoding event proof v1: {:?}", event_id);
					}
				} else {
					// no proof is known for this event yet
					self.witness_record.note_event_metadata(event_id, block, tag);
				}
			}

			// full node can't vote, we're done
			return;
		};

		// Search from (self.best_grandpa_block - notification.block) to find all signing requests
		// Sign and broadcast a witness
		for ProofRequest {
			message,
			event_id,
			tag,
			block,
		} in extract_proof_requests::<B>(&notification.header, self.validator_set.id).into_iter()
		{
			debug!(target: "ethy", "💎 got event proof request. event id: {:?}, message: {:?}", event_id, hex::encode(&message));
			// `message = abi.encode(param0, param1,.., paramN, nonce)`
			let signature = match self.key_store.sign(&authority_id, message.as_ref()) {
				Ok(sig) => sig,
				Err(err) => {
					error!(target: "ethy", "💎 error signing witness: {:?}", err);
					return;
				}
			};
			debug!(target: "ethy", "💎 signed event id: {:?}, validator set: {:?},\nsignature: {:?}", event_id, self.validator_set.id, hex::encode(&signature));
			let witness = Witness {
				digest: sp_core::keccak_256(message.as_ref()),
				validator_set_id: self.validator_set.id,
				event_id,
				authority_id: authority_id.clone(),
				signature,
			};
			let broadcast_witness = witness.encode();

			metric_inc!(self, ethy_witness_sent);
			debug!(target: "ethy", "💎 Sent witness: {:?}", witness);

			// process the witness
			self.witness_record.note_event_metadata(event_id, block, tag);
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
	/// 1) Assemble the aggregated witness (proof)
	/// 2) Add proof to DB
	/// 3) Notify listeners of the proof
	fn handle_witness(&mut self, witness: Witness) {
		// The aggregated signed witness here could be different to another validators.
		// As long as we have threshold of signatures the proof is valid.
		info!(target: "ethy", "💎 got witness: {:?}", witness);

		// only share if it's the first time witnessing the event
		let first_observation = self.witness_record.note(&witness);
		if !first_observation {
			return;
		}

		self.gossip_engine
			.lock()
			.gossip_message(topic::<B>(), witness.encode(), false);

		let threshold = self.validator_set.validators.len() as f32 * PROOF_THRESHOLD;
		if self
			.witness_record
			.has_consensus(witness.event_id, &witness.digest, threshold as usize)
		{
			let signatures = self.witness_record.signatures_for(witness.event_id, &witness.digest);
			info!(target: "ethy", "💎 generating proof for event: {:?}, signatures: {:?}, validator set: {:?}", witness.event_id, signatures, self.validator_set.id);

			let (block, tag) = self
				.witness_record
				.event_metadata(witness.event_id)
				.unwrap_or(&([0_u8; 32], None));

			let event_proof = EventProof {
				digest: witness.digest,
				event_id: witness.event_id,
				validator_set_id: self.validator_set.id,
				block: *block,
				tag: tag.clone(),
				signatures,
			};
			let versioned_event_proof = VersionedEventProof::V1(event_proof.clone());

			// Add proof to the DB that this event has been notarized specifically by the
			// given threshold of validators
			if Backend::insert_aux(
				self.backend.as_ref(),
				&[
					// DB key is (engine_id + proof_id)
					(
						[&ETHY_ENGINE_ID[..], &event_proof.event_id.to_be_bytes()[..]]
							.concat()
							.as_ref(),
						versioned_event_proof.encode().as_ref(),
					),
				],
				&[],
			)
			.is_err()
			{
				// this is a warning for now, because until the round lifecycle is improved, we will
				// conclude certain rounds multiple times.
				warn!(target: "ethy", "💎 failed to store proof: {:?}", event_proof);
			}
			// Notify an subscribers that we've got a witness for a new message e.g. open RPC subscriptions
			self.event_proof_sender.notify(versioned_event_proof);
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

pub struct ProofRequest {
	/// raw message for signing
	message: Vec<u8>,
	/// nonce/event Id of this request
	event_id: EventId,
	/// metadata tag about the proof
	tag: Option<Vec<u8>>,
	/// Block hash whe  proof was requested
	block: [u8; 32],
}
/// Extract event proof requests from a digest in the given header, if any.
/// Returns (digest for signing, event id, optional tag)
fn extract_proof_requests<B>(header: &B::Header, active_validator_set_id: ValidatorSetId) -> Vec<ProofRequest>
where
	B: Block,
{
	let block_hash = header.hash().as_ref().try_into().unwrap_or_default();
	header
		.digest()
		.logs()
		.iter()
		.flat_map(|log| {
			let res: Option<ProofRequest> =
				match log.try_to::<ConsensusLog<Public>>(OpaqueDigestItemId::Consensus(&ETHY_ENGINE_ID)) {
					Some(ConsensusLog::OpaqueSigningRequest((message, event_id))) => Some(ProofRequest {
						message,
						event_id,
						tag: None,
						block: block_hash,
					}),
					// Note: we also handle this in `find_authorities_change` to update the validator set
					// here we want to convert it into an 'OpaqueSigningRequest` to create a proof of the validator set change
					// we must do this before the validators officially change next session (~10 minutes)
					Some(ConsensusLog::PendingAuthoritiesChange((next_validator_set, event_id))) => {
						let message =
							abi_encode_validator_set_change(&next_validator_set, active_validator_set_id, event_id);
						Some(ProofRequest {
							message,
							event_id,
							tag: Some(b"sys:authority-change".to_vec()),
							block: block_hash,
						})
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

/// Ethereum ABI encode a validator set change message
fn abi_encode_validator_set_change(
	next_validator_set: &ValidatorSet<Public>,
	active_validator_set_id: ValidatorSetId,
	event_id: EventId,
) -> Vec<u8> {
	use sp_runtime::traits::Convert;

	// ethereum ABI encode the data
	// https://docs.soliditylang.org/en/develop/abi-spec.html#use-of-dynamic-types
	// types: 'address[]', 'uint', 'uint'
	// header: v_offset, v_id, e_id, v_length, address0,.. addressN
	let validator_count = next_validator_set.validators.len();
	let word_size = 32;
	// need 5 + validator count words to encode
	// - next validator addresses offset
	// - next validator set id
	// - active validator set id (the witnesses)
	// - event id
	// - validator address length
	// - validators addresses x `validator_count`
	let encoded_words = 5 + validator_count;
	let mut message = vec![0_u8; word_size * encoded_words];
	let mut offset = 0;

	// build header section
	// 1) offset for validator address data (4 * word_size)
	message[offset..offset + word_size].copy_from_slice(EthAbiCodec::encode(&(4 * word_size as u64)).as_slice());
	offset += word_size;
	// 2) encode next validator set id
	message[offset..offset + word_size].copy_from_slice(EthAbiCodec::encode(&next_validator_set.id).as_slice());
	offset += word_size;
	// 3) encode current validator set id (witnesses)
	message[offset..offset + word_size].copy_from_slice(EthAbiCodec::encode(&active_validator_set_id).as_slice());
	offset += word_size;
	// 4) encode event id
	message[offset..offset + word_size].copy_from_slice(EthAbiCodec::encode(&event_id).as_slice());
	offset += word_size;
	// end header section

	// start data section
	// encode validators length prefix + addresses
	message[offset..offset + word_size].copy_from_slice(EthAbiCodec::encode(&(validator_count as u64)).as_slice());
	offset += word_size;
	// Convert the validator ECDSA pub keys to addresses and `abi.encode()` them
	for ecdsa_pubkey in next_validator_set.validators.clone().into_iter() {
		// 0-12 should be 0 padded
		// 12-32 contain the address bytes
		message[offset + 12..offset + word_size].copy_from_slice(&EthyEcdsaToEthereum::convert(ecdsa_pubkey)[..]);
		offset += word_size;
	}
	// end data section

	message
}

#[cfg(test)]
mod test {
	use super::*;
	use sp_core::Public as PublicT;

	#[test]
	fn encode_validator_set_change() {
		let abi_encoded = abi_encode_validator_set_change(
			&ValidatorSet::<Public> {
				validators: vec![
					Public::from_slice(
						// `//Alice` ECDSA public key
						&hex::decode(b"0204dad6fc9c291c68498de501c6d6d17bfe28aee69cfbf71b2cc849caafcb0159").unwrap(),
					),
					Public::from_slice(
						// `//Alice` ECDSA public key
						&hex::decode(b"0204dad6fc9c291c68498de501c6d6d17bfe28aee69cfbf71b2cc849caafcb0159").unwrap(),
					),
				],
				id: 598,
			},
			599,
			1_234_567,
		);
		assert_eq!(
			hex::encode(abi_encoded),
			"000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000002560000000000000000000000000000000000000000000000000000000000000257000000000000000000000000000000000000000000000000000000000012d687000000000000000000000000000000000000000000000000000000000000000200000000000000000000000058dad74c38e9c4738bf3471f6aac6124f862faf500000000000000000000000058dad74c38e9c4738bf3471f6aac6124f862faf5"
		);
	}
}
