// Copyright 2019 Centrality Investments Limited
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//! Test utilities

#![cfg(test)]

use crate::{GenesisConfig, Module, Trait};
use fees::OnFeeCharged;
use generic_asset::{SpendingAssetCurrency, StakingAssetCurrency};
use parity_codec::{Decode, Encode};
use primitives::{Blake2Hasher, H256};
use runtime_io;
use runtime_primitives::traits::{Lazy, Verify};
use runtime_primitives::BuildStorage;
use runtime_primitives::{
	testing::{ConvertUintAuthorityId, Digest, DigestItem, Header, UintAuthorityId},
	traits::{BlakeTwo256, Convert, IdentityLookup, Lazy, Verify},
	Permill,
};
use serde::{Deserialize, Serialize};
use serde::{Deserialize, Serialize};
use session::OnSessionChange;
use staking;
use support::impl_outer_origin;

impl_outer_origin! {
	pub enum Origin for Test {}
}

#[derive(Encode, Decode, Serialize, Deserialize, Debug)]
pub struct Signature;

impl Verify for Signature {
	type Signer = u64;
	fn verify<L: Lazy<[u8]>>(&self, _msg: L, _signer: &Self::Signer) -> bool {
		true
	}
}

pub struct CurrencyToVoteHandler;

impl Convert<u128, u64> for CurrencyToVoteHandler {
	fn convert(x: u128) -> u64 {
		x as u64
	}
}

impl Convert<u128, u128> for CurrencyToVoteHandler {
	fn convert(x: u128) -> u128 {
		x
	}
}

// Workaround for https://github.com/rust-lang/rust/issues/26925 . Remove when sorted.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Test;

impl system::Trait for Test {
	type Origin = Origin;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type Digest = Digest;
	type AccountId = u64;
	type Lookup = IdentityLookup<u64>;
	type Header = Header;
	type Event = ();
	type Log = DigestItem;
	type Doughnut = ();
	type DispatchVerifier = ();
}
impl timestamp::Trait for Test {
	type Moment = u64;
	type OnTimestampSet = ();
}
impl generic_asset::Trait for Test {
	type Balance = u128;
	type AssetId = u32;
	type Event = ();
}
impl consensus::Trait for Test {
	type Log = DigestItem;
	type SessionKey = UintAuthorityId;
	type InherentOfflineReport = ();
}
impl session::Trait for Test {
	type ConvertAccountIdToSessionKey = ConvertUintAuthorityId;
	type OnSessionChange = Staking;
	type Event = ();
}
impl staking::Trait for Test {
	type Currency = StakingAssetCurrency<Test>;
	type RewardCurrency = SpendingAssetCurrency<Test>;
	type CurrencyToReward = u128;
	type BalanceToU128 = u128;
	type U128ToBalance = u128;
	type CurrencyToVote = CurrencyToVoteHandler;
	type OnRewardMinted = ();
	type Event = ();
	type Slash = ();
	type Reward = ();
}

impl Trait for Test {}

pub type Rewards = Module<Test>;
pub type Staking = staking::Module<Test>;

// A mock to trigger `on_fee_charged` function.
pub struct ChargeFeeMock;
impl ChargeFeeMock {
	pub fn trigger_rewards_on_fee_charged(amount: u128) {
		<Rewards as OnFeeCharged<u128>>::on_fee_charged(&amount);
	}
}

// A mock to trigger `on_session_change` function.
pub struct SessionChangeMock;
impl SessionChangeMock {
	pub fn trigger_rewards_on_session_change() {
		<Rewards as OnSessionChange<u64>>::on_session_change(10, false);
	}
}

pub struct ExtBuilder {
	block_reward: u128,
	fee_reward_multiplier: Permill,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			block_reward: 0,
			fee_reward_multiplier: Permill::from_percent(100),
		}
	}
}

impl ExtBuilder {
	pub fn block_reward(mut self, reward: u128) -> Self {
		self.block_reward = reward;
		self
	}

	pub fn fee_reward_multiplier(mut self, multiplier: Permill) -> Self {
		self.fee_reward_multiplier = multiplier;
		self
	}

	pub fn build(self) -> runtime_io::TestExternalities<Blake2Hasher> {
		let mut t = system::GenesisConfig::<Test>::default().build_storage().unwrap().0;
		t.extend(
			GenesisConfig::<Test> {
				block_reward: self.block_reward,
				fee_reward_multiplier: self.fee_reward_multiplier,
			}
			.build_storage()
			.unwrap()
			.0,
		);
		t.into()
	}
}
