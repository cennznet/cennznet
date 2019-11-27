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

use crate::{system, GenesisConfig, Module, OnFeeCharged, Trait};
use generic_asset::SpendingAssetCurrency;
use parity_codec::{Decode, Encode};
use primitives::{Blake2Hasher, H256};
use runtime_io;
use runtime_primitives::traits::{Lazy, Verify};
use runtime_primitives::BuildStorage;
use runtime_primitives::{
	testing::{Digest, DigestItem, Header},
	traits::{BlakeTwo256, IdentityLookup, Lazy, Verify},
};
use serde::{Deserialize, Serialize};
use serde::{Deserialize, Serialize};
use support::{decl_module, decl_storage, dispatch::Result, impl_outer_event, impl_outer_origin, StorageValue};

impl_outer_origin! {
	pub enum Origin for Test {}
}

mod fees {
	pub use crate::{Call, Event};
}

impl_outer_event! {
	pub enum TestEvent for Test {
		fees<T>, generic_asset<T>,
	}
}

#[derive(Encode, Decode, Serialize, Deserialize, Debug)]
pub struct Signature;

impl Verify for Signature {
	type Signer = u64;
	fn verify<L: Lazy<[u8]>>(&self, _msg: L, _signer: &Self::Signer) -> bool {
		true
	}
}

#[derive(Clone, PartialEq, Eq, Debug, Encode, Decode)]
pub enum MockFee {
	Base,
	Bytes,
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
	type Event = TestEvent;
	type Log = DigestItem;
	type DispatchVerifier = ();
	type Doughnut = ();
}

impl generic_asset::Trait for Test {
	type Balance = u64;
	type AssetId = u32;
	type Event = TestEvent;
}

pub trait OnFeeChargedMockTrait: system::Trait {}

decl_module! {
	pub struct OnFeeChargedMockModule<T: OnFeeChargedMockTrait> for enum Call where origin: T::Origin {
		pub fn do_nothing() -> Result { Ok(()) }
	}
}

decl_storage! {
	trait Store for OnFeeChargedMockModule<T: OnFeeChargedMockTrait> as F {
		Amount get(amount): u64;
	}
}

impl<T: OnFeeChargedMockTrait> OnFeeCharged<u64> for OnFeeChargedMockModule<T> {
	fn on_fee_charged(fee: &u64) {
		<Amount<T>>::put(fee);
	}
}

impl OnFeeChargedMockTrait for Test {}

impl Trait for Test {
	type Event = TestEvent;
	type Currency = SpendingAssetCurrency<Test>;
	type OnFeeCharged = OnFeeChargedMock;
	type BuyFeeAsset = ();
	type Fee = MockFee;
}

pub type System = system::Module<Test>;
pub type Fees = Module<Test>;
pub type OnFeeChargedMock = OnFeeChargedMockModule<Test>;

pub struct ExtBuilder {
	transaction_base_fee: u64,
	transaction_byte_fee: u64,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			transaction_base_fee: 0,
			transaction_byte_fee: 0,
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> runtime_io::TestExternalities<Blake2Hasher> {
		let (mut t, mut c) = system::GenesisConfig::<Test>::default().build_storage().unwrap();
		let _ = generic_asset::GenesisConfig::<Test> {
			staking_asset_id: 16_000,
			spending_asset_id: 16_001,
			assets: vec![16_001],
			endowed_accounts: vec![0],
			create_asset_stake: 10,
			initial_balance: u64::max_value(),
			next_asset_id: 10_000,
		}
		.assimilate_storage(&mut t, &mut c);
		let _ = GenesisConfig::<Test> {
			_genesis_phantom_data: rstd::marker::PhantomData::<Test>,
			fee_registry: vec![
				(MockFee::Base, self.transaction_base_fee),
				(MockFee::Bytes, self.transaction_byte_fee),
			],
		}
		.assimilate_storage(&mut t, &mut c);

		t.into()
	}
}
