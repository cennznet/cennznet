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
//!
//! CENNZX-SPOT Tests
//!
#![cfg(test)]
use crate::{
	impls::{ExchangeAddressFor, ExchangeAddressGenerator},
	mock::{self, CORE_ASSET_ID, TRADE_ASSET_A_ID, TRADE_ASSET_B_ID},
	types::{FeeRate, LowPrecisionUnsigned, PerMilli, PerMillion},
	Call, CoreAssetId, DefaultFeeRate, Error, GenesisConfig, Module, Trait,
};
use core::convert::TryFrom;
use frame_support::{additional_traits::DummyDispatchVerifier, impl_outer_origin, traits::Currency, StorageValue};
use pallet_generic_asset;
use sp_core::{crypto::UncheckedInto, sr25519, H256};
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
			permissions: vec![],
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
				resolve_asset_id!($a1),
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
				resolve_asset_id!($a1),
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

// Default exchange asset IDs
const DEFAULT_EXCHANGE_KEY: (u32, u32) = (
	resolve_asset_id!(CoreAssetCurrency),
	resolve_asset_id!(TradeAssetCurrencyA),
);

// Alias the types with `Test`, for convenience
type CoreAssetCurrency = mock::CoreAssetCurrency<Test>;
type TradeAssetCurrencyA = mock::TradeAssetCurrencyA<Test>;
type TradeAssetCurrencyB = mock::TradeAssetCurrencyB<Test>;

#[test]
fn investor_can_add_liquidity() {
	ExtBuilder::default().build().execute_with(|| {
		let investor: AccountId = with_account!(CoreAssetCurrency => 100, TradeAssetCurrencyA => 100);

		// First investment
		assert_ok!(CennzXSpot::add_liquidity(
			Origin::signed(investor.clone()),
			resolve_asset_id!(TradeAssetCurrencyA),
			2,  // min_liquidity: T::Balance,
			15, // max_asset_amount: T::Balance,
			10, // core_amount: T::Balance,
		));

		// Second investment
		// because a round up, second time asset amount become 15 + 1
		assert_ok!(CennzXSpot::add_liquidity(
			Origin::signed(H256::from_low_u64_be(1).unchecked_into()),
			resolve_asset_id!(TradeAssetCurrencyA),
			2,  // min_liquidity: T::Balance,
			16, // max_asset_amount: T::Balance,
			10, // core_amount: T::Balance,
		));

		assert_exchange_balance_eq!(CoreAssetCurrency => 20, TradeAssetCurrencyA => 31);
		assert_eq!(CennzXSpot::get_liquidity(&DEFAULT_EXCHANGE_KEY, &investor), 20);
	});
}

#[test]
fn get_output_price_zero_cases() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);

		assert_err!(
			CennzXSpot::get_output_price(100, 0, 10, DefaultFeeRate::get()),
			Error::<Test>::EmptyExchangePool
		);

		assert_err!(
			CennzXSpot::get_output_price(100, 10, 0, DefaultFeeRate::get()),
			Error::<Test>::EmptyExchangePool
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
				LowPrecisionUnsigned::max_value() / 2,
				LowPrecisionUnsigned::max_value() / 2,
				LowPrecisionUnsigned::max_value(),
				DefaultFeeRate::get()
			),
			170651607010850639426882365627031758044
		);
	});
}

/// Formula Price = ((input reserve * output amount) / (output reserve - output amount)) +  1  (round up)
/// and apply fee rate to the price
// Overflows as the both input and output reserve is at max capacity and output amount is little less than max of Balance
#[test]
fn get_output_price_should_fail_with_max_reserve_and_max_amount() {
	ExtBuilder::default().build().execute_with(|| {
		assert_err!(
			CennzXSpot::get_output_price(
				LowPrecisionUnsigned::max_value() - 100,
				LowPrecisionUnsigned::max_value(),
				LowPrecisionUnsigned::max_value(),
				DefaultFeeRate::get()
			),
			Error::<Test>::Overflow
		);
	});
}

/// Formula Price = ((input reserve * output amount) / (output reserve - output amount)) +  1  (round up)
/// and apply fee rate to the price
#[test]
fn get_output_price_max_withdrawal() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);

		assert_err!(
			CennzXSpot::get_output_price(1000, 1000, 1000, DefaultFeeRate::get()),
			Error::<Test>::InsufficientAssetReserve
		);

		assert_err!(
			CennzXSpot::get_output_price(1_000_000, 1000, 1000, DefaultFeeRate::get()),
			Error::<Test>::InsufficientAssetReserve
		);
	});
}

#[test]
fn asset_swap_output_price() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);

		assert_ok!(
			CennzXSpot::get_asset_to_core_output_price(
				&resolve_asset_id!(TradeAssetCurrencyA),
				123,
				DefaultFeeRate::get()
			),
			141
		);

		assert_ok!(
			CennzXSpot::get_core_to_asset_output_price(
				&resolve_asset_id!(TradeAssetCurrencyA),
				123,
				DefaultFeeRate::get()
			),
			141
		);
	});
}

#[test]
fn asset_swap_output_zero_buy_amount() {
	ExtBuilder::default().build().execute_with(|| {
		assert_err!(
			CennzXSpot::get_core_to_asset_output_price(
				&resolve_asset_id!(TradeAssetCurrencyA),
				0,
				DefaultFeeRate::get()
			),
			Error::<Test>::BuyAmountNotPositive
		);
		assert_err!(
			CennzXSpot::get_asset_to_core_output_price(
				&resolve_asset_id!(TradeAssetCurrencyA),
				0,
				DefaultFeeRate::get()
			),
			Error::<Test>::BuyAmountNotPositive
		);
	});
}

#[test]
fn asset_swap_output_insufficient_reserve() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);

		assert_err!(
			CennzXSpot::get_asset_to_core_output_price(
				&resolve_asset_id!(TradeAssetCurrencyA),
				1001, // amount_bought
				DefaultFeeRate::get()
			),
			Error::<Test>::InsufficientAssetReserve
		);

		assert_err!(
			CennzXSpot::get_core_to_asset_output_price(
				&resolve_asset_id!(TradeAssetCurrencyA),
				1001, // amount_bought
				DefaultFeeRate::get()
			),
			Error::<Test>::InsufficientAssetReserve
		);
	});
}

#[test]
fn asset_to_core_swap_output() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 10, TradeAssetCurrencyA => 1000);
		let trader: AccountId = with_account!(CoreAssetCurrency => 2200, TradeAssetCurrencyA => 2200);

		// asset to core swap output
		assert_ok!(CennzXSpot::asset_swap_output(
			Origin::signed(trader.clone()),
			None,
			resolve_asset_id!(TradeAssetCurrencyA),
			<CoreAssetId<Test>>::get(),
			5,    // buy_amount: T::Balance,
			1400, // max_sale: T::Balance,
		));

		assert_exchange_balance_eq!(CoreAssetCurrency => 5, TradeAssetCurrencyA => 2004);
		assert_balance_eq!(trader, TradeAssetCurrencyA => 1196);
	});
}

#[test]
fn make_asset_to_core_swap_output() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 10, TradeAssetCurrencyA => 1000);
		let trader = with_account!(CoreAssetCurrency => 2200, TradeAssetCurrencyA => 2200);

		assert_ok!(
			CennzXSpot::make_asset_to_core_output(
				&trader, // buyer
				&trader, // recipient
				&resolve_asset_id!(TradeAssetCurrencyA),
				5,                                                                          // buy_amount: T::Balance,
				1400,                                                                       // max_sale: T::Balance,
				FeeRate::<PerMillion>::try_from(FeeRate::<PerMilli>::from(3u128)).unwrap(), // fee_rate
			),
			1004
		);

		assert_exchange_balance_eq!(CoreAssetCurrency => 5, TradeAssetCurrencyA => 2004);
		assert_balance_eq!(trader, TradeAssetCurrencyA => 1196);
	});
}

#[test]
fn asset_swap_output_zero_asset_sold() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);
		let trader: AccountId = with_account!(CoreAssetCurrency => 100, TradeAssetCurrencyA => 100);

		// asset to core swap output
		assert_err!(
			CennzXSpot::asset_swap_output(
				Origin::signed(trader.clone()),
				None,
				resolve_asset_id!(TradeAssetCurrencyA),
				<CoreAssetId<Test>>::get(),
				0,   // buy_amount
				100, // max_sale,
			),
			Error::<Test>::BuyAmountNotPositive
		);
		// core to asset swap output
		assert_err!(
			CennzXSpot::asset_swap_output(
				Origin::signed(trader),
				None,
				<CoreAssetId<Test>>::get(),
				resolve_asset_id!(TradeAssetCurrencyA),
				0,   // buy_amount
				100, // max_sale,
			),
			Error::<Test>::BuyAmountNotPositive
		);
	});
}

#[test]
fn asset_swap_output_insufficient_balance() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 500, TradeAssetCurrencyA => 500);
		let trader: AccountId = with_account!(CoreAssetCurrency => 100, TradeAssetCurrencyA => 50);

		// asset to core swap output
		assert_err!(
			CennzXSpot::asset_swap_output(
				Origin::signed(trader.clone()),
				None,
				resolve_asset_id!(TradeAssetCurrencyA),
				<CoreAssetId<Test>>::get(),
				51,  // buy_amount
				500, // max_sale,
			),
			Error::<Test>::InsufficientBuyerTradeAssetBalance
		);
		// core to asset swap output
		assert_err!(
			CennzXSpot::asset_swap_output(
				Origin::signed(trader),
				None,
				<CoreAssetId<Test>>::get(),
				resolve_asset_id!(TradeAssetCurrencyA),
				101, // buy_amount
				500, // max_sale,
			),
			Error::<Test>::InsufficientBuyerCoreAssetBalance
		);
	});
}

#[test]
fn asset_swap_output_exceed_max_sale() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);
		let trader: AccountId = with_account!(CoreAssetCurrency => 50, TradeAssetCurrencyA => 50);

		// asset to core swap output
		assert_err!(
			CennzXSpot::asset_swap_output(
				Origin::signed(trader.clone()),
				None,
				resolve_asset_id!(TradeAssetCurrencyA),
				<CoreAssetId<Test>>::get(),
				50, // buy_amount
				0,  // max_sale,
			),
			Error::<Test>::AssetToCorePriceAboveMaxLimit
		);

		// core to asset swap output
		assert_err!(
			CennzXSpot::asset_swap_output(
				Origin::signed(trader),
				None,
				<CoreAssetId<Test>>::get(),
				resolve_asset_id!(TradeAssetCurrencyA),
				50, // buy_amount
				0,  // max_sale,
			),
			Error::<Test>::CoreToAssetPriceAboveMaxLimit
		);
	});
}

#[test]
fn core_to_asset_swap_output() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 10);
		let trader: AccountId = with_account!(CoreAssetCurrency => 2200, TradeAssetCurrencyA => 2200);

		// core to asset swap output
		assert_ok!(CennzXSpot::asset_swap_output(
			Origin::signed(trader.clone()),
			None,
			<CoreAssetId<Test>>::get(),
			resolve_asset_id!(TradeAssetCurrencyA),
			5,    // buy_amount: T::Balance,
			1400, // max_sale: T::Balance,
		));

		assert_exchange_balance_eq!(CoreAssetCurrency => 2004, TradeAssetCurrencyA => 5);
		assert_balance_eq!(trader, CoreAssetCurrency => 1196);
		assert_balance_eq!(trader, TradeAssetCurrencyA => 2205);
	});
}

#[test]
fn make_core_to_asset_output() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 10);
		let buyer = with_account!(CoreAssetCurrency => 2200, TradeAssetCurrencyA => 2200);
		let recipient = with_account!("bob", CoreAssetCurrency => 0, TradeAssetCurrencyA => 0);

		assert_ok!(
			CennzXSpot::make_core_to_asset_output(
				&buyer,
				&recipient,
				&resolve_asset_id!(TradeAssetCurrencyA),
				5,                                                                          // buy_amount: T::Balance,
				1400,                                                                       // max_sale: T::Balance,
				FeeRate::<PerMillion>::try_from(FeeRate::<PerMilli>::from(3u128)).unwrap(), // fee_rate
			),
			1004
		);

		assert_exchange_balance_eq!(CoreAssetCurrency => 2004, TradeAssetCurrencyA => 5);
		assert_balance_eq!(buyer, CoreAssetCurrency => 1196);
		assert_balance_eq!(recipient, TradeAssetCurrencyA => 5);
	});
}

#[test]
fn remove_liquidity() {
	ExtBuilder::default().build().execute_with(|| {
		let investor: AccountId = with_account!(CoreAssetCurrency => 100, TradeAssetCurrencyA => 100);

		// investment
		let _ = CennzXSpot::add_liquidity(
			Origin::signed(investor.clone()),
			resolve_asset_id!(TradeAssetCurrencyA),
			2,  // min_liquidity: T::Balance,
			15, // max_asset_amount: T::Balance,
			10, // core_amount: T::Balance,
		);

		assert_ok!(CennzXSpot::remove_liquidity(
			Origin::signed(investor.clone()),
			resolve_asset_id!(TradeAssetCurrencyA),
			10, //`asset_amount` - Amount of exchange asset to burn
			4,  //`min_asset_withdraw` - The minimum trade asset withdrawn
			4   //`min_core_withdraw` -  The minimum core asset withdrawn
		));
		assert_exchange_balance_eq!(CoreAssetCurrency => 0, TradeAssetCurrencyA => 0);
		assert_balance_eq!(investor, TradeAssetCurrencyA => 100);
		assert_balance_eq!(investor, CoreAssetCurrency => 100);
	});
}

#[test]
fn remove_liquidity_fails_min_core_asset_limit() {
	ExtBuilder::default().build().execute_with(|| {
		let investor: AccountId = with_account!(CoreAssetCurrency => 100, TradeAssetCurrencyA => 100);

		// investment
		let _ = CennzXSpot::add_liquidity(
			Origin::signed(investor.clone()),
			resolve_asset_id!(TradeAssetCurrencyA),
			2,  // min_liquidity: T::Balance,
			15, // max_asset_amount: T::Balance,
			10, // core_amount: T::Balance,
		);

		assert_err!(
			CennzXSpot::remove_liquidity(
				Origin::signed(investor.clone()),
				resolve_asset_id!(TradeAssetCurrencyA),
				10, //`asset_amount` - Amount of exchange asset to burn
				4,  //`min_asset_withdraw` - The minimum trade asset withdrawn
				14  //`min_core_withdraw` -  The minimum core asset withdrawn
			),
			Error::<Test>::MinimumCoreAssetIsRequired
		);
		assert_exchange_balance_eq!(CoreAssetCurrency => 10, TradeAssetCurrencyA => 15);
		assert_balance_eq!(investor, TradeAssetCurrencyA => 85);
		assert_balance_eq!(investor, CoreAssetCurrency => 90);
	});
}

#[test]
fn remove_liquidity_fails_min_trade_asset_limit() {
	ExtBuilder::default().build().execute_with(|| {
		let investor: AccountId = with_account!(CoreAssetCurrency => 100, TradeAssetCurrencyA => 100);

		// investment
		let _ = CennzXSpot::add_liquidity(
			Origin::signed(investor.clone()),
			resolve_asset_id!(TradeAssetCurrencyA),
			2,  // min_liquidity: T::Balance,
			15, // max_asset_amount: T::Balance,
			10, // core_amount: T::Balance,
		);

		assert_err!(
			CennzXSpot::remove_liquidity(
				Origin::signed(investor.clone()),
				resolve_asset_id!(TradeAssetCurrencyA),
				10, //`asset_amount` - Amount of exchange asset to burn
				18, //`min_asset_withdraw` - The minimum trade asset withdrawn
				4   //`min_core_withdraw` -  The minimum core asset withdrawn
			),
			Error::<Test>::MinimumTradeAssetIsRequired
		);
		assert_exchange_balance_eq!(CoreAssetCurrency => 10, TradeAssetCurrencyA => 15);
		assert_balance_eq!(investor, TradeAssetCurrencyA => 85);
		assert_balance_eq!(investor, CoreAssetCurrency => 90);
	});
}

#[test]
fn remove_liquidity_fails_on_overdraw_liquidity() {
	ExtBuilder::default().build().execute_with(|| {
		let investor: AccountId = with_account!(CoreAssetCurrency => 100, TradeAssetCurrencyA => 100);

		// investment
		let _ = CennzXSpot::add_liquidity(
			Origin::signed(investor.clone()),
			resolve_asset_id!(TradeAssetCurrencyA),
			2,  // min_liquidity: T::Balance,
			15, // max_asset_amount: T::Balance,
			10, // core_amount: T::Balance,
		);

		assert_err!(
			CennzXSpot::remove_liquidity(
				Origin::signed(investor.clone()),
				resolve_asset_id!(TradeAssetCurrencyA),
				20, //`asset_amount` - Amount of exchange asset to burn
				18, //`min_asset_withdraw` - The minimum trade asset withdrawn
				4   //`min_core_withdraw` -  The minimum core asset withdrawn
			),
			Error::<Test>::LiquidityTooLow
		);
		assert_exchange_balance_eq!(CoreAssetCurrency => 10, TradeAssetCurrencyA => 15);
		assert_balance_eq!(investor, TradeAssetCurrencyA => 85);
		assert_balance_eq!(investor, CoreAssetCurrency => 90);
	});
}

#[test]
fn asset_transfer_output() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 10, TradeAssetCurrencyA => 1000);
		let buyer: AccountId = with_account!(CoreAssetCurrency => 2200, TradeAssetCurrencyA => 2200);
		let recipient = with_account!("bob", CoreAssetCurrency => 100, TradeAssetCurrencyA => 100);

		// asset to core swap output
		assert_ok!(CennzXSpot::asset_swap_output(
			Origin::signed(buyer.clone()),
			Some(recipient.clone()),
			resolve_asset_id!(TradeAssetCurrencyA),
			<CoreAssetId<Test>>::get(),
			5,    // buy_amount: T::Balance,
			1400, // max_sale: T::Balance,
		));

		assert_exchange_balance_eq!(CoreAssetCurrency => 5, TradeAssetCurrencyA => 2004);
		assert_balance_eq!(buyer, TradeAssetCurrencyA => 1196);
		assert_balance_eq!(recipient, CoreAssetCurrency => 105);
	});
}

#[test]
fn core_to_asset_transfer_output() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 10, TradeAssetCurrencyA => 1000);
		let buyer: AccountId = with_account!(CoreAssetCurrency => 2200, TradeAssetCurrencyA => 2200);
		let recipient: AccountId = with_account!("bob", CoreAssetCurrency => 100, TradeAssetCurrencyA => 100);

		// core to asset swap output
		assert_ok!(CennzXSpot::asset_swap_output(
			Origin::signed(buyer.clone()),
			Some(recipient.clone()),
			<CoreAssetId<Test>>::get(),
			resolve_asset_id!(TradeAssetCurrencyA),
			5,    // buy_amount: T::Balance,
			1400, // max_sale: T::Balance,
		));

		assert_exchange_balance_eq!(CoreAssetCurrency => 11, TradeAssetCurrencyA => 995);
		assert_balance_eq!(buyer, CoreAssetCurrency => 2199);
		assert_balance_eq!(recipient, TradeAssetCurrencyA => 105);
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
				LowPrecisionUnsigned::max_value(),
				DefaultFeeRate::get()
			),
			281017019450612581324176880746747822
		);
	});
}

/// Calculate input_amount_without_fee using fee rate and input amount and then calculate price
/// Price = (input_amount_without_fee * output reserve) / (input reserve + input_amount_without_fee)
// Input amount is half of max(Balance) and output reserve is max(Balance) and input reserve is half of max(Balance)
#[test]
fn get_input_price_for_max_reserve_balance() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(
			CennzXSpot::get_input_price(
				LowPrecisionUnsigned::max_value() / 2,
				LowPrecisionUnsigned::max_value() / 2,
				LowPrecisionUnsigned::max_value(),
				DefaultFeeRate::get()
			),
			169886353929574869427545984738775941814
		);
	});
}

#[test]
fn asset_swap_input_price() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);

		assert_ok!(
			CennzXSpot::get_asset_to_core_input_price(
				&resolve_asset_id!(TradeAssetCurrencyA),
				123,
				DefaultFeeRate::get()
			),
			108
		);

		assert_ok!(
			CennzXSpot::get_core_to_asset_input_price(
				&resolve_asset_id!(TradeAssetCurrencyA),
				123,
				DefaultFeeRate::get()
			),
			108
		);
	});
}

#[test]
fn asset_swap_input_zero_sell_amount() {
	ExtBuilder::default().build().execute_with(|| {
		assert_err!(
			CennzXSpot::get_asset_to_core_input_price(
				&resolve_asset_id!(TradeAssetCurrencyA),
				0,
				DefaultFeeRate::get()
			),
			Error::<Test>::AssetToCoreSellAmountNotAboveZero
		);
		assert_err!(
			CennzXSpot::get_core_to_asset_input_price(
				&resolve_asset_id!(TradeAssetCurrencyA),
				0,
				DefaultFeeRate::get()
			),
			Error::<Test>::CoreToAssetSellAmountNotAboveZero
		);
	});
}

#[test]
fn asset_swap_input_insufficient_balance() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);

		let trader = with_account!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);
		assert_err!(
			CennzXSpot::make_asset_to_core_input(
				&trader, // seller
				&trader, // recipient
				&resolve_asset_id!(TradeAssetCurrencyA),
				10001, // sell_amount
				100,   // min buy limit
				DefaultFeeRate::get()
			),
			Error::<Test>::InsufficientSellerTradeAssetBalance
		);

		assert_err!(
			CennzXSpot::make_core_to_asset_input(
				&trader, // seller
				&trader, // recipient
				&resolve_asset_id!(TradeAssetCurrencyA),
				10001, // sell_amount
				100,   // min buy limit
				DefaultFeeRate::get()
			),
			Error::<Test>::InsufficientSellerCoreAssetBalance
		);
	});
}

#[test]
fn asset_to_core_swap_input() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);
		let trader: AccountId = with_account!(CoreAssetCurrency => 2200, TradeAssetCurrencyA => 2200);

		// asset to core swap input
		assert_ok!(CennzXSpot::asset_swap_input(
			Origin::signed(trader.clone()),
			None,
			resolve_asset_id!(TradeAssetCurrencyA),
			<CoreAssetId<Test>>::get(),
			100, // sell_amount: T::Balance,
			50,  // min buy limit: T::Balance,
		));
		assert_exchange_balance_eq!(CoreAssetCurrency => 910, TradeAssetCurrencyA => 1100);
		assert_balance_eq!(trader, TradeAssetCurrencyA => 2100);
		assert_balance_eq!(trader, CoreAssetCurrency => 2290);
	});
}

#[test]
fn core_to_asset_swap_input() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);
		let trader: AccountId = with_account!(CoreAssetCurrency => 2200, TradeAssetCurrencyA => 2200);

		// core to asset swap input
		assert_ok!(CennzXSpot::asset_swap_input(
			Origin::signed(trader.clone()),
			None,
			<CoreAssetId<Test>>::get(),
			resolve_asset_id!(TradeAssetCurrencyA),
			100, // sell_amount: T::Balance,
			50,  // min buy limit: T::Balance,
		));

		assert_exchange_balance_eq!(CoreAssetCurrency => 1100, TradeAssetCurrencyA => 910);
		assert_balance_eq!(trader, CoreAssetCurrency => 2100);
		assert_balance_eq!(trader, TradeAssetCurrencyA => 2290);
	});
}

#[test]
fn make_asset_to_core_input() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);
		let trader = with_account!(CoreAssetCurrency => 2200, TradeAssetCurrencyA => 2200);

		assert_ok!(
			CennzXSpot::make_asset_to_core_input(
				&trader, // buyer
				&trader, // recipient
				&resolve_asset_id!(TradeAssetCurrencyA),
				90,                    // sell_amount: T::Balance,
				50,                    // min buy: T::Balance,
				DefaultFeeRate::get()  // fee_rate
			),
			81
		);

		assert_exchange_balance_eq!(CoreAssetCurrency => 919, TradeAssetCurrencyA => 1090);
		assert_balance_eq!(trader, TradeAssetCurrencyA => 2110);
		assert_balance_eq!(trader, CoreAssetCurrency => 2281);
	});
}

#[test]
fn make_core_to_asset_input() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);
		let trader = with_account!(CoreAssetCurrency => 2200, TradeAssetCurrencyA => 2200);

		assert_ok!(
			CennzXSpot::make_core_to_asset_input(
				&trader, // buyer
				&trader, // recipient
				&resolve_asset_id!(TradeAssetCurrencyA),
				90,                    // sell_amount: T::Balance,
				50,                    // min buy: T::Balance,
				DefaultFeeRate::get()  // fee_rate
			),
			81
		);

		assert_exchange_balance_eq!(CoreAssetCurrency => 1090, TradeAssetCurrencyA => 919);
		assert_balance_eq!(trader, CoreAssetCurrency => 2110);
		assert_balance_eq!(trader, TradeAssetCurrencyA => 2281);
	});
}

#[test]
fn asset_swap_input_zero_asset_sold() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);
		let trader: AccountId = with_account!(CoreAssetCurrency => 100, TradeAssetCurrencyA => 100);
		// asset to core swap input
		assert_err!(
			CennzXSpot::asset_swap_input(
				Origin::signed(trader.clone()),
				None,
				resolve_asset_id!(TradeAssetCurrencyA),
				<CoreAssetId<Test>>::get(),
				0,   // sell amount
				100, // min buy,
			),
			Error::<Test>::AssetToCoreSellAmountNotAboveZero
		);
		// core to asset swap input
		assert_err!(
			CennzXSpot::asset_swap_input(
				Origin::signed(trader),
				None,
				<CoreAssetId<Test>>::get(),
				resolve_asset_id!(TradeAssetCurrencyA),
				0,   // sell amount
				100, // min buy,
			),
			Error::<Test>::CoreToAssetSellAmountNotAboveZero
		);
	});
}

#[test]
fn asset_swap_input_less_than_min_sale() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);
		let trader: AccountId = with_account!(CoreAssetCurrency => 50, TradeAssetCurrencyA => 50);

		// asset to core swap input
		assert_err!(
			CennzXSpot::asset_swap_input(
				Origin::signed(trader.clone()),
				None,
				resolve_asset_id!(TradeAssetCurrencyA),
				<CoreAssetId<Test>>::get(),
				50,  // sell_amount
				100, // min buy,
			),
			Error::<Test>::SaleValueBelowRequiredMinimum
		);
		// core to asset swap input
		assert_err!(
			CennzXSpot::asset_swap_input(
				Origin::signed(trader),
				None,
				<CoreAssetId<Test>>::get(),
				resolve_asset_id!(TradeAssetCurrencyA),
				50,  // sell_amount
				100, // min buy,
			),
			Error::<Test>::SaleValueBelowRequiredMinimum
		);
	});
}

#[test]
fn asset_to_core_transfer_input() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);
		let trader: AccountId = with_account!(CoreAssetCurrency => 2200, TradeAssetCurrencyA => 2200);
		let recipient: AccountId = with_account!("bob", CoreAssetCurrency => 100, TradeAssetCurrencyA => 100);

		// asset to core swap input
		assert_ok!(CennzXSpot::asset_swap_input(
			Origin::signed(trader.clone()),
			Some(recipient.clone()),
			resolve_asset_id!(TradeAssetCurrencyA),
			<CoreAssetId<Test>>::get(),
			50, // sell_amount: T::Balance,
			40, // min_sale: T::Balance,
		));

		assert_exchange_balance_eq!(CoreAssetCurrency => 954, TradeAssetCurrencyA => 1050);
		assert_balance_eq!(trader, TradeAssetCurrencyA => 2150);
		assert_balance_eq!(recipient, CoreAssetCurrency => 146);
	});
}

#[test]
fn core_to_asset_transfer_input() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);
		let trader: AccountId = with_account!(CoreAssetCurrency => 2200, TradeAssetCurrencyA => 2200);
		let recipient: AccountId = with_account!("bob", CoreAssetCurrency => 100, TradeAssetCurrencyA => 100);

		// core to asset swap input
		assert_ok!(CennzXSpot::asset_swap_input(
			Origin::signed(trader.clone()),
			Some(recipient.clone()),
			<CoreAssetId<Test>>::get(),
			resolve_asset_id!(TradeAssetCurrencyA),
			50, // sell_amount: T::Balance,
			40, // min_sale: T::Balance,
		));

		assert_exchange_balance_eq!(CoreAssetCurrency => 1050, TradeAssetCurrencyA => 954);
		assert_balance_eq!(trader, CoreAssetCurrency => 2150);
		assert_balance_eq!(recipient, TradeAssetCurrencyA => 146);
	});
}

#[test]
fn asset_to_asset_swap_output() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyB => 1000);
		let trader: AccountId = with_account!(CoreAssetCurrency => 2200, TradeAssetCurrencyA => 2200);

		assert_ok!(CennzXSpot::asset_swap_output(
			Origin::signed(trader.clone()),
			None,                                   // Account to receive asset_bought, defaults to origin if None
			resolve_asset_id!(TradeAssetCurrencyA), // asset_sold
			resolve_asset_id!(TradeAssetCurrencyB), // asset_bought
			150,                                    // buy_amount: T::Balance,
			300,                                    // maximum asset A to sell
		));

		assert_exchange_balance_eq!(CoreAssetCurrency => 823, TradeAssetCurrencyA => 1216);
		assert_exchange_balance_eq!(CoreAssetCurrency => 1177, TradeAssetCurrencyB => 850);
		assert_balance_eq!(trader, TradeAssetCurrencyA => 1984);
		assert_balance_eq!(trader, TradeAssetCurrencyB => 150);
		assert_balance_eq!(trader, CoreAssetCurrency => 2200);
	});
}

#[test]
fn asset_to_asset_swap_output_zero_asset_sold() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyB => 1000);
		let trader = with_account!(CoreAssetCurrency => 100, TradeAssetCurrencyA => 100);

		assert_err!(
			CennzXSpot::asset_swap_output(
				Origin::signed(trader),
				None,                                   // Account to receive asset_bought, defaults to origin if None
				resolve_asset_id!(TradeAssetCurrencyA), // asset_sold
				resolve_asset_id!(TradeAssetCurrencyB), // asset_bought
				0,                                      // buy_amount
				300,                                    // maximum asset A to sell
			),
			Error::<Test>::BuyAmountNotPositive
		);
	});
}

#[test]
fn asset_to_asset_swap_output_insufficient_balance() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 100, TradeAssetCurrencyA => 100);
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyB => 1000);
		let trader = with_account!(CoreAssetCurrency => 100, TradeAssetCurrencyA => 50);

		assert_err!(
			CennzXSpot::asset_swap_output(
				Origin::signed(trader),
				None,                                   // Account to receive asset_bought, defaults to origin if None
				resolve_asset_id!(TradeAssetCurrencyA), // asset_sold
				resolve_asset_id!(TradeAssetCurrencyB), // asset_bought
				51,                                     // buy_amount
				400,                                    // maximum asset A to sell
			),
			Error::<Test>::InsufficientBuyerTradeAssetBalance
		);
	});
}

#[test]
fn asset_to_asset_swap_output_exceed_max_sale() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyB => 1000);
		let trader = with_account!(CoreAssetCurrency => 100, TradeAssetCurrencyA => 100);

		assert_err!(
			CennzXSpot::asset_swap_output(
				Origin::signed(trader),
				None,                                   // Account to receive asset_bought, defaults to origin if None
				resolve_asset_id!(TradeAssetCurrencyA), // asset_sold
				resolve_asset_id!(TradeAssetCurrencyB), // asset_bought
				156,                                    // buy_amount
				100,                                    // maximum asset A to sell
			),
			Error::<Test>::AssetToAssetPriceAboveMaxLimit
		);
	});
}

#[test]
fn asset_to_asset_transfer_output() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyB => 1000);
		let trader: AccountId = with_account!(CoreAssetCurrency => 2200, TradeAssetCurrencyA => 2200);
		let recipient: AccountId = with_account!("bob", CoreAssetCurrency => 100, TradeAssetCurrencyB => 100);

		assert_ok!(CennzXSpot::asset_swap_output(
			Origin::signed(trader.clone()),
			Some(recipient.clone()), // Account to receive asset_bought, defaults to origin if None
			resolve_asset_id!(TradeAssetCurrencyA), // asset_sold
			resolve_asset_id!(TradeAssetCurrencyB), // asset_bought
			150,                     // buy_amount: T::Balance,
			300,                     // maximum asset A to sell
		));

		assert_exchange_balance_eq!(CoreAssetCurrency => 823, TradeAssetCurrencyA => 1216);
		assert_exchange_balance_eq!(CoreAssetCurrency => 1177, TradeAssetCurrencyB => 850);
		assert_balance_eq!(trader, TradeAssetCurrencyA => 1984);
		assert_balance_eq!(recipient, TradeAssetCurrencyB => 250);
		assert_balance_eq!(trader, CoreAssetCurrency => 2200);
	});
}

#[test]
fn asset_to_asset_swap_input() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyB => 1000);
		let trader: AccountId = with_account!(CoreAssetCurrency => 2200, TradeAssetCurrencyA => 2200);

		assert_ok!(CennzXSpot::asset_swap_input(
			Origin::signed(trader.clone()),
			None,                                   // Trader is also recipient so passing None in this case
			resolve_asset_id!(TradeAssetCurrencyA), // asset_sold
			resolve_asset_id!(TradeAssetCurrencyB), // asset_bought
			150,                                    // sell_amount
			100,                                    // min buy limit for asset B
		));

		assert_exchange_balance_eq!(CoreAssetCurrency => 871, TradeAssetCurrencyA => 1150);
		assert_exchange_balance_eq!(CoreAssetCurrency => 1129, TradeAssetCurrencyB => 887);
		assert_balance_eq!(trader, TradeAssetCurrencyA => 2050);
		assert_balance_eq!(trader, TradeAssetCurrencyB => 113);
		assert_balance_eq!(trader, CoreAssetCurrency => 2200);
	});
}

#[test]
fn asset_to_asset_swap_input_zero_asset_sold() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyB => 1000);
		let trader = with_account!(CoreAssetCurrency => 100, TradeAssetCurrencyA => 100);

		assert_err!(
			CennzXSpot::asset_swap_input(
				Origin::signed(trader),
				None,                                   // Trader is also recipient so passing None in this case
				resolve_asset_id!(TradeAssetCurrencyA), // asset_sold
				resolve_asset_id!(TradeAssetCurrencyB), // asset_bought
				0,                                      // sell_amount
				100,                                    // min buy limit for asset B
			),
			Error::<Test>::AssetToCoreSellAmountNotAboveZero
		);
	});
}

#[test]
fn asset_to_asset_swap_input_insufficient_balance() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 100, TradeAssetCurrencyA => 100);
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyB => 1000);
		let trader = with_account!(CoreAssetCurrency => 100, TradeAssetCurrencyA => 50);

		assert_err!(
			CennzXSpot::asset_swap_input(
				Origin::signed(trader),
				None,                                   // Account to receive asset_bought, defaults to origin if None
				resolve_asset_id!(TradeAssetCurrencyA), // asset_sold
				resolve_asset_id!(TradeAssetCurrencyB), // asset_bought
				51,                                     // sell_amount
				100,                                    // min buy limit for asset B
			),
			Error::<Test>::InsufficientSellerTradeAssetBalance
		);
	});
}

#[test]
fn asset_to_asset_swap_input_less_than_min_sale() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyB => 1000);
		let trader = with_account!(CoreAssetCurrency => 100, TradeAssetCurrencyA => 200);

		assert_err!(
			CennzXSpot::asset_swap_input(
				Origin::signed(trader),
				None,                                   // Account to receive asset_bought, defaults to origin if None
				resolve_asset_id!(TradeAssetCurrencyA), // asset_sold
				resolve_asset_id!(TradeAssetCurrencyB), // asset_bought
				156,                                    // sell_amount
				200,                                    // min buy limit for asset B
			),
			Error::<Test>::InsufficientSellAssetForRequiredMinimumBuyAsset
		);
	});
}

#[test]
fn asset_to_asset_transfer_input() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyB => 1000);
		let trader: AccountId = with_account!(CoreAssetCurrency => 2200, TradeAssetCurrencyA => 2200);
		let recipient: AccountId = with_account!("bob", CoreAssetCurrency => 100, TradeAssetCurrencyB => 100);

		assert_ok!(CennzXSpot::asset_swap_input(
			Origin::signed(trader.clone()),
			Some(recipient.clone()), // Account to receive asset_bought, defaults to origin if None
			resolve_asset_id!(TradeAssetCurrencyA), // asset_sold
			resolve_asset_id!(TradeAssetCurrencyB), // asset_bought
			150,                     // sell_amount
			100,                     // min buy limit for asset B
		));

		assert_exchange_balance_eq!(CoreAssetCurrency => 871, TradeAssetCurrencyA => 1150);
		assert_exchange_balance_eq!(CoreAssetCurrency => 1129, TradeAssetCurrencyB => 887);
		assert_balance_eq!(trader, TradeAssetCurrencyA => 2050);
		assert_balance_eq!(recipient, TradeAssetCurrencyB => 213);
		assert_balance_eq!(trader, CoreAssetCurrency => 2200);
	});
}

#[test]
fn set_fee_rate() {
	ExtBuilder::default().build().execute_with(|| {
		let new_fee_rate = FeeRate::<PerMillion>::try_from(FeeRate::<PerMilli>::from(5u128)).unwrap();
		assert_ok!(CennzXSpot::set_fee_rate(Origin::ROOT, new_fee_rate), ());
		assert_eq!(CennzXSpot::fee_rate(), new_fee_rate);
	});
}

#[test]
fn calculate_buy_price_simple() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyB => 1000);
		let _ = CennzXSpot::set_fee_rate(Origin::ROOT, 0.into());

		assert_eq!(
			CennzXSpot::calculate_buy_price(
				resolve_asset_id!(TradeAssetCurrencyB),
				100,
				resolve_asset_id!(TradeAssetCurrencyA),
			),
			Ok(127)
		);
	});
}

#[test]
fn calculate_buy_price_with_fee_rate() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyB => 1000);
		let _ = CennzXSpot::set_fee_rate(Origin::ROOT, 100_000.into());

		assert_eq!(
			CennzXSpot::calculate_buy_price(
				resolve_asset_id!(TradeAssetCurrencyB),
				100,
				resolve_asset_id!(TradeAssetCurrencyA),
			),
			Ok(155)
		);
	});
}

#[test]
fn calculate_buy_price_when_buying_core() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);
		let _ = CennzXSpot::set_fee_rate(Origin::ROOT, 0.into());

		assert_eq!(
			CennzXSpot::calculate_buy_price(
				resolve_asset_id!(CoreAssetCurrency),
				100,
				resolve_asset_id!(TradeAssetCurrencyA),
			),
			Ok(112)
		);
	});
}

#[test]
fn calculate_buy_price_when_selling_core() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);
		let _ = CennzXSpot::set_fee_rate(Origin::ROOT, 0.into());

		assert_eq!(
			CennzXSpot::calculate_buy_price(
				resolve_asset_id!(TradeAssetCurrencyA),
				100,
				resolve_asset_id!(CoreAssetCurrency),
			),
			Ok(112)
		);
	});
}

#[test]
fn calculate_buy_price_same_asset_id_ignored() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);

		assert_err!(
			CennzXSpot::calculate_buy_price(
				resolve_asset_id!(TradeAssetCurrencyA),
				100,
				resolve_asset_id!(TradeAssetCurrencyA),
			),
			Error::<Test>::AssetCannotSwapForItself
		);
	});
}

#[test]
fn calculate_buy_price_low_buy_asset_liquidity_error() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 10, TradeAssetCurrencyA => 10);
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyB => 1000);

		assert_err!(
			CennzXSpot::calculate_buy_price(
				resolve_asset_id!(TradeAssetCurrencyA),
				100,
				resolve_asset_id!(TradeAssetCurrencyB),
			),
			Error::<Test>::InsufficientAssetReserve
		);
	});
}

#[test]
fn calculate_buy_price_low_buy_core_liquidity_error() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);
		with_exchange!(CoreAssetCurrency => 10, TradeAssetCurrencyB => 10);

		assert_err!(
			CennzXSpot::calculate_buy_price(
				resolve_asset_id!(TradeAssetCurrencyA),
				100,
				resolve_asset_id!(TradeAssetCurrencyB),
			),
			Error::<Test>::InsufficientAssetReserve
		);
	});
}

#[test]
fn calculate_buy_price_no_exchange() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);

		assert_err!(
			CennzXSpot::calculate_buy_price(
				resolve_asset_id!(TradeAssetCurrencyA),
				100,
				resolve_asset_id!(TradeAssetCurrencyB),
			),
			Error::<Test>::EmptyExchangePool
		);
	});
}

#[test]
fn calculate_sell_price_simple() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyB => 1000);
		let _ = CennzXSpot::set_fee_rate(Origin::ROOT, 0.into());

		assert_eq!(
			CennzXSpot::calculate_sell_price(
				resolve_asset_id!(TradeAssetCurrencyB),
				100,
				resolve_asset_id!(TradeAssetCurrencyA),
			),
			Ok(82)
		);
	});
}

#[test]
fn calculate_sell_price_with_fee_rate() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyB => 1000);
		let _ = CennzXSpot::set_fee_rate(Origin::ROOT, 100_000.into());

		assert_eq!(
			CennzXSpot::calculate_sell_price(
				resolve_asset_id!(TradeAssetCurrencyB),
				100,
				resolve_asset_id!(TradeAssetCurrencyA),
			),
			Ok(68)
		);
	});
}

#[test]
fn calculate_sell_price_when_selling_core() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);
		let _ = CennzXSpot::set_fee_rate(Origin::ROOT, 0.into());

		assert_eq!(
			CennzXSpot::calculate_sell_price(
				resolve_asset_id!(CoreAssetCurrency),
				100,
				resolve_asset_id!(TradeAssetCurrencyA),
			),
			Ok(90)
		);
	});
}

#[test]
fn calculate_sell_price_when_buying_core() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);
		let _ = CennzXSpot::set_fee_rate(Origin::ROOT, 0.into());

		assert_eq!(
			CennzXSpot::calculate_sell_price(
				resolve_asset_id!(TradeAssetCurrencyA),
				100,
				resolve_asset_id!(CoreAssetCurrency),
			),
			Ok(90)
		);
	});
}

#[test]
fn calculate_sell_price_same_asset_id_ignored() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);

		assert_err!(
			CennzXSpot::calculate_sell_price(
				resolve_asset_id!(TradeAssetCurrencyA),
				100,
				resolve_asset_id!(TradeAssetCurrencyA),
			),
			Error::<Test>::AssetCannotSwapForItself
		);
	});
}

#[test]
fn calculate_sell_price_low_sell_asset_liquidity() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);
		with_exchange!(CoreAssetCurrency => 10, TradeAssetCurrencyB => 10);

		assert_eq!(
			CennzXSpot::calculate_sell_price(
				resolve_asset_id!(TradeAssetCurrencyA),
				100,
				resolve_asset_id!(TradeAssetCurrencyB),
			),
			Ok(8) // unlike buying, we can sell as long as exchange exists
		);
	});
}

#[test]
fn calculate_sell_price_low_sell_core_liquidity() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);
		with_exchange!(CoreAssetCurrency => 10, TradeAssetCurrencyB => 10);

		assert_eq!(
			CennzXSpot::calculate_sell_price(
				resolve_asset_id!(TradeAssetCurrencyA),
				100,
				resolve_asset_id!(TradeAssetCurrencyB),
			),
			Ok(8) // unlike buying, we can sell as long as exchange exists
		);
	});
}

#[test]
fn calculate_sell_price_no_exchange() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CoreAssetCurrency => 1000, TradeAssetCurrencyA => 1000);

		assert_err!(
			CennzXSpot::calculate_sell_price(
				resolve_asset_id!(TradeAssetCurrencyA),
				100,
				resolve_asset_id!(TradeAssetCurrencyB),
			),
			Error::<Test>::EmptyExchangePool
		);
	});
}
