// Copyright (C) 2019 Centrality Investments Limited
// This file is part of CENNZnet.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

//! Define mock currencies
#![cfg(test)]

use crate::Trait;
use frame_support::additional_traits::AssetIdAuthority;
use pallet_generic_asset::AssetCurrency;

pub const CORE_ASSET_ID: u32 = 0;
pub const TRADE_ASSET_A_ID: u32 = 1;
pub const TRADE_ASSET_B_ID: u32 = 2;
pub const FEE_ASSET_ID: u32 = 10;

/// A mock core currency. This is the network spending type e.g. CPAY it is a generic asset
pub(crate) type CoreAssetCurrency<T> = AssetCurrency<T, CoreAssetIdProvider<T>>;
/// A mock trade currency 'A'. It is a generic asset
pub(crate) type TradeAssetCurrencyA<T> = AssetCurrency<T, TradeAssetAIdProvider<T>>;
/// A mock trade currency 'B'. It is a generic asset
pub(crate) type TradeAssetCurrencyB<T> = AssetCurrency<T, TradeAssetBIdProvider<T>>;
/// A mock fee currency. It is a generic asset
pub(crate) type FeeAssetCurrency<T> = AssetCurrency<T, FeeAssetIdProvider<T>>;

pub struct CoreAssetIdProvider<T>(sp_std::marker::PhantomData<T>);
impl<T: Trait> AssetIdAuthority for CoreAssetIdProvider<T> {
	type AssetId = T::AssetId;
	fn asset_id() -> Self::AssetId {
		CORE_ASSET_ID.into()
	}
}

pub struct TradeAssetAIdProvider<T>(sp_std::marker::PhantomData<T>);
impl<T: Trait> AssetIdAuthority for TradeAssetAIdProvider<T> {
	type AssetId = T::AssetId;
	fn asset_id() -> Self::AssetId {
		TRADE_ASSET_A_ID.into()
	}
}

pub struct TradeAssetBIdProvider<T>(sp_std::marker::PhantomData<T>);
impl<T: Trait> AssetIdAuthority for TradeAssetBIdProvider<T> {
	type AssetId = T::AssetId;
	fn asset_id() -> Self::AssetId {
		TRADE_ASSET_B_ID.into()
	}
}

pub struct FeeAssetIdProvider<T>(sp_std::marker::PhantomData<T>);
impl<T: Trait> AssetIdAuthority for FeeAssetIdProvider<T> {
	type AssetId = T::AssetId;
	fn asset_id() -> Self::AssetId {
		FEE_ASSET_ID.into()
	}
}
