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

pub(crate) mod error_code {
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
	pub const UNKNOW_BUY_FEE_ASSET: u8 = 201;

	pub fn buy_fee_asset_error_to_code(error: u8) -> u8 {
		match error {
			4 => INSUFFICIENT_BUYER_TRADE_ASSET_BALANCE,
			5 => INSUFFICIENT_BUYER_CORE_ASSET_BALANCE,
			7 => INSUFFICIENT_SELLER_CORE_ASSET_BALANCE,
			8 => BUY_AMOUNT_NOT_POSITIVE,
			9 => SALE_VALUE_BELOW_REQUIRED_MINIMUM,
			10 => ASSET_SALE_VALUE_NOT_ABOVE_ZERO,
			11 => ASSET_TO_CORE_PRICE_NOT_ABOVE_ZERO,
			13 => ASSET_TO_CORE_PRICE_ABOVE_MAX_LIMIT,
			14 => CORE_TO_ASSET_PRICE_NOT_ABOVE_ZERO,
			15 => CORE_TO_ASSET_PRICE_ABOVE_MAX_LIMIT,
			33 => INVALID_ASSET_ID,
			_ => UNKNOW_BUY_FEE_ASSET,
		}
	}
}
