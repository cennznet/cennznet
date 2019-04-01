//!
//! CENNZX-SPOT
//!
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
#[macro_use]
mod tests;

mod impls;
mod types;
pub use impls::{ExchangeAddressFor, ExchangeAddressGenerator};
pub use types::FeeRate;

#[macro_use]
extern crate srml_support as support;

use cennznet_primitives::{Balance, CennznetExtrinsic, Index, Signature};
use generic_asset;
use parity_codec::Decode;
use rstd::prelude::*;
use runtime_io::twox_128;
use runtime_primitives::traits::{As, Bounded, One, Zero};
use support::{dispatch::Result, Dispatchable, Parameter, StorageDoubleMap, StorageMap, StorageValue};
use system::ensure_signed;

/// A nice type alias for CennznetExtrinsic
pub type CennznetExtrinsicOf<T> =
	CennznetExtrinsic<<T as system::Trait>::AccountId, Index, <T as Trait>::Call, Signature, Balance>;

// (core_asset_id, asset_id)
pub type ExchangeKey<T> = (
	<T as generic_asset::Trait>::AssetId,
	<T as generic_asset::Trait>::AssetId,
);

pub trait Trait: system::Trait + generic_asset::Trait {
	type Call: Parameter + Dispatchable<Origin = <Self as system::Trait>::Origin>;
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	/// A function type to get an exchange address given the asset ID pair.
	type ExchangeAddressGenerator: ExchangeAddressFor<Self::AssetId, Self::AccountId>;
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event<T>() = default;

		/// Swap asset (`asset_id`) to core asset. User specifies maximum input and exact output
		/// `recipient` - Account to receive core asset, defaults to origin if None
		/// `asset_id` - The asset ID to trade
		/// `buy_amount` - Amount of core asset to purchase (output)
		/// `max_sale` -  Maximum asset to sell (input)
		pub fn asset_to_core_swap_output(
			origin,
			recipient: Option<T::AccountId>,
			#[compact] asset_id: T::AssetId,
			#[compact] buy_amount: T::Balance,
			#[compact] max_sale: T::Balance
		) -> Result {
			let buyer = ensure_signed(origin)?;
			// Buyer is also recipient
			let _ = Self::make_asset_to_core_output(
				&buyer,
				&recipient.unwrap_or_else(|| buyer.clone()),
				&asset_id,
				buy_amount,
				max_sale,
				Self::fee_rate()
			)?;

			Ok(())
		}

		/// Swap core asset to trade asset. User specifies maximum input (core asset) and exact output.
		/// Buyer receives output.
		/// `recipient` - Account to receive trade asset, defaults to origin if None
		/// `asset_id` - The asset ID to trade
		/// `buy_amount` - The amount of asset to purchase
		/// `max_sale` - Maximum core asset to sell (input)
		pub fn core_to_asset_swap_output(
			origin,
			recipient: Option<T::AccountId>,
			#[compact] asset_id: T::AssetId,
			#[compact] buy_amount: T::Balance,
			#[compact] max_sale: T::Balance
		) -> Result {
			let buyer = ensure_signed(origin)?;
			let _ = Self::make_core_to_asset_output(
				&buyer,
				&recipient.unwrap_or_else(|| buyer.clone()),
				&asset_id,
				buy_amount,
				max_sale,
				Self::fee_rate()
			)?;

			Ok(())
		}

		/// Convert trade asset1 to trade asset2 via core asset. User specifies maximum
		/// input and exact output.
		///  origin
		/// `recipient` - Account to receive asset_bought, defaults to origin if None
		/// `asset_sold` - asset ID 1 to sell
		/// `asset_bought` - asset ID 2 to buy
		/// `buy_amount` - The amount of asset '2' to purchase
		/// `max_trade_asset_sale` - Maximum trade asset '1' to sell
		pub fn asset_to_asset_swap_output(
			origin,
			recipient: Option<T::AccountId>,
			#[compact] asset_sold: T::AssetId,
			#[compact] asset_bought: T::AssetId,
			#[compact] buy_amount: T::Balance,
			#[compact] max_trade_asset_sale: T::Balance
		) -> Result {
			let buyer = ensure_signed(origin)?;
			let _ = Self::make_asset_to_asset_output(
				 &buyer,
				 &recipient.unwrap_or_else(|| buyer.clone()),
				 &asset_sold,
				 &asset_bought,
				 buy_amount,
				 max_trade_asset_sale,
				 Self::fee_rate()
			)?;

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
			#[compact] asset_id: T::AssetId,
			#[compact] min_liquidity: T::Balance,
			#[compact] max_asset_amount: T::Balance,
			#[compact] core_amount: T::Balance
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
			let exchange_address = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, asset_id);

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
			#[compact] asset_id: T::AssetId,
			#[compact] liquidity_withdrawn: T::Balance,
			#[compact] min_asset_withdraw: T::Balance,
			#[compact] min_core_withdraw: T::Balance
		) -> Result {
			let from_account = ensure_signed(origin)?;
			ensure!(liquidity_withdrawn > Zero::zero(), "Amount of exchange asset to burn should exist");
			ensure!(min_asset_withdraw > Zero::zero() && min_core_withdraw > Zero::zero(), "Assets withdrawn to be greater than zero");

			let core_asset_id = Self::core_asset_id();
			let exchange_key = (core_asset_id, asset_id);
			let account_liquidity = Self::get_liquidity(&exchange_key, &from_account);
			ensure!(account_liquidity >= liquidity_withdrawn, "Tried to overdraw liquidity");

			let total_liquidity = Self::get_total_supply(&exchange_key);
			let exchange_address = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, asset_id);
			ensure!(total_liquidity > Zero::zero(), "Liquidity should exist");

			let trade_asset_reserve = <generic_asset::Module<T>>::free_balance(&asset_id, &exchange_address);
			let core_asset_reserve = <generic_asset::Module<T>>::free_balance(&core_asset_id, &exchange_address);
			let core_asset_amount = liquidity_withdrawn * core_asset_reserve / total_liquidity;
			let trade_asset_amount = liquidity_withdrawn * trade_asset_reserve / total_liquidity;
			ensure!(core_asset_amount >= min_core_withdraw, "Minimum core asset is required");
			ensure!(trade_asset_amount >= min_asset_withdraw, "Minimum trade asset is required");

			<generic_asset::Module<T>>::make_transfer(&core_asset_id, &exchange_address, &from_account, core_asset_amount)?;
			<generic_asset::Module<T>>::make_transfer(&asset_id, &exchange_address, &from_account, trade_asset_amount)?;
			Self::set_liquidity(&exchange_key, &from_account,
									account_liquidity - liquidity_withdrawn);
			Self::burn_total_supply(&exchange_key, liquidity_withdrawn);
			Self::deposit_event(RawEvent::RemoveLiquidity(from_account, core_asset_amount, asset_id, trade_asset_amount));
			Ok(())
		}

		/// Swap asset (`asset_id`) to core asset. User specifies min output and exact input
		/// `recipient` - Account to receive core asset, defaults to origin if None
		/// `asset_id` - The asset ID to trade
		/// `sell_amount` - Amount of trade asset to sell (input)
		/// `min_sale` - Min core asset to receive from sale (output)
		pub fn asset_to_core_swap_input(
			origin,
			recipient: Option<T::AccountId>,
			#[compact] asset_id: T::AssetId,
			#[compact] sell_amount: T::Balance,
			#[compact] min_sale: T::Balance
		) -> Result {
			let seller = ensure_signed(origin)?;
			let _ = Self::make_asset_to_core_input(
				&seller,
				&recipient.unwrap_or_else(|| seller.clone()),
				&asset_id,
				sell_amount,
				min_sale,
				Self::fee_rate()
			)?;

			Ok(())
		}

		/// Swap `sell_amount` of core asset for trade asset at the current exchange rate.
		/// Seller specifies exact input (core asset to sell) and minimum output (trade asset to receive).
		/// `recipient` - Account to receive trade asset, defaults to origin if None
		/// `asset_id` - Trade asset ID
		/// `sell_amount` - Exact amount of core asset to be sold
		/// `min_sale` - The min. trade asset to receive from sale
		pub fn core_to_asset_swap_input(
			origin,
			recipient: Option<T::AccountId>,
			#[compact] asset_id: T::AssetId,
			#[compact] sell_amount: T::Balance,
			#[compact] min_sale: T::Balance
		) -> Result {
			let seller = ensure_signed(origin)?;
			let _ = Self::make_core_to_asset_input(
				&seller,
				&recipient.unwrap_or_else(|| seller.clone()),
				&asset_id,
				sell_amount,
				min_sale,
				Self::fee_rate()
			)?;

			Ok(())
		}

		/// Convert trade asset1 to trade asset2 via core asset.
		/// Seller specifies exact input (asset 1) and minimum output (trade asset and core asset)
		/// `recipient` - Account to receive asset_bought, defaults to origin if None
		/// `asset_sold` - asset ID 1 to sell
		/// `asset_bought` - asset ID 2 to buy
		/// `sell_amount` - The amount of asset '1' to sell
		/// `min_trade_asset_sale` - Minimum trade asset '2' to receive from sale
		pub fn asset_to_asset_swap_input(
			origin,
			recipient: Option<T::AccountId>,
			#[compact] asset_sold: T::AssetId,
			#[compact] asset_bought: T::AssetId,
			#[compact] sell_amount: T::Balance,
			#[compact] min_trade_asset_sale: T::Balance
		) -> Result {
			let seller = ensure_signed(origin)?;
			let _ = Self::make_asset_to_asset_input(
				&seller,
				&recipient.unwrap_or_else(|| seller.clone()),
				&asset_sold,
				&asset_bought,
				sell_amount,
				min_trade_asset_sale,
				Self::fee_rate()
			)?;

			Ok(())
		}

		/// Set the spot exchange wide fee rate (root only)
		pub fn set_fee_rate(new_fee_rate: FeeRate) -> Result {
			<DefaultFeeRate<T>>::mutate(|fee_rate| *fee_rate = new_fee_rate);
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
		// AssetSold, AssetBought, CoreAsset, Buyer, SoldAmount, BoughtAmount, CoreAmount
		AssetToAssetPurchase(AssetId, AssetId, AssetId, AccountId, Balance, Balance, Balance),
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
	type Key2 = T::AccountId;
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
		pub DefaultFeeRate get(fee_rate) config(): FeeRate;
		/// Total supply of exchange token in existence.
		/// it will always be less than the core asset's total supply
		/// Key: `(asset id, core asset id)`
		pub TotalSupply get(total_supply): map ExchangeKey<T> => T::Balance;
	}
}

// The main implementation block for the module.
impl<T: Trait> Module<T> {
	// Storage R/W
	fn get_total_supply(exchange_key: &ExchangeKey<T>) -> T::Balance {
		<TotalSupply<T>>::get(exchange_key)
	}

	/// mint total supply for an exchange pool
	fn mint_total_supply(exchange_key: &ExchangeKey<T>, increase: T::Balance) {
		<TotalSupply<T>>::mutate(exchange_key, |balance| *balance += increase); // will not overflow because it's limited by core assets's total supply
	}

	fn burn_total_supply(exchange_key: &ExchangeKey<T>, decrease: T::Balance) {
		<TotalSupply<T>>::mutate(exchange_key, |balance| *balance -= decrease); // will not underflow for the same reason
	}

	fn set_liquidity(exchange_key: &ExchangeKey<T>, who: &T::AccountId, balance: T::Balance) {
		<LiquidityBalance<T>>::insert(exchange_key, who, balance);
	}

	pub fn get_liquidity(exchange_key: &ExchangeKey<T>, who: &T::AccountId) -> T::Balance {
		<LiquidityBalance<T>>::get(exchange_key, who).unwrap_or_else(Default::default)
	}

	/// Trade core asset for asset (`asset_id`) at the given `fee_rate`.
	/// `seller` - The address selling input asset
	/// `recipient` - The address receiving payment of output asset
	/// `asset_id` - The asset ID to trade
	/// `sell_amount` - Amount of core asset to sell (input)
	/// `min_sale` -  The minimum trade asset value of the sale (output)
	/// `fee_rate` - The % of exchange fees for the trade
	pub fn make_core_to_asset_input(
		seller: &T::AccountId,
		recipient: &T::AccountId,
		asset_id: &T::AssetId,
		sell_amount: T::Balance,
		min_sale: T::Balance,
		fee_rate: FeeRate,
	) -> rstd::result::Result<T::Balance, &'static str> {
		let sale_value = Self::get_core_to_asset_input_price(asset_id, sell_amount, fee_rate)?;

		ensure!(
			sale_value > Zero::zero(),
			"Asset sale value should be greater than zero"
		);
		ensure!(
			sale_value >= min_sale,
			"The sale value of input is less than the required min."
		);
		let core_asset_id = Self::core_asset_id();
		ensure!(
			<generic_asset::Module<T>>::free_balance(&core_asset_id, seller) >= sell_amount,
			"Insufficient core asset balance in seller account"
		);

		let exchange_address = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, *asset_id);

		let _ = <generic_asset::Module<T>>::make_transfer(&core_asset_id, seller, &exchange_address, sell_amount).and(
			<generic_asset::Module<T>>::make_transfer(asset_id, &exchange_address, recipient, sale_value),
		);

		Self::deposit_event(RawEvent::TradeAssetPurchase(
			*asset_id,
			seller.clone(),
			sell_amount,
			sale_value,
		));

		Ok(sale_value)
	}

	/// Trade asset (`asset_id`) to core asset at the given `fee_rate`
	/// `asset_id` - The asset ID to trade
	/// `buy_amount` - Amount of core asset to purchase (output)
	/// `max_sale` -  Maximum asset to sell (input)
	/// `fee_rate` - The % of exchange fees for the trade
	pub fn make_asset_to_core_output(
		buyer: &T::AccountId,
		recipient: &T::AccountId,
		asset_id: &T::AssetId,
		buy_amount: T::Balance,
		max_sale: T::Balance,
		fee_rate: FeeRate,
	) -> rstd::result::Result<T::Balance, &'static str> {
		let sold_amount = Self::get_asset_to_core_output_price(asset_id, buy_amount, fee_rate)?;
		ensure!(
			sold_amount > Zero::zero(),
			"Amount of asset sold should be greater than zero"
		);
		ensure!(
			max_sale >= sold_amount,
			"Amount of asset sold would exceed the specified max. limit"
		);
		ensure!(
			<generic_asset::Module<T>>::free_balance(asset_id, buyer) >= sold_amount,
			"Insufficient asset balance in buyer account"
		);

		let core_asset_id = Self::core_asset_id();
		let exchange_address = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, *asset_id);

		let _ = <generic_asset::Module<T>>::make_transfer(asset_id, buyer, &exchange_address, sold_amount).and(
			<generic_asset::Module<T>>::make_transfer(&core_asset_id, &exchange_address, recipient, buy_amount),
		);

		Self::deposit_event(RawEvent::CoreAssetPurchase(
			*asset_id,
			buyer.clone(),
			sold_amount,
			buy_amount,
		));

		Ok(sold_amount)
	}

	/// Trade core asset to asset (`asset_id`) at the given `fee_rate`
	/// `buyer` - Account buying core asset for trade asset
	/// `recipient` - Account receiving trade asset
	/// `asset_id` - The asset ID to trade
	/// `buy_amount` - Amount of core asset to purchase (output)
	/// `max_sale` -  Maximum asset to sell (input)
	/// `fee_rate` - The % of exchange fees for the trade
	pub fn make_core_to_asset_output(
		buyer: &T::AccountId,
		recipient: &T::AccountId,
		asset_id: &T::AssetId,
		buy_amount: T::Balance,
		max_sale: T::Balance,
		fee_rate: FeeRate,
	) -> rstd::result::Result<T::Balance, &'static str> {
		let sold_amount = Self::get_core_to_asset_output_price(asset_id, buy_amount, fee_rate)?;
		ensure!(
			sold_amount > Zero::zero(),
			"Amount of core asset sold should be greater than zero"
		);
		ensure!(
			max_sale >= sold_amount,
			"Amount of core asset sold would exceed the specified max. limit"
		);
		let core_asset_id = Self::core_asset_id();
		ensure!(
			<generic_asset::Module<T>>::free_balance(&core_asset_id, buyer) >= sold_amount,
			"Insufficient core asset balance in buyer account"
		);

		let exchange_address = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, *asset_id);

		let _ = <generic_asset::Module<T>>::make_transfer(&core_asset_id, buyer, &exchange_address, sold_amount).and(
			<generic_asset::Module<T>>::make_transfer(asset_id, &exchange_address, recipient, buy_amount),
		);

		Self::deposit_event(RawEvent::TradeAssetPurchase(
			*asset_id,
			buyer.clone(),
			sold_amount,
			buy_amount,
		));

		Ok(sold_amount)
	}

	/// Convert trade asset1 to trade asset2 via core asset. User specifies maximum
	/// input and exact output.
	/// `buyer` - Account buying core asset for trade asset
	/// `recipient` - Account receiving trade asset
	/// `asset_a` - asset ID to sell
	/// `asset_b` - asset ID to buy
	/// `buy_amount_b` - The amount of asset 'b' to purchase (output)
	/// `max_a_for_sale` - Maximum trade asset 'a' to sell
	/// `fee_rate` - The % of exchange fees for the trade
	pub fn make_asset_to_asset_output(
		buyer: &T::AccountId,
		recipient: &T::AccountId,
		asset_a: &T::AssetId,
		asset_b: &T::AssetId,
		buy_amount_for_b: T::Balance,
		max_a_for_sale: T::Balance,
		fee_rate: FeeRate,
	) -> rstd::result::Result<T::Balance, &'static str> {
		// Calculate amount of core token needed to buy trade asset 2 of #buy_amount amount
		let core_for_b = Self::get_core_to_asset_output_price(asset_b, buy_amount_for_b, fee_rate)?;
		let core_asset_id = Self::core_asset_id();
		let exchange_address_a = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, *asset_a);
		let asset_sold_a = Self::get_asset_to_core_output_price(asset_a, core_for_b, fee_rate)?;
		// sold asset is always > 0
		ensure!(
			max_a_for_sale >= asset_sold_a,
			"Amount of asset sold would exceed the specified max. limit"
		);
		ensure!(
			<generic_asset::Module<T>>::free_balance(&asset_a, buyer) >= asset_sold_a,
			"Insufficient asset balance in buyer account"
		);

		let core_asset_a = Self::get_core_to_asset_output_price(asset_b, buy_amount_for_b, fee_rate)?;
		ensure!(
			core_asset_a > Zero::zero(),
			"Amount of core asset sold should be greater than zero"
		);
		ensure!(
			<generic_asset::Module<T>>::free_balance(&core_asset_id, &exchange_address_a) >= core_asset_a,
			"Insufficient core asset balance in exchange account"
		);

		let exchange_address_b = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, *asset_b);
		let _ = <generic_asset::Module<T>>::make_transfer(&asset_a, buyer, &exchange_address_a, asset_sold_a)
			.and(<generic_asset::Module<T>>::make_transfer(
				&core_asset_id,
				&exchange_address_a,
				&exchange_address_b,
				core_asset_a,
			))
			.and(<generic_asset::Module<T>>::make_transfer(
				asset_b,
				&exchange_address_b,
				recipient,
				buy_amount_for_b,
			));

		Self::deposit_event(RawEvent::AssetToAssetPurchase(
			*asset_a,         // asset sold
			*asset_b,         // asset bought
			core_asset_id,    // core asset
			buyer.clone(),    // buyer
			asset_sold_a,     // sold amount
			buy_amount_for_b, // bought amount
			core_for_b,       // core amount
		));

		Ok(asset_sold_a)
	}

	/// Convert trade asset to core asset. User specifies exact
	/// input (trade asset) and minimum output.
	///
	/// `asset_id` - Trade asset ID
	/// `sell_amount` - Exact amount of trade asset to be sold
	/// `min_sale` - Minimum amount of core asset to receive from sale
	pub fn make_asset_to_core_input(
		buyer: &T::AccountId,
		recipient: &T::AccountId,
		asset_id: &T::AssetId,
		sell_amount: T::Balance,
		min_sale: T::Balance,
		fee_rate: FeeRate,
	) -> rstd::result::Result<T::Balance, &'static str> {
		ensure!(
			<generic_asset::Module<T>>::free_balance(asset_id, buyer) >= sell_amount,
			"Insufficient asset balance in seller account"
		);

		let sale_value = Self::get_asset_to_core_input_price(asset_id, sell_amount, fee_rate)?;

		ensure!(
			sale_value >= min_sale,
			"The sale value of input is less than the required min."
		);

		let core_asset_id = Self::core_asset_id();
		let exchange_address = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, *asset_id);

		ensure!(
			<generic_asset::Module<T>>::free_balance(&core_asset_id, &exchange_address) >= sale_value,
			"Insufficient asset balance in exchange account"
		);

		let _ = <generic_asset::Module<T>>::make_transfer(asset_id, buyer, &exchange_address, sell_amount).and(
			<generic_asset::Module<T>>::make_transfer(&core_asset_id, &exchange_address, recipient, sale_value),
		);

		Self::deposit_event(RawEvent::CoreAssetPurchase(
			*asset_id,
			buyer.clone(),
			sell_amount,
			sale_value,
		));

		Ok(sale_value)
	}

	/// Convert trade asset1 to trade asset2 via core asset.
	/// Seller specifies exact input (asset 1) and minimum output (trade asset and core asset)
	/// `recipient` - Receiver of asset_bought
	/// `asset_a` - asset ID to sell
	/// `asset_b` - asset ID to buy
	/// `sell_amount_for_a` - The amount of asset to sell
	/// `min_b_from_sale` - Minimum trade asset 'b' to receive from sale
	pub fn make_asset_to_asset_input(
		seller: &T::AccountId,
		recipient: &T::AccountId,
		asset_a: &T::AssetId,
		asset_b: &T::AssetId,
		sell_amount_for_a: T::Balance,
		min_b_from_sale: T::Balance,
		fee_rate: FeeRate,
	) -> rstd::result::Result<T::Balance, &'static str> {
		ensure!(
			<generic_asset::Module<T>>::free_balance(&asset_a, seller) >= sell_amount_for_a,
			"Insufficient asset balance in seller account"
		);

		let core_asset_id = Self::core_asset_id();
		let exchange_address_a = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, *asset_a);
		let sale_value_a = Self::get_asset_to_core_input_price(asset_a, sell_amount_for_a, fee_rate)?;
		let asset_b_received = Self::get_core_to_asset_input_price(asset_b, sale_value_a, fee_rate)?;

		ensure!(
			asset_b_received > Zero::zero(),
			"Asset sale value should be greater than zero"
		);
		ensure!(
			asset_b_received >= min_b_from_sale,
			"The sale value of input is less than the required min"
		);
		ensure!(
			<generic_asset::Module<T>>::free_balance(&core_asset_id, &exchange_address_a) >= sale_value_a,
			"Insufficient core asset balance in exchange account"
		);

		let exchange_address_b = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, *asset_b);

		let _ = <generic_asset::Module<T>>::make_transfer(&asset_a, seller, &exchange_address_a, sell_amount_for_a)
			.and(<generic_asset::Module<T>>::make_transfer(
				&core_asset_id,
				&exchange_address_a,
				&exchange_address_b,
				sale_value_a,
			))
			.and(<generic_asset::Module<T>>::make_transfer(
				asset_b,
				&exchange_address_b,
				recipient,
				asset_b_received,
			));

		Self::deposit_event(RawEvent::AssetToAssetPurchase(
			*asset_a,          // asset sold
			*asset_b,          // asset bought
			core_asset_id,     // core asset
			seller.clone(),    // buyer
			sell_amount_for_a, // sold amount
			asset_b_received,  // bought amount
			sale_value_a,      // core amount
		));

		Ok(asset_b_received)
	}

	//
	// Get Prices
	//

	/// `asset_id` - Trade asset
	/// `buy_amount`- Amount of the trade asset to buy
	/// Returns the amount of core asset needed to purchase `buy_amount` of trade asset.
	pub fn get_core_to_asset_output_price(
		asset_id: &T::AssetId,
		buy_amount: T::Balance,
		fee_rate: FeeRate,
	) -> rstd::result::Result<T::Balance, &'static str> {
		ensure!(buy_amount > Zero::zero(), "Buy amount must be a positive value");

		let core_asset_id = Self::core_asset_id();
		let exchange_address = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, *asset_id);

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
	pub fn get_asset_to_core_input_price(
		asset_id: &T::AssetId,
		sell_amount: T::Balance,
		fee_rate: FeeRate,
	) -> rstd::result::Result<T::Balance, &'static str> {
		ensure!(sell_amount > Zero::zero(), "Sell amount must be a positive value");

		let core_asset_id = Self::core_asset_id();
		let exchange_address = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, *asset_id);

		let asset_reserve = <generic_asset::Module<T>>::free_balance(asset_id, &exchange_address);
		let core_reserve = <generic_asset::Module<T>>::free_balance(&core_asset_id, &exchange_address);
		Ok(Self::get_input_price(
			sell_amount,
			asset_reserve,
			core_reserve,
			fee_rate,
		))
	}

	fn get_output_price(
		output_amount: T::Balance,
		input_reserve: T::Balance,
		output_reserve: T::Balance,
		fee_rate: FeeRate,
	) -> T::Balance {
		if input_reserve.is_zero() || output_reserve.is_zero() {
			return Zero::zero();
		}

		// Special case, in theory price should progress towards infinity
		if output_amount >= output_reserve {
			return T::Balance::max_value();
		}

		let numerator: T::Balance = input_reserve * output_amount;
		let denominator = output_reserve - output_amount;
		let output: T::Balance = numerator / denominator + One::one();

		(FeeRate::one() + fee_rate) * output
	}

	fn get_input_price(
		input_amount: T::Balance,
		input_reserve: T::Balance,
		output_reserve: T::Balance,
		fee_rate: FeeRate,
	) -> T::Balance {
		if input_reserve.is_zero() || output_reserve.is_zero() {
			return Zero::zero();
		}
		let div_rate = FeeRate::one() + fee_rate;
		// This operation rounds away necessary decimal points. In order to-
		// counteract this, we scale the input amount
		let input_amount_less_fee_scaled = FeeRate::div(
			input_amount * T::Balance::sa(1_000_000), // scale up
			div_rate,
		);
		let numerator: T::Balance = input_amount_less_fee_scaled * output_reserve;
		let denominator: T::Balance = FeeRate::div(input_amount, div_rate) + input_reserve;

		numerator / denominator / T::Balance::sa(1_000_000) // undo scaling
	}

	/// `asset_id` - Trade asset
	/// `buy_amount` - Amount of output core
	/// `fee_rate` - The % of exchange fees for the trade
	/// Returns the amount of trade assets needed to buy `buy_amount` core assets.
	pub fn get_asset_to_core_output_price(
		asset_id: &T::AssetId,
		buy_amount: T::Balance,
		fee_rate: FeeRate,
	) -> rstd::result::Result<T::Balance, &'static str> {
		ensure!(buy_amount > Zero::zero(), "Buy amount must be a positive value");

		let core_asset_id = Self::core_asset_id();
		let exchange_address = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, *asset_id);

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

	/// Returns the amount of trade asset to pay for `sell_amount` of core sold.
	///
	/// `asset_id` - Trade asset
	/// `sell_amount` - Amount of input core to sell
	/// `fee_rate` - The % of exchange fees for the trade
	pub fn get_core_to_asset_input_price(
		asset_id: &T::AssetId,
		sell_amount: T::Balance,
		fee_rate: FeeRate,
	) -> rstd::result::Result<T::Balance, &'static str> {
		ensure!(sell_amount > Zero::zero(), "Sell amount must be a positive value");

		let core_asset_id = Self::core_asset_id();
		let exchange_address = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, *asset_id);
		let core_asset_reserve = <generic_asset::Module<T>>::free_balance(&core_asset_id, &exchange_address);
		let trade_asset_reserve = <generic_asset::Module<T>>::free_balance(asset_id, &exchange_address);

		let output_amount = Self::get_input_price(sell_amount, core_asset_reserve, trade_asset_reserve, fee_rate);

		ensure!(
			trade_asset_reserve > output_amount,
			"Insufficient trade asset reserve in exchange"
		);

		Ok(output_amount)
	}

	/// Get the currently executing extrinsic
	fn current_extrinsic() -> rstd::result::Result<CennznetExtrinsicOf<T>, &'static str> {
		let extrinsic_index: u32 = <system::Module<T>>::extrinsic_index().ok_or("No extrinsic index found")?;
		let extrinsic_data: Vec<u8> = <system::Module<T>>::extrinsic_data(extrinsic_index);
		Decode::decode(&mut &extrinsic_data[..]).ok_or("Got extrinsic with bad encoding")
	}
}
