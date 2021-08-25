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

use std::collections::{BTreeMap, HashMap, VecDeque};
use codec::{Decode, Encode};
use log::{debug, trace};
use parking_lot::RwLock;

use sc_network::PeerId;
use sc_network_gossip::{MessageIntent, ValidationResult, Validator, ValidatorContext};

use sp_runtime::traits::{Block, Hash, Header, NumberFor};

use cennznet_primitives::eth::{
	crypto::{AuthorityId as Public, AuthoritySignature as Signature},
	Nonce, Witness,
};

use crate::{keystore::EthyKeystore, witness_record::WitnessRecord};

/// Gossip engine messages topic
pub(crate) fn topic<B: Block>() -> B::Hash
where
	B: Block,
{
	<<B::Header as Header>::Hashing as Hash>::hash(b"ethy")
}

/// ETHY gossip validator
///
/// Validate ETHY gossip messages
///
///All messaging is handled in a single ETHY global topic.
pub(crate) struct GossipValidator<B>
where
	B: Block,
{
	topic: B::Hash,
	known_votes: RwLock<BTreeMap<Nonce, Vec<Public>>>,
}

impl<B> GossipValidator<B>
where
	B: Block,
{
	pub fn new() -> GossipValidator<B> {
		GossipValidator { 
			topic: topic::<B>(),
			known_votes: RwLock::new(Default::default()),
		}
	}

	/// Make a vote for nonce as complete
	pub fn mark_complete(&self, nonce: Nonce) {
		let mut known_votes = self.known_votes.write();
		known_votes.remove(&nonce);
	}
}

impl<B> Validator<B> for GossipValidator<B>
where
	B: Block,
{
	fn validate(
		&self,
		_context: &mut dyn ValidatorContext<B>,
		sender: &PeerId,
		mut data: &[u8],
	) -> ValidationResult<B::Hash> {
		if let Ok(msg) = Witness::decode(&mut data) {

			let known_votes = self.known_votes.read();
			let maybe_known = known_votes.get(&msg.proof_nonce).map(|v| v.binary_search(&msg.authority_id));
			if maybe_known.is_some() && maybe_known.unwrap().is_ok() {
				return ValidationResult::Discard
			}

			// TODO: vote must be from a valid authority

			if EthyKeystore::verify(
				&msg.authority_id,
				&msg.signature,
				&(&msg.digest, msg.proof_nonce).encode(),
			) {
				// Make the vote as seen
				let mut known_votes = self.known_votes.write();
				match maybe_known {
					Some(insert_index) => {
						// we've seen this nonce and need to add the new vote
						// insert_index is guaranteed to be `Err` as it has not been recorded yet
						let index = insert_index.err().unwrap();
						known_votes.get_mut(&msg.proof_nonce).map(|v| v.insert(index, msg.authority_id));
					}
					None => {
					// we haven't seen this nonce yet
						known_votes.insert(msg.proof_nonce, vec![msg.authority_id]);
					}
				}

				return ValidationResult::ProcessAndKeep(self.topic);
			} else {
				// TODO: report peer
				debug!(target: "ethy", "💎 Bad signature on message: {:?}, from: {:?}", msg, sender);
			}
		}

		ValidationResult::Discard
	}

	// fn message_expired<'a>(&'a self) -> Box<dyn FnMut(B::Hash, &[u8]) -> bool + 'a> {
	// 	let live_rounds = self.live_rounds.read();
	// 	Box::new(move |_topic, mut data| {
	// 		let msg = match Witness::decode(&mut data) {
	// 			Ok(vote) => vote,
	// 			Err(_) => return true,
	// 		};

	// 		let expired = !GossipValidator::<B>::is_live(&live_rounds, msg.witness.block_number);

	// 		trace!(target: "ethy", "💎 Message for round #{} expired: {}", msg.witness.block_number, expired);

	// 		expired
	// 	})
	// }

	// #[allow(clippy::type_complexity)]
	// fn message_allowed<'a>(&'a self) -> Box<dyn FnMut(&PeerId, MessageIntent, &B::Hash, &[u8]) -> bool + 'a> {
	// 	let live_rounds = self.live_rounds.read();
	// 	Box::new(move |_who, _intent, _topic, mut data| {
	// 		let msg = match Witness::decode(&mut data) {
	// 			Ok(vote) => vote,
	// 			Err(_) => return true,
	// 		};

	// 		let allowed = GossipValidator::<B>::is_live(&live_rounds, msg.witness.block_number);

	// 		trace!(target: "ethy", "💎 Message for round #{} allowed: {}", msg.witness.block_number, allowed);

	// 		allowed
	// 	})
	// }
}

#[cfg(test)]
mod tests {
	use super::{GossipValidator, MAX_LIVE_GOSSIP_ROUNDS};
	use sc_network_test::Block;

	#[test]
	fn note_round_works() {
		let gv = GossipValidator::<Block>::new();

		gv.note_round(1u64);

		let live = gv.live_rounds.read();
		assert!(GossipValidator::<Block>::is_live(&live, 1u64));

		drop(live);

		gv.note_round(3u64);
		gv.note_round(7u64);
		gv.note_round(10u64);

		let live = gv.live_rounds.read();

		assert_eq!(live.len(), MAX_LIVE_GOSSIP_ROUNDS);

		assert!(!GossipValidator::<Block>::is_live(&live, 1u64));
		assert!(GossipValidator::<Block>::is_live(&live, 3u64));
		assert!(GossipValidator::<Block>::is_live(&live, 7u64));
		assert!(GossipValidator::<Block>::is_live(&live, 10u64));
	}

	#[test]
	fn keeps_most_recent_max_rounds() {
		let gv = GossipValidator::<Block>::new();

		gv.note_round(3u64);
		gv.note_round(7u64);
		gv.note_round(10u64);
		gv.note_round(1u64);

		let live = gv.live_rounds.read();

		assert_eq!(live.len(), MAX_LIVE_GOSSIP_ROUNDS);

		assert!(GossipValidator::<Block>::is_live(&live, 3u64));
		assert!(!GossipValidator::<Block>::is_live(&live, 1u64));

		drop(live);

		gv.note_round(23u64);
		gv.note_round(15u64);
		gv.note_round(20u64);
		gv.note_round(2u64);

		let live = gv.live_rounds.read();

		assert_eq!(live.len(), MAX_LIVE_GOSSIP_ROUNDS);

		assert!(GossipValidator::<Block>::is_live(&live, 15u64));
		assert!(GossipValidator::<Block>::is_live(&live, 20u64));
		assert!(GossipValidator::<Block>::is_live(&live, 23u64));
	}

	#[test]
	fn note_same_round_twice() {
		let gv = GossipValidator::<Block>::new();

		gv.note_round(3u64);
		gv.note_round(7u64);
		gv.note_round(10u64);

		let live = gv.live_rounds.read();

		assert_eq!(live.len(), MAX_LIVE_GOSSIP_ROUNDS);

		drop(live);

		// note round #7 again -> should not change anything
		gv.note_round(7u64);

		let live = gv.live_rounds.read();

		assert_eq!(live.len(), MAX_LIVE_GOSSIP_ROUNDS);

		assert!(GossipValidator::<Block>::is_live(&live, 3u64));
		assert!(GossipValidator::<Block>::is_live(&live, 7u64));
		assert!(GossipValidator::<Block>::is_live(&live, 10u64));
	}
}
