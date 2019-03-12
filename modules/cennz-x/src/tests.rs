// The testing primitives are very useful for avoiding having to work with signatures
// or public keys. `u64` is used as the `AccountId` and no `Signature`s are required.
use runtime_io::with_externalities;
use runtime_primitives::{
	testing::{Digest, DigestItem, Header},
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage,
};
use substrate_primitives::{Blake2Hasher, H256};

use super::*;

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
	type AccountId = H256;
	type Lookup = IdentityLookup<H256>;
	type Header = Header;
	type Event = ();
	type Log = DigestItem;
}

impl generic_asset::Trait for Test {
	type Balance = u128;
	type AssetId = u32;
	type Event = ();
}

impl Trait for Test {
	type AccountId = H256;
	type Event = ();
}

type CennzXSpot = Module<Test>;

pub struct ExtBuilder {
	core_asset_id: u32,
	fee_rate: Permill,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			core_asset_id: 0,
			fee_rate: Permill::from_millionths(3000),
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> runtime_io::TestExternalities<Blake2Hasher> {
		let mut t = system::GenesisConfig::<Test>::default().build_storage().unwrap().0;
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
				let exchange_key = ($a1, $a2);
				let exchange_address = CennzXSpot::generate_exchange_address(&exchange_key);
				<generic_asset::Module<Test>>::set_free_balance(&$a1, &exchange_address, $b1);
				<generic_asset::Module<Test>>::set_free_balance(&$a2, &exchange_address, $b2);
			}
		};
	);

/// Assert an exchange pair has a balance equal to
/// `assert_exchange_balance_eq!(0 => 10, 1 => 15)`
macro_rules! assert_exchange_balance_eq (
		($a1:expr => $b1:expr, $a2:expr => $b2:expr) => {
			{
				let exchange_key = ($a1, $a2);
				let exchange_address = CennzXSpot::generate_exchange_address(&exchange_key);
				assert_eq!(<generic_asset::Module<Test>>::free_balance(&$a1, &exchange_address), $b1);
				assert_eq!(<generic_asset::Module<Test>>::free_balance(&$a2, &exchange_address), $b2);
			}
		};
	);

/// Initializes a preset address with the given exchange balance.
/// Examples
/// ```
/// let andrea = with_account!(0 => 10, 1 => 20)`
/// let bob = with_account!("bob", 0 => 10, 1 => 20)`
/// ```
macro_rules! with_account (
		($a1:expr => $b1:expr, $a2:expr => $b2:expr) => {
			{
				<generic_asset::Module<Test>>::set_free_balance(&$a1, &default_address(), $b1);
				<generic_asset::Module<Test>>::set_free_balance(&$a2, &default_address(), $b2);
				default_address()
			}
		};
		($name:expr, $a1:expr => $b1:expr, $a2:expr => $b2:expr) => {
			{
				let account = match $name {
					"andrea" => H256::from_low_u64_be(1),
					"bob" => H256::from_low_u64_be(2),
					"charlie" => H256::from_low_u64_be(3),
					_ => H256::from_low_u64_be(1), // default back to "andrea"
				};
				<generic_asset::Module<Test>>::set_free_balance(&$a1, &account, $b1);
				<generic_asset::Module<Test>>::set_free_balance(&$a2, &account, $b2);
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

/// A default user address
fn default_address() -> H256 {
	H256::from_low_u64_be(1)
}

// Default exchange asset IDs
const CORE_ASSET_ID: u32 = 0;
const TRADE_ASSET_ID: u32 = 1;
const DEFAULT_EXCHANGE_KEY: (u32, u32) = (CORE_ASSET_ID, TRADE_ASSET_ID);

#[test]
fn investor_can_add_liquidity() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		let investor = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_ID => 100);

		/// First investment
		assert_ok!(CennzXSpot::add_liquidity(
			Origin::signed(investor),
			TRADE_ASSET_ID,
			2,  // min_liquidity: T::Balance,
			15, // max_asset_amount: T::Balance,
			10, // core_amount: T::Balance,
		));

		// Second investment
		// because a round up, second time asset amount become 15 + 1
		assert_ok!(CennzXSpot::add_liquidity(
			Origin::signed(H256::from_low_u64_be(1)),
			TRADE_ASSET_ID,
			2,  // min_liquidity: T::Balance,
			16, // max_asset_amount: T::Balance,
			10, // core_amount: T::Balance,
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 20, TRADE_ASSET_ID => 31);
		assert_eq!(CennzXSpot::get_liquidity(&DEFAULT_EXCHANGE_KEY, &investor), 20);
	});
}

#[test]
fn get_output_price_zero_cases() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_ID => 1000);

		assert_eq!(
			CennzXSpot::get_output_price(100, 0, 10, <FeeRate<Test>>::get()),
			Zero::zero()
		);

		assert_eq!(
			CennzXSpot::get_output_price(100, 10, 0, <FeeRate<Test>>::get()),
			Zero::zero()
		);
	});
}

#[test]
fn get_output_price() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_ID => 1000);

		assert_eq!(
			CennzXSpot::get_output_price(123, 1000, 1000, <FeeRate<Test>>::get()),
			141
		);
	});
}

#[test]
fn asset_swap_output_price() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_ID => 1000);

		assert_ok!(
			CennzXSpot::get_asset_to_core_output_price(&TRADE_ASSET_ID, 123, <FeeRate<Test>>::get()),
			141
		);

		assert_ok!(
			CennzXSpot::get_core_to_asset_output_price(&TRADE_ASSET_ID, 123, <FeeRate<Test>>::get()),
			141
		);
	});
}

#[test]
fn asset_swap_output_zero_buy_amount() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		assert_err!(
			CennzXSpot::get_core_to_asset_output_price(&TRADE_ASSET_ID, 0, <FeeRate<Test>>::get()),
			"Buy amount must be a positive value"
		);
		assert_err!(
			CennzXSpot::get_asset_to_core_output_price(&TRADE_ASSET_ID, 0, <FeeRate<Test>>::get()),
			"Buy amount must be a positive value"
		);
	});
}

#[test]
fn asset_swap_output_insufficient_reserve() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_ID => 1000);

		assert_err!(
			CennzXSpot::get_asset_to_core_output_price(
				&TRADE_ASSET_ID,
				1001, // amount_bought
				<FeeRate<Test>>::get()
			),
			"Insufficient core asset reserve in exchange"
		);

		assert_err!(
			CennzXSpot::get_core_to_asset_output_price(
				&TRADE_ASSET_ID,
				1001, // amount_bought
				<FeeRate<Test>>::get()
			),
			"Insufficient asset reserve in exchange"
		);
	});
}

#[test]
fn asset_to_core_swap_output() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 10, TRADE_ASSET_ID => 1000);
		let trader = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_ID => 2200);

		assert_ok!(CennzXSpot::asset_to_core_swap_output(
			Origin::signed(trader),
			TRADE_ASSET_ID,
			5,    // buy_amount: T::Balance,
			1400, // max_sale: T::Balance,
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 5, TRADE_ASSET_ID => 2004);
		assert_balance_eq!(trader, TRADE_ASSET_ID => 1196);
	});
}

#[test]
fn make_asset_to_core_swap_output() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 10, TRADE_ASSET_ID => 1000);
		let trader = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_ID => 2200);

		assert_ok!(
			CennzXSpot::make_asset_to_core_output(
				&trader, // buyer
				&trader, // reciever
				&TRADE_ASSET_ID,
				5,                              // buy_amount: T::Balance,
				1400,                           // max_sale: T::Balance,
				Permill::from_millionths(3000), // fee_rate
			),
			1004
		);

		assert_exchange_balance_eq!(CORE_ASSET_ID => 5, TRADE_ASSET_ID => 2004);
		assert_balance_eq!(trader, TRADE_ASSET_ID => 1196);
	});
}

#[test]
fn asset_swap_output_zero_asset_sold() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_ID => 1000);
		let trader = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_ID => 100);

		assert_err!(
			CennzXSpot::asset_to_core_swap_output(
				Origin::signed(trader),
				TRADE_ASSET_ID,
				0,   // buy_amount
				100, // max_sale,
			),
			"Buy amount must be a positive value"
		);

		assert_err!(
			CennzXSpot::core_to_asset_swap_output(
				Origin::signed(trader),
				TRADE_ASSET_ID,
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
		with_exchange!(CORE_ASSET_ID => 500, TRADE_ASSET_ID => 500);
		let trader = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_ID => 50);

		assert_err!(
			CennzXSpot::asset_to_core_swap_output(
				Origin::signed(trader),
				TRADE_ASSET_ID,
				51,  // buy_amount
				500, // max_sale,
			),
			"Insufficient asset balance in buyer account"
		);

		assert_err!(
			CennzXSpot::core_to_asset_swap_output(
				Origin::signed(trader),
				TRADE_ASSET_ID,
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
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_ID => 1000);
		let trader = with_account!(CORE_ASSET_ID => 50, TRADE_ASSET_ID => 50);

		assert_err!(
			CennzXSpot::asset_to_core_swap_output(
				Origin::signed(trader),
				TRADE_ASSET_ID,
				50, // buy_amount
				0,  // max_sale,
			),
			"Amount of asset sold would exceed the specified max. limit"
		);

		assert_err!(
			CennzXSpot::core_to_asset_swap_output(
				Origin::signed(trader),
				TRADE_ASSET_ID,
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
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_ID => 10);
		let trader = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_ID => 2200);

		assert_ok!(CennzXSpot::core_to_asset_swap_output(
			Origin::signed(trader),
			TRADE_ASSET_ID,
			5,    // buy_amount: T::Balance,
			1400, // max_sale: T::Balance,
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 2004, TRADE_ASSET_ID => 5);
		assert_balance_eq!(trader, CORE_ASSET_ID => 1196);
		assert_balance_eq!(trader, TRADE_ASSET_ID => 2205);
	});
}

#[test]
fn make_core_to_asset_output() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_ID => 10);
		let buyer = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_ID => 2200);
		let receiver = with_account!("bob", CORE_ASSET_ID => 0, TRADE_ASSET_ID => 0);

		assert_ok!(
			CennzXSpot::make_core_to_asset_output(
				&buyer,
				&receiver,
				&TRADE_ASSET_ID,
				5,                              // buy_amount: T::Balance,
				1400,                           // max_sale: T::Balance,
				Permill::from_millionths(3000), // fee_rate
			),
			1004
		);

		assert_exchange_balance_eq!(CORE_ASSET_ID => 2004, TRADE_ASSET_ID => 5);
		assert_balance_eq!(buyer, CORE_ASSET_ID => 1196);
		assert_balance_eq!(receiver, TRADE_ASSET_ID => 5);
	});
}

#[test]
fn u64_to_bytes_works() {
	assert_eq!(u64_to_bytes(80000), [128, 56, 1, 0, 0, 0, 0, 0]);
}

#[test]
fn remove_liquidity() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		let investor = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_ID => 100);

		// investment
		let _ = CennzXSpot::add_liquidity(
			Origin::signed(investor),
			TRADE_ASSET_ID,
			2,  // min_liquidity: T::Balance,
			15, // max_asset_amount: T::Balance,
			10, // core_amount: T::Balance,
		);

		assert_ok!(CennzXSpot::remove_liquidity(
			Origin::signed(investor),
			TRADE_ASSET_ID,
			10, //`asset_amount` - Amount of exchange asset to burn
			4,  //`min_asset_withdraw` - The minimum trade asset withdrawn
			4   //`min_core_withdraw` -  The minimum core asset withdrawn
		));
		assert_exchange_balance_eq!(CORE_ASSET_ID => 0, TRADE_ASSET_ID => 0);
		assert_balance_eq!(investor, TRADE_ASSET_ID => 100);
		assert_balance_eq!(investor, CORE_ASSET_ID => 100);
	});
}

#[test]
fn remove_liquidity_fails_min_core_asset_limit() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		let investor = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_ID => 100);

		// investment
		let _ = CennzXSpot::add_liquidity(
			Origin::signed(investor),
			TRADE_ASSET_ID,
			2,  // min_liquidity: T::Balance,
			15, // max_asset_amount: T::Balance,
			10, // core_amount: T::Balance,
		);

		assert_err!(
			CennzXSpot::remove_liquidity(
				Origin::signed(investor),
				TRADE_ASSET_ID,
				10, //`asset_amount` - Amount of exchange asset to burn
				4,  //`min_asset_withdraw` - The minimum trade asset withdrawn
				14  //`min_core_withdraw` -  The minimum core asset withdrawn
			),
			"Minimum core asset is required"
		);
		assert_exchange_balance_eq!(CORE_ASSET_ID => 10, TRADE_ASSET_ID => 15);
		assert_balance_eq!(investor, TRADE_ASSET_ID => 85);
		assert_balance_eq!(investor, CORE_ASSET_ID => 90);
	});
}

#[test]
fn remove_liquidity_fails_min_trade_asset_limit() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		let investor = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_ID => 100);

		// investment
		let _ = CennzXSpot::add_liquidity(
			Origin::signed(investor),
			TRADE_ASSET_ID,
			2,  // min_liquidity: T::Balance,
			15, // max_asset_amount: T::Balance,
			10, // core_amount: T::Balance,
		);

		assert_err!(
			CennzXSpot::remove_liquidity(
				Origin::signed(investor),
				TRADE_ASSET_ID,
				10, //`asset_amount` - Amount of exchange asset to burn
				18, //`min_asset_withdraw` - The minimum trade asset withdrawn
				4   //`min_core_withdraw` -  The minimum core asset withdrawn
			),
			"Minimum trade asset is required"
		);
		assert_exchange_balance_eq!(CORE_ASSET_ID => 10, TRADE_ASSET_ID => 15);
		assert_balance_eq!(investor, TRADE_ASSET_ID => 85);
		assert_balance_eq!(investor, CORE_ASSET_ID => 90);
	});
}

#[test]
fn remove_liquidity_fails_on_overdraw_liquidity() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		let investor = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_ID => 100);

		// investment
		let _ = CennzXSpot::add_liquidity(
			Origin::signed(investor),
			TRADE_ASSET_ID,
			2,  // min_liquidity: T::Balance,
			15, // max_asset_amount: T::Balance,
			10, // core_amount: T::Balance,
		);

		assert_err!(
			CennzXSpot::remove_liquidity(
				Origin::signed(investor),
				TRADE_ASSET_ID,
				20, //`asset_amount` - Amount of exchange asset to burn
				18, //`min_asset_withdraw` - The minimum trade asset withdrawn
				4   //`min_core_withdraw` -  The minimum core asset withdrawn
			),
			"Tried to overdraw liquidity"
		);
		assert_exchange_balance_eq!(CORE_ASSET_ID => 10, TRADE_ASSET_ID => 15);
		assert_balance_eq!(investor, TRADE_ASSET_ID => 85);
		assert_balance_eq!(investor, CORE_ASSET_ID => 90);
	});
}

#[test]
fn asset_transfer_output() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 10, TRADE_ASSET_ID => 1000);
		let buyer = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_ID => 2200);
		let recipient = with_account!("bob", CORE_ASSET_ID => 100, TRADE_ASSET_ID => 100);

		assert_ok!(CennzXSpot::asset_to_core_transfer_output(
			Origin::signed(buyer),
			recipient,
			TRADE_ASSET_ID,
			5,    // buy_amount: T::Balance,
			1400, // max_sale: T::Balance,
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 5, TRADE_ASSET_ID => 2004);
		assert_balance_eq!(buyer, TRADE_ASSET_ID => 1196);
		assert_balance_eq!(recipient, CORE_ASSET_ID => 105);
	});
}

#[test]
fn core_to_asset_transfer_output() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		with_exchange!(CORE_ASSET_ID => 10, TRADE_ASSET_ID => 1000);
		let buyer = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_ID => 2200);
		let recipient = with_account!("bob", CORE_ASSET_ID => 100, TRADE_ASSET_ID => 100);

		assert_ok!(CennzXSpot::core_to_asset_transfer_output(
			Origin::signed(buyer),
			recipient,
			TRADE_ASSET_ID,
			5,    // buy_amount: T::Balance,
			1400, // max_sale: T::Balance,
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 11, TRADE_ASSET_ID => 995);
		assert_balance_eq!(buyer, CORE_ASSET_ID => 2199);
		assert_balance_eq!(recipient, TRADE_ASSET_ID => 105);
	});
}
