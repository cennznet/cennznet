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

//! # Transaction Payment Pallet
//!
//! Transaction Payment Customized Error Code Constants

pub mod error_code {
	pub const MINIMUM_BUY_REQUIREMENT_NOT_MET: u8 = 197;
	pub const CANNOT_TRADE_ZERO: u8 = 199;
	pub const INSUFFICIENT_FEE_ASSET_BALANCE: u8 = 200;
	pub const INVALID_ASSET_ID: u8 = 201;
	pub const UNKNOWN_BUY_FEE_ASSET: u8 = 202;
	pub const INSUFFICIENT_LIQUIDITY: u8 = 203;
	pub const ASSET_CANNOT_SWAP_FOR_ITSELF: u8 = 204;
	pub const INSUFFICIENT_EXCHANGE_POOL_RESERVE: u8 = 205;
	pub const MAXIMUM_SELL_REQUIREMENT_NOT_MET: u8 = 206;
	pub const INSUFFICIENT_BALANCE: u8 = 195;

	// Matches and converts crml-cennzx module errors, such that
	// they are propagated in crml-transaction-payment module
	pub fn buy_fee_asset_error_msg_to_code(message: &'static str) -> u8 {
		match message {
			"InsufficientBalance" => INSUFFICIENT_BALANCE,
			"InsufficientExchangePoolReserve" => INSUFFICIENT_EXCHANGE_POOL_RESERVE,
			"CannotTradeZero" => CANNOT_TRADE_ZERO,
			"MinimumBuyRequirementNotMet" => MINIMUM_BUY_REQUIREMENT_NOT_MET,
			"AssetCannotSwapForItself" => ASSET_CANNOT_SWAP_FOR_ITSELF,
			"MaximumSellRequirementNotMet" => MAXIMUM_SELL_REQUIREMENT_NOT_MET,
			"InvalidAssetId" => INVALID_ASSET_ID,
			"InsufficientLiquidity" => INSUFFICIENT_LIQUIDITY,
			_ => UNKNOWN_BUY_FEE_ASSET,
		}
	}
}
