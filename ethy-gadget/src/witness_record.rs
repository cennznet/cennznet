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

use cennznet_primitives::eth::{
	crypto::{AuthorityId, AuthoritySignature as Signature},
	EventId, Witness,
};
use log::trace;
use std::collections::HashMap;

/// Tracks live witnesses
///
/// Stores witnesses per message event_id and digest
/// event_id -> digest -> [](authority, signature)
/// this structure allows resiliency incase different digests are witnessed, maliciously or not.
#[derive(Default)]
pub struct WitnessRecord {
	record: HashMap<EventId, HashMap<[u8; 32], Vec<(AuthorityId, Signature)>>>,
	has_voted: HashMap<EventId, Vec<AuthorityId>>,
}

impl WitnessRecord {
	/// Remove a witness record from memory
	pub fn clear(&mut self, event_id: EventId) {
		self.record.remove(&event_id);
	}
	/// Return all known signatures for the witness on (event_id, digest)
	pub fn signatures_for(
		&self,
		event_id: EventId,
		digest: &[u8; 32],
		validators: Vec<AuthorityId>,
	) -> Vec<Option<Signature>> {
		// TODO: can probably do better by storing this in sorted order to begin with...
		let proofs = self.record.get(&event_id).unwrap().get(digest).unwrap();
		validators
			.iter()
			.map(|v| proofs.iter().find(|(id, _sig)| v == id).map(|(_id, sig)| sig.clone()))
			.collect()
	}
	/// Does the event identified by `event_id` `digest` have >= `threshold` support
	pub fn has_consensus(&self, event_id: EventId, digest: &[u8; 32], threshold: usize) -> bool {
		self.record
			.get(&event_id)
			.and_then(|x| x.get(digest))
			.and_then(|v| Some(v.len()))
			.unwrap_or_default()
			>= threshold
	}
	/// Note a witness if we haven't seen it before
	pub fn note(&mut self, witness: &Witness) {
		if self
			.has_voted
			.get(&witness.event_id)
			.map(|votes| votes.binary_search(&witness.authority_id).is_ok())
			.unwrap_or_default()
		{
			// TODO: log/ return something useful
			trace!(target: "ethy", "💎 witness previously seen: {:?}", witness.event_id);
			return;
		}

		if !self.record.contains_key(&witness.event_id) {
			// first witness for this event_id
			let mut digest_signatures = HashMap::<[u8; 32], Vec<(AuthorityId, Signature)>>::default();
			digest_signatures.insert(
				witness.digest,
				vec![(witness.authority_id.clone(), witness.signature.clone())],
			);
			self.record.insert(witness.event_id, digest_signatures);
		} else if !self
			.record
			.get(&witness.event_id)
			.map(|x| x.contains_key(&witness.digest))
			.unwrap_or(false)
		{
			// first witness for this digest
			let digest_signatures = vec![(witness.authority_id.clone(), witness.signature.clone())];
			self.record
				.get_mut(&witness.event_id)
				.unwrap()
				.insert(witness.digest, digest_signatures);
		} else {
			// add witness to known (event_id, digest)
			self.record
				.get_mut(&witness.event_id)
				.unwrap()
				.get_mut(&witness.digest)
				.unwrap()
				.push((witness.authority_id.clone(), witness.signature.clone()));
		}
		trace!(target: "ethy", "💎 witness recorded: {:?}", witness.event_id);

		// Mark authority as voted
		match self.has_voted.get_mut(&witness.event_id) {
			None => {
				// first vote for this event_id we've seen
				self.has_voted
					.insert(witness.event_id, vec![witness.authority_id.clone()]);
			}
			Some(votes) => {
				// subsequent vote for a known event_id
				if let Err(idx) = votes.binary_search(&witness.authority_id) {
					votes.insert(idx, witness.authority_id.clone());
				}
			}
		}
	}
}
