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

use codec::Decode;
use log::{trace, warn};
use parking_lot::{Mutex, RwLock};
use std::{
	collections::{BTreeMap, VecDeque},
	time::{Duration, Instant},
};

use sc_network::PeerId;
use sc_network_gossip::{MessageIntent, ValidationResult, Validator, ValidatorContext};

use sp_runtime::traits::{Block, Hash, Header};

use cennznet_primitives::eth::{crypto::AuthorityId as Public, EventId, Witness};

use crate::keystore::EthyKeystore;

/// Gossip engine messages topic
pub(crate) fn topic<B: Block>() -> B::Hash
where
	B: Block,
{
	<<B::Header as Header>::Hashing as Hash>::hash(b"ethy")
}

/// Number of recent complete events to keep in memory
const MAX_COMPLETE_EVENT_CACHE: usize = 30;

// Timeout for rebroadcasting messages.
const REBROADCAST_AFTER: Duration = Duration::from_secs(60 * 5);

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
	/// Pruned list of recently completed events
	complete_events: RwLock<VecDeque<EventId>>,
	/// Public (ECDSA session) keys of active ethy validators
	active_validators: RwLock<Vec<Public>>,
	/// Scheduled time for rebroad casting event witnesses
	next_rebroadcast: Mutex<Instant>,
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
			complete_events: RwLock::new(Default::default()),
			next_rebroadcast: Mutex::new(Instant::now() + REBROADCAST_AFTER),
		}
	}

	/// Make a vote for an event as complete
	pub fn mark_complete(&self, event_id: EventId) {
		let mut known_votes = self.known_votes.write();
		known_votes.remove(&event_id);
		let mut complete_events = self.complete_events.write();
		if complete_events.len() > MAX_COMPLETE_EVENT_CACHE {
			complete_events.pop_front();
		}
		match complete_events.binary_search(&event_id) {
			Ok(_idx) => {
				// this shouldn't happen
				warn!(target: "ethy", "💎 double event complete: {:?} in {:?}", event_id, complete_events);
			}
			Err(idx) => {
				complete_events.insert(idx, event_id);
			}
		}
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

			if !self.active_validators.read().iter().any(|v| *v == authority_id) {
				trace!(target: "ethy", "💎 witness from: {:?}, event: {:?} is not an active authority", &authority_id, event_id);
				return ValidationResult::Discard;
			}

			if EthyKeystore::verify_prehashed(&authority_id, &signature, &digest) {
				// Make the vote as seen
				trace!(target: "ethy", "💎 verify prehashed OK, waiting lock: {:?}, event: {:?}", &authority_id, event_id);
				match maybe_known {
					Some(insert_index) => {
						// we've seen this nonce and need to add the new vote
						// insert_index is guaranteed to be `Err` as it has not been recorded yet
						let index = insert_index.err().unwrap();
						if let Some(v) = known_votes.get_mut(&event_id) {
							v.insert(index, authority_id.clone())
						}
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
				warn!(target: "ethy", "💎 bad signature: {:?}, event: {:?}", authority_id, event_id);
			}
		}

		trace!(target: "ethy", "💎 invalid witness from sender: {:?}, could not decode: {:?}", sender, data);
		ValidationResult::Discard
	}

	fn message_expired<'a>(&'a self) -> Box<dyn FnMut(B::Hash, &[u8]) -> bool + 'a> {
		let complete_events = self.complete_events.read();
		Box::new(move |_topic, mut data| {
			let witness = match Witness::decode(&mut data) {
				Ok(w) => w,
				Err(_) => return true,
			};

			let expired = complete_events.binary_search(&witness.event_id).is_ok();
			trace!(target: "ethy", "💎 Message for event #{} expired: {}", witness.event_id, expired);

			expired
		})
	}

	#[allow(clippy::type_complexity)]
	fn message_allowed<'a>(&'a self) -> Box<dyn FnMut(&PeerId, MessageIntent, &B::Hash, &[u8]) -> bool + 'a> {
		let do_rebroadcast = {
			let now = Instant::now();
			let mut next_rebroadcast = self.next_rebroadcast.lock();
			if now >= *next_rebroadcast {
				*next_rebroadcast = now + REBROADCAST_AFTER;
				true
			} else {
				false
			}
		};

		let complete_events = self.complete_events.read();
		Box::new(move |_who, intent, _topic, mut data| {
			if let MessageIntent::PeriodicRebroadcast = intent {
				return do_rebroadcast;
			}

			let witness = match Witness::decode(&mut data) {
				Ok(w) => w,
				Err(_) => return true,
			};

			// Check if message is incomplete
			let allowed = complete_events.binary_search(&witness.event_id).is_err();

			trace!(target: "ethy", "💎 Message for round #{} allowed: {}", &witness.event_id, allowed);

			allowed
		})
	}
}

// #[cfg(test)]
// mod tests {
// 	use super::{GossipValidator, MAX_COMPLETE_EVENT_CACHE};
// 	use sc_network_test::Block;

// 	#[test]
// 	fn note_round_works() {
// 		let gv = GossipValidator::<Block>::new();

// 		gv.note_round(1u64);

// 		let live = gv.live_events.read();
// 		assert!(GossipValidator::<Block>::is_live(&live, 1u64));

// 		drop(live);

// 		gv.note_round(3u64);
// 		gv.note_round(7u64);
// 		gv.note_round(10u64);

// 		let live = gv.live_events.read();

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

// 		let live = gv.live_events.read();

// 		assert_eq!(live.len(), MAX_LIVE_GOSSIP_ROUNDS);

// 		assert!(GossipValidator::<Block>::is_live(&live, 3u64));
// 		assert!(!GossipValidator::<Block>::is_live(&live, 1u64));

// 		drop(live);

// 		gv.note_round(23u64);
// 		gv.note_round(15u64);
// 		gv.note_round(20u64);
// 		gv.note_round(2u64);

// 		let live = gv.live_events.read();

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

// 		let live = gv.live_events.read();

// 		assert_eq!(live.len(), MAX_LIVE_GOSSIP_ROUNDS);

// 		drop(live);

// 		// note round #7 again -> should not change anything
// 		gv.note_round(7u64);

// 		let live = gv.live_events.read();

// 		assert_eq!(live.len(), MAX_LIVE_GOSSIP_ROUNDS);

// 		assert!(GossipValidator::<Block>::is_live(&live, 3u64));
// 		assert!(GossipValidator::<Block>::is_live(&live, 7u64));
// 		assert!(GossipValidator::<Block>::is_live(&live, 10u64));
// 	}
// }
