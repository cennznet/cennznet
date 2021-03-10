/* Copyright 2020 Centrality Investments Limited
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

//! Staking reward types

use crate::{EraIndex, Exposure};
use codec::{Decode, Encode, HasCompact};
use frame_support::weights::Weight;
use sp_runtime::{traits::Saturating, Perbill};
use sp_std::collections::btree_map::BTreeMap;

/// Something that can run payouts
/// We need some information from staking module and rewards module to execute a payout lazily
pub trait RunScheduledPayout {
	type AccountId;
	type Balance;
	/// Execute a staking payout
	fn run_payout(validator_stash: &Self::AccountId, _total_payout: Self::Balance) -> Weight;
}

/// A type which can be notified of a staking era end
pub trait OnEndEra {
	type AccountId;
	/// Receives the stash addresses of the validator set and the `era_index` which is finishing
	fn on_end_era(_validator_stashes: &[Self::AccountId], _era_index: EraIndex) {}
}

/// Detailed parts of a total reward
pub struct NextRewardParts<Balance: Clone + HasCompact + Saturating> {
	/// How much of this reward is due to base inflation
	pub inflation: Balance,
	/// How much of this reward is due to transaction fees
	pub transaction_fees: Balance,
}

impl<Balance: Clone + HasCompact + Saturating> NextRewardParts<Balance> {
	/// Calculate the total reward from its parts
	pub fn total(self) -> Balance {
		self.transaction_fees.saturating_add(self.inflation)
	}
}

/// Something which can perform reward calculation
pub trait RewardCalculation {
	/// The system account ID type
	type AccountId;
	/// The system balance type
	type Balance: Clone + HasCompact + Saturating;

	/// Calculate the value of the next reward payout as of right now.
	/// i.e calling `enqueue_reward_payouts` would distribute this total value among stakers.
	fn calculate_total_reward() -> NextRewardParts<Self::Balance>;
	/// Calculate the next reward payout (accrued as of right now) for the given stash id.
	/// Requires commission rates and exposures for the relevant validator set
	fn calculate_individual_reward(
		stash: &Self::AccountId,
		validator_commission_stake_map: &[(Self::AccountId, Perbill, Exposure<Self::AccountId, Self::Balance>)],
	) -> Self::Balance;
}

pub trait HandlePayee {
	/// The system account ID type
	type AccountId;

	/// (Re-)set the payment target for a stash account.
	/// If payee is not different from stash, do no operations.
	fn set_payee(stash: &Self::AccountId, payee: &Self::AccountId);
	/// Remove the corresponding stash-payee from the look up. Do no operations if stash not found.
	fn remove_payee(stash: &Self::AccountId);
	/// Return the reward destination for the given stash account.
	fn payee(stash: &Self::AccountId) -> Self::AccountId;
}

/// Counter for the number of "reward" points earned by a given validator.
pub type RewardPoint = u32;

/// Reward points of an era. Used to split era total payout between validators.
///
/// These points will be used to reward validators and their respective nominators.
#[derive(PartialEq, Clone, Encode, Decode, Default)]
pub struct EraRewardPoints<AccountId: Ord> {
	/// Total number of points. Equals the sum of reward points for each validator.
	pub total: RewardPoint,
	/// The reward points earned by a given validator.
	pub individual: BTreeMap<AccountId, RewardPoint>,
}
