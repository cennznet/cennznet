//!
//! CENNZ-X
//!
#![cfg_attr(not(feature = "std"), no_std)]
// TODO: Suppress warnings from unimplemented stubs. Remove when complete
#![allow(unused_variables)]

#[macro_use]
extern crate srml_support as support;

use generic_asset;
use rstd::{mem, prelude::*};
use runtime_io::twox_128;
use runtime_primitives::{
	traits::{As, Hash, One, Zero},
	Permill,
};
use support::{dispatch::Result, StorageDoubleMap, StorageMap, StorageValue};
use system::ensure_signed;
// An alias for the system wide `AccountId` type
pub type AccountIdOf<T> = <T as system::Trait>::AccountId;
// (core_asset_id, asset_id)
pub type ExchangeKey<T> = (
	<T as generic_asset::Trait>::AssetId,
	<T as generic_asset::Trait>::AssetId,
);

pub trait Trait: system::Trait + generic_asset::Trait {
	// This type is used as a shim from `system::Trait::Hash` to `system::Trait::AccountId`
	type AccountId: From<<Self as system::Trait>::Hash> + Into<<Self as system::Trait>::AccountId>;
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event<T>() = default;

		/// Trade asset (`asset_id`) to core asset. User specifies maximum input and exact output
		/// `asset_id` - The asset ID to trade
		/// `buy_amount` - Amount of core asset to purchase (output)
		/// `max_sale` -  Maximum asset to sell (input)
		pub fn asset_to_core_swap_output(
			origin,
			asset_id: T::AssetId,
			buy_amount: T::Balance,
			max_sale: T::Balance
		) -> Result {
			let buyer = ensure_signed(origin)?;
			let sold_amount = Self::make_asset_to_core_swap_output(&buyer, &asset_id, buy_amount, max_sale, Self::fee_rate())?;
			Self::deposit_event(RawEvent::CoreAssetPurchase(asset_id, buyer, sold_amount, buy_amount));

			Ok(())
		}

		//
		// Manage Liquidity
		//

		/// Deposit core asset and trade asset at current ratio to mint liquidity
		/// Returns amount of liquidity minted.
		///
		/// `origin`
		/// `asset_id` - The trade asset ID
		/// `min_liquidity` - The minimum liquidity to add
		/// `asset_amount` - Amount of trade asset to add
		/// `core_amount` - Amount of core asset to add
		pub fn add_liquidity(
			origin,
			asset_id: T::AssetId,
			min_liquidity: T::Balance,
			max_asset_amount: T::Balance,
			core_amount: T::Balance
		) {
			let from_account = ensure_signed(origin)?;
			let core_asset_id = Self::core_asset_id();
			ensure!(!max_asset_amount.is_zero(), "trade asset amount must be greater than zero");
			ensure!(!core_amount.is_zero(), "core asset amount must be greater than zero");
			ensure!(<generic_asset::Module<T>>::free_balance(&core_asset_id, &from_account) >= core_amount,
				"no enough core asset balance"
			);
			ensure!(<generic_asset::Module<T>>::free_balance(&asset_id, &from_account) >= max_asset_amount,
				"no enough trade asset balance"
			);
			let exchange_key = (core_asset_id, asset_id);
			let total_liquidity = Self::get_total_supply(&exchange_key);
			let exchange_address = Self::generate_exchange_address(&exchange_key);
			if total_liquidity.is_zero() {
				// new exchange pool
				<generic_asset::Module<T>>::make_transfer(&core_asset_id, &from_account, &exchange_address, core_amount)?;
				<generic_asset::Module<T>>::make_transfer(&asset_id, &from_account, &exchange_address, max_asset_amount)?;
				let trade_asset_amount = max_asset_amount;
				let initial_liquidity = core_amount;
				Self::set_liquidity(&exchange_key, &from_account, initial_liquidity);
				Self::mint_total_supply(&exchange_key, initial_liquidity);
				Self::deposit_event(RawEvent::AddLiquidity(from_account, initial_liquidity, asset_id, trade_asset_amount));
			} else {
				// TODO: shall i use total_balance instead? in which case the exchange address will have reserve balance?
				let trade_asset_reserve = <generic_asset::Module<T>>::free_balance(&asset_id, &exchange_address);
				let core_asset_reserve = <generic_asset::Module<T>>::free_balance(&core_asset_id, &exchange_address);
				let trade_asset_amount = core_amount * trade_asset_reserve / core_asset_reserve + One::one();
				let liquidity_minted = core_amount * total_liquidity / core_asset_reserve;
				ensure!(liquidity_minted >= min_liquidity, "Minimum liquidity is required");
				ensure!(max_asset_amount >= trade_asset_amount, "Token liquidity check unsuccessful");

				<generic_asset::Module<T>>::make_transfer(&core_asset_id, &from_account, &exchange_address, core_amount)?;
				<generic_asset::Module<T>>::make_transfer(&asset_id, &from_account, &exchange_address, trade_asset_amount)?;

				Self::set_liquidity(&exchange_key, &from_account,
									Self::get_liquidity(&exchange_key, &from_account) + liquidity_minted);
				Self::mint_total_supply(&exchange_key, liquidity_minted);
				Self::deposit_event(RawEvent::AddLiquidity(from_account, core_amount, asset_id, trade_asset_amount));
			}
		}

		/// Burn exchange assets to withdraw core asset and trade asset at current ratio
		///
		/// `asset_id` - The trade asset ID
		/// `asset_amount` - Amount of exchange asset to burn
		/// `min_asset_withdraw` - The minimum trade asset withdrawn
		/// `min_core_withdraw` -  The minimum core asset withdrawn
		pub fn remove_liquidity(
			origin,
			asset_id: T::AssetId,
			asset_amount: T::Balance,
			min_asset_withdraw: T::Balance,
			min_core_withdraw: T::Balance
		) -> Result {
			let from_account = ensure_signed(origin)?;
			ensure!(asset_amount > Zero::zero(), "Amount of exchange asset to burn should exist");
			ensure!(min_asset_withdraw > Zero::zero() && min_core_withdraw > Zero::zero(), "Assets withdrawn to be greater than zero");

			let core_asset_id = Self::core_asset_id();
			let exchange_key = (core_asset_id, asset_id);
			let account_liquidity = Self::get_liquidity(&exchange_key, &from_account);
			ensure!(account_liquidity >= asset_amount, "Tried to overdraw liquidity");

			let total_liquidity = Self::get_total_supply(&exchange_key);
			let exchange_address = Self::generate_exchange_address(&exchange_key);
			ensure!(total_liquidity > Zero::zero(), "Liquidity should exist");

			let trade_asset_reserve = <generic_asset::Module<T>>::free_balance(&asset_id, &exchange_address);
			let core_asset_reserve = <generic_asset::Module<T>>::free_balance(&core_asset_id, &exchange_address);
			let core_asset_amount = asset_amount * core_asset_reserve / total_liquidity;
			let trade_asset_amount = asset_amount * trade_asset_reserve / total_liquidity;
			ensure!(core_asset_amount >= min_core_withdraw, "Minimum core asset is required");
			ensure!(trade_asset_amount >= min_asset_withdraw, "Minimum trade asset is required");

			<generic_asset::Module<T>>::make_transfer(&core_asset_id, &exchange_address, &from_account, core_asset_amount)?;
			<generic_asset::Module<T>>::make_transfer(&asset_id, &exchange_address, &from_account, trade_asset_amount)?;
			Self::set_liquidity(&exchange_key, &from_account,
									account_liquidity - asset_amount);
			Self::burn_total_supply(&exchange_key, asset_amount);
			Self::deposit_event(RawEvent::RemoveLiquidity(from_account, core_asset_amount, asset_id, trade_asset_amount));
			Ok(())
		}
	}
}

decl_event!(
	pub enum Event<T>
	where
		<T as system::Trait>::AccountId,
		<T as generic_asset::Trait>::AssetId,
		<T as generic_asset::Trait>::Balance
	{
		// Provider, core asset amount, trade asset id, trade asset amount
		AddLiquidity(AccountId, Balance, AssetId, Balance),
		// Provider, core asset amount, trade asset id, trade asset amount
		RemoveLiquidity(AccountId, Balance, AssetId, Balance),
		// Trade AssetId, Buyer, trade asset sold, core asset bought
		CoreAssetPurchase(AssetId, AccountId, Balance, Balance),
		// Trade AssetId, Buyer, core asset sold, trade asset bought
		TradeAssetPurchase(AssetId, AccountId, Balance, Balance),
		// Trade asset id, core asset id
		NewPool(AssetId, AssetId),
	}
);

/// Asset balance of each user in each exchange pool.
/// Key: `(core asset id, trade asset id), account_id`
pub(crate) struct LiquidityBalance<T>(rstd::marker::PhantomData<T>);

/// store all user's liquidity in each exchange pool
impl<T: Trait> StorageDoubleMap for LiquidityBalance<T> {
	const PREFIX: &'static [u8] = b"cennz-x-spot:liquidity";
	type Key1 = ExchangeKey<T>;
	// Delete the whole pool
	type Key2 = AccountIdOf<T>;
	type Value = T::Balance;

	fn derive_key1(key1_data: Vec<u8>) -> Vec<u8> {
		twox_128(&key1_data).to_vec()
	}

	fn derive_key2(key2_data: Vec<u8>) -> Vec<u8> {
		key2_data
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as CennzX {
		/// AssetId of Core Asset
		pub CoreAssetId get(core_asset_id) config(): T::AssetId;
		/// Default Trading fee rate
		pub FeeRate get(fee_rate) config(): Permill;
		/// Total supply of exchange token in existence.
		/// it will always be less than the core asset's total supply
		/// Key: `(asset id, core asset id)`
		pub TotalSupply get(total_supply): map ExchangeKey<T> => T::Balance;
	}
}

/// Convert a `u64` into its byte array representation
fn u64_to_bytes(x: u64) -> [u8; 8] {
	unsafe { mem::transmute(x.to_le()) }
}

// The main implementation block for the module.
impl<T: Trait> Module<T> {
	/// Generates an exchange address for the given asset pair
	pub fn generate_exchange_address(exchange_key: &ExchangeKey<T>) -> AccountIdOf<T> {
		let (core_asset, asset) = exchange_key;
		let mut buf = Vec::new();
		buf.extend_from_slice(b"cennz-x-spot:");
		buf.extend_from_slice(&u64_to_bytes(As::as_(*core_asset)));
		buf.extend_from_slice(&u64_to_bytes(As::as_(*asset)));

		// Use shim `system::Trait::Hash` -> `Trait::AccountId` -> system::Trait::AccountId`
		<T as Trait>::AccountId::from(T::Hashing::hash(&buf[..])).into()
	}

	// Storage R/W
	fn get_total_supply(exchange_key: &ExchangeKey<T>) -> T::Balance {
		<TotalSupply<T>>::get(exchange_key)
	}

	/// mint total supply for an exchange pool
	fn mint_total_supply(exchange_key: &ExchangeKey<T>, increase: T::Balance) {
		<TotalSupply<T>>::mutate(exchange_key, |balance| *balance += increase); // will not overflow because it's limited by core assets's total supply
	}

	fn burn_total_supply(exchange_key: &ExchangeKey<T>, decrease: T::Balance) {
		<TotalSupply<T>>::mutate(exchange_key, |balance| *balance -= decrease); // will not downflow for the same reason
	}

	fn set_liquidity(exchange_key: &ExchangeKey<T>, who: &AccountIdOf<T>, balance: T::Balance) {
		<LiquidityBalance<T>>::insert(exchange_key, who, balance);
	}

	pub fn get_liquidity(exchange_key: &ExchangeKey<T>, who: &AccountIdOf<T>) -> T::Balance {
		<LiquidityBalance<T>>::get(exchange_key, who).unwrap_or_else(Default::default)
	}

	//
	// Trade core to other asset
	//

	/// Convert core asset to trade asset. User specifies
	/// exact input(core asset) and minimum output.
	///
	/// `asset_id` - Trade asset ID
	/// `amount_sold` - Exact amount of trade asset to be sold
	/// `min_amount_bought` - Minimum core assets bought
	pub fn core_to_asset_swap_input(
		asset_id: &T::AssetId,
		amount_sold: T::Balance,
		min_amount_bought: T::Balance,
		fee_rate: Permill,
	) {
	}

	/// Convert core asset to trade asset and transfer the trade asset to recipient from system account.
	/// User specifies exact input (core asset) and minimum output.
	///
	/// `asset_id` - Trade asset ID
	/// `amount_sold` - Exact amount of trade asset to be sold
	/// `min_amount_bought` - Minimum core assets bought
	/// `recipient` - The address that receives the output asset
	pub fn core_to_asset_transfer_input(
		asset_id: &T::AssetId,
		amount_sold: T::Balance,
		min_amount_bought: T::Balance,
		recipient: AccountIdOf<T>,
		fee_rate: Permill,
	) {
	}

	/// Convert core asset to trade asset. User specifies
	/// maximum input (core asset) and exact output.
	///
	/// `asset_id` - Trade asset ID
	/// `amount_bought` - Amount of core asset purchased
	/// `max_amount_sold` -  Maximum trade asset sold
	pub fn core_to_asset_swap_output(
		asset_id: &T::AssetId,
		amount_bought: T::Balance,
		max_amount_sold: T::Balance,
		fee_rate: Permill,
	) {
	}

	/// Convert core asset to trade asset and transfer the trade asset to recipient
	/// from system account. User specifies maximum input (core asset) and exact output
	///
	/// `asset_id` - Trade asset ID
	/// `amount_bought` - Amount of core asset purchased
	/// `max_amount_sold` -  Maximum trade asset sold
	/// `recipient` - The address that receives the output asset
	pub fn core_to_asset_transfer_output(
		asset_id: &T::AssetId,
		amount_bought: T::Balance,
		max_amount_sold: T::Balance,
		recipient: &AccountIdOf<T>,
		fee_rate: Permill,
	) {
	}

	//
	// Trade asset with core asset
	//

	/// Trade asset (`asset_id`) to core asset at the given `fee_rate`
	/// `asset_id` - The asset ID to trade
	/// `buy_amount` - Amount of core asset to purchase (output)
	/// `max_sale` -  Maximum asset to sell (input)
	/// `fee_rate` - The % of exchange fees for the trade
	pub fn make_asset_to_core_swap_output(
		buyer: &AccountIdOf<T>,
		asset_id: &T::AssetId,
		buy_amount: T::Balance,
		max_sale: T::Balance,
		fee_rate: Permill,
	) -> rstd::result::Result<T::Balance, &'static str> {
		let sold_amount = Self::get_asset_to_core_output_price(&asset_id, buy_amount, fee_rate)?;
		ensure!(
			sold_amount > Zero::zero(),
			"Amount of asset sold should be greater than zero"
		);
		ensure!(
			max_sale > sold_amount,
			"Amount of asset sold would exceed the specified max. limit"
		);
		ensure!(
			<generic_asset::Module<T>>::free_balance(&asset_id, &buyer) >= sold_amount,
			"Insufficient asset balance in buyer account"
		);

		let core_asset_id = Self::core_asset_id();
		let exchange_key = (core_asset_id, *asset_id);
		let exchange_address = Self::generate_exchange_address(&exchange_key);
		<generic_asset::Module<T>>::make_transfer(&core_asset_id, &exchange_address, &buyer, buy_amount)?;
		<generic_asset::Module<T>>::make_transfer(&asset_id, &buyer, &exchange_address, sold_amount)?;

		Ok(sold_amount)
	}

	/// Convert trade asset to core asset. User specifies exact
	/// input (trade asset) and minimum output.
	///
	/// `asset_id` - Trade asset ID
	/// `amount_sold` - Exact amount of trade asset to be sold
	/// `min_amount_bought` - Minimum core assets bought
	pub fn asset_to_core_swap_input(
		asset_id: &T::AssetId,
		amount_sold: T::Balance,
		min_amount_bought: T::Balance,
		fee_rate: Permill,
	) {
	}

	/// Convert trade asset to core asset and transfer the core asset to recipient from system account.
	/// User specifies exact input (core asset) and minimum output.
	///
	/// `asset_id` - Trade asset ID
	/// `amount_sold` - Exact amount of trade asset to be sold
	/// `min_amount_bought` - Minimum core assets bought
	/// `recipient` - The address that receives the output asset
	pub fn asset_to_core_transfer_input(
		asset_id: &T::AssetId,
		amount_sold: T::Balance,
		min_amount_bought: T::Balance,
		recipient: &AccountIdOf<T>,
		fee_rate: Permill,
	) {
	}

	/// Convert core asset to trade asset and transfer the trade asset to recipient from system account.
	/// User specifies maximum input (core asset) and exact output.
	///
	/// `asset_id` - Trade asset ID
	/// `amount_bought` - Amount of core asset purchased
	/// `max_amount_sold` -  Maximum trade asset sold
	/// `recipient` - The address that receives the output asset
	pub fn asset_to_core_transfer_output(
		asset_id: &T::AssetId,
		amount_bought: T::Balance,
		max_amount_sold: T::Balance,
		recipient: &AccountIdOf<T>,
		fee_rate: Permill,
	) {
	}

	//
	// Trade non-core asset to non-core asset
	//

	/// Convert trade asset1 to trade asset2 via core asset. User specifies
	/// exact input and minimum output.
	///
	/// `asset_sold` - Trade asset1 ID
	/// `asset_bought` - asset2 ID
	/// `amount_sold` - Exact amount of trade asset to be sold
	/// `min_amount_bought` - Minimum trade asset2 purchased
	/// `min_core_bought` - Minimum core purchased as intermediary
	pub fn asset_to_asset_swap_input(
		asset_sold: &T::AssetId,
		asset_bought: &T::AssetId,
		amount_sold: T::Balance,
		min_amount_bought: T::Balance,
		min_core_bought: T::Balance,
		fee_rate: Permill,
	) {
	}

	/// Convert trade asset1 to trade asset2 via core asset and transfer the
	/// trade asset2 to recipient from system account.User specifies exact input
	/// and minimum output.
	///
	/// `asset_sold` - Trade asset1 ID
	/// `asset_bought` - asset2 ID
	/// `amount_sold` - Exact amount of trade asset to be sold
	/// `min_amount_bought` - Minimum trade asset2 purchased
	/// `min_core_bought` - Minimum core purchased as intermediary
	/// `recipient` - The address that receives the output asset
	pub fn asset_to_asset_transfer_input(
		asset_sold: &T::AssetId,
		asset_bought: &T::AssetId,
		amount_sold: T::Balance,
		min_amount_bought: T::Balance,
		min_core_bought: T::Balance,
		recipient: &AccountIdOf<T>,
		fee_rate: Permill,
	) {
	}

	/// Convert trade asset1 to trade asset2 via core asset. User specifies maximum
	/// input and exact output.
	///
	/// `asset_sold` - Trade asset1 ID
	/// `asset_bought` - Asset2 ID
	/// `amount_bought` - Amount of trade asset2 bought
	/// `max_amount_sold` - Maximum trade asset1 sold
	/// `max_core_sold` - Maximum core asset purchased as intermediary
	pub fn asset_to_asset_swap_output(
		asset_sold: &T::AssetId,
		asset_bought: &T::AssetId,
		amount_bought: T::Balance,
		max_amount_sold: T::Balance,
		max_core_sold: T::Balance,
		fee_rate: Permill,
	) {
	}

	/// Convert trade asset1 to trade asset2 via core asset and transfer the trade asset2
	/// to recipient from system account.
	///
	/// User specifies maximum input and exact output
	/// `asset_sold` - Trade asset1
	/// `asset_bought` - Asset2
	/// `amount_bought` - Amount of trade asset2 bought
	/// `max_amount_sold` - Maximum trade asset1 sold
	/// `max_core_sold` - Maximum core asset purchased as intermediary
	/// `recipient` - The address that receives the output asset
	pub fn asset_to_asset_transfer_output(
		asset_sold: &T::AssetId,
		asset_bought: &T::AssetId,
		amount_bought: T::Balance,
		max_amount_sold: T::Balance,
		max_core_sold: T::Balance,
		recipient: &AccountIdOf<T>,
		fee_rate: Permill,
	) {
	}

	//
	// Get Prices
	//

	/// `asset_id` - Trade asset
	/// `amount_sold` - Amount of core sold
	/// Returns amount of asset that can be bought with the input core
	pub fn core_to_asset_input_price(asset_id: &T::AssetId, amount_sold: T::Balance, fee_rate: Permill) -> T::Balance {
		T::Balance::sa(0)
	}

	/// `asset_id` - Trade asset
	/// `buy_amount`- Amount of the trade asset to buy
	/// Returns the amount of core asset needed to purchase `buy_amount` of trade asset.
	pub fn get_core_to_asset_output_price(
		asset_id: &T::AssetId,
		buy_amount: T::Balance,
		fee_rate: Permill,
	) -> rstd::result::Result<T::Balance, &'static str> {
		ensure!(buy_amount > Zero::zero(), "Buy amount must be a positive value");

		let core_asset_id = Self::core_asset_id();
		let exchange_key = (core_asset_id, *asset_id);
		let exchange_address = Self::generate_exchange_address(&exchange_key);

		let asset_reserve = <generic_asset::Module<T>>::free_balance(asset_id, &exchange_address);
		ensure!(asset_reserve > buy_amount, "Insufficient asset reserve in exchange");

		let core_reserve = <generic_asset::Module<T>>::free_balance(&core_asset_id, &exchange_address);

		Ok(Self::get_output_price(
			buy_amount,
			core_reserve,
			asset_reserve,
			fee_rate,
		))
	}

	/// `asset_id` - Trade asset
	/// `amount_sold` - Amount of trade assets sold
	/// Returns amount of core that can be bought with input assets.
	pub fn asset_to_core_input_price(asset_id: &T::AssetId, amount_sold: T::Balance, fee_rate: Permill) -> T::Balance {
		T::Balance::sa(0)
	}

	fn get_output_price(
		output_amount: T::Balance,
		input_reserve: T::Balance,
		output_reserve: T::Balance,
		fee_rate: Permill,
	) -> T::Balance {
		if input_reserve.is_zero() || output_reserve.is_zero() {
			return Zero::zero();
		}

		let numerator: T::Balance = input_reserve * output_amount;
		let denominator = output_reserve - output_amount;
		let output = numerator / denominator + One::one();
		fee_rate * output + output
	}

	/// `asset_id` - Trade asset
	/// `buy_amount` - Amount of output core
	/// Returns the amount of trade assets needed to buy `buy_amount` core assets.
	pub fn get_asset_to_core_output_price(
		asset_id: &T::AssetId,
		buy_amount: T::Balance,
		fee_rate: Permill,
	) -> rstd::result::Result<T::Balance, &'static str> {
		ensure!(buy_amount > Zero::zero(), "Buy amount must be a positive value");

		let core_asset_id = Self::core_asset_id();
		let exchange_key = (core_asset_id, *asset_id);
		let exchange_address = Self::generate_exchange_address(&exchange_key);

		let core_asset_reserve = <generic_asset::Module<T>>::free_balance(&core_asset_id, &exchange_address);
		ensure!(
			core_asset_reserve > buy_amount,
			"Insufficient core asset reserve in exchange"
		);

		let trade_asset_reserve = <generic_asset::Module<T>>::free_balance(&asset_id, &exchange_address);

		Ok(Self::get_output_price(
			buy_amount,
			trade_asset_reserve,
			core_asset_reserve,
			fee_rate,
		))
	}
}

#[cfg(test)]
mod tests {
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

	/// Initializes an address with the given exchange balance.
	/// e.g. `let investor = with_account!(0 => 10, 1 => 20)`
	macro_rules! with_account (
		($a1:expr => $b1:expr, $a2:expr => $b2:expr) => {
			{
				<generic_asset::Module<Test>>::set_free_balance(&$a1, &default_address(), $b1);
				<generic_asset::Module<Test>>::set_free_balance(&$a2, &default_address(), $b2);
				default_address()
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
	fn get_asset_to_core_output_price() {
		with_externalities(&mut ExtBuilder::default().build(), || {
			with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_ID => 1000);

			assert_ok!(
				CennzXSpot::get_asset_to_core_output_price(&TRADE_ASSET_ID, 123, <FeeRate<Test>>::get()),
				141
			);
		});
	}

	#[test]
	fn get_core_to_asset_output_price() {
		with_externalities(&mut ExtBuilder::default().build(), || {
			with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_ID => 1000);

			assert_ok!(
				CennzXSpot::get_core_to_asset_output_price(&TRADE_ASSET_ID, 123, <FeeRate<Test>>::get()),
				141
			);
		});
	}

	#[test]
	fn get_core_asset_output_price_zero_buy() {
		with_externalities(&mut ExtBuilder::default().build(), || {
			assert_err!(
				CennzXSpot::get_core_to_asset_output_price(&TRADE_ASSET_ID, 0, <FeeRate<Test>>::get()),
				"Buy amount must be a positive value"
			);
		});
	}

	#[test]
	fn get_asset_to_core_output_insufficient_reserve() {
		with_externalities(&mut ExtBuilder::default().build(), || {
			with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_ID => 1000);
			let investor = with_account!(CORE_ASSET_ID => 1000, TRADE_ASSET_ID => 1000);

			assert_err!(
				CennzXSpot::get_asset_to_core_output_price(
					&TRADE_ASSET_ID,
					1001, // amount_bought
					<FeeRate<Test>>::get()
				),
				"Insufficient core asset reserve in exchange"
			);
		});
	}

	#[test]
	fn asset_to_core_swap_output() {
		with_externalities(&mut ExtBuilder::default().build(), || {
			with_exchange!(CORE_ASSET_ID => 10, TRADE_ASSET_ID => 1000);
			let investor = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_ID => 2200);

			assert_ok!(CennzXSpot::asset_to_core_swap_output(
				Origin::signed(investor),
				TRADE_ASSET_ID,
				5,    // buy_amount: T::Balance,
				1400, // max_sale: T::Balance,
			));

			assert_exchange_balance_eq!(CORE_ASSET_ID => 5, TRADE_ASSET_ID => 2004);
			assert_balance_eq!(investor, TRADE_ASSET_ID => 1196);
		});
	}

	#[test]
	fn make_asset_to_core_swap_output() {
		with_externalities(&mut ExtBuilder::default().build(), || {
			with_exchange!(CORE_ASSET_ID => 10, TRADE_ASSET_ID => 1000);
			let investor = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_ID => 2200);

			assert_ok!(
				CennzXSpot::make_asset_to_core_swap_output(
					&investor,
					&TRADE_ASSET_ID,
					5,                              // buy_amount: T::Balance,
					1400,                           // max_sale: T::Balance,
					Permill::from_millionths(3000), // fee_rate
				),
				1004
			);

			assert_exchange_balance_eq!(CORE_ASSET_ID => 5, TRADE_ASSET_ID => 2004);
			assert_balance_eq!(investor, TRADE_ASSET_ID => 1196);
		});
	}

	#[test]
	fn asset_to_core_swap_output_zero_asset_sold_fails() {
		with_externalities(&mut ExtBuilder::default().build(), || {
			with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_ID => 1000);
			let investor = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_ID => 100);

			assert_err!(
				CennzXSpot::asset_to_core_swap_output(
					Origin::signed(investor),
					TRADE_ASSET_ID,
					0,   // buy_amount
					100, // max_sale,
				),
				"Buy amount must be a positive value"
			);
		});
	}

	#[test]
	fn asset_to_core_swap_output_insufficient_balance() {
		with_externalities(&mut ExtBuilder::default().build(), || {
			with_exchange!(CORE_ASSET_ID => 500, TRADE_ASSET_ID => 500);
			let investor = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_ID => 100);

			assert_err!(
				CennzXSpot::asset_to_core_swap_output(
					Origin::signed(investor),
					TRADE_ASSET_ID,
					101, // buy_amount
					500, // max_sale,
				),
				"Insufficient asset balance in buyer account"
			);
		});
	}

	#[test]
	fn asset_to_core_swap_output_exceed_max_asset() {
		with_externalities(&mut ExtBuilder::default().build(), || {
			with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_ID => 1000);
			let investor = with_account!(CORE_ASSET_ID => 50, TRADE_ASSET_ID => 50);

			assert_err!(
				CennzXSpot::asset_to_core_swap_output(
					Origin::signed(investor),
					TRADE_ASSET_ID,
					50, // buy_amount
					0,  // max_sale,
				),
				"Amount of asset sold would exceed the specified max. limit"
			);
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
}
