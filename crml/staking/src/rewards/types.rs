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

use crate::Exposure;
use codec::HasCompact;
use sp_arithmetic::{FixedI128, Perbill};

/// Something which can perform reward payment to staked validators
pub trait StakerRewardPayment {
	/// The system account ID type
	type AccountId;
	/// The system balance type
	type Balance: HasCompact;
	/// Make a staking reward payout to validators and nominators.
	/// `validator_commission_stake_map` is a mapping of a validator payment account, validator commission %, and
	/// a validator + nominator exposure map.
	fn make_reward_payout(
		validator_commission_stake_map: &[(Self::AccountId, Perbill, Exposure<Self::AccountId, Self::Balance>)],
	);
	/// Calculate the value of the next reward payout as of right now.
	/// i.e calling `make_reward_payout` would distribute this total value among stakers.
	fn calculate_next_reward_payout() -> FixedI128;
}
