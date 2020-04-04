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
//! CENNZX-Spot exchange
//!
#![cfg_attr(not(feature = "std"), no_std)]

mod mock;
#[macro_use]
mod tests;

mod impls;
mod types;
pub use impls::{ExchangeAddressFor, ExchangeAddressGenerator};
pub use types::{FeeRate, HighPrecisionUnsigned, LowPrecisionUnsigned, PerMilli, PerMillion};

#[macro_use]
extern crate frame_support;

use core::convert::TryFrom;
use frame_support::{dispatch::Dispatchable, sp_runtime::traits::Saturating, Parameter, StorageDoubleMap};
use frame_system::{ensure_root, ensure_signed};
use pallet_generic_asset;
use sp_runtime::traits::{One, Zero};
use sp_runtime::{DispatchError, DispatchResult};
use sp_std::prelude::*;

// (core_asset_id, asset_id)
pub type ExchangeKey<T> = (
	<T as pallet_generic_asset::Trait>::AssetId,
	<T as pallet_generic_asset::Trait>::AssetId,
);

pub trait Trait: frame_system::Trait + pallet_generic_asset::Trait {
	type Call: Parameter + Dispatchable<Origin = <Self as frame_system::Trait>::Origin>;
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
	/// A function type to get an exchange address given the asset ID pair.
	type ExchangeAddressGenerator: ExchangeAddressFor<Self::AssetId, Self::AccountId>;
	type BalanceToUnsignedInt: From<<Self as pallet_generic_asset::Trait>::Balance> + Into<LowPrecisionUnsigned>;
	type UnsignedIntToBalance: From<LowPrecisionUnsigned> + Into<<Self as pallet_generic_asset::Trait>::Balance>;
}

decl_error! {
	pub enum Error for Module<T: Trait> {
		/// Exchange pool is empty.
		EmptyExchangePool,
		// Insufficient asset reserve in exchange
		InsufficientAssetReserve,
		// Trader has insufficient balance
		InsufficientBalance,
		// Buy amount must be a positive value
		BuyAmountNotPositive,
		// The sale value of input is less than the required minimum.
		SaleValueBelowRequiredMinimum,
		// Price exceeds the specified max. limit
		PriceAboveMaxLimit,
		// Tried to overdraw liquidity
		LiquidityTooLow,
		// Minimum trade asset is required
		MinimumTradeAssetIsRequired,
		// Minimum core asset is required
		MinimumCoreAssetIsRequired,
		// Assets withdrawn to be greater than zero
		AssetToWithdrawNotAboveZero,
		// Amount of exchange asset to burn should exist
		LiquidityToWithdrawNotAboveZero,
		// Liquidity should exist
		NoLiquidityToRemove,
		// trade asset amount must be greater than zero
		TradeAssetToAddLiquidityNotAboveZero,
		// core asset amount must be greater than zero
		CoreAssetToAddLiquidityNotAboveZero,
		// not enough core asset in balance
		CoreAssetBalanceToAddLiquidityTooLow,
		// not enough trade asset balance
		TradeAssetBalanceToAddLiquidityTooLow,
		// Minimum liquidity is required
		LiquidityMintableLowerThanRequired,
		// Token liquidity check unsuccessful
		TradeAssetToAddLiquidityAboveMaxAmount,
		// Asset to core sell amount must be a positive value
		AssetToCoreSellAmountNotAboveZero,
		// Core to Asset sell amount must be a positive value
		CoreToAssetSellAmountNotAboveZero,
		// Asset to swap should not be equal
		AssetCannotSwapForItself,
		// Asset id doesn't exist
		InvalidAssetId,
		Overflow,
		DivideByZero,
	}
}

decl_module! {

	pub struct Module<T: Trait> for enum Call where origin: T::Origin, system = frame_system {
		type Error = Error<T>;

		fn deposit_event() = default;

		/// Buy `asset_to_buy` with `asset_to_sell`.
		/// User specifies an exact `buy_amount` and a `maximum_sell` amount.
		///
		/// `recipient` - Account to receive `buy_amount`, defaults to `origin` if None
		/// `asset_to_sell` - asset ID to sell
		/// `asset_to_buy` - asset ID to buy
		/// `buy_amount` - The amount `asset_to_buy` to purchase
		/// `maximum_sell` - Maximum `asset_to_sell` to pay
		pub fn buy_asset(
			origin,
			recipient: Option<T::AccountId>,
			#[compact] asset_to_sell: T::AssetId,
			#[compact] asset_to_buy: T::AssetId,
			#[compact] buy_amount: T::Balance,
			#[compact] maximum_sell: T::Balance
		) -> DispatchResult {
			let trader = ensure_signed(origin)?;
			let _ = Self::execute_buy(
				&trader,
				&recipient.unwrap_or_else(|| trader.clone()),
				&asset_to_sell,
				&asset_to_buy,
				buy_amount,
				maximum_sell,
			)?;
			Ok(())
		}

		/// Sell `asset_to_sell` for `asset_to_buy`.
		/// User specifies an exact `sell_amount` and a `minimum_buy` amount.
		///
		/// `recipient` - Account to receive `buy_amount`, defaults to `origin` if None
		/// `asset_to_sell` - asset ID to sell
		/// `asset_to_buy` - asset ID to buy
		/// `sell_amount` - The amount `asset_to_buy` to purchase
		/// `minimum_buy` - Maximum `asset_to_sell` to pay
		pub fn sell_asset(
			origin,
			recipient: Option<T::AccountId>,
			#[compact] asset_to_sell: T::AssetId,
			#[compact] asset_to_buy: T::AssetId,
			#[compact] sell_amount: T::Balance,
			#[compact] minimum_buy: T::Balance
		) -> DispatchResult {
			let trader = ensure_signed(origin)?;
			let _ = Self::execute_sell(
				&trader,
				&recipient.unwrap_or_else(|| trader.clone()),
				&asset_to_sell,
				&asset_to_buy,
				sell_amount,
				minimum_buy
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
			ensure!(
				!max_asset_amount.is_zero(),
				Error::<T>::TradeAssetToAddLiquidityNotAboveZero
			);
			ensure!(
				!core_amount.is_zero(),
				Error::<T>::CoreAssetToAddLiquidityNotAboveZero
			);
			ensure!(
				<pallet_generic_asset::Module<T>>::free_balance(&core_asset_id, &from_account) >= core_amount,
				Error::<T>::CoreAssetBalanceToAddLiquidityTooLow
			);
			ensure!(
				<pallet_generic_asset::Module<T>>::free_balance(&asset_id, &from_account) >= max_asset_amount,
				Error::<T>::TradeAssetBalanceToAddLiquidityTooLow
			);
			let exchange_key = (core_asset_id, asset_id);
			let total_liquidity = <TotalLiquidity<T>>::get(&exchange_key);
			let exchange_address = T::ExchangeAddressGenerator::exchange_address_for(asset_id);
			let core_asset_reserve = <pallet_generic_asset::Module<T>>::free_balance(&core_asset_id, &exchange_address);

			let (trade_asset_amount, liquidity_minted) = if total_liquidity.is_zero() || core_asset_reserve.is_zero() {
				// new exchange pool
				(max_asset_amount, core_amount)
			} else {
				let trade_asset_reserve = <pallet_generic_asset::Module<T>>::free_balance(&asset_id, &exchange_address);
				let trade_asset_amount = core_amount * trade_asset_reserve / core_asset_reserve + One::one();
				let liquidity_minted = core_amount * total_liquidity / core_asset_reserve;
				ensure!(
					liquidity_minted >= min_liquidity,
					Error::<T>::LiquidityMintableLowerThanRequired
				);
				ensure!(
					max_asset_amount >= trade_asset_amount,
					Error::<T>::TradeAssetToAddLiquidityAboveMaxAmount
				);
				(trade_asset_amount, liquidity_minted)
			};

			<pallet_generic_asset::Module<T>>::make_transfer(&core_asset_id, &from_account, &exchange_address, core_amount)?;
			<pallet_generic_asset::Module<T>>::make_transfer(&asset_id, &from_account, &exchange_address, trade_asset_amount)?;

			Self::mint_liquidity(&exchange_key, &from_account, liquidity_minted);
			Self::deposit_event(RawEvent::AddLiquidity(from_account, core_amount, asset_id, trade_asset_amount));
		}

		/// Burn exchange assets to withdraw core asset and trade asset at current ratio
		///
		/// `asset_id` - The trade asset ID
		/// `liquidity_to_withdraw` - Amount of user's liquidity to withdraw
		/// `min_asset_withdraw` - The minimum trade asset withdrawn
		/// `min_core_withdraw` -  The minimum core asset withdrawn
		pub fn remove_liquidity(
			origin,
			#[compact] asset_id: T::AssetId,
			#[compact] liquidity_to_withdraw: T::Balance,
			#[compact] min_asset_withdraw: T::Balance,
			#[compact] min_core_withdraw: T::Balance
		) -> DispatchResult {
			let from_account = ensure_signed(origin)?;

			let core_asset_id = Self::core_asset_id();
			let exchange_key = (core_asset_id, asset_id);
			let account_liquidity = <LiquidityBalance<T>>::get(&exchange_key, &from_account);
			ensure!(
				account_liquidity >= liquidity_to_withdraw,
				Error::<T>::LiquidityTooLow
			);

			let total_liquidity = <TotalLiquidity<T>>::get(&exchange_key);
			let exchange_address = T::ExchangeAddressGenerator::exchange_address_for(asset_id);
			ensure!(
				total_liquidity > Zero::zero(),
				Error::<T>::NoLiquidityToRemove
			);

			let trade_asset_reserve = <pallet_generic_asset::Module<T>>::free_balance(&asset_id, &exchange_address);
			let core_asset_reserve = <pallet_generic_asset::Module<T>>::free_balance(&core_asset_id, &exchange_address);
			let core_asset_amount = liquidity_to_withdraw * core_asset_reserve / total_liquidity;
			let trade_asset_amount = liquidity_to_withdraw * trade_asset_reserve / total_liquidity;
			ensure!(
				core_asset_amount >= min_core_withdraw,
				Error::<T>::MinimumCoreAssetIsRequired
			);
			ensure!(
				trade_asset_amount >= min_asset_withdraw,
				Error::<T>::MinimumTradeAssetIsRequired
			);

			<pallet_generic_asset::Module<T>>::make_transfer(&core_asset_id, &exchange_address, &from_account, core_asset_amount)?;
			<pallet_generic_asset::Module<T>>::make_transfer(&asset_id, &exchange_address, &from_account, trade_asset_amount)?;
			Self::burn_liquidity(&exchange_key, &from_account, liquidity_to_withdraw);
			Self::deposit_event(RawEvent::RemoveLiquidity(from_account, core_asset_amount, asset_id, trade_asset_amount));
			Ok(())
		}

		/// Set the spot exchange wide fee rate (root only)
		pub fn set_fee_rate(origin, new_fee_rate: FeeRate<PerMillion>) -> DispatchResult {
			ensure_root(origin)?;
			DefaultFeeRate::mutate(|fee_rate| *fee_rate = new_fee_rate);
			Ok(())
		}
	}
}

decl_event!(
	pub enum Event<T>
	where
		<T as frame_system::Trait>::AccountId,
		<T as pallet_generic_asset::Trait>::AssetId,
		<T as pallet_generic_asset::Trait>::Balance
	{
		/// Provider, core asset amount, trade asset id, trade asset amount
		AddLiquidity(AccountId, Balance, AssetId, Balance),
		/// Provider, core asset amount, trade asset id, trade asset amount
		RemoveLiquidity(AccountId, Balance, AssetId, Balance),
		/// AssetSold, AssetBought, Buyer, SoldAmount, BoughtAmount
		AssetPurchase(AssetId, AssetId, AccountId, Balance, Balance),
	}
);

decl_storage! {
	trait Store for Module<T: Trait> as CennzxSpot {
		/// AssetId of Core Asset
		pub CoreAssetId get(core_asset_id) config(): T::AssetId;
		/// Default Trading fee rate
		pub DefaultFeeRate get(fee_rate) config(): FeeRate<PerMillion>;
		/// Total liquidity holdings of all investers in an exchange.
		/// ie/ total_liquidity(exchange) == sum(liquidity_balance(exchange, user)) at all times
		pub TotalLiquidity get(total_liquidity): map hasher(twox_64_concat) ExchangeKey<T> => T::Balance;

		/// Liquidity holdings of a user in an exchange pool.
		/// Key: `(core_asset_id, trade_asset_id), account_id`
		pub LiquidityBalance get(liquidity_balance): double_map hasher(twox_64_concat) ExchangeKey<T>, hasher(blake2_128_concat) T::AccountId => T::Balance;
	}
}

// The main implementation block for the module.
impl<T: Trait> Module<T> {
	/// Mint liquidity holdings for a user in a specified exchange
	fn mint_liquidity(exchange_key: &ExchangeKey<T>, who: &T::AccountId, increase: T::Balance) {
		let balance = <LiquidityBalance<T>>::get(exchange_key, who);
		let new_balance = balance.saturating_add(increase);
		<LiquidityBalance<T>>::insert(exchange_key, who, new_balance);
		<TotalLiquidity<T>>::mutate(exchange_key, |balance| *balance = balance.saturating_add(increase));
	}

	/// Burn liquidity holdings from a user in a specified exchange
	fn burn_liquidity(exchange_key: &ExchangeKey<T>, who: &T::AccountId, decrease: T::Balance) {
		let balance = <LiquidityBalance<T>>::get(exchange_key, who);
		let decrease = decrease.min(balance);
		let new_balance = balance - decrease;
		<LiquidityBalance<T>>::insert(exchange_key, who, new_balance);
		<TotalLiquidity<T>>::mutate(exchange_key, |balance| *balance = balance.saturating_sub(decrease));
	}

	//
	// Get Prices
	//

	/// Get the buy price of some asset for another
	/// In simple terms: 'If I want to buy _x_ amount of asset _a_ how much of asset _b_ will it cost?'
	/// `asset_to_buy` is the asset to buy
	/// `amount_to_buy` is the amount of `asset_to_buy` required
	/// `asset_to_pay` is the asset to use for payment (the final price will be given in this asset)
	pub fn get_buy_price(
		asset_to_buy: T::AssetId,
		amount_to_buy: T::Balance,
		asset_to_pay: T::AssetId,
	) -> Result<T::Balance, DispatchError> {
		ensure!(asset_to_buy != asset_to_pay, Error::<T>::AssetCannotSwapForItself);

		// Find the cost of `amount_to_buy` of `asset_to_buy` in terms of core asset
		// (how much core asset does it cost?).
		let core_asset_amount = if asset_to_buy == Self::core_asset_id() {
			amount_to_buy
		} else {
			Self::get_core_to_asset_buy_price(&asset_to_buy, amount_to_buy)?
		};

		// Find the price of `core_asset_amount` in terms of `asset_to_pay`
		// (how much `asset_to_pay` does `core_asset_amount` cost?)
		let pay_asset_amount = if asset_to_pay == Self::core_asset_id() {
			core_asset_amount
		} else {
			Self::get_asset_to_core_buy_price(&asset_to_pay, core_asset_amount)?
		};

		Ok(pay_asset_amount)
	}

	/// `asset_id` - Trade asset
	/// `buy_amount` - Amount of output core
	/// Returns the amount of trade assets needed to buy `buy_amount` core assets.
	pub fn get_asset_to_core_buy_price(
		asset_id: &T::AssetId,
		buy_amount: T::Balance,
	) -> sp_std::result::Result<T::Balance, DispatchError> {
		ensure!(buy_amount > Zero::zero(), Error::<T>::BuyAmountNotPositive);

		let (core_reserve, asset_reserve) = Self::get_exchange_reserves(asset_id);
		Self::calculate_buy_price(buy_amount, asset_reserve, core_reserve)
	}

	/// `asset_id` - Trade asset
	/// `buy_amount`- Amount of the trade asset to buy
	/// Returns the amount of core asset needed to purchase `buy_amount` of trade asset.
	pub fn get_core_to_asset_buy_price(
		asset_id: &T::AssetId,
		buy_amount: T::Balance,
	) -> sp_std::result::Result<T::Balance, DispatchError> {
		ensure!(buy_amount > Zero::zero(), Error::<T>::BuyAmountNotPositive);

		let (core_reserve, asset_reserve) = Self::get_exchange_reserves(asset_id);
		Self::calculate_buy_price(buy_amount, core_reserve, asset_reserve)
	}

	/// `buy_amount` - Amount to buy
	/// `sell_reserve`- How much of the asset to sell is in the exchange
	/// `buy_reserve` - How much of the asset to buy is in the exchange
	/// Returns the amount of sellable asset is required
	fn calculate_buy_price(
		buy_amount: T::Balance,
		sell_reserve: T::Balance,
		buy_reserve: T::Balance,
	) -> sp_std::result::Result<T::Balance, DispatchError> {
		ensure!(
			!sell_reserve.is_zero() && !buy_reserve.is_zero(),
			Error::<T>::EmptyExchangePool
		);
		ensure!(buy_reserve > buy_amount, Error::<T>::InsufficientAssetReserve);

		let buy_amount_hp = HighPrecisionUnsigned::from(T::BalanceToUnsignedInt::from(buy_amount).into());
		let buy_reserve_hp = HighPrecisionUnsigned::from(T::BalanceToUnsignedInt::from(buy_reserve).into());
		let sell_reserve_hp = HighPrecisionUnsigned::from(T::BalanceToUnsignedInt::from(sell_reserve).into());
		let denominator_hp = buy_reserve_hp - buy_amount_hp;
		let price_hp = sell_reserve_hp
			.saturating_mul(buy_amount_hp)
			.checked_div(denominator_hp)
			.ok_or::<Error<T>>(Error::<T>::DivideByZero)?;

		let price_lp_result: Result<LowPrecisionUnsigned, &'static str> = LowPrecisionUnsigned::try_from(price_hp);
		ensure!(price_lp_result.is_ok(), Error::<T>::Overflow);

		let price_lp = price_lp_result.unwrap();
		let price_plus_one = price_lp
			.checked_add(One::one())
			.ok_or::<Error<T>>(Error::<T>::Overflow)?;
		let fee_rate_plus_one = Self::fee_rate()
			.checked_add(FeeRate::<PerMillion>::one())
			.ok_or::<Error<T>>(Error::<T>::Overflow)?;
		let output = fee_rate_plus_one
			.checked_mul(price_plus_one.into())
			.ok_or::<Error<T>>(Error::<T>::Overflow)?;
		Ok(T::UnsignedIntToBalance::from(output.into()).into())
	}

	/// Get the sell price of some asset for another
	/// In simple terms: 'If I sell _x_ amount of asset _a_ how much of asset _b_ will I get in return?'
	/// `asset_to_sell` is the asset to be sold
	/// `amount_to_sell` is the amount of `asset_to_sell` to be sold
	/// `asset_to_payout` is the asset to be paid out in exchange for the sale of `asset_to_sell` (the final sale value is given in this asset)
	pub fn get_sell_price(
		asset_to_sell: T::AssetId,
		amount_to_sell: T::Balance,
		asset_to_payout: T::AssetId,
	) -> Result<T::Balance, DispatchError> {
		ensure!(asset_to_sell != asset_to_payout, Error::<T>::AssetCannotSwapForItself);

		// Find the value of `amount_to_sell` of `asset_to_sell` in terms of core asset
		// (how much core asset is the sale worth?)
		let core_asset_amount = if asset_to_sell == Self::core_asset_id() {
			amount_to_sell
		} else {
			Self::get_asset_to_core_sell_price(&asset_to_sell, amount_to_sell)?
		};

		// Skip payout asset price if asset to be paid out is core
		// (how much `asset_to_payout` is the sale worth?)
		let payout_asset_value = if asset_to_payout == Self::core_asset_id() {
			core_asset_amount
		} else {
			Self::get_core_to_asset_sell_price(&asset_to_payout, core_asset_amount)?
		};

		Ok(payout_asset_value)
	}

	/// `asset_id` - Trade asset
	/// `amount_sold` - Amount of the trade asset to sell
	/// Returns amount of core that can be bought with input assets.
	pub fn get_asset_to_core_sell_price(
		asset_id: &T::AssetId,
		sell_amount: T::Balance,
	) -> sp_std::result::Result<T::Balance, DispatchError> {
		ensure!(
			sell_amount > Zero::zero(),
			Error::<T>::AssetToCoreSellAmountNotAboveZero
		);

		let (core_reserve, asset_reserve) = Self::get_exchange_reserves(asset_id);
		Self::calculate_sell_price(sell_amount, asset_reserve, core_reserve)
	}

	/// Returns the amount of trade asset to pay for `sell_amount` of core sold.
	///
	/// `asset_id` - Trade asset
	/// `sell_amount` - Amount of input core to sell
	pub fn get_core_to_asset_sell_price(
		asset_id: &T::AssetId,
		sell_amount: T::Balance,
	) -> sp_std::result::Result<T::Balance, DispatchError> {
		ensure!(
			sell_amount > Zero::zero(),
			Error::<T>::CoreToAssetSellAmountNotAboveZero
		);

		let (core_reserve, asset_reserve) = Self::get_exchange_reserves(asset_id);
		Self::calculate_sell_price(sell_amount, core_reserve, asset_reserve)
	}

	/// `sell_amount` - Amount to sell
	/// `sell_reserve`- How much of the asset to sell is in the exchange
	/// `buy_reserve` - How much of the asset to buy is in the exchange
	/// Returns the amount of buyable asset that would be received
	fn calculate_sell_price(
		sell_amount: T::Balance,
		sell_reserve: T::Balance,
		buy_reserve: T::Balance,
	) -> sp_std::result::Result<T::Balance, DispatchError> {
		ensure!(
			!sell_reserve.is_zero() && !buy_reserve.is_zero(),
			Error::<T>::EmptyExchangePool
		);

		let div_rate: FeeRate<PerMillion> = Self::fee_rate()
			.checked_add(FeeRate::<PerMillion>::one())
			.ok_or::<Error<T>>(Error::<T>::Overflow)?;

		let sell_amount_scaled = FeeRate::<PerMillion>::from(T::BalanceToUnsignedInt::from(sell_amount).into())
			.checked_div(div_rate)
			.ok_or::<Error<T>>(Error::<T>::DivideByZero)?;

		let sell_reserve_hp = HighPrecisionUnsigned::from(T::BalanceToUnsignedInt::from(sell_reserve).into());
		let buy_reserve_hp = HighPrecisionUnsigned::from(T::BalanceToUnsignedInt::from(buy_reserve).into());
		let sell_amount_scaled_hp = HighPrecisionUnsigned::from(LowPrecisionUnsigned::from(sell_amount_scaled));
		let denominator_hp = sell_amount_scaled_hp + sell_reserve_hp;
		let price_hp = buy_reserve_hp
			.saturating_mul(sell_amount_scaled_hp)
			.checked_div(denominator_hp)
			.ok_or::<Error<T>>(Error::<T>::DivideByZero)?;

		let price_lp_result: Result<LowPrecisionUnsigned, &'static str> = LowPrecisionUnsigned::try_from(price_hp);
		ensure!(price_lp_result.is_ok(), Error::<T>::Overflow);
		let price_lp = price_lp_result.unwrap();

		let price = T::UnsignedIntToBalance::from(price_lp).into();
		ensure!(buy_reserve > price, Error::<T>::InsufficientAssetReserve);
		Ok(price)
	}

	/// A helper for pricing functions
	/// Fetches the reserves from an exchange for a particular `asset_id`
	fn get_exchange_reserves(asset_id: &T::AssetId) -> (T::Balance, T::Balance) {
		let exchange_address = T::ExchangeAddressGenerator::exchange_address_for(*asset_id);

		let core_reserve = <pallet_generic_asset::Module<T>>::free_balance(&Self::core_asset_id(), &exchange_address);
		let asset_reserve = <pallet_generic_asset::Module<T>>::free_balance(asset_id, &exchange_address);
		(core_reserve, asset_reserve)
	}

	//
	// Trade functions
	//

	/// Buy `amount_to_buy` of `asset_to_buy` with `asset_to_sell`.
	///
	/// `trader` - Account selling `asset_to_sell`
	/// `recipient` - Account to receive `asset_to_buy`
	/// `asset_to_sell` - asset ID to sell
	/// `asset_to_buy` - asset ID to buy
	/// `amount_to_buy` - The amount of `asset_to_buy` to buy
	/// `maximum_sell` - Maximum acceptable amount of `asset_to_sell` the trader will sell
	pub fn execute_buy(
		trader: &T::AccountId,
		recipient: &T::AccountId,
		asset_to_sell: &T::AssetId,
		asset_to_buy: &T::AssetId,
		amount_to_buy: T::Balance,
		maximum_sell: T::Balance,
	) -> sp_std::result::Result<T::Balance, DispatchError> {
		// Check the sell amount meets the maximum requirement
		let amount_to_sell = Self::get_buy_price(*asset_to_buy, amount_to_buy, *asset_to_sell)?;
		ensure!(amount_to_sell <= maximum_sell, Error::<T>::PriceAboveMaxLimit);

		// Check the trader has enough balance
		ensure!(
			<pallet_generic_asset::Module<T>>::free_balance(&asset_to_sell, trader) >= amount_to_sell,
			Error::<T>::InsufficientBalance
		);

		Self::execute_trade(
			trader,
			recipient,
			asset_to_sell,
			asset_to_buy,
			amount_to_sell,
			amount_to_buy,
		)?;

		Ok(amount_to_sell)
	}

	/// Sell `asset_to_sell` for at least `minimum_buy` of `asset_to_buy`.
	///
	/// `trader` - Account selling `asset_to_sell`
	/// `recipient` - Account to receive `asset_to_buy`
	/// `asset_to_sell` - asset ID to sell
	/// `asset_to_buy` - asset ID to buy
	/// `amount_to_sell` - The amount of `asset_to_sell` to sell
	/// `minimum_buy` - The minimum acceptable amount of `asset_to_buy` to receive
	pub fn execute_sell(
		trader: &T::AccountId,
		recipient: &T::AccountId,
		asset_to_sell: &T::AssetId,
		asset_to_buy: &T::AssetId,
		amount_to_sell: T::Balance,
		minimum_buy: T::Balance,
	) -> sp_std::result::Result<T::Balance, DispatchError> {
		// Check the trader has enough balance
		ensure!(
			<pallet_generic_asset::Module<T>>::free_balance(&asset_to_sell, trader) >= amount_to_sell,
			Error::<T>::InsufficientBalance
		);

		// Check the buy amount meets the minimum requirement
		let amount_to_buy = Self::get_sell_price(*asset_to_sell, amount_to_sell, *asset_to_buy)?;
		ensure!(amount_to_buy >= minimum_buy, Error::<T>::SaleValueBelowRequiredMinimum);

		Self::execute_trade(
			trader,
			recipient,
			asset_to_sell,
			asset_to_buy,
			amount_to_sell,
			amount_to_buy,
		)?;

		Ok(amount_to_buy)
	}

	fn execute_trade(
		trader: &T::AccountId,
		recipient: &T::AccountId,
		asset_to_sell: &T::AssetId,
		asset_to_buy: &T::AssetId,
		amount_to_sell: T::Balance,
		amount_to_buy: T::Balance,
	) -> DispatchResult {
		let core_asset_id = Self::core_asset_id();

		// If either asset is core, we only need to make one exchange
		// otherwise, we make two exchanges
		if *asset_to_sell == core_asset_id || *asset_to_buy == core_asset_id {
			let exchange_address = if *asset_to_buy == core_asset_id {
				T::ExchangeAddressGenerator::exchange_address_for(*asset_to_sell)
			} else {
				T::ExchangeAddressGenerator::exchange_address_for(*asset_to_buy)
			};
			let _ = <pallet_generic_asset::Module<T>>::make_transfer(
				&asset_to_sell,
				trader,
				&exchange_address,
				amount_to_sell,
			)
			.and(<pallet_generic_asset::Module<T>>::make_transfer(
				&asset_to_buy,
				&exchange_address,
				recipient,
				amount_to_buy,
			));
		} else {
			let core_amount = Self::get_asset_to_core_sell_price(asset_to_sell, amount_to_sell)?;
			let exchange_address_a = T::ExchangeAddressGenerator::exchange_address_for(*asset_to_sell);
			let exchange_address_b = T::ExchangeAddressGenerator::exchange_address_for(*asset_to_buy);

			let _ = <pallet_generic_asset::Module<T>>::make_transfer(
				asset_to_sell,
				trader,
				&exchange_address_a,
				amount_to_sell,
			)
			.and(<pallet_generic_asset::Module<T>>::make_transfer(
				&core_asset_id,
				&exchange_address_a,
				&exchange_address_b,
				core_amount,
			))
			.and(<pallet_generic_asset::Module<T>>::make_transfer(
				asset_to_buy,
				&exchange_address_b,
				recipient,
				amount_to_buy,
			));
		};

		Self::deposit_event(RawEvent::AssetPurchase(
			*asset_to_sell,
			*asset_to_buy,
			trader.clone(),
			amount_to_sell,
			amount_to_buy,
		));

		Ok(())
	}
}
