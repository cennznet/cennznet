// Copyright (C) 2019 Centrality Investments Limited
// This file is part of CENNZnet.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.
//!
//! CENNZX-SPOT Tests
//!
#![cfg(test)]
use crate::{
	impls::{ExchangeAddressFor, ExchangeAddressGenerator},
	support::StorageMap,
	types::FeeRate,
	Call, CoreAssetId, DefaultFeeRate, GenesisConfig, Module, Trait,
};
use codec::{alloc::collections::HashMap, Encode};
use core::convert::TryInto;
use generic_asset;
use primitives::{crypto::UncheckedInto, map, sr25519, Blake2Hasher, H256};
use runtime_primitives::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup, Verify},
	Perbill,
};
use state_machine::TestExternalities;
use support::{impl_outer_origin, StorageDoubleMap, StorageValue};

pub type AccountId = <sr25519::Signature as Verify>::Signer;

impl_outer_origin! {
	pub enum Origin for Test {}
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

impl system::Trait for Test {
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
	type DelegatedDispatchVerifier = ();
	type MaximumBlockWeight = MaximumBlockWeight;
	type MaximumBlockLength = MaximumBlockLength;
	type AvailableBlockRatio = AvailableBlockRatio;
	type Version = ();
}

impl generic_asset::Trait for Test {
	type Balance = u128;
	type AssetId = u32;
	type Event = ();
}

pub struct U128ToBalance(u128);
impl From<u128> for U128ToBalance {
	fn from(u: u128) -> Self {
		U128ToBalance(u)
	}
}
impl From<U128ToBalance> for u128 {
	fn from(u: U128ToBalance) -> u128 {
		u.0.try_into().unwrap_or(u128::max_value())
	}
}

impl Trait for Test {
	type Call = Call<Self>;
	type Event = ();
	type ExchangeAddressGenerator = ExchangeAddressGenerator<Self>;
	type BalanceToU128 = u128;
	type U128ToBalance = U128ToBalance;
}

pub type CennzXSpot = Module<Test>;

pub struct ExtBuilder {
	core_asset_id: u32,
	fee_rate: FeeRate,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			core_asset_id: 0,
			fee_rate: FeeRate::from_milli(3),
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> runtime_io::TestExternalities {
		let mut t = system::GenesisConfig::default().build_storage::<Test>().unwrap();
		generic_asset::GenesisConfig::<Test> {
			assets: Vec::new(),
			initial_balance: 0,
			endowed_accounts: Vec::new(),
			next_asset_id: 100,
			// create_asset_stake: 1000,
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
		runtime_io::TestExternalities::new(t)
	}
}

/// Builds storage as a HashMap based on `account_1` and `account_2`.
/// The function is primarily used in with_exchange! and with_account! macros.
fn map_build(account_1: (u32, AccountId, u128), account_2: (u32, AccountId, u128)) -> HashMap<Vec<u8>, Vec<u8>> {
	map![
		<generic_asset::FreeBalance<Test>>::hashed_key_for(&account_1.0, &account_1.1) => account_1.2.encode(),
		<generic_asset::FreeBalance<Test>>::hashed_key_for(&account_2.0, &account_2.1) => account_2.2.encode(),
		<generic_asset::TotalIssuance<Test>>::hashed_key_for(account_1.0) => account_1.2.encode(),
		<generic_asset::TotalIssuance<Test>>::hashed_key_for(account_2.0) => account_2.2.encode(),
		<system::BlockHash<Test>>::hashed_key_for(0).to_vec() => vec![0u8; 32],
		<CoreAssetId::<Test>>::hashed_key().to_vec() => 0.encode(),
		DefaultFeeRate::hashed_key().to_vec() => FeeRate::from_milli(3).encode(),
	]
}

/// Returns TestExternalities based on `storage`.
fn ext_build(storage: HashMap<Vec<u8>, Vec<u8>>) -> TestExternalities<Blake2Hasher, u64> {
	TestExternalities::<Blake2Hasher, u64>::new((storage, map![]))
}

/// Merges multiple HashMaps into a single HashMap.
macro_rules! merge (
	($map:expr, $( $maps:expr ), +) => {
		$map.into_iter()
		$(
			.chain($maps)
		)*
		.collect::<HashMap<Vec<u8>, Vec<u8>>>()
	};
);

/// Initializes an exchange pair with the given liquidity
/// `with_exchange!(asset1_id => balance, asset2_id => balance)`
macro_rules! with_exchange (
	($a1:expr => $b1:expr, $a2:expr => $b2:expr) => {
		{
			let exchange_address = <Test as Trait>::ExchangeAddressGenerator::exchange_address_for($a1, $a2);
			map_build(($a1, exchange_address.clone(), $b1), ($a2, exchange_address, $b2))
		}
	};
);

/// Assert an exchange pair has a balance equal to
/// `assert_exchange_balance_eq!(0 => 10, 1 => 15)`
macro_rules! assert_exchange_balance_eq (
	($a1:expr => $b1:expr, $a2:expr => $b2:expr) => {
		{
			let exchange_address = <Test as Trait>::ExchangeAddressGenerator::exchange_address_for($a1, $a2);
			assert_eq!(<generic_asset::Module<Test>>::free_balance(&$a1, &exchange_address), $b1);
			assert_eq!(<generic_asset::Module<Test>>::free_balance(&$a2, &exchange_address), $b2);
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
	($a1:expr => $b1:expr, $a2:expr => $b2:expr) => {
		{
			let account: AccountId = H256::from_low_u64_be(1).unchecked_into();
			(
				account.clone(),
				map_build(($a1, account.clone(), $b1), ($a2, account.clone(), $b2)),
			)
		}
	};
	($name:expr, $a1:expr => $b1:expr, $a2:expr => $b2:expr) => {
		{
			let account: AccountId = match $name {
				"andrea" => H256::from_low_u64_be(1).unchecked_into(),
				"bob" => H256::from_low_u64_be(2).unchecked_into(),
				"charlie" => H256::from_low_u64_be(3).unchecked_into(),
				_ => H256::from_low_u64_be(1).unchecked_into(), // default back to "andrea"
			};
			(
				account.clone(),
				map_build(($a1, account.clone(), $b1), ($a2, account.clone(), $b2)),
			)
		}
	};
);

/// Assert account has asset balance equal to
// alias for `assert_eq!(<generic_asset::Module<Test>>::free_balance(asset_id, address), amount)`
macro_rules! assert_balance_eq (
	($address:expr, $asset_id:expr => $balance:expr) => {
		{
			assert_eq!(<generic_asset::Module<Test>>::free_balance(&$asset_id, &$address), $balance);
		}
	};
);

// Default exchange asset IDs
const CORE_ASSET_ID: u32 = 0;
const TRADE_ASSET_A: u32 = 1;
const TRADE_ASSET_B: u32 = 2;
const DEFAULT_EXCHANGE_KEY: (u32, u32) = (CORE_ASSET_ID, TRADE_ASSET_A);

#[test]
fn investor_can_add_liquidity() {
	let (investor, map) = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);
	ext_build(map).execute_with(|| {
		// First investment
		assert_ok!(CennzXSpot::add_liquidity(
			Origin::signed(investor.clone()),
			TRADE_ASSET_A,
			2,  // min_liquidity: T::Balance,
			15, // max_asset_amount: T::Balance,
			10, // core_amount: T::Balance,
		));

		// Second investment
		// because a round up, second time asset amount become 15 + 1
		assert_ok!(CennzXSpot::add_liquidity(
			Origin::signed(H256::from_low_u64_be(1).unchecked_into()),
			TRADE_ASSET_A,
			2,  // min_liquidity: T::Balance,
			16, // max_asset_amount: T::Balance,
			10, // core_amount: T::Balance,
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 20, TRADE_ASSET_A => 31);
		assert_eq!(CennzXSpot::get_liquidity(&DEFAULT_EXCHANGE_KEY, &investor), 20);
	});
}

#[test]
fn get_output_price_zero_cases() {
	let map = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
	ext_build(map).execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);

		assert_err!(
			CennzXSpot::get_output_price(100, 0, 10, DefaultFeeRate::get()),
			"Pool is empty"
		);

		assert_err!(
			CennzXSpot::get_output_price(100, 10, 0, DefaultFeeRate::get()),
			"Pool is empty"
		);
	});
}

/// Formula Price = ((input reserve * output amount) / (output reserve - output amount)) +  1  (round up)
/// and apply fee rate to the price
#[test]
fn get_output_price_for_valid_data() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(
			CennzXSpot::get_output_price(123, 1000, 1000, DefaultFeeRate::get()),
			141
		);

		assert_ok!(
			CennzXSpot::get_output_price(
				100_000_000_000_000,
				120_627_710_511_649_660,
				20_627_710_511_649_660,
				DefaultFeeRate::get()
			),
			589396433540516
		);
	});
}

/// Formula Price = ((input reserve * output amount) / (output reserve - output amount)) +  1  (round up)
/// and apply fee rate to the price
#[test]
fn get_output_price_for_max_reserve_balance() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(
			CennzXSpot::get_output_price(
				u128::max_value() / 2,
				u128::max_value() / 2,
				u128::max_value(),
				DefaultFeeRate::get()
			),
			170651607010850639426882365627031758044
		);
	});
}

/// Formula Price = ((input reserve * output amount) / (output reserve - output amount)) +  1  (round up)
/// and apply fee rate to the price
// Overflows as the both input and output reserve is at max capacity and output amount is little less than max of u128
#[test]
fn get_output_price_should_fail_with_max_reserve_and_max_amount() {
	ExtBuilder::default().build().execute_with(|| {
		assert_err!(
			CennzXSpot::get_output_price(
				u128::max_value() - 100,
				u128::max_value(),
				u128::max_value(),
				DefaultFeeRate::get()
			),
			"Overflow error"
		);
	});
}

/// Formula Price = ((input reserve * output amount) / (output reserve - output amount)) +  1  (round up)
/// and apply fee rate to the price
#[test]
fn get_output_price_max_withdrawal() {
	let map = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
	ext_build(map).execute_with(|| {
		assert_ok!(
			CennzXSpot::get_output_price(1000, 1000, 1000, DefaultFeeRate::get()),
			<Test as generic_asset::Trait>::Balance::max_value()
		);

		assert_ok!(
			CennzXSpot::get_output_price(1_000_000, 1000, 1000, DefaultFeeRate::get()),
			<Test as generic_asset::Trait>::Balance::max_value()
		);
	});
}

#[test]
fn asset_swap_output_price() {
	let map = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
	ext_build(map).execute_with(|| {
		assert_ok!(
			CennzXSpot::get_asset_to_core_output_price(&TRADE_ASSET_A, 123, DefaultFeeRate::get()),
			141
		);

		assert_ok!(
			CennzXSpot::get_core_to_asset_output_price(&TRADE_ASSET_A, 123, DefaultFeeRate::get()),
			141
		);
	});
}

#[test]
fn asset_swap_output_zero_buy_amount() {
	ExtBuilder::default().build().execute_with(|| {
		assert_err!(
			CennzXSpot::get_core_to_asset_output_price(&TRADE_ASSET_A, 0, DefaultFeeRate::get()),
			"Buy amount must be a positive value"
		);
		assert_err!(
			CennzXSpot::get_asset_to_core_output_price(&TRADE_ASSET_A, 0, DefaultFeeRate::get()),
			"Buy amount must be a positive value"
		);
	});
}

#[test]
fn asset_swap_output_insufficient_reserve() {
	let map = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
	ext_build(map).execute_with(|| {
		assert_err!(
			CennzXSpot::get_asset_to_core_output_price(
				&TRADE_ASSET_A,
				1001, // amount_bought
				DefaultFeeRate::get()
			),
			"Insufficient core asset reserve in exchange"
		);

		assert_err!(
			CennzXSpot::get_core_to_asset_output_price(
				&TRADE_ASSET_A,
				1001, // amount_bought
				DefaultFeeRate::get()
			),
			"Insufficient asset reserve in exchange"
		);
	});
}

#[test]
fn asset_to_core_swap_output() {
	let map1 = with_exchange!(CORE_ASSET_ID => 10, TRADE_ASSET_A => 1000);
	let (trader, map2) = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A => 2200);
	let map = merge!(map1, map2);
	ext_build(map).execute_with(|| {
		// asset to core swap output
		assert_ok!(CennzXSpot::asset_swap_output(
			Origin::signed(trader.clone()),
			None,
			TRADE_ASSET_A,
			<CoreAssetId<Test>>::get(),
			5,    // buy_amount: T::Balance,
			1400, // max_sale: T::Balance,
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 5, TRADE_ASSET_A => 2004);
		assert_balance_eq!(trader, TRADE_ASSET_A => 1196);
	});
}

#[test]
fn make_asset_to_core_swap_output() {
	let map1 = with_exchange!(CORE_ASSET_ID => 10, TRADE_ASSET_A => 1000);
	let (trader, map2) = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A => 2200);
	let map = map1.into_iter().chain(map2).collect();
	ext_build(map).execute_with(|| {
		assert_ok!(
			CennzXSpot::make_asset_to_core_output(
				&trader, // buyer
				&trader, // recipient
				&TRADE_ASSET_A,
				5,                      // buy_amount: T::Balance,
				1400,                   // max_sale: T::Balance,
				FeeRate::from_milli(3), // fee_rate
			),
			1004
		);

		assert_exchange_balance_eq!(CORE_ASSET_ID => 5, TRADE_ASSET_A => 2004);
		assert_balance_eq!(trader, TRADE_ASSET_A => 1196);
	});
}

#[test]
fn asset_swap_output_zero_asset_sold() {
	let map1 = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
	let (trader, map2) = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);
	let map = map1.into_iter().chain(map2).collect();
	ext_build(map).execute_with(|| {
		// with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
		// let trader: AccountId = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);

		// asset to core swap output
		assert_err!(
			CennzXSpot::asset_swap_output(
				Origin::signed(trader.clone()),
				None,
				TRADE_ASSET_A,
				<CoreAssetId<Test>>::get(),
				0,   // buy_amount
				100, // max_sale,
			),
			"Buy amount must be a positive value"
		);
		// core to asset swap output
		assert_err!(
			CennzXSpot::asset_swap_output(
				Origin::signed(trader),
				None,
				<CoreAssetId<Test>>::get(),
				TRADE_ASSET_A,
				0,   // buy_amount
				100, // max_sale,
			),
			"Buy amount must be a positive value"
		);
	});
}

#[test]
fn asset_swap_output_insufficient_balance() {
	let map1 = with_exchange!(CORE_ASSET_ID => 500, TRADE_ASSET_A => 500);
	let (trader, map2) = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 50);
	let map = map1.into_iter().chain(map2).collect();
	ext_build(map).execute_with(|| {
		// asset to core swap output
		assert_err!(
			CennzXSpot::asset_swap_output(
				Origin::signed(trader.clone()),
				None,
				TRADE_ASSET_A,
				<CoreAssetId<Test>>::get(),
				51,  // buy_amount
				500, // max_sale,
			),
			"Insufficient asset balance in buyer account"
		);
		// core to asset swap output
		assert_err!(
			CennzXSpot::asset_swap_output(
				Origin::signed(trader),
				None,
				<CoreAssetId<Test>>::get(),
				TRADE_ASSET_A,
				101, // buy_amount
				500, // max_sale,
			),
			"Insufficient core asset balance in buyer account"
		);
	});
}

#[test]
fn asset_swap_output_exceed_max_sale() {
	let map1 = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
	let (trader, map2) = with_account!(CORE_ASSET_ID => 50, TRADE_ASSET_A => 50);
	let map = merge!(map1, map2);
	ext_build(map).execute_with(|| {
		// asset to core swap output
		assert_err!(
			CennzXSpot::asset_swap_output(
				Origin::signed(trader.clone()),
				None,
				TRADE_ASSET_A,
				<CoreAssetId<Test>>::get(),
				50, // buy_amount
				0,  // max_sale,
			),
			"Amount of asset sold would exceed the specified max. limit"
		);

		// core to asset swap output
		assert_err!(
			CennzXSpot::asset_swap_output(
				Origin::signed(trader),
				None,
				<CoreAssetId<Test>>::get(),
				TRADE_ASSET_A,
				50, // buy_amount
				0,  // max_sale,
			),
			"Amount of core asset sold would exceed the specified max. limit"
		);
	});
}

#[test]
fn core_to_asset_swap_output() {
	let map1 = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 10);
	let (trader, map2) = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A => 2200);
	let map = merge!(map1, map2);
	ext_build(map).execute_with(|| {
		// core to asset swap output
		assert_ok!(CennzXSpot::asset_swap_output(
			Origin::signed(trader.clone()),
			None,
			<CoreAssetId<Test>>::get(),
			TRADE_ASSET_A,
			5,    // buy_amount: T::Balance,
			1400, // max_sale: T::Balance,
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 2004, TRADE_ASSET_A => 5);
		assert_balance_eq!(trader, CORE_ASSET_ID => 1196);
		assert_balance_eq!(trader, TRADE_ASSET_A => 2205);
	});
}

#[test]
fn make_core_to_asset_output() {
	let map1 = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 10);
	let (buyer, map2) = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A => 2200);
	let (recipient, map3) = with_account!("bob", CORE_ASSET_ID => 0, TRADE_ASSET_A => 0);
	let map = map1.into_iter().chain(map2).chain(map3).collect();
	ext_build(map).execute_with(|| {
		assert_ok!(
			CennzXSpot::make_core_to_asset_output(
				&buyer,
				&recipient,
				&TRADE_ASSET_A,
				5,                      // buy_amount: T::Balance,
				1400,                   // max_sale: T::Balance,
				FeeRate::from_milli(3), // fee_rate
			),
			1004
		);

		assert_exchange_balance_eq!(CORE_ASSET_ID => 2004, TRADE_ASSET_A => 5);
		assert_balance_eq!(buyer, CORE_ASSET_ID => 1196);
		assert_balance_eq!(recipient, TRADE_ASSET_A => 5);
	});
}

#[test]
fn remove_liquidity() {
	let (investor, map) = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);
	ext_build(map).execute_with(|| {
		// investment
		let _ = CennzXSpot::add_liquidity(
			Origin::signed(investor.clone()),
			TRADE_ASSET_A,
			2,  // min_liquidity: T::Balance,
			15, // max_asset_amount: T::Balance,
			10, // core_amount: T::Balance,
		);

		assert_ok!(CennzXSpot::remove_liquidity(
			Origin::signed(investor.clone()),
			TRADE_ASSET_A,
			10, //`asset_amount` - Amount of exchange asset to burn
			4,  //`min_asset_withdraw` - The minimum trade asset withdrawn
			4   //`min_core_withdraw` -  The minimum core asset withdrawn
		));
		assert_exchange_balance_eq!(CORE_ASSET_ID => 0, TRADE_ASSET_A => 0);
		assert_balance_eq!(investor, TRADE_ASSET_A => 100);
		assert_balance_eq!(investor, CORE_ASSET_ID => 100);
	});
}

#[test]
fn remove_liquidity_fails_min_core_asset_limit() {
	let (investor, map) = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);
	ext_build(map).execute_with(|| {
		// investment
		let _ = CennzXSpot::add_liquidity(
			Origin::signed(investor.clone()),
			TRADE_ASSET_A,
			2,  // min_liquidity: T::Balance,
			15, // max_asset_amount: T::Balance,
			10, // core_amount: T::Balance,
		);

		assert_err!(
			CennzXSpot::remove_liquidity(
				Origin::signed(investor.clone()),
				TRADE_ASSET_A,
				10, //`asset_amount` - Amount of exchange asset to burn
				4,  //`min_asset_withdraw` - The minimum trade asset withdrawn
				14  //`min_core_withdraw` -  The minimum core asset withdrawn
			),
			"Minimum core asset is required"
		);
		assert_exchange_balance_eq!(CORE_ASSET_ID => 10, TRADE_ASSET_A => 15);
		assert_balance_eq!(investor, TRADE_ASSET_A => 85);
		assert_balance_eq!(investor, CORE_ASSET_ID => 90);
	});
}

#[test]
fn remove_liquidity_fails_min_trade_asset_limit() {
	let (investor, map) = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);
	ext_build(map).execute_with(|| {
		// investment
		let _ = CennzXSpot::add_liquidity(
			Origin::signed(investor.clone()),
			TRADE_ASSET_A,
			2,  // min_liquidity: T::Balance,
			15, // max_asset_amount: T::Balance,
			10, // core_amount: T::Balance,
		);

		assert_err!(
			CennzXSpot::remove_liquidity(
				Origin::signed(investor.clone()),
				TRADE_ASSET_A,
				10, //`asset_amount` - Amount of exchange asset to burn
				18, //`min_asset_withdraw` - The minimum trade asset withdrawn
				4   //`min_core_withdraw` -  The minimum core asset withdrawn
			),
			"Minimum trade asset is required"
		);
		assert_exchange_balance_eq!(CORE_ASSET_ID => 10, TRADE_ASSET_A => 15);
		assert_balance_eq!(investor, TRADE_ASSET_A => 85);
		assert_balance_eq!(investor, CORE_ASSET_ID => 90);
	});
}

#[test]
fn remove_liquidity_fails_on_overdraw_liquidity() {
	let (investor, map) = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);
	ext_build(map).execute_with(|| {
		// investment
		let _ = CennzXSpot::add_liquidity(
			Origin::signed(investor.clone()),
			TRADE_ASSET_A,
			2,  // min_liquidity: T::Balance,
			15, // max_asset_amount: T::Balance,
			10, // core_amount: T::Balance,
		);

		assert_err!(
			CennzXSpot::remove_liquidity(
				Origin::signed(investor.clone()),
				TRADE_ASSET_A,
				20, //`asset_amount` - Amount of exchange asset to burn
				18, //`min_asset_withdraw` - The minimum trade asset withdrawn
				4   //`min_core_withdraw` -  The minimum core asset withdrawn
			),
			"Tried to overdraw liquidity"
		);
		assert_exchange_balance_eq!(CORE_ASSET_ID => 10, TRADE_ASSET_A => 15);
		assert_balance_eq!(investor, TRADE_ASSET_A => 85);
		assert_balance_eq!(investor, CORE_ASSET_ID => 90);
	});
}

#[test]
fn asset_transfer_output() {
	let map1 = with_exchange!(CORE_ASSET_ID => 10, TRADE_ASSET_A => 1000);
	let (buyer, map2) = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A => 2200);
	let (recipient, map3) = with_account!("bob", CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);
	let map = merge!(map1, map2, map3);
	ext_build(map).execute_with(|| {
		// asset to core swap output
		assert_ok!(CennzXSpot::asset_swap_output(
			Origin::signed(buyer.clone()),
			Some(recipient.clone()),
			TRADE_ASSET_A,
			<CoreAssetId<Test>>::get(),
			5,    // buy_amount: T::Balance,
			1400, // max_sale: T::Balance,
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 5, TRADE_ASSET_A => 2004);
		assert_balance_eq!(buyer, TRADE_ASSET_A => 1196);
		assert_balance_eq!(recipient, CORE_ASSET_ID => 105);
	});
}

#[test]
fn core_to_asset_transfer_output() {
	let map1 = with_exchange!(CORE_ASSET_ID => 10, TRADE_ASSET_A => 1000);
	let (buyer, map2) = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A => 2200);
	let (recipient, map3) = with_account!("bob", CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);
	let map = merge!(map1, map2, map3);
	ext_build(map).execute_with(|| {
		// core to asset swap output
		assert_ok!(CennzXSpot::asset_swap_output(
			Origin::signed(buyer.clone()),
			Some(recipient.clone()),
			<CoreAssetId<Test>>::get(),
			TRADE_ASSET_A,
			5,    // buy_amount: T::Balance,
			1400, // max_sale: T::Balance,
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 11, TRADE_ASSET_A => 995);
		assert_balance_eq!(buyer, CORE_ASSET_ID => 2199);
		assert_balance_eq!(recipient, TRADE_ASSET_A => 105);
	});
}

/// Calculate input_amount_without_fee using fee rate and input amount and then calculate price
/// Price = (input_amount_without_fee * output reserve) / (input reserve + input_amount_without_fee)
#[test]
fn get_input_price_for_valid_data() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CennzXSpot::get_input_price(123, 1000, 1000, DefaultFeeRate::get()), 108);

		// No f32/f64 types, so we use large values to test precision
		assert_ok!(
			CennzXSpot::get_input_price(123_000_000, 1_000_000_000, 1_000_000_000, DefaultFeeRate::get()),
			109236233
		);

		assert_ok!(
			CennzXSpot::get_input_price(
				100_000_000_000_000,
				120_627_710_511_649_660,
				4_999_727_416_279_531_363,
				DefaultFeeRate::get()
			),
			4128948876492407
		);

		assert_ok!(
			CennzXSpot::get_input_price(
				100_000_000_000_000,
				120_627_710_511_649_660,
				u128::max_value(),
				DefaultFeeRate::get()
			),
			281017019450612581324176880746747822
		);
	});
}

/// Calculate input_amount_without_fee using fee rate and input amount and then calculate price
/// Price = (input_amount_without_fee * output reserve) / (input reserve + input_amount_without_fee)
// Input amount is half of max(u128) and output reserve is max(u128) and input reserve is half of max(u128)
#[test]
fn get_input_price_for_max_reserve_balance() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(
			CennzXSpot::get_input_price(
				u128::max_value() / 2,
				u128::max_value() / 2,
				u128::max_value(),
				DefaultFeeRate::get()
			),
			169886353929574869427545984738775941814
		);
	});
}

/// Calculate input_amount_without_fee using fee rate and input amount and then calculate price
/// Price = (input_amount_without_fee * output reserve) / (input reserve + input_amount_without_fee)
// Overflows as the input reserve, output reserve and input amount is at max capacity(u128)
#[test]
fn get_input_price_should_fail_with_max_reserve_and_max_amount() {
	ExtBuilder::default().build().execute_with(|| {
		assert_err!(
			CennzXSpot::get_input_price(
				u128::max_value(),
				u128::max_value(),
				u128::max_value(),
				DefaultFeeRate::get()
			),
			"Overflow error"
		);
	});
}

#[test]
fn asset_swap_input_price() {
	let map = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
	ext_build(map).execute_with(|| {
		assert_ok!(
			CennzXSpot::get_asset_to_core_input_price(&TRADE_ASSET_A, 123, DefaultFeeRate::get()),
			108
		);

		assert_ok!(
			CennzXSpot::get_core_to_asset_input_price(&TRADE_ASSET_A, 123, DefaultFeeRate::get()),
			108
		);
	});
}

#[test]
fn asset_swap_input_zero_sell_amount() {
	ExtBuilder::default().build().execute_with(|| {
		assert_err!(
			CennzXSpot::get_asset_to_core_input_price(&TRADE_ASSET_A, 0, DefaultFeeRate::get()),
			"Sell amount must be a positive value"
		);
		assert_err!(
			CennzXSpot::get_core_to_asset_input_price(&TRADE_ASSET_A, 0, DefaultFeeRate::get()),
			"Sell amount must be a positive value"
		);
	});
}

#[test]
fn asset_swap_input_insufficient_balance() {
	let map1 = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
	let (trader, map2) = with_account!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
	let map = merge!(map1, map2);
	ext_build(map).execute_with(|| {
		assert_err!(
			CennzXSpot::make_asset_to_core_input(
				&trader, // seller
				&trader, // recipient
				&TRADE_ASSET_A,
				10001, // sell_amount
				100,   // min buy limit
				DefaultFeeRate::get()
			),
			"Insufficient asset balance in seller account"
		);

		assert_err!(
			CennzXSpot::make_core_to_asset_input(
				&trader, // seller
				&trader, // recipient
				&TRADE_ASSET_A,
				10001, // sell_amount
				100,   // min buy limit
				DefaultFeeRate::get()
			),
			"Insufficient core asset balance in seller account"
		);
	});
}

#[test]
fn asset_to_core_swap_input() {
	let map1 = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
	let (trader, map2) = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A => 2200);
	let map = merge!(map1, map2);
	ext_build(map).execute_with(|| {
		// asset to core swap input
		assert_ok!(CennzXSpot::asset_swap_input(
			Origin::signed(trader.clone()),
			None,
			TRADE_ASSET_A,
			<CoreAssetId<Test>>::get(),
			100, // sell_amount: T::Balance,
			50,  // min buy limit: T::Balance,
		));
		assert_exchange_balance_eq!(CORE_ASSET_ID => 910, TRADE_ASSET_A => 1100);
		assert_balance_eq!(trader, TRADE_ASSET_A => 2100);
		assert_balance_eq!(trader, CORE_ASSET_ID => 2290);
	});
}

#[test]
fn core_to_asset_swap_input() {
	let map1 = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
	let (trader, map2) = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A => 2200);
	let map = merge!(map1, map2);
	ext_build(map).execute_with(|| {
		// core to asset swap input
		assert_ok!(CennzXSpot::asset_swap_input(
			Origin::signed(trader.clone()),
			None,
			<CoreAssetId<Test>>::get(),
			TRADE_ASSET_A,
			100, // sell_amount: T::Balance,
			50,  // min buy limit: T::Balance,
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 1100, TRADE_ASSET_A => 910);
		assert_balance_eq!(trader, CORE_ASSET_ID => 2100);
		assert_balance_eq!(trader, TRADE_ASSET_A => 2290);
	});
}

#[test]
fn make_asset_to_core_input() {
	let map1 = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
	let (trader, map2) = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A => 2200);
	let map = merge!(map1, map2);
	ext_build(map).execute_with(|| {
		assert_ok!(
			CennzXSpot::make_asset_to_core_input(
				&trader, // buyer
				&trader, // recipient
				&TRADE_ASSET_A,
				90,                    // sell_amount: T::Balance,
				50,                    // min buy: T::Balance,
				DefaultFeeRate::get()  // fee_rate
			),
			81
		);

		assert_exchange_balance_eq!(CORE_ASSET_ID => 919, TRADE_ASSET_A => 1090);
		assert_balance_eq!(trader, TRADE_ASSET_A => 2110);
		assert_balance_eq!(trader, CORE_ASSET_ID => 2281);
	});
}

#[test]
fn make_core_to_asset_input() {
	let map1 = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
	let (trader, map2) = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A => 2200);
	let map = merge!(map1, map2);
	ext_build(map).execute_with(|| {
		assert_ok!(
			CennzXSpot::make_core_to_asset_input(
				&trader, // buyer
				&trader, // recipient
				&TRADE_ASSET_A,
				90,                    // sell_amount: T::Balance,
				50,                    // min buy: T::Balance,
				DefaultFeeRate::get()  // fee_rate
			),
			81
		);

		assert_exchange_balance_eq!(CORE_ASSET_ID => 1090, TRADE_ASSET_A => 919);
		assert_balance_eq!(trader, CORE_ASSET_ID => 2110);
		assert_balance_eq!(trader, TRADE_ASSET_A => 2281);
	});
}

#[test]
fn asset_swap_input_zero_asset_sold() {
	let map1 = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
	let (trader, map2) = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);
	let map = merge!(map1, map2);
	ext_build(map).execute_with(|| {
		// asset to core swap input
		assert_err!(
			CennzXSpot::asset_swap_input(
				Origin::signed(trader.clone()),
				None,
				TRADE_ASSET_A,
				<CoreAssetId<Test>>::get(),
				0,   // sell amount
				100, // min buy,
			),
			"Sell amount must be a positive value"
		);
		// core to asset swap input
		assert_err!(
			CennzXSpot::asset_swap_input(
				Origin::signed(trader),
				None,
				<CoreAssetId<Test>>::get(),
				TRADE_ASSET_A,
				0,   // sell amount
				100, // min buy,
			),
			"Sell amount must be a positive value"
		);
	});
}

#[test]
fn asset_swap_input_less_than_min_sale() {
	let map1 = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
	let (trader, map2) = with_account!(CORE_ASSET_ID => 50, TRADE_ASSET_A => 50);
	let map = merge!(map1, map2);
	ext_build(map).execute_with(|| {
		// asset to core swap input
		assert_err!(
			CennzXSpot::asset_swap_input(
				Origin::signed(trader.clone()),
				None,
				TRADE_ASSET_A,
				<CoreAssetId<Test>>::get(),
				50,  // sell_amount
				100, // min buy,
			),
			"The sale value of input is less than the required min."
		);
		// core to asset swap input
		assert_err!(
			CennzXSpot::asset_swap_input(
				Origin::signed(trader),
				None,
				<CoreAssetId<Test>>::get(),
				TRADE_ASSET_A,
				50,  // sell_amount
				100, // min buy,
			),
			"The sale value of input is less than the required min."
		);
	});
}

#[test]
fn asset_to_core_transfer_input() {
	let map1 = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
	let (trader, map2) = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A => 2200);
	let (recipient, map3) = with_account!("bob", CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);
	let map = merge!(map1, map2, map3);
	ext_build(map).execute_with(|| {
		// asset to core swap input
		assert_ok!(CennzXSpot::asset_swap_input(
			Origin::signed(trader.clone()),
			Some(recipient.clone()),
			TRADE_ASSET_A,
			<CoreAssetId<Test>>::get(),
			50, // sell_amount: T::Balance,
			40, // min_sale: T::Balance,
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 954, TRADE_ASSET_A => 1050);
		assert_balance_eq!(trader, TRADE_ASSET_A => 2150);
		assert_balance_eq!(recipient, CORE_ASSET_ID => 146);
	});
}

#[test]
fn core_to_asset_transfer_input() {
	let map1 = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
	let (trader, map2) = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A => 2200);
	let (recipient, map3) = with_account!("bob", CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);
	let map = merge!(map1, map2, map3);
	ext_build(map).execute_with(|| {
		// core to asset swap input
		assert_ok!(CennzXSpot::asset_swap_input(
			Origin::signed(trader.clone()),
			Some(recipient.clone()),
			<CoreAssetId<Test>>::get(),
			TRADE_ASSET_A,
			50, // sell_amount: T::Balance,
			40, // min_sale: T::Balance,
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 1050, TRADE_ASSET_A => 954);
		assert_balance_eq!(trader, CORE_ASSET_ID => 2150);
		assert_balance_eq!(recipient, TRADE_ASSET_A => 146);
	});
}

#[test]
fn asset_to_asset_swap_output() {
	let map1 = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
	let map2 = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B => 1000);
	let (trader, map3) = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A => 2200);
	let map = merge!(map1, map2, map3);
	ext_build(map).execute_with(|| {
		assert_ok!(CennzXSpot::asset_swap_output(
			Origin::signed(trader.clone()),
			None,          // Account to receive asset_bought, defaults to origin if None
			TRADE_ASSET_A, // asset_sold
			TRADE_ASSET_B, // asset_bought
			150,           // buy_amount: T::Balance,
			300,           // maximum asset A to sell
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 823, TRADE_ASSET_A => 1216);
		assert_exchange_balance_eq!(CORE_ASSET_ID => 1177, TRADE_ASSET_B => 850);
		assert_balance_eq!(trader, TRADE_ASSET_A => 1984);
		assert_balance_eq!(trader, TRADE_ASSET_B => 150);
		assert_balance_eq!(trader, CORE_ASSET_ID => 2200);
	});
}

#[test]
fn asset_to_asset_swap_output_zero_asset_sold() {
	let map1 = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
	let map2 = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B => 1000);
	let (trader, map3) = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);
	let map = merge!(map1, map2, map3);
	ext_build(map).execute_with(|| {
		assert_err!(
			CennzXSpot::asset_swap_output(
				Origin::signed(trader),
				None,          // Account to receive asset_bought, defaults to origin if None
				TRADE_ASSET_A, // asset_sold
				TRADE_ASSET_B, // asset_bought
				0,             // buy_amount
				300,           // maximum asset A to sell
			),
			"Buy amount must be a positive value"
		);
	});
}

#[test]
fn asset_to_asset_swap_output_insufficient_balance() {
	let map1 = with_exchange!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);
	let map2 = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B => 1000);
	let (trader, map3) = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 50);
	let map = merge!(map1, map2, map3);
	ext_build(map).execute_with(|| {
		assert_err!(
			CennzXSpot::asset_swap_output(
				Origin::signed(trader),
				None,          // Account to receive asset_bought, defaults to origin if None
				TRADE_ASSET_A, // asset_sold
				TRADE_ASSET_B, // asset_bought
				51,            // buy_amount
				400,           // maximum asset A to sell
			),
			"Insufficient asset balance in buyer account"
		);
	});
}

#[test]
fn asset_to_asset_swap_output_exceed_max_sale() {
	let map1 = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
	let map2 = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B => 1000);
	let (trader, map3) = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);
	let map = merge!(map1, map2, map3);
	ext_build(map).execute_with(|| {
		assert_err!(
			CennzXSpot::asset_swap_output(
				Origin::signed(trader),
				None,          // Account to receive asset_bought, defaults to origin if None
				TRADE_ASSET_A, // asset_sold
				TRADE_ASSET_B, // asset_bought
				156,           // buy_amount
				100,           // maximum asset A to sell
			),
			"Amount of asset sold would exceed the specified max. limit"
		);
	});
}

#[test]
fn asset_to_asset_transfer_output() {
	let map1 = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
	let map2 = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B => 1000);
	let (trader, map3) = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A => 2200);
	let (recipient, map4) = with_account!("bob", CORE_ASSET_ID => 100, TRADE_ASSET_B => 100);
	let map = merge!(map1, map2, map3, map4);
	ext_build(map).execute_with(|| {
		assert_ok!(CennzXSpot::asset_swap_output(
			Origin::signed(trader.clone()),
			Some(recipient.clone()), // Account to receive asset_bought, defaults to origin if None
			TRADE_ASSET_A,           // asset_sold
			TRADE_ASSET_B,           // asset_bought
			150,                     // buy_amount: T::Balance,
			300,                     // maximum asset A to sell
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 823, TRADE_ASSET_A => 1216);
		assert_exchange_balance_eq!(CORE_ASSET_ID => 1177, TRADE_ASSET_B => 850);
		assert_balance_eq!(trader, TRADE_ASSET_A => 1984);
		assert_balance_eq!(recipient, TRADE_ASSET_B => 250);
		assert_balance_eq!(trader, CORE_ASSET_ID => 2200);
	});
}

#[test]
fn asset_to_asset_swap_input() {
	let map1 = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
	let map2 = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B => 1000);
	let (trader, map3) = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A => 2200);
	let map = merge!(map1, map2, map3);
	ext_build(map).execute_with(|| {
		assert_ok!(CennzXSpot::asset_swap_input(
			Origin::signed(trader.clone()),
			None,          // Trader is also recipient so passing None in this case
			TRADE_ASSET_A, // asset_sold
			TRADE_ASSET_B, // asset_bought
			150,           // sell_amount
			100,           // min buy limit for asset B
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 871, TRADE_ASSET_A => 1150);
		assert_exchange_balance_eq!(CORE_ASSET_ID => 1129, TRADE_ASSET_B => 887);
		assert_balance_eq!(trader, TRADE_ASSET_A => 2050);
		assert_balance_eq!(trader, TRADE_ASSET_B => 113);
		assert_balance_eq!(trader, CORE_ASSET_ID => 2200);
	});
}

#[test]
fn asset_to_asset_swap_input_zero_asset_sold() {
	let map1 = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
	let map2 = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B => 1000);
	let (trader, map3) = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);
	let map = merge!(map1, map2, map3);
	ext_build(map).execute_with(|| {
		assert_err!(
			CennzXSpot::asset_swap_input(
				Origin::signed(trader),
				None,          // Trader is also recipient so passing None in this case
				TRADE_ASSET_A, // asset_sold
				TRADE_ASSET_B, // asset_bought
				0,             // sell_amount
				100,           // min buy limit for asset B
			),
			"Sell amount must be a positive value"
		);
	});
}

#[test]
fn asset_to_asset_swap_input_insufficient_balance() {
	let map1 = with_exchange!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);
	let map2 = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B => 1000);
	let (trader, map3) = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 50);
	let map = merge!(map1, map2, map3);
	ext_build(map).execute_with(|| {
		assert_err!(
			CennzXSpot::asset_swap_input(
				Origin::signed(trader),
				None,          // Account to receive asset_bought, defaults to origin if None
				TRADE_ASSET_A, // asset_sold
				TRADE_ASSET_B, // asset_bought
				51,            // sell_amount
				100,           // min buy limit for asset B
			),
			"Insufficient asset balance in seller account"
		);
	});
}

#[test]
fn asset_to_asset_swap_input_less_than_min_sale() {
	let map1 = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
	let map2 = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B => 1000);
	let (trader, map3) = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 200);
	let map = merge!(map1, map2, map3);
	ext_build(map).execute_with(|| {
		assert_err!(
			CennzXSpot::asset_swap_input(
				Origin::signed(trader),
				None,          // Account to receive asset_bought, defaults to origin if None
				TRADE_ASSET_A, // asset_sold
				TRADE_ASSET_B, // asset_bought
				156,           // sell_amount
				200,           // min buy limit for asset B
			),
			"The sale value of input is less than the required min"
		);
	});
}

#[test]
fn asset_to_asset_transfer_input() {
	let map1 = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
	let map2 = with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B => 1000);
	let (trader, map3) = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A => 2200);
	let (recipient, map4) = with_account!("bob", CORE_ASSET_ID => 100, TRADE_ASSET_B => 100);
	let map = merge!(map1, map2, map3, map4);
	ext_build(map).execute_with(|| {
		assert_ok!(CennzXSpot::asset_swap_input(
			Origin::signed(trader.clone()),
			Some(recipient.clone()), // Account to receive asset_bought, defaults to origin if None
			TRADE_ASSET_A,           // asset_sold
			TRADE_ASSET_B,           // asset_bought
			150,                     // sell_amount
			100,                     // min buy limit for asset B
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 871, TRADE_ASSET_A => 1150);
		assert_exchange_balance_eq!(CORE_ASSET_ID => 1129, TRADE_ASSET_B => 887);
		assert_balance_eq!(trader, TRADE_ASSET_A => 2050);
		assert_balance_eq!(recipient, TRADE_ASSET_B => 213);
		assert_balance_eq!(trader, CORE_ASSET_ID => 2200);
	});
}

#[test]
fn set_fee_rate() {
	ExtBuilder::default().build().execute_with(|| {
		let new_fee_rate = FeeRate::from_milli(5);
		assert_ok!(CennzXSpot::set_fee_rate(Origin::ROOT, new_fee_rate), ());
		assert_eq!(CennzXSpot::fee_rate(), new_fee_rate);
	});
}
