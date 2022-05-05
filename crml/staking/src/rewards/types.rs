/* Copyright 2020-2021 Centrality Investments Limited
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
use scale_info::TypeInfo;
use sp_runtime::{traits::AtLeast32BitUnsigned, Perbill};
use sp_std::collections::btree_map::BTreeMap;

/// Something that can run payouts
/// nb: need information from staking module and rewards module to execute a payout after the fact
pub trait RunScheduledPayout {
	type AccountId;
	type Balance;
	/// Execute a staking payout for stash or amount, earned in era
	fn run_payout(_validator_stash: &Self::AccountId, _amount: Self::Balance, _era_index: EraIndex) -> Weight;
}

/// A type which can be notified of a staking era end
pub trait OnEndEra {
	type AccountId;
	/// Receives the stash addresses of the validator set and the `era_index` which is finishing
	fn on_end_era(
		_validator_stashes: &[Self::AccountId],
		_era_index: EraIndex,
		_era_duration_ms: u64,
		_is_forced: bool,
	) {
	}
}

/// Detailed parts of a total reward
#[derive(Debug, PartialEq)]
pub struct RewardParts<Balance: Copy + HasCompact + AtLeast32BitUnsigned> {
	/// How much of this reward is due to base inflation
	pub inflation: Balance,
	/// How much of this reward is due to transaction fees
	pub transaction_fees: Balance,
	/// What fraction of the total reward should go to the treasury
	pub treasury_rate: Perbill,
	/// total amount to the treasury
	pub treasury_cut: Balance,
	/// total amount to stakers
	pub stakers_cut: Balance,
	/// total payout
	pub total: Balance,
}

impl<Balance: Copy + HasCompact + AtLeast32BitUnsigned> RewardParts<Balance> {
	/// Create a `RewardParts`
	pub fn new(inflation: Balance, transaction_fees: Balance, treasury_rate: Perbill) -> Self {
		let total = transaction_fees.saturating_add(inflation);
		let treasury_cut = treasury_rate * transaction_fees;
		let stakers_cut = total.saturating_sub(treasury_cut);

		Self {
			inflation,
			transaction_fees,
			treasury_rate,
			treasury_cut,
			stakers_cut,
			total,
		}
	}
}

/// Something which can perform reward calculation
pub trait RewardCalculation {
	/// The system account ID type
	type AccountId;
	/// The system balance type
	type Balance: Copy + HasCompact + AtLeast32BitUnsigned;
	/// The maximum era duration in milliseconds
	const FULL_ERA_DURATION: u64;

	/// Calculate the value of the next reward payout as of right now.
	/// i.e this amount would be distributed on reward payout
	fn calculate_total_reward(era_duration_ms: u64) -> RewardParts<Self::Balance>;
	/// Calculate the next reward payout (accrued as of right now) for the given stash id and era duration.
	/// Requires commission rates and exposures for the relevant validator set
	fn calculate_individual_reward(
		stash: &Self::AccountId,
		era_duration_ms: u64,
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
#[derive(PartialEq, Clone, Encode, Decode, Default, TypeInfo)]
pub struct EraRewardPoints<AccountId: Ord> {
	/// Total number of points. Equals the sum of reward points for each validator.
	pub total: RewardPoint,
	/// The reward points earned by a given validator.
	pub individual: BTreeMap<AccountId, RewardPoint>,
}

#[cfg(test)]
mod test {
	use super::*;
	use sp_runtime::traits::Saturating;

	#[test]
	fn create_reward_parts() {
		let inflation = 1_000_u32;
		let fees = 200_u32;
		let treasury_rate = Perbill::from_percent(30);
		let reward_parts = RewardParts::new(inflation, fees, treasury_rate);
		assert_eq!(reward_parts.total, inflation + fees);
		assert_eq!(reward_parts.transaction_fees, fees,);
		assert_eq!(reward_parts.inflation, inflation,);
		assert_eq!(
			reward_parts.stakers_cut,
			inflation + (Perbill::one().saturating_sub(treasury_rate)) * fees,
		);
		assert_eq!(reward_parts.treasury_cut, treasury_rate * fees,);
	}
}
