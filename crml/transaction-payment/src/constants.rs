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
	pub const CORE_TO_ASSET_PRICE_NOT_ABOVE_ZERO: u8 = 190;
	pub const CORE_TO_ASSET_PRICE_ABOVE_MAX_LIMIT: u8 = 191;
	pub const INSUFFICIENT_BUYER_CORE_ASSET_BALANCE: u8 = 192;

	pub const ASSET_TO_CORE_PRICE_NOT_ABOVE_ZERO: u8 = 193;
	pub const ASSET_TO_CORE_PRICE_ABOVE_MAX_LIMIT: u8 = 194;
	pub const INSUFFICIENT_BUYER_TRADE_ASSET_BALANCE: u8 = 195;

	pub const ASSET_SALE_VALUE_NOT_ABOVE_ZERO: u8 = 196;
	pub const SALE_VALUE_BELOW_REQUIRED_MINIMUM: u8 = 197;
	pub const INSUFFICIENT_SELLER_CORE_ASSET_BALANCE: u8 = 198;
	pub const BUY_AMOUNT_NOT_POSITIVE: u8 = 199;
	pub const INVALID_ASSET_ID: u8 = 200;
	pub const UNKNOWN_BUY_FEE_ASSET: u8 = 201;

	pub fn buy_fee_asset_error_msg_to_code(message: &'static str) -> u8 {
		match message {
			"InsufficientBuyerTradeAssetBalance" => INSUFFICIENT_BUYER_TRADE_ASSET_BALANCE,
			"InsufficientBuyerCoreAssetBalance" => INSUFFICIENT_BUYER_CORE_ASSET_BALANCE,
			"InsufficientSellerCoreAssetBalance" => INSUFFICIENT_SELLER_CORE_ASSET_BALANCE,
			"BuyAmountNotPositive" => BUY_AMOUNT_NOT_POSITIVE,
			"SaleValueBelowRequiredMinimum" => SALE_VALUE_BELOW_REQUIRED_MINIMUM,
			"AssetSaleValueNotAboveZero" => ASSET_SALE_VALUE_NOT_ABOVE_ZERO,
			"AssetToCorePriceNotAboveZero" => ASSET_TO_CORE_PRICE_NOT_ABOVE_ZERO,
			"AssetToCorePriceAboveMaxLimit" => ASSET_TO_CORE_PRICE_ABOVE_MAX_LIMIT,
			"CoreToAssetPriceNotAboveZero" => CORE_TO_ASSET_PRICE_NOT_ABOVE_ZERO,
			"CoreToAssetPriceAboveMaxLimit" => CORE_TO_ASSET_PRICE_ABOVE_MAX_LIMIT,
			"InvalidAssetId" => INVALID_ASSET_ID,
			_ => UNKNOWN_BUY_FEE_ASSET,
		}
	}
}
