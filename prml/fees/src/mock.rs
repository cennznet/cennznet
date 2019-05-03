// Copyright 2019 Parity Technologies (UK) Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

//! Test utilities
#![cfg(test)]

use crate::{system, GenesisConfig, Module, OnFeeCharged, Trait};
use generic_asset::SpendingAssetCurrency;
use parity_codec::{Decode, Encode};
use primitives::{Blake2Hasher, H256};
use runtime_io;
use runtime_primitives::BuildStorage;
use runtime_primitives::{
	testing::{Digest, DigestItem, Header},
	traits::{BlakeTwo256, IdentityLookup},
};
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
