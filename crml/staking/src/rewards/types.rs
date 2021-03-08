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
use sp_runtime::Perbill;
use sp_std::collections::btree_map::BTreeMap;

/// Validator stake info required for reward calculation
pub struct ValidatorStakeInfo<AccountId, Balance: HasCompact> {
	pub commission: Perbill,
	pub exposures: Exposure<AccountId, Balance>,
}

pub trait StakeInfoProvider {
	/// The system account ID type
	type AccountId;
	/// The system balance type
	type Balance: HasCompact;
	/// Return the commission and exposure information for the given validator stash at `era`
	fn stake_info_for(_validator_stash: &Self::AccountId, _era: EraIndex) -> ValidatorStakeInfo<Self::AccountId, Self::Balance>;
}

/// Something which can perform reward payment to staked validators
pub trait StakerRewardPayment {
	/// The system account ID type
	type AccountId;
	/// The system balance type
	type Balance: HasCompact;

	/// Schedule a reward payout for the given validators at the given era
	fn schedule_reward_payouts(validators: &[Self::AccountId], era: EraIndex);
	/// Calculate the value of the next reward payout as of right now.
	/// i.e calling `enqueue_reward_payouts` would distribute this total value among stakers.
	fn calculate_next_reward_payout() -> Self::Balance;
	/// Calculate the next reward payout (accrued as of right now) for the given stash id.
	fn payee_next_reward_payout(
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
#[derive(PartialEq, Encode, Decode, Default)]
pub struct EraRewardPoints<AccountId: Ord> {
	/// Total number of points. Equals the sum of reward points for each validator.
	pub total: RewardPoint,
	/// The reward points earned by a given validator.
	pub individual: BTreeMap<AccountId, RewardPoint>,
}
