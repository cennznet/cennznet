/* Copyright 2019-2021 Centrality Investments Limited
*
* Licensed under the LGPL, Version 3.0 (the "License");
* you may not use this file except in compliance with the License.
* Unless required by applicable law or agreed to in writing, software
* distributed under the License is distributed on an "AS IS" BASIS,
* WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
* See the License for the specific language governing permissions and
* limitations under the License.
* You may obtain a copy of the License at the root of this project source code,
* or at:
*     https://centrality.ai/licenses/gplv3.txt
*     https://centrality.ai/licenses/lgplv3.txt
*/

use codec::{Decode, Encode};
use sp_std::prelude::*;

/// Identifies proposals
pub type ProposalId = u64;

/// A governance proposal
#[derive(Debug, Default, PartialEq, Encode, Decode)]
pub struct Proposal<T: crate::Config> {
	/// The submitter of the proposal
	pub sponsor: T::AccountId,
	/// Justification document URI
	pub justification_uri: Vec<u8>,
	/// Enactment delay in blocks
	/// how soon `call` will be applied after gaining approval
	pub enactment_delay: T::BlockNumber,
}

#[derive(Debug, PartialEq, Encode, Decode)]
pub enum ProposalStatusInfo {
	/// Council is deliberating
	Deliberation,
	/// Proposal approved, waiting enactment
	ApprovedWaitingEnactment,
	/// Proposal approved and enacted (success/fail)
	ApprovedEnacted(bool),
	/// Proposal was approved but enactment cancelled
	ApprovedEnactmentCancelled,
	/// The council voted against this proposal
	Disapproved,
}

/// Votes on a proposal
/// Tracks vote and participation of council member by index
#[derive(Debug, Default, PartialEq, Encode, Decode)]
pub struct ProposalVoteInfo {
	/// Bit field, records the index of councillor and their vote: 0/against, 1/for
	vote_bits: (u128, u128),
	/// Bit field, records whether actively voted or not, 0/absent, 1/voted
	/// distinguishes between a vote of 0 as intentional or absent
	active_bits: (u128, u128),
}
#[derive(Debug, PartialEq)]
/// Represents current status of council counted votes
pub struct CouncilVoteCount {
	pub yes: u32,
	pub no: u32,
}
impl ProposalVoteInfo {
	pub fn active_bits(&self) -> (u128, u128) {
		self.active_bits
	}
	pub fn vote_bits(&self) -> (u128, u128) {
		self.vote_bits
	}
	/// Record a vote for or against for council member at `index`
	pub fn record_vote(&mut self, index: u8, vote: bool) {
		// bitfields has max capacity for votes up to u8 / 255
		match index {
			0..=127 => {
				let on_mask = 1_u128 << index;
				self.active_bits.0 |= on_mask;
				if vote {
					self.vote_bits.0 |= on_mask;
				}
			}
			128..=255 => {
				let on_mask = 1_u128 << (index - 128);
				self.active_bits.1 |= on_mask;
				if vote {
					self.vote_bits.1 |= on_mask;
				}
			}
		}
	}
	/// Count the votes yes, no
	pub fn count_votes(&self) -> CouncilVoteCount {
		let active_count = self.active_bits.0.count_ones() + self.active_bits.1.count_ones();
		let yes_count = self.vote_bits.0.count_ones() + self.vote_bits.1.count_ones();
		let no_count = active_count - yes_count;

		CouncilVoteCount {
			yes: yes_count,
			no: no_count,
		}
	}
	/// Insert a new voter at index
	pub fn insert_voter(&mut self, index: u8) {
		// bitfields has max capacity for votes up to u8 / 255
		// shift all votes at index to the right
		match index {
			0..=127 => {
				let low_mask = (1_u128 << index) - 1;
				let high_mask = !low_mask;
				self.active_bits.0 = ((self.active_bits.0 & high_mask) << 1) | (self.active_bits.0 & low_mask);
				self.vote_bits.0 = ((self.vote_bits.0 & high_mask) << 1) | (self.vote_bits.0 & low_mask);
			}
			128..=255 => {
				let low_mask = (1_u128 << (index - 128)) - 1;
				let high_mask = !low_mask;
				self.active_bits.1 = ((self.active_bits.1 & high_mask) << 1) | (self.active_bits.1 & low_mask);
				self.vote_bits.1 = ((self.vote_bits.1 & high_mask) << 1) | (self.vote_bits.1 & low_mask);
			}
		}
	}
	/// Remove a voter at index
	/// shifts all votes after index to the left
	pub fn remove_voter(&mut self, index: u8) {
		// bitfields has max capacity for votes up to u8 / 255
		// Shift all votes at index to the right
		match index {
			0..=127 => {
				let low_mask = (1_u128 << index) - 1;
				let high_mask = !low_mask;
				self.active_bits.0 = ((self.active_bits.0 & high_mask) >> 1) | (self.active_bits.0 & low_mask);
				self.vote_bits.0 = ((self.vote_bits.0 & high_mask) >> 1) | (self.vote_bits.0 & low_mask);
			}
			128..=255 => {
				let index = index - 128;
				let low_mask = (1_u128 << index) - 1;
				let high_mask = !low_mask;
				self.active_bits.1 = ((self.active_bits.1 & high_mask) >> 1) | (self.active_bits.1 & low_mask);
				self.vote_bits.1 = ((self.vote_bits.1 & high_mask) >> 1) | (self.vote_bits.1 & low_mask);
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::{CouncilVoteCount, ProposalVoteInfo};

	#[test]
	fn add_voter() {
		let mut votes = ProposalVoteInfo::default();
		votes.record_vote(0_u8, true);
		votes.record_vote(1_u8, false);
		votes.record_vote(2_u8, true);
		assert_eq!(votes.vote_bits().0, 5); // 1010_0000
		assert_eq!(votes.active_bits().0, 7); // 1110_0000

		// insert new voter at index 2
		votes.insert_voter(2);
		assert_eq!(votes.vote_bits().0, 9); // 1001_0000
		assert_eq!(votes.active_bits().0, 11); // 1101_0000

		// insert new voter at index 0
		votes.insert_voter(0);
		assert_eq!(votes.vote_bits().0, 18); // 0100_1000
		assert_eq!(votes.active_bits().0, 22); // 0110_1000

		// insert new voter at index 0
		votes.record_vote(128, true);
		votes.insert_voter(128);
		assert_eq!(votes.vote_bits().1, 2); // 0100_0000
		assert_eq!(votes.active_bits().1, 2); // 0100_0000

		// insert new voter at max index
		votes.record_vote(u8::MAX, true);
		assert_eq!(votes.vote_bits().1, (1 << 127) + 2);
		assert_eq!(votes.active_bits().1, (1 << 127) + 2);
	}

	#[test]
	fn record_vote() {
		let mut votes = ProposalVoteInfo::default();
		votes.record_vote(u8::MAX, true);
		assert_eq!(votes.vote_bits().1, 1_u128 << 127);

		let mut votes = ProposalVoteInfo::default();
		votes.record_vote(127, true);
		assert_eq!(votes.vote_bits().0, 1_u128 << 127);
		votes.record_vote(0, true);
		assert_eq!(votes.vote_bits().0, (1_u128 << 127) + 1);
	}

	#[test]
	fn remove_vote() {
		let mut votes = ProposalVoteInfo::default();
		votes.record_vote(0_u8, true);
		votes.record_vote(1_u8, true);
		votes.record_vote(2_u8, true);
		// 1010_0000
		// 1110_0000
		votes.remove_voter(0);
		assert_eq!(votes.vote_bits().0, 0b0000_0011 as u128);
		assert_eq!(votes.active_bits().0, 0b0000_0011 as u128);

		votes.record_vote(2_u8, true);
		assert_eq!(votes.vote_bits().0, 0b0000_0111 as u128);
		assert_eq!(votes.active_bits().0, 0b0000_0111 as u128);
		votes.remove_voter(1);
		assert_eq!(votes.vote_bits().0, 0b0000_0011 as u128);
		assert_eq!(votes.active_bits().0, 0b0000_0011 as u128);
		votes.record_vote(3_u8, true);
		votes.remove_voter(1);
		assert_eq!(votes.vote_bits().0, 0b0000_0101 as u128);
		assert_eq!(votes.active_bits().0, 0b0000_0101 as u128);
	}

	#[test]
	fn count_votes() {
		let mut votes = ProposalVoteInfo::default();
		votes.record_vote(0_u8, true);
		votes.record_vote(1_u8, false);
		votes.record_vote(2_u8, true);
		assert_eq!(votes.count_votes(), CouncilVoteCount { yes: 2, no: 1 });
	}
}
