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
