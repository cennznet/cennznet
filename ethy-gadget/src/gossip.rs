// Copyright (C) 2020-2021 Parity Technologies (UK) Ltd. & Centrality Investments Ltd
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

use codec::{Decode, Encode};
use log::{debug, trace};
use parking_lot::{RwLock, RwLockUpgradableReadGuard};
use std::collections::{BTreeMap, HashMap, VecDeque};

use sc_network::PeerId;
use sc_network_gossip::{MessageIntent, ValidationResult, Validator, ValidatorContext};

use sp_runtime::traits::{Block, Hash, Header, NumberFor};

use cennznet_primitives::eth::{
	crypto::{AuthorityId as Public, AuthoritySignature as Signature},
	EventId, Witness,
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
	known_votes: RwLock<BTreeMap<EventId, Vec<Public>>>,
	active_validators: RwLock<Vec<Public>>,
}

impl<B> GossipValidator<B>
where
	B: Block,
{
	pub fn new(active_validators: Vec<Public>) -> GossipValidator<B> {
		GossipValidator {
			topic: topic::<B>(),
			known_votes: RwLock::new(BTreeMap::new()),
			active_validators: RwLock::new(active_validators),
		}
	}

	/// Make a vote for nonce as complete
	pub fn mark_complete(&self, nonce: EventId) {
		let mut known_votes = self.known_votes.write();
		known_votes.remove(&nonce);
	}

	pub fn set_active_validators(&self, new_active_validators: Vec<Public>) {
		let mut active_validators = self.active_validators.write();
		let _old = std::mem::replace(&mut *active_validators, new_active_validators);
		trace!(target: "ethy", "💎 set gossip active validators: {:?}", active_validators);
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
		if let Ok(Witness {
			authority_id,
			event_id,
			digest,
			signature,
		}) = Witness::decode(&mut data)
		{
			trace!(target: "ethy", "💎 witness from: {:?}, event: {:?}", authority_id, event_id);

			let mut known_votes = self.known_votes.write();
			let maybe_known = known_votes.get(&event_id).map(|v| v.binary_search(&authority_id));
			if maybe_known.is_some() && maybe_known.unwrap().is_ok() {
				trace!(target: "ethy", "💎 witness from: {:?}, event: {:?} is already known", &authority_id, event_id);
				return ValidationResult::Discard;
			}

			if self
				.active_validators
				.read()
				.iter()
				.find(|v| *v == &authority_id)
				.is_none()
			{
				trace!(target: "ethy", "💎 witness from: {:?}, event: {:?} is not an active authority", &authority_id, event_id);
				return ValidationResult::Discard;
			}

			trace!(target: "ethy", "💎 verify prehashed: {:?}, event: {:?}", &authority_id, event_id);
			if EthyKeystore::verify_prehashed(&authority_id, &signature, &digest) {
				// Make the vote as seen
				trace!(target: "ethy", "💎 verify prehashed OK, waiting lock: {:?}, event: {:?}", &authority_id, event_id);
				match maybe_known {
					Some(insert_index) => {
						// we've seen this nonce and need to add the new vote
						// insert_index is guaranteed to be `Err` as it has not been recorded yet
						let index = insert_index.err().unwrap();
						known_votes
							.get_mut(&event_id)
							.map(|v| v.insert(index, authority_id.clone()));
					}
					None => {
						// we haven't seen this nonce yet
						known_votes.insert(event_id, vec![authority_id.clone()]);
					}
				}

				trace!(target: "ethy", "💎 valid witness: {:?}, event: {:?}", &authority_id, event_id);
				return ValidationResult::ProcessAndKeep(self.topic);
			} else {
				// TODO: report peer
				debug!(target: "ethy", "💎 bad signature: {:?}, event: {:?}", authority_id, event_id);
			}
		}

		trace!(target: "ethy", "💎 invalid witness from sender: {:?}, could not decode: {:?}", sender, data);
		ValidationResult::Discard
	}

	// TODO: discard old messages
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
	// }
}

// #[cfg(test)]
// mod tests {
// 	use super::{GossipValidator, MAX_LIVE_GOSSIP_ROUNDS};
// 	use sc_network_test::Block;

// 	#[test]
// 	fn note_round_works() {
// 		let gv = GossipValidator::<Block>::new();

// 		gv.note_round(1u64);

// 		let live = gv.live_rounds.read();
// 		assert!(GossipValidator::<Block>::is_live(&live, 1u64));

// 		drop(live);

// 		gv.note_round(3u64);
// 		gv.note_round(7u64);
// 		gv.note_round(10u64);

// 		let live = gv.live_rounds.read();

// 		assert_eq!(live.len(), MAX_LIVE_GOSSIP_ROUNDS);

// 		assert!(!GossipValidator::<Block>::is_live(&live, 1u64));
// 		assert!(GossipValidator::<Block>::is_live(&live, 3u64));
// 		assert!(GossipValidator::<Block>::is_live(&live, 7u64));
// 		assert!(GossipValidator::<Block>::is_live(&live, 10u64));
// 	}

// 	#[test]
// 	fn keeps_most_recent_max_rounds() {
// 		let gv = GossipValidator::<Block>::new();

// 		gv.note_round(3u64);
// 		gv.note_round(7u64);
// 		gv.note_round(10u64);
// 		gv.note_round(1u64);

// 		let live = gv.live_rounds.read();

// 		assert_eq!(live.len(), MAX_LIVE_GOSSIP_ROUNDS);

// 		assert!(GossipValidator::<Block>::is_live(&live, 3u64));
// 		assert!(!GossipValidator::<Block>::is_live(&live, 1u64));

// 		drop(live);

// 		gv.note_round(23u64);
// 		gv.note_round(15u64);
// 		gv.note_round(20u64);
// 		gv.note_round(2u64);

// 		let live = gv.live_rounds.read();

// 		assert_eq!(live.len(), MAX_LIVE_GOSSIP_ROUNDS);

// 		assert!(GossipValidator::<Block>::is_live(&live, 15u64));
// 		assert!(GossipValidator::<Block>::is_live(&live, 20u64));
// 		assert!(GossipValidator::<Block>::is_live(&live, 23u64));
// 	}

// 	#[test]
// 	fn note_same_round_twice() {
// 		let gv = GossipValidator::<Block>::new();

// 		gv.note_round(3u64);
// 		gv.note_round(7u64);
// 		gv.note_round(10u64);

// 		let live = gv.live_rounds.read();

// 		assert_eq!(live.len(), MAX_LIVE_GOSSIP_ROUNDS);

// 		drop(live);

// 		// note round #7 again -> should not change anything
// 		gv.note_round(7u64);

// 		let live = gv.live_rounds.read();

// 		assert_eq!(live.len(), MAX_LIVE_GOSSIP_ROUNDS);

// 		assert!(GossipValidator::<Block>::is_live(&live, 3u64));
// 		assert!(GossipValidator::<Block>::is_live(&live, 7u64));
// 		assert!(GossipValidator::<Block>::is_live(&live, 10u64));
// 	}
// }
