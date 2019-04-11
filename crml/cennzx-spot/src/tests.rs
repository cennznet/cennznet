//!
//! CENNZX-SPOT Tests
//!
#![cfg(test)]
use crate::{
	impls::{ExchangeAddressFor, ExchangeAddressGenerator},
	types::FeeRate,
	Call, DefaultFeeRate, GenesisConfig, Module, Trait,
};
use cennznet_primitives::AccountId;
use generic_asset;
use runtime_io::with_externalities;
use runtime_primitives::{
	testing::{Digest, DigestItem, Header},
	traits::{BlakeTwo256, IdentityLookup, Zero},
	BuildStorage,
};
use substrate_primitives::{crypto::UncheckedInto, Blake2Hasher, H256};
use support::{additional_traits::DummyChargeFee, impl_outer_origin, StorageValue};

impl_outer_origin! {
	pub enum Origin for Test {}
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
	type AccountId = AccountId;
	type Lookup = IdentityLookup<AccountId>;
	type Header = Header;
	type Event = ();
	type Log = DigestItem;
}

impl generic_asset::Trait for Test {
	type Balance = u128;
	type AssetId = u32;
	type Event = ();
	type ChargeFee = DummyChargeFee<AccountId, u128>;
}

impl Trait for Test {
	type Call = Call<Self>;
	type Event = ();
	type ExchangeAddressGenerator = ExchangeAddressGenerator<Self>;
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
	pub fn build(self) -> runtime_io::TestExternalities<Blake2Hasher> {
		let mut t = system::GenesisConfig::<Test>::default().build_storage().unwrap().0;
		t.extend(
			generic_asset::GenesisConfig::<Test> {
				assets: Vec::new(),
				initial_balance: 0,
				endowed_accounts: Vec::new(),
				next_asset_id: 100,
				create_asset_stake: 1000,
				transfer_fee: 0,
				staking_asset_id: 0,
				spending_asset_id: 10,
			}
			.build_storage()
			.unwrap()
			.0,
		);
		t.extend(
			GenesisConfig::<Test> {
				core_asset_id: self.core_asset_id,
				fee_rate: self.fee_rate,
			}
			.build_storage()
			.unwrap()
			.0,
		);
		runtime_io::TestExternalities::new(t)
	}
}

/// Initializes an exchange pair with the given liquidity
/// `with_exchange!(asset1_id => balance, asset2_id => balance)`
macro_rules! with_exchange (
	($a1:expr => $b1:expr, $a2:expr => $b2:expr) => {
		{
			let exchange_address = <Test as Trait>::ExchangeAddressGenerator::exchange_address_for($a1, $a2);
			// TODO: `set_free_balance` changed to private fn. This is a workaround to deposit into
			// an account. Use `deposit` instead when ga module fn supports.
			assert_ok!(<generic_asset::Module<Test>>::reward(&$a1, &exchange_address, $b1));
			assert_ok!(<generic_asset::Module<Test>>::reward(&$a2, &exchange_address, $b2));
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
			// TODO: `set_free_balance` changed to private fn. This is a workaround to deposit into
			// an account. Use `deposit` instead when ga module fn supports.
			assert_ok!(<generic_asset::Module<Test>>::reward(&$a1, &H256::from_low_u64_be(1).unchecked_into(), $b1));
			assert_ok!(<generic_asset::Module<Test>>::reward(&$a2, &H256::from_low_u64_be(1).unchecked_into(), $b2));
			H256::from_low_u64_be(1).unchecked_into()
		}
	};
	($name:expr, $a1:expr => $b1:expr, $a2:expr => $b2:expr) => {
		{
			let account = match $name {
				"andrea" => H256::from_low_u64_be(1).unchecked_into(),
				"bob" => H256::from_low_u64_be(2).unchecked_into(),
				"charlie" => H256::from_low_u64_be(3).unchecked_into(),
				_ => H256::from_low_u64_be(1).unchecked_into(), // default back to "andrea"
			};
			// TODO: `set_free_balance` changed to private fn. This is a workaround to deposit into
			// an account. Use `deposit` instead when ga module fn supports.
			assert_ok!(<generic_asset::Module<Test>>::reward(&$a1, &account, $b1));
			assert_ok!(<generic_asset::Module<Test>>::reward(&$a2, &account, $b2));
			account
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
	with_externalities(&mut ExtBuilder::default().build(), || {
		let investor: AccountId = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);

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
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);

		assert_eq!(
			CennzXSpot::get_output_price(100, 0, 10, <DefaultFeeRate<Test>>::get()),
			Zero::zero()
		);

		assert_eq!(
			CennzXSpot::get_output_price(100, 10, 0, <DefaultFeeRate<Test>>::get()),
			Zero::zero()
		);
	});
}

#[test]
fn get_output_price() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);

		assert_eq!(
			CennzXSpot::get_output_price(123, 1000, 1000, <DefaultFeeRate<Test>>::get()),
			141
		);
	});
}

#[test]
fn get_output_price_max_withdrawal() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);

		assert_eq!(
			CennzXSpot::get_output_price(1000, 1000, 1000, <DefaultFeeRate<Test>>::get()),
			<Test as generic_asset::Trait>::Balance::max_value()
		);

		assert_eq!(
			CennzXSpot::get_output_price(1_000_000, 1000, 1000, <DefaultFeeRate<Test>>::get()),
			<Test as generic_asset::Trait>::Balance::max_value()
		);
	});
}

#[test]
fn asset_swap_output_price() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);

		assert_ok!(
			CennzXSpot::get_asset_to_core_output_price(&TRADE_ASSET_A, 123, <DefaultFeeRate<Test>>::get()),
			141
		);

		assert_ok!(
			CennzXSpot::get_core_to_asset_output_price(&TRADE_ASSET_A, 123, <DefaultFeeRate<Test>>::get()),
			141
		);
	});
}

#[test]
fn asset_swap_output_zero_buy_amount() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		assert_err!(
			CennzXSpot::get_core_to_asset_output_price(&TRADE_ASSET_A, 0, <DefaultFeeRate<Test>>::get()),
			"Buy amount must be a positive value"
		);
		assert_err!(
			CennzXSpot::get_asset_to_core_output_price(&TRADE_ASSET_A, 0, <DefaultFeeRate<Test>>::get()),
			"Buy amount must be a positive value"
		);
	});
}

#[test]
fn asset_swap_output_insufficient_reserve() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);

		assert_err!(
			CennzXSpot::get_asset_to_core_output_price(
				&TRADE_ASSET_A,
				1001, // amount_bought
				<DefaultFeeRate<Test>>::get()
			),
			"Insufficient core asset reserve in exchange"
		);

		assert_err!(
			CennzXSpot::get_core_to_asset_output_price(
				&TRADE_ASSET_A,
				1001, // amount_bought
				<DefaultFeeRate<Test>>::get()
			),
			"Insufficient asset reserve in exchange"
		);
	});
}

#[test]
fn asset_to_core_swap_output() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 10, TRADE_ASSET_A => 1000);
		let trader: AccountId = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A => 2200);

		assert_ok!(CennzXSpot::asset_to_core_swap_output(
			Origin::signed(trader.clone()),
			None,
			TRADE_ASSET_A,
			5,    // buy_amount: T::Balance,
			1400, // max_sale: T::Balance,
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 5, TRADE_ASSET_A => 2004);
		assert_balance_eq!(trader, TRADE_ASSET_A => 1196);
	});
}

#[test]
fn make_asset_to_core_swap_output() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 10, TRADE_ASSET_A => 1000);
		let trader = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A => 2200);

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
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
		let trader: AccountId = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);

		assert_err!(
			CennzXSpot::asset_to_core_swap_output(
				Origin::signed(trader.clone()),
				None,
				TRADE_ASSET_A,
				0,   // buy_amount
				100, // max_sale,
			),
			"Buy amount must be a positive value"
		);

		assert_err!(
			CennzXSpot::core_to_asset_swap_output(
				Origin::signed(trader),
				None,
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
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 500, TRADE_ASSET_A => 500);
		let trader: AccountId = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 50);

		assert_err!(
			CennzXSpot::asset_to_core_swap_output(
				Origin::signed(trader.clone()),
				None,
				TRADE_ASSET_A,
				51,  // buy_amount
				500, // max_sale,
			),
			"Insufficient asset balance in buyer account"
		);

		assert_err!(
			CennzXSpot::core_to_asset_swap_output(
				Origin::signed(trader),
				None,
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
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
		let trader: AccountId = with_account!(CORE_ASSET_ID => 50, TRADE_ASSET_A => 50);

		assert_err!(
			CennzXSpot::asset_to_core_swap_output(
				Origin::signed(trader.clone()),
				None,
				TRADE_ASSET_A,
				50, // buy_amount
				0,  // max_sale,
			),
			"Amount of asset sold would exceed the specified max. limit"
		);

		assert_err!(
			CennzXSpot::core_to_asset_swap_output(
				Origin::signed(trader),
				None,
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
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 10);
		let trader: AccountId = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A => 2200);

		assert_ok!(CennzXSpot::core_to_asset_swap_output(
			Origin::signed(trader.clone()),
			None,
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
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 10);
		let buyer = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A => 2200);
		let recipient = with_account!("bob", CORE_ASSET_ID => 0, TRADE_ASSET_A => 0);

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
	with_externalities(&mut ExtBuilder::default().build(), || {
		let investor: AccountId = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);

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
	with_externalities(&mut ExtBuilder::default().build(), || {
		let investor: AccountId = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);

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
	with_externalities(&mut ExtBuilder::default().build(), || {
		let investor: AccountId = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);

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
	with_externalities(&mut ExtBuilder::default().build(), || {
		let investor: AccountId = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);

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
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 10, TRADE_ASSET_A => 1000);
		let buyer: AccountId = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A => 2200);
		let recipient = with_account!("bob", CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);

		assert_ok!(CennzXSpot::asset_to_core_swap_output(
			Origin::signed(buyer.clone()),
			Some(recipient.clone()),
			TRADE_ASSET_A,
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
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 10, TRADE_ASSET_A => 1000);
		let buyer: AccountId = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A => 2200);
		let recipient: AccountId = with_account!("bob", CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);

		assert_ok!(CennzXSpot::core_to_asset_swap_output(
			Origin::signed(buyer.clone()),
			Some(recipient.clone()),
			TRADE_ASSET_A,
			5,    // buy_amount: T::Balance,
			1400, // max_sale: T::Balance,
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 11, TRADE_ASSET_A => 995);
		assert_balance_eq!(buyer, CORE_ASSET_ID => 2199);
		assert_balance_eq!(recipient, TRADE_ASSET_A => 105);
	});
}

#[test]
fn get_input_price() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		assert_eq!(
			CennzXSpot::get_input_price(123, 1000, 1000, <DefaultFeeRate<Test>>::get()),
			109
		);

		// No f32/f64 types, so we use large values to test precision
		assert_eq!(
			CennzXSpot::get_input_price(123_000_000, 1_000_000_000, 1_000_000_000, <DefaultFeeRate<Test>>::get()),
			109236234
		);
	});
}

#[test]
fn asset_swap_input_price() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);

		assert_ok!(
			CennzXSpot::get_asset_to_core_input_price(&TRADE_ASSET_A, 123, <DefaultFeeRate<Test>>::get()),
			109
		);

		assert_ok!(
			CennzXSpot::get_core_to_asset_input_price(&TRADE_ASSET_A, 123, <DefaultFeeRate<Test>>::get()),
			109
		);
	});
}

#[test]
fn asset_swap_input_zero_sell_amount() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		assert_err!(
			CennzXSpot::get_asset_to_core_input_price(&TRADE_ASSET_A, 0, <DefaultFeeRate<Test>>::get()),
			"Sell amount must be a positive value"
		);
		assert_err!(
			CennzXSpot::get_core_to_asset_input_price(&TRADE_ASSET_A, 0, <DefaultFeeRate<Test>>::get()),
			"Sell amount must be a positive value"
		);
	});
}

#[test]
fn asset_swap_input_insufficient_balance() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);

		let trader = with_account!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
		assert_err!(
			CennzXSpot::make_asset_to_core_input(
				&trader, // seller
				&trader, // recipient
				&TRADE_ASSET_A,
				10001, // sell_amount
				100,   // min buy limit
				<DefaultFeeRate<Test>>::get()
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
				<DefaultFeeRate<Test>>::get()
			),
			"Insufficient core asset balance in seller account"
		);
	});
}

#[test]
fn asset_to_core_swap_input() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
		let trader: AccountId = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A => 2200);

		assert_ok!(CennzXSpot::asset_to_core_swap_input(
			Origin::signed(trader.clone()),
			None,
			TRADE_ASSET_A,
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
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
		let trader: AccountId = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A => 2200);

		assert_ok!(CennzXSpot::core_to_asset_swap_input(
			Origin::signed(trader.clone()),
			None,
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
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
		let trader = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A => 2200);

		assert_ok!(
			CennzXSpot::make_asset_to_core_input(
				&trader, // buyer
				&trader, // recipient
				&TRADE_ASSET_A,
				90,                            // sell_amount: T::Balance,
				50,                            // min buy: T::Balance,
				<DefaultFeeRate<Test>>::get()  // fee_rate
			),
			82
		);

		assert_exchange_balance_eq!(CORE_ASSET_ID => 918, TRADE_ASSET_A => 1090);
		assert_balance_eq!(trader, TRADE_ASSET_A => 2110);
		assert_balance_eq!(trader, CORE_ASSET_ID => 2282);
	});
}

#[test]
fn make_core_to_asset_input() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
		let trader = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A => 2200);

		assert_ok!(
			CennzXSpot::make_core_to_asset_input(
				&trader, // buyer
				&trader, // recipient
				&TRADE_ASSET_A,
				90,                            // sell_amount: T::Balance,
				50,                            // min buy: T::Balance,
				<DefaultFeeRate<Test>>::get()  // fee_rate
			),
			82
		);

		assert_exchange_balance_eq!(CORE_ASSET_ID => 1090, TRADE_ASSET_A => 918);
		assert_balance_eq!(trader, CORE_ASSET_ID => 2110);
		assert_balance_eq!(trader, TRADE_ASSET_A => 2282);
	});
}

#[test]
fn asset_swap_input_zero_asset_sold() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
		let trader: AccountId = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);

		assert_err!(
			CennzXSpot::asset_to_core_swap_input(
				Origin::signed(trader.clone()),
				None,
				TRADE_ASSET_A,
				0,   // sell amount
				100, // min buy,
			),
			"Sell amount must be a positive value"
		);
		assert_err!(
			CennzXSpot::core_to_asset_swap_input(
				Origin::signed(trader),
				None,
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
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
		let trader: AccountId = with_account!(CORE_ASSET_ID => 50, TRADE_ASSET_A => 50);

		assert_err!(
			CennzXSpot::asset_to_core_swap_input(
				Origin::signed(trader.clone()),
				None,
				TRADE_ASSET_A,
				50,  // sell_amount
				100, // min buy,
			),
			"The sale value of input is less than the required min."
		);
		assert_err!(
			CennzXSpot::core_to_asset_swap_input(
				Origin::signed(trader),
				None,
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
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
		let trader: AccountId = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A => 2200);
		let recipient: AccountId = with_account!("bob", CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);

		assert_ok!(CennzXSpot::asset_to_core_swap_input(
			Origin::signed(trader.clone()),
			Some(recipient.clone()),
			TRADE_ASSET_A,
			50, // sell_amount: T::Balance,
			40, // min_sale: T::Balance,
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 953, TRADE_ASSET_A => 1050);
		assert_balance_eq!(trader, TRADE_ASSET_A => 2150);
		assert_balance_eq!(recipient, CORE_ASSET_ID => 147);
	});
}

#[test]
fn core_to_asset_transfer_input() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
		let trader: AccountId = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A => 2200);
		let recipient: AccountId = with_account!("bob", CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);

		assert_ok!(CennzXSpot::core_to_asset_swap_input(
			Origin::signed(trader.clone()),
			Some(recipient.clone()),
			TRADE_ASSET_A,
			50, // sell_amount: T::Balance,
			40, // min_sale: T::Balance,
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 1050, TRADE_ASSET_A => 953);
		assert_balance_eq!(trader, CORE_ASSET_ID => 2150);
		assert_balance_eq!(recipient, TRADE_ASSET_A => 147);
	});
}

#[test]
fn asset_to_asset_swap_output() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B => 1000);
		let trader: AccountId = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A => 2200);

		assert_ok!(CennzXSpot::asset_to_asset_swap_output(
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
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B => 1000);
		let trader = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);

		assert_err!(
			CennzXSpot::asset_to_asset_swap_output(
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
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B => 1000);
		let trader = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 50);

		assert_err!(
			CennzXSpot::asset_to_asset_swap_output(
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
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B => 1000);
		let trader = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);

		assert_err!(
			CennzXSpot::asset_to_asset_swap_output(
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
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B => 1000);
		let trader: AccountId = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A => 2200);
		let recipient: AccountId = with_account!("bob", CORE_ASSET_ID => 100, TRADE_ASSET_B => 100);

		assert_ok!(CennzXSpot::asset_to_asset_swap_output(
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
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B => 1000);
		let trader: AccountId = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A => 2200);

		assert_ok!(CennzXSpot::asset_to_asset_swap_input(
			Origin::signed(trader.clone()),
			None,          // Trader is also recipient so passing None in this case
			TRADE_ASSET_A, // asset_sold
			TRADE_ASSET_B, // asset_bought
			150,           // sell_amount
			100,           // min buy limit for asset B
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 870, TRADE_ASSET_A => 1150);
		assert_exchange_balance_eq!(CORE_ASSET_ID => 1130, TRADE_ASSET_B => 886);
		assert_balance_eq!(trader, TRADE_ASSET_A => 2050);
		assert_balance_eq!(trader, TRADE_ASSET_B => 114);
		assert_balance_eq!(trader, CORE_ASSET_ID => 2200);
	});
}

#[test]
fn asset_to_asset_swap_input_zero_asset_sold() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B => 1000);
		let trader = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);

		assert_err!(
			CennzXSpot::asset_to_asset_swap_input(
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
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 100);
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B => 1000);
		let trader = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 50);

		assert_err!(
			CennzXSpot::asset_to_asset_swap_input(
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
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B => 1000);
		let trader = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A => 200);

		assert_err!(
			CennzXSpot::asset_to_asset_swap_input(
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
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A => 1000);
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B => 1000);
		let trader: AccountId = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A => 2200);
		let recipient: AccountId = with_account!("bob", CORE_ASSET_ID => 100, TRADE_ASSET_B => 100);

		assert_ok!(CennzXSpot::asset_to_asset_swap_input(
			Origin::signed(trader.clone()),
			Some(recipient.clone()), // Account to receive asset_bought, defaults to origin if None
			TRADE_ASSET_A,           // asset_sold
			TRADE_ASSET_B,           // asset_bought
			150,                     // sell_amount
			100,                     // min buy limit for asset B
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 870, TRADE_ASSET_A => 1150);
		assert_exchange_balance_eq!(CORE_ASSET_ID => 1130, TRADE_ASSET_B => 886);
		assert_balance_eq!(trader, TRADE_ASSET_A => 2050);
		assert_balance_eq!(recipient, TRADE_ASSET_B => 214);
		assert_balance_eq!(trader, CORE_ASSET_ID => 2200);
	});
}

#[test]
fn set_fee_rate() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		let new_fee_rate = FeeRate::from_milli(5);
		assert_ok!(CennzXSpot::set_fee_rate(new_fee_rate), ());
		assert_eq!(CennzXSpot::fee_rate(), new_fee_rate);
	});
}
