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
//! CENNZX spot exchange
//!
#![cfg_attr(not(feature = "std"), no_std)]

use core::convert::TryFrom;
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage, ensure,
	traits::{ExistenceRequirement, Imbalance},
	transactional,
	weights::Weight,
	Parameter, StorageDoubleMap,
};
use frame_system::{ensure_root, ensure_signed};
use prml_support::MultiCurrencyAccounting;
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, MaybeSerializeDeserialize, Member, One, Saturating, Zero},
	DispatchError, DispatchResult, SaturatedConversion,
};
use sp_std::prelude::*;

// import `mock` first so its macros are defined in `impl` and `tests`.
#[macro_use]
mod mock;
mod impls;
mod tests;
mod types;

pub use impls::{ExchangeAddressFor, ExchangeAddressGenerator};
pub use types::{FeeRate, HighPrecisionUnsigned, LowPrecisionUnsigned, PerMillion, PerThousand};

// (core_asset_id, asset_id)
pub type ExchangeKey<T> = (<T as Trait>::AssetId, <T as Trait>::AssetId);

/// Represents the value of an amount of liquidity in an exchange
/// Liquidity is always traded for a combination of `core_asset` and `trade_asset`
///
/// `liquidity` represents the volume of liquidity holdings being valued
/// `core` represents the balance of `core_asset` that the liquidity would yield
/// `asset` represents the balance of `trade_asset` that the liquidity
pub struct LiquidityValue<Balance> {
	pub liquidity: Balance,
	pub core: Balance,
	pub asset: Balance,
}

/// Represents the price to buy liquidity from an exchange
/// Liquidity is always traded for a combination of `core_asset` and `trade_asset`
///
/// `core` represents the balance of `core_asset` required
/// `asset` represents the balance of `trade_asset` required
pub struct LiquidityPrice<Balance> {
	pub core: Balance,
	pub asset: Balance,
}

/// Alias for the multi-currency provided balance type
type BalanceOf<T> = <<T as Trait>::MultiCurrency as MultiCurrencyAccounting>::Balance;

pub trait Trait: frame_system::Trait {
	/// The system event type
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
	/// Type for identifying assets
	type AssetId: Parameter + Member + AtLeast32BitUnsigned + Default + Copy + Into<u64>;
	/// Something which can provide multi currency asset management
	type MultiCurrency: MultiCurrencyAccounting<AccountId = Self::AccountId, CurrencyId = Self::AssetId>;
	/// Something which can generate addresses for exchange pools
	type ExchangeAddressFor: ExchangeAddressFor<AccountId = Self::AccountId, AssetId = Self::AssetId>;
	/// Provides the public call to weight mapping
	type WeightInfo: WeightInfo;
}

pub trait WeightInfo {
	fn buy_asset() -> Weight;
	fn sell_asset() -> Weight;
	fn add_liquidity() -> Weight;
	fn remove_liquidity() -> Weight;
	fn set_fee_rate() -> Weight;
}

impl WeightInfo for () {
	fn buy_asset() -> Weight {
		1_000_000
	}
	fn sell_asset() -> Weight {
		1_000_000
	}
	fn add_liquidity() -> Weight {
		1_000_000
	}
	fn remove_liquidity() -> Weight {
		1_000_000
	}
	fn set_fee_rate() -> Weight {
		1_000_000
	}
}

decl_error! {
	pub enum Error for Module<T: Trait> {
		EmptyExchangePool,
		InsufficientExchangePoolReserve,
		InsufficientBalance,
		InsufficientLiquidity,
		InsufficientTradeAssetBalance,
		InsufficientCoreAssetBalance,
		CannotTradeZero,
		CannotAddLiquidityWithZero,
		MinimumBuyRequirementNotMet,
		MaximumSellRequirementNotMet,
		MinimumTradeAssetRequirementNotMet,
		MinimumCoreAssetRequirementNotMet,
		MinimumLiquidityRequirementNotMet,
		MaximumTradeAssetRequirementNotMet,
		AssetCannotSwapForItself,
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
		/// Caller specifies an exact `buy_amount` and a `maximum_sell` amount to pay.
		///
		/// `recipient` - Account to receive assets, defaults to `origin` if None
		/// `asset_to_sell` - asset ID to sell
		/// `asset_to_buy` - asset ID to buy
		/// `buy_amount` - The amount of `asset_to_buy` to receive
		/// `maximum_sell` - Maximum `asset_to_sell` caller should pay
		#[weight = T::WeightInfo::buy_asset()]
		pub fn buy_asset(
			origin,
			recipient: Option<T::AccountId>,
			#[compact] asset_to_sell: T::AssetId,
			#[compact] asset_to_buy: T::AssetId,
			#[compact] buy_amount: BalanceOf<T>,
			#[compact] maximum_sell: BalanceOf<T>
		) -> DispatchResult {
			let trader = ensure_signed(origin)?;
			let _ = Self::execute_buy(
				&trader,
				&recipient.unwrap_or_else(|| trader.clone()),
				asset_to_sell,
				asset_to_buy,
				buy_amount,
				maximum_sell,
			)?;
			Ok(())
		}

		/// Sell `asset_to_sell` for `asset_to_buy`.
		/// Caller specifies an exact `sell_amount` and a `minimum_buy` amount to receive.
		///
		/// `recipient` - Account to receive assets, defaults to `origin` if None
		/// `asset_to_sell` - asset ID to sell
		/// `asset_to_buy` - asset ID to buy
		/// `sell_amount` - The amount of `asset_to_sell` the caller should pay
		/// `minimum_buy` - The minimum `asset_to_buy` to receive
		#[weight = T::WeightInfo::sell_asset()]
		pub fn sell_asset(
			origin,
			recipient: Option<T::AccountId>,
			#[compact] asset_to_sell: T::AssetId,
			#[compact] asset_to_buy: T::AssetId,
			#[compact] sell_amount: BalanceOf<T>,
			#[compact] minimum_buy: BalanceOf<T>
		) -> DispatchResult {
			let trader = ensure_signed(origin)?;
			let _ = Self::execute_sell(
				&trader,
				&recipient.unwrap_or_else(|| trader.clone()),
				asset_to_sell,
				asset_to_buy,
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
		#[weight = T::WeightInfo::add_liquidity()]
		pub fn add_liquidity(
			origin,
			#[compact] asset_id: T::AssetId,
			#[compact] min_liquidity: BalanceOf<T>,
			#[compact] max_asset_amount: BalanceOf<T>,
			#[compact] core_amount: BalanceOf<T>
		) {
			let from_account = ensure_signed(origin)?;
			let core_asset_id = Self::core_asset_id();
			ensure!(
				!max_asset_amount.is_zero() && !core_amount.is_zero(),
				Error::<T>::CannotAddLiquidityWithZero
			);
			ensure!(
				T::MultiCurrency::free_balance(&from_account, Some(core_asset_id)) >= core_amount,
				Error::<T>::InsufficientCoreAssetBalance
			);
			ensure!(
				T::MultiCurrency::free_balance(&from_account, Some(asset_id)) >= max_asset_amount,
				Error::<T>::InsufficientTradeAssetBalance
			);
			let exchange_key = (core_asset_id, asset_id);
			let total_liquidity = <TotalLiquidity<T>>::get(&exchange_key);
			let exchange_address = T::ExchangeAddressFor::exchange_address_for(asset_id);
			let core_asset_reserve = T::MultiCurrency::free_balance(&exchange_address, Some(core_asset_id));

			let (trade_asset_amount, liquidity_minted) = if total_liquidity.is_zero() || core_asset_reserve.is_zero() {
				// new exchange pool
				(max_asset_amount, core_amount)
			} else {
				let trade_asset_reserve = T::MultiCurrency::free_balance(&exchange_address, Some(asset_id));
				let trade_asset_amount = core_amount * trade_asset_reserve / core_asset_reserve + One::one();
				let liquidity_minted = core_amount * total_liquidity / core_asset_reserve;

				(trade_asset_amount, liquidity_minted)
			};
			ensure!(
				liquidity_minted >= min_liquidity,
				Error::<T>::MinimumLiquidityRequirementNotMet
			);
			ensure!(
				max_asset_amount >= trade_asset_amount,
				Error::<T>::MaximumTradeAssetRequirementNotMet
			);

			T::MultiCurrency::transfer(&from_account, &exchange_address, Some(core_asset_id), core_amount, ExistenceRequirement::KeepAlive)?;
			T::MultiCurrency::transfer(&from_account, &exchange_address, Some(asset_id), trade_asset_amount, ExistenceRequirement::KeepAlive)?;

			Self::mint_liquidity(&exchange_key, &from_account, liquidity_minted);
			Self::deposit_event(Event::<T>::AddLiquidity(from_account, core_amount, asset_id, trade_asset_amount));
		}

		/// Burn exchange assets to withdraw core asset and trade asset at current ratio
		///
		/// `asset_id` - The trade asset ID
		/// `liquidity_to_withdraw` - Amount of user's liquidity to withdraw
		/// `min_asset_withdraw` - The minimum trade asset withdrawn
		/// `min_core_withdraw` -  The minimum core asset withdrawn
		#[weight = T::WeightInfo::remove_liquidity()]
		pub fn remove_liquidity(
			origin,
			#[compact] asset_id: T::AssetId,
			#[compact] liquidity_to_withdraw: BalanceOf<T>,
			#[compact] min_asset_withdraw: BalanceOf<T>,
			#[compact] min_core_withdraw: BalanceOf<T>
		) -> DispatchResult {
			let from_account = ensure_signed(origin)?;

			let core_asset_id = Self::core_asset_id();
			let exchange_key = (core_asset_id, asset_id);
			let account_liquidity = <LiquidityBalance<T>>::get(&exchange_key, &from_account);
			ensure!(
				account_liquidity >= liquidity_to_withdraw,
				Error::<T>::InsufficientLiquidity
			);

			let withdraw_value = Self::liquidity_value(asset_id, liquidity_to_withdraw);
			let total_liquidity = <TotalLiquidity<T>>::get(&exchange_key);

			ensure!(
				total_liquidity > Zero::zero(),
				Error::<T>::EmptyExchangePool
			);
			ensure!(
				withdraw_value.core >= min_core_withdraw,
				Error::<T>::MinimumCoreAssetRequirementNotMet
			);
			ensure!(
				withdraw_value.asset >= min_asset_withdraw,
				Error::<T>::MinimumTradeAssetRequirementNotMet
			);
			let exchange_address = T::ExchangeAddressFor::exchange_address_for(asset_id);
			T::MultiCurrency::transfer(&exchange_address, &from_account, Some(core_asset_id), withdraw_value.core, ExistenceRequirement::KeepAlive)?;
			T::MultiCurrency::transfer(&exchange_address, &from_account, Some(asset_id), withdraw_value.asset, ExistenceRequirement::KeepAlive)?;
			Self::burn_liquidity(&exchange_key, &from_account, liquidity_to_withdraw);
			Self::deposit_event(Event::<T>::RemoveLiquidity(from_account, withdraw_value.core, asset_id, withdraw_value.asset));
			Ok(())
		}

		/// Set the spot exchange wide fee rate (root only)
		#[weight = T::WeightInfo::set_fee_rate()]
		pub fn set_fee_rate(origin, new_fee_rate: FeeRate<PerMillion>) -> DispatchResult {
			ensure_root(origin)?;
			DefaultFeeRate::mutate(|fee_rate| *fee_rate = new_fee_rate);
			Ok(())
		}
	}
}

decl_event! {
	pub enum Event<T> where
		AccountId = <T as frame_system::Trait>::AccountId,
		AssetId = <T as Trait>::AssetId,
		Balance = BalanceOf<T>,
	{
		/// Provider, core asset amount, trade asset id, trade asset amount
		AddLiquidity(AccountId, Balance, AssetId, Balance),
		/// Provider, core asset amount, trade asset id, trade asset amount
		RemoveLiquidity(AccountId, Balance, AssetId, Balance),
		/// AssetSold, AssetBought, Buyer, SoldAmount, BoughtAmount
		AssetBought(AssetId, AssetId, AccountId, Balance, Balance),
		/// AssetSold, AssetBought, Buyer, SoldAmount, BoughtAmount
		AssetSold(AssetId, AssetId, AccountId, Balance, Balance),
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as Cennzx {
		/// Asset Id of the core liquidity asset
		pub CoreAssetId get(fn core_asset_id) config(): T::AssetId;
		/// Default trading fee rate
		pub DefaultFeeRate get(fn fee_rate) config(): FeeRate<PerMillion>;
		/// Total liquidity holdings of all investors in an exchange.
		/// ie/ total_liquidity(exchange) == sum(liquidity_balance(exchange, user)) at all times
		pub TotalLiquidity get(fn total_liquidity): map hasher(twox_64_concat) ExchangeKey<T> => BalanceOf<T>;
		/// Liquidity holdings of a user in an exchange pool.
		/// Key: `(core_asset_id, trade_asset_id), account_id`
		pub LiquidityBalance get(fn liquidity_balance): double_map hasher(twox_64_concat) ExchangeKey<T>, hasher(blake2_128_concat) T::AccountId => BalanceOf<T>;
	}
}

// The main implementation block for the module.
impl<T: Trait> Module<T> {
	//
	// Liquidity
	//

	/// Mint liquidity holdings for a user in a specified exchange
	fn mint_liquidity(exchange_key: &ExchangeKey<T>, who: &T::AccountId, increase: BalanceOf<T>) {
		let balance = <LiquidityBalance<T>>::get(exchange_key, who);
		let new_balance = balance.saturating_add(increase);
		<LiquidityBalance<T>>::insert(exchange_key, who, new_balance);
		<TotalLiquidity<T>>::mutate(exchange_key, |balance| *balance = balance.saturating_add(increase));
	}

	/// Burn liquidity holdings from a user in a specified exchange
	fn burn_liquidity(exchange_key: &ExchangeKey<T>, who: &T::AccountId, decrease: BalanceOf<T>) {
		let balance = <LiquidityBalance<T>>::get(exchange_key, who);
		let decrease = decrease.min(balance);
		let new_balance = balance - decrease;
		<LiquidityBalance<T>>::insert(exchange_key, who, new_balance);
		<TotalLiquidity<T>>::mutate(exchange_key, |balance| *balance = balance.saturating_sub(decrease));
	}

	/// The Price of Liquidity for a particular `asset_id` exchange
	///
	/// The price includes
	///   * a required amount of core asset
	///   * a required amount of `asset_id`
	///
	/// Note: if the exchange does not exist, the cost in `asset` is 1, because the investor
	///       determines the exchange rate
	pub fn liquidity_price(asset_id: T::AssetId, liquidity_to_buy: BalanceOf<T>) -> LiquidityPrice<BalanceOf<T>> {
		let core_asset_id = Self::core_asset_id();
		let exchange_key = (core_asset_id, asset_id);
		let total_liquidity = <TotalLiquidity<T>>::get(&exchange_key);
		let exchange_address = T::ExchangeAddressFor::exchange_address_for(asset_id);
		let core_reserve = T::MultiCurrency::free_balance(&exchange_address, Some(core_asset_id));

		let (core_amount, asset_amount) = if total_liquidity.is_zero() || core_reserve.is_zero() {
			// empty exchange pool
			(liquidity_to_buy, One::one())
		} else {
			let core_amount = liquidity_to_buy * core_reserve / total_liquidity;
			let asset_reserve = T::MultiCurrency::free_balance(&exchange_address, Some(asset_id));
			let asset_amount = core_amount * asset_reserve / core_reserve + One::one();

			(core_amount, asset_amount)
		};
		LiquidityPrice {
			core: core_amount,
			asset: asset_amount,
		}
	}

	/// Account Liquidity Value
	///
	/// Returns a struct containing:
	///   * the total liquidity in an account for a given asset ID
	///   * the total withdrawable core asset
	///   * the total withdrawable trade asset
	pub fn account_liquidity_value(who: &T::AccountId, asset_id: T::AssetId) -> LiquidityValue<BalanceOf<T>> {
		let core_asset_id = Self::core_asset_id();
		let exchange_key = (core_asset_id, asset_id);
		let account_liquidity = <LiquidityBalance<T>>::get(&exchange_key, who);
		Self::liquidity_value(asset_id, account_liquidity)
	}

	/// The Value of a specific amount of liquidity
	///
	/// Takes an `asset_id` and an amount of `liquidity_to_withdraw` and returns its value
	/// from the exchange.
	///
	/// Returns a struct containing:
	///   * the withdrawable liquidity for the given `asset_id`
	///   * the core asset exchangable for the `liquidity_to_withdraw`
	///   * the trade asset exchangable for the `liquidity_to_withdraw`
	fn liquidity_value(asset_id: T::AssetId, liquidity_to_withdraw: BalanceOf<T>) -> LiquidityValue<BalanceOf<T>> {
		let core_asset_id = Self::core_asset_id();
		let exchange_key = (core_asset_id, asset_id);
		let total_liquidity = <TotalLiquidity<T>>::get(&exchange_key);
		let exchange_address = T::ExchangeAddressFor::exchange_address_for(asset_id);
		let asset_reserve = T::MultiCurrency::free_balance(&exchange_address, Some(asset_id));
		let core_reserve = T::MultiCurrency::free_balance(&exchange_address, Some(core_asset_id));
		Self::calculate_liquidity_value(asset_reserve, core_reserve, liquidity_to_withdraw, total_liquidity)
	}

	/// Calculate the Value of Liquidity
	///
	/// Simple helper function to calculate the value of liquidity
	fn calculate_liquidity_value(
		asset_reserve: BalanceOf<T>,
		core_reserve: BalanceOf<T>,
		liquidity_to_withdraw: BalanceOf<T>,
		total_liquidity: BalanceOf<T>,
	) -> LiquidityValue<BalanceOf<T>> {
		if total_liquidity.is_zero() {
			LiquidityValue {
				liquidity: Zero::zero(),
				core: Zero::zero(),
				asset: Zero::zero(),
			}
		} else {
			let liquidity_amount = liquidity_to_withdraw.min(total_liquidity);
			let core_amount = liquidity_amount * core_reserve / total_liquidity;
			let asset_amount = liquidity_amount * asset_reserve / total_liquidity;
			LiquidityValue {
				liquidity: liquidity_amount,
				core: core_amount,
				asset: asset_amount,
			}
		}
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
		amount_to_buy: BalanceOf<T>,
		asset_to_pay: T::AssetId,
	) -> Result<BalanceOf<T>, DispatchError> {
		ensure!(asset_to_buy != asset_to_pay, Error::<T>::AssetCannotSwapForItself);

		// Find the cost of `amount_to_buy` of `asset_to_buy` in terms of core asset
		// (how much core asset does it cost?).
		let core_asset_amount = if asset_to_buy == Self::core_asset_id() {
			amount_to_buy
		} else {
			Self::get_core_to_asset_buy_price(asset_to_buy, amount_to_buy)?
		};

		// Find the price of `core_asset_amount` in terms of `asset_to_pay`
		// (how much `asset_to_pay` does `core_asset_amount` cost?)
		let pay_asset_amount = if asset_to_pay == Self::core_asset_id() {
			core_asset_amount
		} else {
			Self::get_asset_to_core_buy_price(asset_to_pay, core_asset_amount)?
		};

		Ok(pay_asset_amount)
	}

	/// `asset_id` - Trade asset
	/// `buy_amount` - Amount of output core
	/// Returns the amount of trade assets needed to buy `buy_amount` core assets.
	pub fn get_asset_to_core_buy_price(
		asset_id: T::AssetId,
		buy_amount: BalanceOf<T>,
	) -> sp_std::result::Result<BalanceOf<T>, DispatchError> {
		ensure!(buy_amount > Zero::zero(), Error::<T>::CannotTradeZero);

		let (core_reserve, asset_reserve) = Self::get_exchange_reserves(asset_id);
		Self::calculate_buy_price(buy_amount, asset_reserve, core_reserve)
	}

	/// `asset_id` - Trade asset
	/// `buy_amount`- Amount of the trade asset to buy
	/// Returns the amount of core asset needed to purchase `buy_amount` of trade asset.
	pub fn get_core_to_asset_buy_price(
		asset_id: T::AssetId,
		buy_amount: BalanceOf<T>,
	) -> sp_std::result::Result<BalanceOf<T>, DispatchError> {
		ensure!(buy_amount > Zero::zero(), Error::<T>::CannotTradeZero);

		let (core_reserve, asset_reserve) = Self::get_exchange_reserves(asset_id);
		Self::calculate_buy_price(buy_amount, core_reserve, asset_reserve)
	}

	/// `buy_amount` - Amount to buy
	/// `sell_reserve`- How much of the asset to sell is in the exchange
	/// `buy_reserve` - How much of the asset to buy is in the exchange
	/// Returns the amount of sellable asset is required
	fn calculate_buy_price(
		buy_amount: BalanceOf<T>,
		sell_reserve: BalanceOf<T>,
		buy_reserve: BalanceOf<T>,
	) -> sp_std::result::Result<BalanceOf<T>, DispatchError> {
		ensure!(
			!sell_reserve.is_zero() && !buy_reserve.is_zero(),
			Error::<T>::EmptyExchangePool
		);
		ensure!(buy_reserve > buy_amount, Error::<T>::InsufficientExchangePoolReserve);

		let buy_amount_hp = HighPrecisionUnsigned::from(buy_amount.saturated_into());
		let buy_reserve_hp = HighPrecisionUnsigned::from(buy_reserve.saturated_into());
		let sell_reserve_hp = HighPrecisionUnsigned::from(sell_reserve.saturated_into());
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
		Ok(BalanceOf::<T>::saturated_from(output.into()))
	}

	/// Get the sell price of some asset for another
	/// In simple terms: 'If I sell _x_ amount of asset _a_ how much of asset _b_ will I get in return?'
	/// `asset_to_sell` is the asset to be sold
	/// `amount_to_sell` is the amount of `asset_to_sell` to be sold
	/// `asset_to_payout` is the asset to be paid out in exchange for the sale of `asset_to_sell` (the final sale value is given in this asset)
	pub fn get_sell_price(
		asset_to_sell: T::AssetId,
		amount_to_sell: BalanceOf<T>,
		asset_to_payout: T::AssetId,
	) -> Result<BalanceOf<T>, DispatchError> {
		ensure!(asset_to_sell != asset_to_payout, Error::<T>::AssetCannotSwapForItself);

		// Find the value of `amount_to_sell` of `asset_to_sell` in terms of core asset
		// (how much core asset is the sale worth?)
		let core_asset_amount = if asset_to_sell == Self::core_asset_id() {
			amount_to_sell
		} else {
			Self::get_asset_to_core_sell_price(asset_to_sell, amount_to_sell)?
		};

		// Skip payout asset price if asset to be paid out is core
		// (how much `asset_to_payout` is the sale worth?)
		let payout_asset_value = if asset_to_payout == Self::core_asset_id() {
			core_asset_amount
		} else {
			Self::get_core_to_asset_sell_price(asset_to_payout, core_asset_amount)?
		};

		Ok(payout_asset_value)
	}

	/// `asset_id` - Trade asset
	/// `amount_sold` - Amount of the trade asset to sell
	/// Returns amount of core that can be bought with input assets.
	pub fn get_asset_to_core_sell_price(
		asset_id: T::AssetId,
		sell_amount: BalanceOf<T>,
	) -> sp_std::result::Result<BalanceOf<T>, DispatchError> {
		ensure!(sell_amount > Zero::zero(), Error::<T>::CannotTradeZero);

		let (core_reserve, asset_reserve) = Self::get_exchange_reserves(asset_id);
		Self::calculate_sell_price(sell_amount, asset_reserve, core_reserve)
	}

	/// Returns the amount of trade asset to pay for `sell_amount` of core sold.
	///
	/// `asset_id` - Trade asset
	/// `sell_amount` - Amount of input core to sell
	pub fn get_core_to_asset_sell_price(
		asset_id: T::AssetId,
		sell_amount: BalanceOf<T>,
	) -> sp_std::result::Result<BalanceOf<T>, DispatchError> {
		ensure!(sell_amount > Zero::zero(), Error::<T>::CannotTradeZero);

		let (core_reserve, asset_reserve) = Self::get_exchange_reserves(asset_id);
		Self::calculate_sell_price(sell_amount, core_reserve, asset_reserve)
	}

	/// `sell_amount` - Amount to sell
	/// `sell_reserve`- How much of the asset to sell is in the exchange
	/// `buy_reserve` - How much of the asset to buy is in the exchange
	/// Returns the amount of buyable asset that would be received
	fn calculate_sell_price(
		sell_amount: BalanceOf<T>,
		sell_reserve: BalanceOf<T>,
		buy_reserve: BalanceOf<T>,
	) -> sp_std::result::Result<BalanceOf<T>, DispatchError> {
		ensure!(
			!sell_reserve.is_zero() && !buy_reserve.is_zero(),
			Error::<T>::EmptyExchangePool
		);

		let div_rate: FeeRate<PerMillion> = Self::fee_rate()
			.checked_add(FeeRate::<PerMillion>::one())
			.ok_or::<Error<T>>(Error::<T>::Overflow)?;
		let sell_amount_scaled = FeeRate::<PerMillion>::from(sell_amount.saturated_into())
			.checked_div(div_rate)
			.ok_or::<Error<T>>(Error::<T>::DivideByZero)?;
		let sell_reserve_hp = HighPrecisionUnsigned::from(sell_reserve.saturated_into());
		let buy_reserve_hp = HighPrecisionUnsigned::from(buy_reserve.saturated_into());
		let sell_amount_scaled_hp = HighPrecisionUnsigned::from(sell_amount_scaled);
		let denominator_hp = sell_amount_scaled_hp + sell_reserve_hp;
		let price_hp = buy_reserve_hp
			.saturating_mul(sell_amount_scaled_hp)
			.checked_div(denominator_hp)
			.ok_or::<Error<T>>(Error::<T>::DivideByZero)?;

		let price_lp_result: Result<LowPrecisionUnsigned, &'static str> = LowPrecisionUnsigned::try_from(price_hp);
		ensure!(price_lp_result.is_ok(), Error::<T>::Overflow);
		let price_lp = price_lp_result.unwrap();

		let price: BalanceOf<T> = price_lp.saturated_into();
		ensure!(buy_reserve > price, Error::<T>::InsufficientExchangePoolReserve);
		Ok(price)
	}

	/// A helper for pricing functions
	/// Fetches the reserves from an exchange for a particular `asset_id`
	fn get_exchange_reserves(asset_id: T::AssetId) -> (BalanceOf<T>, BalanceOf<T>) {
		let exchange_address = T::ExchangeAddressFor::exchange_address_for(asset_id);

		let core_reserve = T::MultiCurrency::free_balance(&exchange_address, Some(Self::core_asset_id()));
		let asset_reserve = T::MultiCurrency::free_balance(&exchange_address, Some(asset_id));
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
		asset_to_sell: T::AssetId,
		asset_to_buy: T::AssetId,
		amount_to_buy: BalanceOf<T>,
		maximum_sell: BalanceOf<T>,
	) -> sp_std::result::Result<BalanceOf<T>, DispatchError> {
		// Check the sell amount meets the maximum requirement
		let amount_to_sell = Self::get_buy_price(asset_to_buy, amount_to_buy, asset_to_sell)?;
		ensure!(amount_to_sell <= maximum_sell, Error::<T>::MaximumSellRequirementNotMet);

		// Check the trader has enough balance
		ensure!(
			T::MultiCurrency::free_balance(trader, Some(asset_to_sell),) >= amount_to_sell,
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

		Self::deposit_event(Event::<T>::AssetBought(
			asset_to_sell,
			asset_to_buy,
			trader.clone(),
			amount_to_sell,
			amount_to_buy,
		));

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
		asset_to_sell: T::AssetId,
		asset_to_buy: T::AssetId,
		amount_to_sell: BalanceOf<T>,
		minimum_buy: BalanceOf<T>,
	) -> sp_std::result::Result<BalanceOf<T>, DispatchError> {
		// Check the trader has enough balance
		ensure!(
			T::MultiCurrency::free_balance(trader, Some(asset_to_sell),) >= amount_to_sell,
			Error::<T>::InsufficientBalance
		);

		// Check the buy amount meets the minimum requirement
		let amount_to_buy = Self::get_sell_price(asset_to_sell, amount_to_sell, asset_to_buy)?;
		ensure!(amount_to_buy >= minimum_buy, Error::<T>::MinimumBuyRequirementNotMet);

		Self::execute_trade(
			trader,
			recipient,
			asset_to_sell,
			asset_to_buy,
			amount_to_sell,
			amount_to_buy,
		)?;

		Self::deposit_event(Event::<T>::AssetSold(
			asset_to_sell,
			asset_to_buy,
			trader.clone(),
			amount_to_sell,
			amount_to_buy,
		));

		Ok(amount_to_buy)
	}

	/// Perform the transfer of funds between `trader`/`recipient` and the target exchange pools.
	/// Note: this operation is atomic, if one intermediate transfer fails, then the entire trade will be rolled back and return error.
	#[transactional]
	fn execute_trade(
		trader: &T::AccountId,
		recipient: &T::AccountId,
		asset_to_sell: T::AssetId,
		asset_to_buy: T::AssetId,
		amount_to_sell: BalanceOf<T>,
		amount_to_buy: BalanceOf<T>,
	) -> DispatchResult {
		let core_asset_id = Self::core_asset_id();

		// If either asset is core, we only need to make one exchange
		// otherwise, we make two exchanges
		if asset_to_sell == core_asset_id || asset_to_buy == core_asset_id {
			let exchange_address = if asset_to_buy == core_asset_id {
				T::ExchangeAddressFor::exchange_address_for(asset_to_sell)
			} else {
				T::ExchangeAddressFor::exchange_address_for(asset_to_buy)
			};

			T::MultiCurrency::transfer(
				trader,
				&exchange_address,
				Some(asset_to_sell),
				amount_to_sell,
				ExistenceRequirement::KeepAlive,
			)
			.and(T::MultiCurrency::transfer(
				&exchange_address,
				recipient,
				Some(asset_to_buy),
				amount_to_buy,
				ExistenceRequirement::KeepAlive,
			))
		} else {
			let exchange_address_a = T::ExchangeAddressFor::exchange_address_for(asset_to_sell);
			let exchange_address_b = T::ExchangeAddressFor::exchange_address_for(asset_to_buy);

			Self::get_asset_to_core_sell_price(asset_to_sell, amount_to_sell).and_then(|core_amount| {
				T::MultiCurrency::transfer(
					trader,
					&exchange_address_a,
					Some(asset_to_sell),
					amount_to_sell,
					ExistenceRequirement::KeepAlive,
				)
				.and(T::MultiCurrency::transfer(
					&exchange_address_a,
					&exchange_address_b,
					Some(core_asset_id),
					core_amount,
					ExistenceRequirement::KeepAlive,
				))
				.and(T::MultiCurrency::transfer(
					&exchange_address_b,
					recipient,
					Some(asset_to_buy),
					amount_to_buy,
					ExistenceRequirement::KeepAlive,
				))
			})
		}
	}
}
