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
//! Mocks for the module.

#![cfg(test)]

use parity_codec::{Decode, Encode};
use primitives::{Blake2Hasher, H256};
use runtime_primitives::{
	testing::{Digest, DigestItem, Header},
	traits::{BlakeTwo256, IdentityLookup, Lazy, Verify},
	BuildStorage,
};
use serde::{Deserialize, Serialize};
use support::{impl_outer_event, impl_outer_origin};

use super::*;

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

// For testing the module, we construct most of a mock runtime. This means
// first constructing a configuration type (`Test`) which `impl`s each of the
// configuration traits of modules we want to use.
#[derive(Clone, Eq, PartialEq)]
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
	type Event = TestEvent;
	type Log = DigestItem;
	type DispatchVerifier = ();
	type Doughnut = ();
}

impl Trait for Test {
	type Balance = u64;
	type AssetId = u32;
	type Event = TestEvent;
}

mod generic_asset {
	pub use crate::Event;
}

impl_outer_event! {
	pub enum TestEvent for Test {
		generic_asset<T>,
	}
}

pub type GenericAsset = Module<Test>;

pub type System = system::Module<Test>;

pub struct ExtBuilder {
	asset_id: u32,
	next_asset_id: u32,
	accounts: Vec<u64>,
	initial_balance: u64,
}

// Returns default values for genesis config
impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			asset_id: 0,
			next_asset_id: 1000,
			accounts: vec![0],
			initial_balance: 0,
		}
	}
}

impl ExtBuilder {
	// Sets free balance to genesis config
	pub fn free_balance(mut self, free_balance: (u32, u64, u64)) -> Self {
		self.asset_id = free_balance.0;
		self.accounts = vec![free_balance.1];
		self.initial_balance = free_balance.2;
		self
	}

	pub fn next_asset_id(mut self, asset_id: u32) -> Self {
		self.next_asset_id = asset_id;
		self
	}

	// builds genesis config
	pub fn build(self) -> runtime_io::TestExternalities<Blake2Hasher> {
		let mut t = system::GenesisConfig::<Test>::default().build_storage().unwrap().0;

		t.extend(
			GenesisConfig::<Test> {
				assets: vec![self.asset_id],
				endowed_accounts: self.accounts,
				initial_balance: self.initial_balance,
				next_asset_id: self.next_asset_id,
				create_asset_stake: 10,
				staking_asset_id: 16000,
				spending_asset_id: 16001,
			}
			.build_storage()
			.unwrap()
			.0,
		);

		t.into()
	}
}

// This function basically just builds a genesis storage key/value store according to
// our desired mockup.
pub fn new_test_ext() -> runtime_io::TestExternalities<Blake2Hasher> {
	system::GenesisConfig::<Test>::default()
		.build_storage()
		.unwrap()
		.0
		.into()
}
