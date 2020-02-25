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

pub (crate) mod error_code {
    pub const CoreToAssetPriceNotAboveZero: u8 = 190;
    pub const CoreToAssetPriceAboveMaxLimit: u8 = 191;
    pub const InsufficientBuyerCoreAssetBalance: u8 = 192;

    pub const AssetToCorePriceNotAboveZero: u8 = 193;
    pub const AssetToCorePriceAboveMaxLimit: u8 = 194;
    pub const InsufficientBuyerTradeAssetBalance: u8 = 195;

    pub const AssetSaleValueNotAboveZero: u8 = 196;
    pub const SaleValueBelowRequiredMinimum: u8 = 197;
    pub const InsufficientSellerCoreAssetBalance: u8 = 198;
    pub const BuyAmountNotPositive: u8 = 199;

    // pub const LiquidityRestrictions: u8 = 200
}
