// Copyright 2018-2020 Parity Technologies (UK) Ltd. and Centrality Investments Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

//! # Transaction Payment Module
//!
//! Transaction Payment Customized Error Code Constants

pub mod error_code {
	pub const INSUFFICIENT_FEE_ASSET_BALANCE: u8 = 190;
	pub const UNKNOWN_BUY_FEE_ASSET: u8 = 191;
	// Cennzx-spot exchange module errors
	pub const EMPTY_EXCHANGE_POOL: u8 = 192;
	pub const INSUFFICIENT_TRADE_ASSET_RESERVE: u8 = 193;
	pub const INSUFFICIENT_CORE_ASSET_RESERVE: u8 = 194;
	pub const INSUFFICIENT_BUYER_TRADE_ASSET_BALANCE: u8 = 195;
	pub const INSUFFICIENT_BUYER_CORE_ASSET_BALANCE: u8 = 196;
	pub const INSUFFICIENT_SELLER_TRADE_ASSET_BALANCE: u8 = 197;
	pub const INSUFFICIENT_SELLER_CORE_ASSET_BALANCE: u8 = 198;
	pub const BUY_AMOUNT_NOT_POSITIVE: u8 = 199;
	pub const SALE_VALUE_BELOW_REQUIRED_MINIMUM: u8 = 200;
	pub const ASSET_SALE_VALUE_NOT_ABOVE_ZERO: u8 = 201;
	pub const ASSET_TO_CORE_PRICE_NOT_ABOVE_ZERO: u8 = 202;
	pub const INSUFFICIENT_CORE_ASSET_IN_EXCHANGE_BALANCE: u8 = 203;
	pub const ASSET_TO_CORE_PRICE_ABOVE_MAX_LIMIT: u8 = 204;
	pub const CORE_TO_ASSET_PRICE_NOT_ABOVE_ZERO: u8 = 205;
	pub const CORE_TO_ASSET_PRICE_ABOVE_MAX_LIMIT: u8 = 206;
	pub const ASSET_TO_ASSET_PRICE_ABOVE_MAX_LIMIT: u8 = 207;
	pub const LIQUIDITY_TOO_LOW: u8 = 208;
	pub const MINIMUM_TRADE_ASSET_IS_REQUIRED: u8 = 209;
	pub const MINIMUM_CORE_ASSET_IS_REQUIRED: u8 = 210;
	pub const ASSET_TO_WITHDRAW_NOT_ABOVE_ZERO: u8 = 211;
	pub const LIQUIDITY_TO_WITHDRAW_NOT_ABOVE_ZERO: u8 = 212;
	pub const NO_LIQUIDITY_TO_REMOVE: u8 = 213;
	pub const TRADE_ASSET_TO_ADD_LIQUIDITY_NOT_ABOVE_ZERO: u8 = 214;
	pub const CORE_ASSET_TO_ADD_LIQUIDITY_NOT_ABOVE_ZERO: u8 = 215;
	pub const CORE_ASSET_BALANCE_TO_ADD_LIQUIDITY_TOO_LOW: u8 = 216;
	pub const TRADE_ASSET_BALANCE_TO_ADD_LIQUIDITY_TOO_LOW: u8 = 217;
	pub const LIQUIDITY_MINTABLE_LOWER_THAN_REQUIRED: u8 = 218;
	pub const TRADE_ASSET_TO_ADD_LIQUIDITY_ABOVE_MAX_AMOUNT: u8 = 219;
	pub const ASSET_TO_CORE_SELL_AMOUNT_NOT_ABOVE_ZERO: u8 = 220;
	pub const CORE_TO_ASSET_SELL_AMOUNT_NOT_ABOVE_ZERO: u8 = 221;
	pub const INSUFFICIENT_SELL_ASSET_FOR_REQUIRED_MINIMUM_BUY_ASSET: u8 = 222;
	pub const ASSET_CANNOT_SWAP_FOR_ITSELF: u8 = 223;
	pub const INVALID_ASSET_ID: u8 = 224;
	pub const OVERFLOW: u8 = 225;
	pub const DIVIDE_BY_ZERO: u8 = 226;

	// Matches and converts crml-cennzx-spot module errors, such that
	// they are propagated in crml-transaction-payment module
	pub fn buy_fee_asset_error_msg_to_code(message: &'static str) -> u8 {
		match message {
			_ => UNKNOWN_BUY_FEE_ASSET,
			"EmptyExchangePool" => EMPTY_EXCHANGE_POOL,
			"InsufficientTradeAssetReserve" => INSUFFICIENT_TRADE_ASSET_RESERVE,
			"InsufficientCoreAssetReserve" => INSUFFICIENT_CORE_ASSET_RESERVE,
			"InsufficientBuyerTradeAssetBalance" => INSUFFICIENT_BUYER_TRADE_ASSET_BALANCE,
			"InsufficientBuyerCoreAssetBalance" => INSUFFICIENT_BUYER_CORE_ASSET_BALANCE,
			"InsufficientSellerTradeAssetBalance" => INSUFFICIENT_SELLER_TRADE_ASSET_BALANCE,
			"InsufficientSellerCoreAssetBalance" => INSUFFICIENT_SELLER_CORE_ASSET_BALANCE,
			"BuyAmountNotPositive" => BUY_AMOUNT_NOT_POSITIVE,
			"SaleValueBelowRequiredMinimum" => SALE_VALUE_BELOW_REQUIRED_MINIMUM,
			"AssetSaleValueNotAboveZero" => ASSET_SALE_VALUE_NOT_ABOVE_ZERO,
			"AssetToCorePriceNotAboveZero" => ASSET_TO_CORE_PRICE_NOT_ABOVE_ZERO,
			"InsufficientCoreAssetInExchangeBalance" => INSUFFICIENT_CORE_ASSET_IN_EXCHANGE_BALANCE,
			"AssetToCorePriceAboveMaxLimit" => ASSET_TO_CORE_PRICE_ABOVE_MAX_LIMIT,
			"CoreToAssetPriceNotAboveZero" => CORE_TO_ASSET_PRICE_NOT_ABOVE_ZERO,
			"CoreToAssetPriceAboveMaxLimit" => CORE_TO_ASSET_PRICE_ABOVE_MAX_LIMIT,
			"AssetToAssetPriceAboveMaxLimit" => ASSET_TO_ASSET_PRICE_ABOVE_MAX_LIMIT,
			"LiquidityTooLow" => LIQUIDITY_TOO_LOW,
			"MinimumTradeAssetIsRequired" => MINIMUM_TRADE_ASSET_IS_REQUIRED,
			"MinimumCoreAssetIsRequired" => MINIMUM_CORE_ASSET_IS_REQUIRED,
			"AssetToWithdrawNotAboveZero" => ASSET_TO_WITHDRAW_NOT_ABOVE_ZERO,
			"LiquidityToWithdrawNotAboveZero" => LIQUIDITY_TO_WITHDRAW_NOT_ABOVE_ZERO,
			"NoLiquidityToRemove" => NO_LIQUIDITY_TO_REMOVE,
			"TradeAssetToAddLiquidityNotAboveZero" => TRADE_ASSET_TO_ADD_LIQUIDITY_NOT_ABOVE_ZERO,
			"CoreAssetToAddLiquidityNotAboveZero" => CORE_ASSET_TO_ADD_LIQUIDITY_NOT_ABOVE_ZERO,
			"CoreAssetBalanceToAddLiquidityTooLow" => CORE_ASSET_BALANCE_TO_ADD_LIQUIDITY_TOO_LOW,
			"TradeAssetBalanceToAddLiquidityTooLow" => TRADE_ASSET_BALANCE_TO_ADD_LIQUIDITY_TOO_LOW,
			"LiquidityMintableLowerThanRequired" => LIQUIDITY_MINTABLE_LOWER_THAN_REQUIRED,
			"TradeAssetToAddLiquidityAboveMaxAmount" => TRADE_ASSET_TO_ADD_LIQUIDITY_ABOVE_MAX_AMOUNT,
			"AssetToCoreSellAmountNotAboveZero" => ASSET_TO_CORE_SELL_AMOUNT_NOT_ABOVE_ZERO,
			"CoreToAssetSellAmountNotAboveZero" => CORE_TO_ASSET_SELL_AMOUNT_NOT_ABOVE_ZERO,
			"InsufficientSellAssetForRequiredMinimumBuyAsset" => INSUFFICIENT_SELL_ASSET_FOR_REQUIRED_MINIMUM_BUY_ASSET,
			"AssetCannotSwapForItself" => ASSET_CANNOT_SWAP_FOR_ITSELF,
			"InvalidAssetId" => INVALID_ASSET_ID,
			"Overflow" => OVERFLOW,
			"DivideByZero" => DIVIDE_BY_ZERO,
		}
	}
}
