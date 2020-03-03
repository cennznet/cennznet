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

mod exchange;
mod impls;
mod liquidity;
mod price;
mod traits;
mod types;

pub use impls::{ExchangeAddressFor, ExchangeAddressGenerator};
pub use types::{FeeRate, HighPrecisionUnsigned, LowPrecisionUnsigned, PerMilli, PerMillion};

use liquidity::ExchangeKey;

#[macro_use]
extern crate frame_support;

use core::convert::TryFrom;
use frame_support::{dispatch::Dispatchable, Parameter, StorageDoubleMap};
use frame_system::{ensure_root, ensure_signed};
use pallet_generic_asset;
use sp_runtime::traits::{Bounded, One, Zero};
use sp_runtime::{DispatchError, DispatchResult};
use sp_std::{prelude::*, result};

type Result<T> = result::Result<<T as pallet_generic_asset::Trait>::Balance, DispatchError>;

pub trait Trait: pallet_generic_asset::Trait {
	type Call: Parameter + Dispatchable<Origin = <Self as frame_system::Trait>::Origin>;
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
	/// A function type to get an exchange address given the asset ID pair.
	type ExchangeAddressGenerator: ExchangeAddressFor<Self::AssetId, Self::AccountId>;

	// TODO avoid unnecessary conversion
	type BalanceToUnsignedInt: From<<Self as pallet_generic_asset::Trait>::Balance> + Into<LowPrecisionUnsigned>;
	type UnsignedIntToBalance: From<LowPrecisionUnsigned> + Into<<Self as pallet_generic_asset::Trait>::Balance>;
}

decl_error! {
	pub enum Error for Module<T: Trait> {
		/// Exchange pool is empty.
		EmptyExchangePool,
		// Insufficient trade asset reserve in exchange
		InsufficientTradeAssetReserve,
		// Insufficient core asset reserve in exchange
		InsufficientCoreAssetReserve,
		// Insufficient asset balance in buyer account
		InsufficientBuyerTradeAssetBalance,
		// Insufficient core asset balance in buyer account
		InsufficientBuyerCoreAssetBalance,
		// Insufficient asset balance in seller account
		InsufficientSellerTradeAssetBalance,
		// Insufficient core asset balance in seller account
		InsufficientSellerCoreAssetBalance,
		// Buy amount must be a positive value
		BuyAmountNotPositive,
		// The sale value of input is less than the required minimum.
		SaleValueBelowRequiredMinimum,
		// Asset sale value should be greater than zero
		AssetSaleValueNotAboveZero,
		// Asset to core sale price should be greater than zero
		AssetToCorePriceNotAboveZero,
		// Insufficient core asset balance in exchange account
		InsufficientCoreAssetInExchangeBalance,
		// Asset to core sale price exceeds the specified max. limit
		AssetToCorePriceAboveMaxLimit,
		// Core to asset sale price should be greater than zero
		CoreToAssetPriceNotAboveZero,
		// Core to asset sale price exceeds the specified max. limit
		CoreToAssetPriceAboveMaxLimit,
		// Asset to asset sale price exceeds the specified max. limit
		AssetToAssetPriceAboveMaxLimit,
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
		// The sale value of input is less than the required min
		InsufficientSellAssetForRequiredMinimumBuyAsset,
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
		/// Total supply of exchange token in existence.
		/// it will always be less than the core asset's total supply
		/// Key: `(asset id, core asset id)`
		pub TotalSupply get(total_supply): map ExchangeKey<T> => T::Balance;

		/// Asset balance of each user in each exchange pool.
		/// Key: `(core_asset_id, trade_asset_id), account_id`
		pub LiquidityBalance get(liquidity_balance): double_map  ExchangeKey<T>, hasher(twox_128) T::AccountId => T::Balance;
	}
}
