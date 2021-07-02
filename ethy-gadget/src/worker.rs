// Copyright (C) 2020-2021 Parity Technologies (UK) Ltd.
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

use std::{fmt::Debug, marker::PhantomData, sync::Arc};

use codec::{Codec, Decode, Encode};
use futures::{future, FutureExt, StreamExt};
use log::{debug, error, info, trace, warn};
use parking_lot::Mutex;

use sc_client_api::{Backend, FinalityNotification, FinalityNotifications};
use sc_network_gossip::GossipEngine;

use sp_api::BlockId;
use sp_arithmetic::traits::AtLeast32Bit;
use sp_runtime::{
	generic::OpaqueDigestItemId,
	traits::{Block, Header, NumberFor},
	SaturatedConversion,
};

use ethy_primitives::{
	crypto::{Public, Signature},
	EthyApi, Commitment, ConsensusLog, MmrRootHash, SignedCommitment, ValidatorSet, VersionedCommitment, VoteMessage,
	ETHY_ENGINE_ID, GENESIS_AUTHORITY_SET_ID,
};

use crate::{
	gossip::{topic, GossipValidator},
	keystore::EthyKeystore,
	metric_inc, metric_set,
	metrics::Metrics,
	notification, round, Client,
};

pub(crate) struct WorkerParams<B, BE, C>
where
	B: Block,
{
	pub client: Arc<C>,
	pub backend: Arc<BE>,
	pub key_store: EthyKeystore,
	pub signed_commitment_sender: notification::EthySignedCommitmentSender<B>,
	pub gossip_engine: GossipEngine<B>,
	pub gossip_validator: Arc<GossipValidator<B>>,
	pub min_block_delta: u32,
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
	signed_commitment_sender: notification::EthySignedCommitmentSender<B>,
	gossip_engine: Arc<Mutex<GossipEngine<B>>>,
	gossip_validator: Arc<GossipValidator<B>>,
	/// Min delta in block numbers between two blocks, ETHY should vote on
	min_block_delta: u32,
	metrics: Option<Metrics>,
	rounds: round::Rounds<MmrRootHash, NumberFor<B>>,
	finality_notifications: FinalityNotifications<B>,
	/// Best block we received a GRANDPA notification for
	best_grandpa_block: NumberFor<B>,
	/// Best block a ETHY voting round has been concluded for
	best_ethy_block: Option<NumberFor<B>>,
	/// Validator set id for the last signed commitment
	last_signed_id: u64,
	// keep rustc happy
	_backend: PhantomData<BE>,
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
			signed_commitment_sender,
			gossip_engine,
			gossip_validator,
			min_block_delta,
			metrics,
		} = worker_params;

		EthyWorker {
			client: client.clone(),
			backend,
			key_store,
			signed_commitment_sender,
			gossip_engine: Arc::new(Mutex::new(gossip_engine)),
			gossip_validator,
			min_block_delta,
			metrics,
			rounds: round::Rounds::new(ValidatorSet::empty()),
			finality_notifications: client.finality_notification_stream(),
			best_grandpa_block: client.info().finalized_number,
			best_ethy_block: None,
			last_signed_id: 0,
			_backend: PhantomData,
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
	/// Return `true`, if we should vote on block `number`
	fn should_vote_on(&self, number: NumberFor<B>) -> bool {
		let best_ethy_block = if let Some(block) = self.best_ethy_block {
			block
		} else {
			debug!(target: "ethy", "💎 Missing best ETHY block - won't vote for: {:?}", number);
			return false;
		};

		let target = vote_target(self.best_grandpa_block, best_ethy_block, self.min_block_delta);

		trace!(target: "ethy", "💎 should_vote_on: #{:?}, next_block_to_vote_on: #{:?}", number, target);

		metric_set!(self, ethy_should_vote_on, target);

		number == target
	}

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
			self.client.runtime_api().validator_set(&at).ok();
		};

		trace!(target: "ethy", "💎 active validator set: {:?}", new);

		new
	}

	// For Ethy this would be a notification from something polling Ethereum full nodes
	fn handle_finality_notification(&mut self, notification: FinalityNotification<B>) {
		trace!(target: "ethy", "💎 Finality notification: {:?}", notification);

		// update best GRANDPA finalized block we have seen
		self.best_grandpa_block = *notification.header.number();

		if let Some(active) = self.validator_set(&notification.header) {
			// Authority set change or genesis set id triggers new voting rounds
			//
			// TODO: (adoerr) Enacting a new authority set will also implicitly 'conclude'
			// the currently active ETHY voting round by starting a new one. This is
			// temporary and needs to be replaced by proper round life cycle handling.

			// this block has a different validator set id to the one we know about OR
			// it's the first block
			if active.id != self.rounds.validator_set_id()
				|| (active.id == GENESIS_AUTHORITY_SET_ID && self.best_ethy_block.is_none())
			{
				debug!(target: "ethy", "💎 New active validator set id: {:?}", active);
				metric_set!(self, ethy_validator_set_id, active.id);

				// ETHY should produce a signed commitment for each session
				if active.id != self.last_signed_id + 1 && active.id != GENESIS_AUTHORITY_SET_ID {
					metric_inc!(self, ethy_skipped_sessions);
				}

				self.rounds = round::Rounds::new(active.clone());

				debug!(target: "ethy", "💎 New Rounds for id: {:?}", active.id);

				self.best_ethy_block = Some(*notification.header.number());

				// this metric is kind of 'fake'. Best ETHY block should only be updated once we have a
				// signed commitment for the block. Remove once the above TODO is done.
				metric_set!(self, ethy_best_block, *notification.header.number());
			}
		}

		if self.should_vote_on(*notification.header.number()) {
			let authority_id = if let Some(id) = self.key_store.authority_id(self.rounds.validators().as_slice()) {
				trace!(target: "ethy", "💎 Local authority id: {:?}", id);
				id
			} else {
				trace!(target: "ethy", "💎 Missing validator id - can't vote for: {:?}", notification.header.hash());
				return;
			};

			let mmr_root = if let Some(hash) = find_mmr_root_digest::<B, Public>(&notification.header) {
				hash
			} else {
				warn!(target: "ethy", "💎 No MMR root digest found for: {:?}", notification.header.hash());
				return;
			};

			let commitment = Commitment {
				payload: mmr_root,
				block_number: notification.header.number(),
				validator_set_id: self.rounds.validator_set_id(),
			};

			let signature = match self.key_store.sign(&authority_id, commitment.encode().as_ref()) {
				Ok(sig) => sig,
				Err(err) => {
					warn!(target: "ethy", "💎 Error signing commitment: {:?}", err);
					return;
				}
			};

			let message = VoteMessage {
				commitment,
				id: authority_id,
				signature,
			};

			let encoded_message = message.encode();

			metric_inc!(self, ethy_votes_sent);

			debug!(target: "ethy", "💎 Sent vote message: {:?}", message);

			self.handle_vote(
				(message.commitment.payload, *message.commitment.block_number),
				(message.id, message.signature),
			);

			self.gossip_engine
				.lock()
				.gossip_message(topic::<B>(), encoded_message, false);
		}
	}

	fn handle_vote(&mut self, round: (MmrRootHash, NumberFor<B>), vote: (Public, Signature)) {
		self.gossip_validator.note_round(round.1);

		let vote_added = self.rounds.add_vote(round, vote);

		if vote_added && self.rounds.is_done(&round) {
			if let Some(signatures) = self.rounds.drop(&round) {
				// signatures.len() == validator_set.len()
				// let validators = [0, 1, 2, 3];
				// let sigs = [None, Some(sig1), None, Some(sig(3))];
				// i-th signature is None if the i-th validator did not sign (or was not received)
				// i-th signature is Some(sig) if the i-th validator signed

				// id is stored for skipped session metric calculation
				self.last_signed_id = self.rounds.validator_set_id();

				let commitment = Commitment {
					payload: round.0, // mmr root hash [0x0, 0x1, 0x2] -> 0xa (this will be an Ethereum block hash)
					block_number: round.1, // block number where hash the mmr root applies
					validator_set_id: self.last_signed_id,
				};

				// TODO: The signed commitment here could be different to another validators
				// does it matter?
				// - either justifications are allowed to be different (do some research on this)
				// - or we're assuming they will always be the same (can't assume this)
				let signed_commitment = SignedCommitment { commitment, signatures };

				metric_set!(self, ethy_round_concluded, round.1);

				info!(target: "ethy", "💎 Round #{} concluded, committed: {:?}.", round.1, signed_commitment);

				// We can add proof to the DB that this block has been finalized specifically by the
				// given threshold of validators
				if self
					.backend
					.append_justification(
						BlockId::Number(round.1),
						(
							ETHY_ENGINE_ID,
							VersionedCommitment::V1(signed_commitment.clone()).encode(),
						),
					)
					.is_err()
				{
					// this is a warning for now, because until the round lifecycle is improved, we will
					// conclude certain rounds multiple times.
					warn!(target: "ethy", "💎 Failed to append justification: {:?}", signed_commitment);
				}

				// Notify an subscribers that we've got proof of finality for a new block e.g. open RPC subscriptions
				self.signed_commitment_sender.notify(signed_commitment);

				// We've reached consensus on this block
				self.best_ethy_block = Some(round.1);

				metric_set!(self, ethy_best_block, round.1);
			}
		}
	}

	pub(crate) async fn run(mut self) {
		let mut votes = Box::pin(self.gossip_engine.lock().messages_for(topic::<B>()).filter_map(
			|notification| async move {
				trace!(target: "ethy", "💎 Got vote message: {:?}", notification);

				VoteMessage::<MmrRootHash, NumberFor<B>, Public, Signature>::decode(&mut &notification.message[..]).ok()
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
				vote = votes.next().fuse() => {
					if let Some(vote) = vote {
						self.handle_vote(
							(vote.commitment.payload, vote.commitment.block_number),
							(vote.id, vote.signature),
						);
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

/// Extract the MMR root hash from a digest in the given header, if it exists.
fn find_mmr_root_digest<B, Id>(header: &B::Header) -> Option<MmrRootHash>
where
	B: Block,
	Id: Codec,
{
	header.digest().logs().iter().find_map(|log| {
		match log.try_to::<ConsensusLog<Id>>(OpaqueDigestItemId::Consensus(&ETHY_ENGINE_ID)) {
			Some(ConsensusLog::MmrRoot(root)) => Some(root),
			_ => None,
		}
	})
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

/// Calculate next block number to vote on
fn vote_target<N>(best_grandpa: N, best_ethy: N, min_delta: u32) -> N
where
	N: AtLeast32Bit + Copy + Debug,
{
	let diff = best_grandpa.saturating_sub(best_ethy);
	let diff = diff.saturated_into::<u32>();
	let target = best_ethy + min_delta.max(diff.next_power_of_two()).into();

	trace!(
		target: "ethy",
		"💎 vote target - diff: {:?}, next_power_of_two: {:?}, target block: #{:?}",
		diff,
		diff.next_power_of_two(),
		target,
	);

	target
}

#[cfg(test)]
mod tests {
	use super::vote_target;

	#[test]
	fn vote_on_min_block_delta() {
		let t = vote_target(1u32, 0, 4);
		assert_eq!(4, t);
		let t = vote_target(2u32, 0, 4);
		assert_eq!(4, t);
		let t = vote_target(3u32, 0, 4);
		assert_eq!(4, t);
		let t = vote_target(4u32, 0, 4);
		assert_eq!(4, t);

		let t = vote_target(4u32, 4, 4);
		assert_eq!(8, t);

		let t = vote_target(10u32, 10, 4);
		assert_eq!(14, t);
		let t = vote_target(11u32, 10, 4);
		assert_eq!(14, t);
		let t = vote_target(12u32, 10, 4);
		assert_eq!(14, t);
		let t = vote_target(13u32, 10, 4);
		assert_eq!(14, t);

		let t = vote_target(10u32, 10, 8);
		assert_eq!(18, t);
		let t = vote_target(11u32, 10, 8);
		assert_eq!(18, t);
		let t = vote_target(12u32, 10, 8);
		assert_eq!(18, t);
		let t = vote_target(13u32, 10, 8);
		assert_eq!(18, t);
	}

	#[test]
	fn vote_on_power_of_two() {
		let t = vote_target(1008u32, 1000, 4);
		assert_eq!(1008, t);

		let t = vote_target(1016u32, 1000, 4);
		assert_eq!(1016, t);

		let t = vote_target(1032u32, 1000, 4);
		assert_eq!(1032, t);

		let t = vote_target(1064u32, 1000, 4);
		assert_eq!(1064, t);

		let t = vote_target(1128u32, 1000, 4);
		assert_eq!(1128, t);

		let t = vote_target(1256u32, 1000, 4);
		assert_eq!(1256, t);

		let t = vote_target(1512u32, 1000, 4);
		assert_eq!(1512, t);

		let t = vote_target(1024u32, 0, 4);
		assert_eq!(1024, t);
	}

	#[test]
	fn vote_on_target_block() {
		let t = vote_target(1008u32, 1002, 4);
		assert_eq!(1010, t);
		let t = vote_target(1010u32, 1002, 4);
		assert_eq!(1010, t);

		let t = vote_target(1016u32, 1006, 4);
		assert_eq!(1022, t);
		let t = vote_target(1022u32, 1006, 4);
		assert_eq!(1022, t);

		let t = vote_target(1032u32, 1012, 4);
		assert_eq!(1044, t);
		let t = vote_target(1044u32, 1012, 4);
		assert_eq!(1044, t);

		let t = vote_target(1064u32, 1014, 4);
		assert_eq!(1078, t);
		let t = vote_target(1078u32, 1014, 4);
		assert_eq!(1078, t);

		let t = vote_target(1128u32, 1008, 4);
		assert_eq!(1136, t);
		let t = vote_target(1136u32, 1008, 4);
		assert_eq!(1136, t);
	}
}
