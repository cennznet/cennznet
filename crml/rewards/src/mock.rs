//! Test utilities

#![cfg(test)]

use crate::{GenesisConfig, Module, Trait};
use fees::OnFeeCharged;
use generic_asset::{AssetCurrency, RewardAssetIdProvider};
use primitives::{Blake2Hasher, H256};
use runtime_io;
use runtime_primitives::BuildStorage;
use runtime_primitives::{
	testing::{ConvertUintAuthorityId, Digest, DigestItem, Header, UintAuthorityId},
	traits::{BlakeTwo256, IdentityLookup},
};
use session::OnSessionChange;
use staking;
use support::additional_traits::DummyChargeFee;
use support::impl_outer_origin;

impl_outer_origin! {
	pub enum Origin for Test {}
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
}
impl timestamp::Trait for Test {
	type Moment = u64;
	type OnTimestampSet = ();
}
impl generic_asset::Trait for Test {
	type Balance = u64;
	type AssetId = u32;
	type Event = ();
	type ChargeFee = DummyChargeFee<u64, u64>;
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
	type Currency = AssetCurrency<Test, RewardAssetIdProvider<Test>>;
	type OnRewardMinted = ();
	type Event = ();
}

impl Trait for Test {}

pub type Rewards = Module<Test>;
pub type Staking = staking::Module<Test>;

// A mock to trigger `on_fee_charged` function.
pub struct ChargeFeeMock;
impl ChargeFeeMock {
	pub fn trigger_rewards_on_fee_charged(amount: u64) {
		<Rewards as OnFeeCharged<u64>>::on_fee_charged(&amount);
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
	block_reward: u64,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self { block_reward: 0 }
	}
}

impl ExtBuilder {
	pub fn block_reward(mut self, reward: u64) -> Self {
		self.block_reward = reward;
		self
	}

	pub fn build(self) -> runtime_io::TestExternalities<Blake2Hasher> {
		let mut t = system::GenesisConfig::<Test>::default().build_storage().unwrap().0;
		t.extend(
			GenesisConfig::<Test> {
				block_reward: self.block_reward,
			}
			.build_storage()
			.unwrap()
			.0,
		);
		t.into()
	}
}
