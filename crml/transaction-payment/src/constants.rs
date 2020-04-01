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
	pub const SALE_VALUE_BELOW_REQUIRED_MINIMUM: u8 = 197;
	pub const BUY_AMOUNT_NOT_POSITIVE: u8 = 199;
	pub const INSUFFICIENT_FEE_ASSET_BALANCE: u8 = 200;
	pub const INVALID_ASSET_ID: u8 = 201;
	pub const UNKNOWN_BUY_FEE_ASSET: u8 = 202;
	pub const LIQUIDITY_TOO_LOW: u8 = 203;
	pub const ASSET_CANNOT_SWAP_FOR_ITSELF: u8 = 204;
	pub const INSUFFICIENT_ASSET_RESERVE: u8 = 205;
	pub const PRICE_ABOVE_MAX_LIMIT: u8 = 206;
	pub const INSUFFICIENT_BALANCE: u8 = 195;

	// Matches and converts crml-cennzx-spot module errors, such that
	// they are propagated in crml-transaction-payment module
	pub fn buy_fee_asset_error_msg_to_code(message: &'static str) -> u8 {
		match message {
			"InsufficientBalance" => INSUFFICIENT_BALANCE,
			"InsufficientAssetReserve" => INSUFFICIENT_ASSET_RESERVE,
			"BuyAmountNotPositive" => BUY_AMOUNT_NOT_POSITIVE,
			"SaleValueBelowRequiredMinimum" => SALE_VALUE_BELOW_REQUIRED_MINIMUM,
			"AssetCannotSwapForItself" => ASSET_CANNOT_SWAP_FOR_ITSELF,
			"PriceAboveMaxLimit" => PRICE_ABOVE_MAX_LIMIT,
			"InvalidAssetId" => INVALID_ASSET_ID,
			"LiquidityTooLow" => LIQUIDITY_TOO_LOW,
			_ => UNKNOWN_BUY_FEE_ASSET,
		}
	}
}
