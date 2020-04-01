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

//! Define mock currencies
#![cfg(test)]

#![macro_use]

use frame_support::additional_traits::AssetIdAuthority;
use pallet_generic_asset::AssetCurrency;

use crate::{
	impls::ExchangeAddressGenerator,
	types::{FeeRate, LowPrecisionUnsigned, PerMilli, PerMillion},
	Call, GenesisConfig, Module, Trait,
};
use core::convert::TryFrom;
use frame_support::{additional_traits::DummyDispatchVerifier, impl_outer_origin};
use pallet_generic_asset;
use sp_core::{sr25519, H256};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
	Perbill,
};

pub type AccountId = <<sr25519::Signature as Verify>::Signer as IdentifyAccount>::AccountId;

impl_outer_origin! {
	pub enum Origin for Test where system = frame_system {}
}

// For testing the module, we construct most of a mock runtime. This means
// first constructing a configuration type (`Test`) which `impl`s each of the
// configuration traits of modules we want to use.
#[derive(Clone, Eq, PartialEq)]
pub struct Test;

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: u32 = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::one();
}

impl frame_system::Trait for Test {
	type Origin = Origin;
	type Call = ();
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<AccountId>;
	type Header = Header;
	type Event = ();
	type BlockHashCount = BlockHashCount;
	type Doughnut = ();
	type DelegatedDispatchVerifier = DummyDispatchVerifier<Self::Doughnut, Self::AccountId>;
	type MaximumBlockWeight = MaximumBlockWeight;
	type MaximumBlockLength = MaximumBlockLength;
	type AvailableBlockRatio = AvailableBlockRatio;
	type Version = ();
	type ModuleToIndex = ();
}

impl pallet_generic_asset::Trait for Test {
	type Balance = LowPrecisionUnsigned;
	type AssetId = u32;
	type Event = ();
}

pub struct UnsignedIntToBalance(LowPrecisionUnsigned);
impl From<LowPrecisionUnsigned> for UnsignedIntToBalance {
	fn from(u: LowPrecisionUnsigned) -> Self {
		UnsignedIntToBalance(u)
	}
}
impl From<UnsignedIntToBalance> for LowPrecisionUnsigned {
	fn from(u: UnsignedIntToBalance) -> Self {
		u.0
	}
}

impl Trait for Test {
	type Call = Call<Self>;
	type Event = ();
	type ExchangeAddressGenerator = ExchangeAddressGenerator<Self>;
	type BalanceToUnsignedInt = LowPrecisionUnsigned;
	type UnsignedIntToBalance = UnsignedIntToBalance;
}

pub type CennzXSpot = Module<Test>;

pub const CORE_ASSET_ID: u32 = 0;
pub const TRADE_ASSET_A_ID: u32 = 1;
pub const TRADE_ASSET_B_ID: u32 = 2;
pub const FEE_ASSET_ID: u32 = 10;

/// A mock core currency. This is the network spending type e.g. CPAY it is a generic asset
pub(crate) type CoreAssetCurrency<T> = AssetCurrency<T, CoreAssetIdProvider<T>>;
/// A mock trade currency 'A'. It is a generic asset
pub(crate) type TradeAssetCurrencyA<T> = AssetCurrency<T, TradeAssetAIdProvider<T>>;
/// A mock trade currency 'B'. It is a generic asset
pub(crate) type TradeAssetCurrencyB<T> = AssetCurrency<T, TradeAssetBIdProvider<T>>;
/// A mock fee currency. It is a generic asset
pub(crate) type FeeAssetCurrency<T> = AssetCurrency<T, FeeAssetIdProvider<T>>;

pub struct CoreAssetIdProvider<T>(sp_std::marker::PhantomData<T>);
impl<T: Trait> AssetIdAuthority for CoreAssetIdProvider<T> {
	type AssetId = T::AssetId;
	fn asset_id() -> Self::AssetId {
		CORE_ASSET_ID.into()
	}
}

pub struct TradeAssetAIdProvider<T>(sp_std::marker::PhantomData<T>);
impl<T: Trait> AssetIdAuthority for TradeAssetAIdProvider<T> {
	type AssetId = T::AssetId;
	fn asset_id() -> Self::AssetId {
		TRADE_ASSET_A_ID.into()
	}
}

pub struct TradeAssetBIdProvider<T>(sp_std::marker::PhantomData<T>);
impl<T: Trait> AssetIdAuthority for TradeAssetBIdProvider<T> {
	type AssetId = T::AssetId;
	fn asset_id() -> Self::AssetId {
		TRADE_ASSET_B_ID.into()
	}
}

pub struct FeeAssetIdProvider<T>(sp_std::marker::PhantomData<T>);
impl<T: Trait> AssetIdAuthority for FeeAssetIdProvider<T> {
	type AssetId = T::AssetId;
	fn asset_id() -> Self::AssetId {
		FEE_ASSET_ID.into()
	}
}

pub struct ExtBuilder {
	core_asset_id: u32,
	fee_rate: FeeRate<PerMillion>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			core_asset_id: 0,
			fee_rate: FeeRate::<PerMillion>::try_from(FeeRate::<PerMilli>::from(3u128)).unwrap(),
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();
		pallet_generic_asset::GenesisConfig::<Test> {
			assets: Vec::new(),
			initial_balance: 0,
			endowed_accounts: Vec::new(),
			next_asset_id: 100,
			staking_asset_id: 0,
			spending_asset_id: 10,
		}
		.assimilate_storage(&mut t)
		.unwrap();
		GenesisConfig::<Test> {
			core_asset_id: self.core_asset_id,
			fee_rate: self.fee_rate,
		}
		.assimilate_storage(&mut t)
		.unwrap();
		sp_io::TestExternalities::new(t)
	}
}

// Helper Macros

/// Returns the matching asset ID for a currency given it's type alias
/// It's a quick work around to avoid complex trait logic using `AssetIdAuthority`
macro_rules! resolve_asset_id (
	(CoreAssetCurrency) => { CORE_ASSET_ID };
	(TradeAssetCurrencyA) => { TRADE_ASSET_A_ID };
	(TradeAssetCurrencyB) => { TRADE_ASSET_B_ID };
	(FeeAssetCurrency) => { FEE_ASSET_ID };
	($unknown:literal) => { panic!("cannot resolve asset ID for unknown currency: {}", $unknown) };
);

/// Initializes an exchange pair with the given liquidity
/// `with_exchange!(asset1_id => balance, asset2_id => balance)`
macro_rules! with_exchange (
	($a1:ident => $b1:expr, $a2:ident => $b2:expr) => {
		{
			let exchange_address = <Test as Trait>::ExchangeAddressGenerator::exchange_address_for(
				resolve_asset_id!($a2),
			);
			let _ = $a1::deposit_creating(&exchange_address, $b1);
			let _ = $a2::deposit_creating(&exchange_address, $b2);
		}
	};
);

/// Assert an exchange pair has a balance equal to
/// `assert_exchange_balance_eq!(0 => 10, 1 => 15)`
macro_rules! assert_exchange_balance_eq (
	($a1:ident => $b1:expr, $a2:ident => $b2:expr) => {
		{
			let exchange_address = <Test as Trait>::ExchangeAddressGenerator::exchange_address_for(
				resolve_asset_id!($a2),
			);
			let bal1 = $a1::free_balance(&exchange_address);
			let bal2 = $a2::free_balance(&exchange_address);
			assert_eq!(bal1, $b1);
			assert_eq!(bal2, $b2);
		}
	};
);

/// Initializes a preset address with the given exchange balance.
/// Examples
/// ```
/// let andrea = with_account!(0 => 10, 1 => 20);
/// let bob = with_account!("bob", 0 => 10, 1 => 20);
/// ```
macro_rules! with_account (
	($a1:ident => $b1:expr, $a2:ident => $b2:expr) => {
		{
			let _ = $a1::deposit_creating(&H256::from_low_u64_be(1).unchecked_into(), $b1);
			let _ = $a2::deposit_creating(&H256::from_low_u64_be(1).unchecked_into(), $b2);
			H256::from_low_u64_be(1).unchecked_into()
		}
	};
	($name:expr, $a1:ident => $b1:expr, $a2:ident => $b2:expr) => {
		{
			let account = match $name {
				"andrea" => H256::from_low_u64_be(1).unchecked_into(),
				"bob" => H256::from_low_u64_be(2).unchecked_into(),
				"charlie" => H256::from_low_u64_be(3).unchecked_into(),
				_ => H256::from_low_u64_be(1).unchecked_into(), // default back to "andrea"
			};
			let _ = $a1::deposit_creating(&account, $b1);
			let _ = $a2::deposit_creating(&account, $b2);
			account
		}
	};
);

/// Assert account has asset balance equal to
// alias for `assert_eq!(<pallet_generic_asset::Module<Test>>::free_balance(asset_id, address), amount)`
macro_rules! assert_balance_eq (
	($address:expr, $asset_id:ident => $balance:expr) => {
		{
			assert_eq!($asset_id::free_balance(&$address), $balance);
		}
	};
);
