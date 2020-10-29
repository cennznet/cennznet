/* Copyright 2019-2020 Centrality Investments Limited
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

//! Define test runtime and storage
#![cfg(test)]

/// The main liquidity asset ID
pub const CORE_ASSET_ID: AssetId = 1;
/// A trade-able asset ID
pub const TRADE_ASSET_A_ID: AssetId = 2;
/// Another trade-able asset ID
pub const TRADE_ASSET_B_ID: AssetId = 3;
/// An asset ID used to pay network fees
pub const FEE_ASSET_ID: AssetId = 10;

use crate::{
	impls::ExchangeAddressGenerator,
	types::{FeeRate, PerMillion, PerThousand},
	GenesisConfig, Module, Trait,
};
pub(crate) use cennznet_primitives::types::{AccountId, AssetId, Balance};
use core::convert::TryFrom;
use frame_support::{impl_outer_event, impl_outer_origin};
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

pub type Cennzx = Module<Test>;

impl_outer_origin! {
	pub enum Origin for Test where system = frame_system {}
}

// alias the Event from this crate under `cennzx` for event testing
mod cennzx {
	pub use crate::Event;
}

impl_outer_event! {
	pub enum Event for Test {
		cennzx<T>,
		frame_system<T>,
		prml_generic_asset<T>,
	}
}

// For testing the module, we construct most of a mock runtime. This means
// first constructing a configuration type (`Test`) which `impl`s each of the
// configuration traits of modules we want to use.
#[derive(Clone, Eq, PartialEq)]
pub struct Test;

impl frame_system::Trait for Test {
	type BaseCallFilter = ();
	type Origin = Origin;
	type Index = u64;
	type Call = ();
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = ();
	type MaximumBlockWeight = ();
	type DbWeight = ();
	type BlockExecutionWeight = ();
	type ExtrinsicBaseWeight = ();
	type MaximumExtrinsicWeight = ();
	type AvailableBlockRatio = ();
	type MaximumBlockLength = ();
	type Version = ();
	type PalletInfo = ();
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
}

impl prml_generic_asset::Trait for Test {
	type AssetId = AssetId;
	type Balance = Balance;
	type Event = Event;
	type WeightInfo = ();
}

impl Trait for Test {
	type Event = Event;
	type AssetId = AssetId;
	type ExchangeAddressFor = ExchangeAddressGenerator<Self>;
	type MultiCurrency = prml_generic_asset::Module<Self>;
	type WeightInfo = ();
}
pub struct ExtBuilder {
	core_asset_id: u32,
	fee_rate: FeeRate<PerMillion>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			core_asset_id: CORE_ASSET_ID,
			fee_rate: FeeRate::<PerMillion>::try_from(FeeRate::<PerThousand>::from(3u128)).unwrap(),
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();
		prml_generic_asset::GenesisConfig::<Test> {
			assets: Vec::new(),
			initial_balance: 0,
			endowed_accounts: Vec::new(),
			next_asset_id: 100,
			staking_asset_id: 0,
			spending_asset_id: FEE_ASSET_ID,
			permissions: vec![],
			asset_meta: vec![],
		}
		.assimilate_storage(&mut t)
		.unwrap();
		GenesisConfig::<Test> {
			core_asset_id: self.core_asset_id,
			fee_rate: self.fee_rate,
		}
		.assimilate_storage(&mut t)
		.unwrap();
		let mut ext = sp_io::TestExternalities::new(t);

		// Run in the context of the first block
		ext.execute_with(|| frame_system::Module::<Test>::set_block_number(1));
		ext
	}
}

// Helper Macros

/// Initializes an exchange pair with the given liquidity
/// `with_exchange!(asset1_id => balance, asset2_id => balance)`
#[macro_export]
macro_rules! with_exchange (
	($a1:ident => $b1:expr, $a2:ident => $b2:expr) => {
		{
			let exchange_address = crate::impls::ExchangeAddressGenerator::<Test>::exchange_address_for($a2);
			let _ = <prml_generic_asset::Module<Test>>::deposit_creating(&exchange_address, Some($a1), $b1);
			let _ = <prml_generic_asset::Module<Test>>::deposit_creating(&exchange_address, Some($a2), $b2);
			exchange_address
		}
	}
);

/// Assert an exchange pair has a balance equal to
/// `assert_exchange_balance_eq!(0 => 10, 1 => 15)`
#[macro_export]
macro_rules! assert_exchange_balance_eq (
	($a1:ident => $b1:expr, $a2:ident => $b2:expr) => {
		let exchange_address = crate::impls::ExchangeAddressGenerator::<Test>::exchange_address_for($a2);
		let bal1 = <prml_generic_asset::Module<Test>>::free_balance($a1, &exchange_address);
		let bal2 = <prml_generic_asset::Module<Test>>::free_balance($a2, &exchange_address);
		assert_eq!(bal1, $b1);
		assert_eq!(bal2, $b2);
	};
);

/// Initializes a preset address with the given exchange balance.
/// Examples
/// ```
/// let andrea = with_account!(0 => 10, 1 => 20);
/// let bob = with_account!("bob", 0 => 10, 1 => 20);
/// ```
#[macro_export]
macro_rules! with_account (
	($a1:ident => $b1:expr, $a2:ident => $b2:expr) => {
		{
			let account = sp_keyring::AccountKeyring::Alice.into();
			let _ = <prml_generic_asset::Module<Test>>::deposit_creating(&account, Some($a1), $b1);
			let _ = <prml_generic_asset::Module<Test>>::deposit_creating(&account, Some($a2), $b2);
			assert_eq!(
				<prml_generic_asset::Module<Test>>::free_balance($a1, &account),
				$b1
			);
			assert_eq!(
				<prml_generic_asset::Module<Test>>::free_balance($a2, &account),
				$b2
			);
			account
		}
	};
	($name:expr, $a1:ident => $b1:expr, $a2:ident => $b2:expr) => {
		{
			let account = match $name {
				"andrea" => sp_keyring::AccountKeyring::Alice.into(),
				"bob" => sp_keyring::AccountKeyring::Bob.into(),
				"charlie" => sp_keyring::AccountKeyring::Charlie.into(),
				_ =>  sp_keyring::AccountKeyring::Alice.into(), // default back to "andrea"
			};
			let _ = <prml_generic_asset::Module<Test>>::deposit_creating(&account, Some($a1), $b1);
			let _ = <prml_generic_asset::Module<Test>>::deposit_creating(&account, Some($a2), $b2);
			assert_eq!(
				<prml_generic_asset::Module<Test>>::free_balance($a1, &account),
				$b1
			);
			assert_eq!(
				<prml_generic_asset::Module<Test>>::free_balance($a2, &account),
				$b2
			);
			account
		}
	};
);

/// Assert account has asset balance equal to
// alias for `assert_eq!(<prml_generic_asset::Module<Test>>::free_balance(asset_id, address), amount)`
#[macro_export]
macro_rules! assert_balance_eq (
	($address:expr, $asset_id:ident => $balance:expr) => {
		assert_eq!(
			<prml_generic_asset::Module<Test>>::free_balance($asset_id, &$address),
			$balance,
		);
	};
);

/// Returns the last recorded block event
pub fn last_event() -> Event {
	frame_system::Module::<Test>::events()
		.pop()
		.expect("Event expected")
		.event
}
