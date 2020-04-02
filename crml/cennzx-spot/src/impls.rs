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
//! Extra CENNZX-Spot traits + implementations
//!
use super::Trait;
use crate::{Error, Module};
use cennznet_primitives::{traits::BuyFeeAsset, types::FeeExchange};
use frame_support::{dispatch::DispatchError, StorageMap};
use sp_core::crypto::{UncheckedFrom, UncheckedInto};
use sp_runtime::traits::Hash;
use sp_std::{marker::PhantomData, prelude::*};

/// A function that generates an `AccountId` for a CENNZX-SPOT exchange / (core, asset) pair
pub trait ExchangeAddressFor<AssetId: Sized, AccountId: Sized> {
	fn exchange_address_for(asset_id: AssetId) -> AccountId;
}

// A CENNZX-Spot exchange address generator implementation
pub struct ExchangeAddressGenerator<T: Trait>(PhantomData<T>);

impl<T: Trait> ExchangeAddressFor<T::AssetId, T::AccountId> for ExchangeAddressGenerator<T>
where
	T::AccountId: UncheckedFrom<T::Hash> + AsRef<[u8]>,
	T::AssetId: Into<u64>,
{
	/// Generates an exchange address for the given core / asset pair
	fn exchange_address_for(asset_id: T::AssetId) -> T::AccountId {
		let mut buf = Vec::new();
		let core_asset_id = Module::<T>::core_asset_id();
		buf.extend_from_slice(b"cennz-x-spot:");
		buf.extend_from_slice(&core_asset_id.into().to_le_bytes());
		buf.extend_from_slice(&asset_id.into().to_le_bytes());

		T::Hashing::hash(&buf[..]).unchecked_into()
	}
}

impl<T: Trait> BuyFeeAsset for Module<T> {
	type AccountId = T::AccountId;
	type Balance = T::Balance;
	type FeeExchange = FeeExchange<T::AssetId, T::Balance>;

	/// Use the CENNZX-Spot exchange to seamlessly buy fee asset
	fn buy_fee_asset(
		who: &Self::AccountId,
		amount: Self::Balance,
		exchange_op: &Self::FeeExchange,
	) -> Result<Self::Balance, DispatchError> {
		// check whether exchange asset id exist
		let fee_exchange_asset_id = exchange_op.asset_id();
		ensure!(
			<pallet_generic_asset::TotalIssuance<T>>::contains_key(&fee_exchange_asset_id),
			Error::<T>::InvalidAssetId,
		);

		// TODO: Hard coded to use spending asset ID
		let fee_asset_id = <pallet_generic_asset::Module<T>>::spending_asset_id();

		Self::execute_buy(
			&who,
			&who,
			&fee_exchange_asset_id,
			&fee_asset_id,
			amount,
			exchange_op.max_payment(),
		)
	}
}

#[cfg(test)]
pub(crate) mod impl_tests {
	use super::*;
	use crate::{
		mock::{self, FEE_ASSET_ID, TRADE_ASSET_A_ID},
		tests::{CennzXSpot, ExtBuilder, Test},
		Error,
	};
	use frame_support::traits::Currency;
	use sp_core::H256;

	type CoreAssetCurrency = mock::CoreAssetCurrency<Test>;
	type TradeAssetCurrencyA = mock::TradeAssetCurrencyA<Test>;
	type FeeAssetCurrency = mock::FeeAssetCurrency<Test>;
	type TestFeeExchange = FeeExchange<u32, u128>;

	#[test]
	fn buy_fee_asset() {
		ExtBuilder::default().build().execute_with(|| {
			with_exchange!(CoreAssetCurrency => 10_000, TradeAssetCurrencyA => 10_000);
			with_exchange!(CoreAssetCurrency => 10_000, FeeAssetCurrency => 10_000);

			let user = with_account!(CoreAssetCurrency => 0, TradeAssetCurrencyA => 1_000);
			let target_fee = 510;
			let scale_factor = 1_000_000;
			let fee_rate = 3_000; // fee is 0.3%
			let fee_rate_factor = scale_factor + fee_rate; // 1_000_000 + 3_000

			assert_ok!(
				<CennzXSpot as BuyFeeAsset>::buy_fee_asset(
					&user,
					target_fee,
					&TestFeeExchange::new_v1(TRADE_ASSET_A_ID, 2_000_000)
				),
				571
			);

			// For more detail, see `fn get_output_price` in lib.rs
			let core_asset_price = {
				let output_amount = target_fee;
				let input_reserve = 10_000; // CoreAssetCurrency reserve
				let output_reserve = 10_000; // FeeAssetCurrency reserve
				let denom = output_reserve - output_amount; // 10000 - 510 = 9490
				let res = (input_reserve * output_amount) / denom; // 537 (decimals truncated)
				let price = res + 1; // 537 + 1 = 538
				(price * fee_rate_factor) / scale_factor // price adjusted with fee
			};

			let trade_asset_price = {
				let output_amount = core_asset_price;
				let input_reserve = 10_000; // TradeAssetCurrencyA reserve
				let output_reserve = 10_000; // CoreAssetCurrency reserve
				let denom = output_reserve - output_amount; // 10000 - 539 = 9461
				let res = (input_reserve * output_amount) / denom; // 569 (decimals truncated)
				let price = res + 1; // 569 + 1 = 570
				(price * fee_rate_factor) / scale_factor // price adjusted with fee
			};

			// This is calculated independently from `fn get_output_price` in lib.rs
			let core_asset_price = 538;

			assert_eq!(trade_asset_price, 571);

			let exchange1_core = 10_000 - core_asset_price;
			let exchange1_trade = 10_000 + trade_asset_price;

			let exchange2_core = 10_000 + core_asset_price;
			let exchange2_fee = 10_000 - target_fee;

			assert_exchange_balance_eq!(
				CoreAssetCurrency => exchange1_core,
				TradeAssetCurrencyA => exchange1_trade
			);
			assert_exchange_balance_eq!(
				CoreAssetCurrency => exchange2_core,
				FeeAssetCurrency => exchange2_fee
			);

			let trade_asset_remainder = 1_000 - trade_asset_price;
			assert_balance_eq!(user, CoreAssetCurrency => 0);
			assert_balance_eq!(user, FeeAssetCurrency => target_fee);
			assert_balance_eq!(user, TradeAssetCurrencyA => trade_asset_remainder);
		});
	}

	#[test]
	fn buy_fee_asset_insufficient_trade_asset() {
		ExtBuilder::default().build().execute_with(|| {
			with_exchange!(CoreAssetCurrency => 0, TradeAssetCurrencyA => 100);
			with_exchange!(CoreAssetCurrency => 0, FeeAssetCurrency => 100);
			let user = with_account!(CoreAssetCurrency => 0, TradeAssetCurrencyA => 10);

			assert_err!(
				<CennzXSpot as BuyFeeAsset>::buy_fee_asset(
					&user,
					51,
					&TestFeeExchange::new_v1(TRADE_ASSET_A_ID, 2_000_000),
				),
				Error::<Test>::EmptyExchangePool
			);

			assert_balance_eq!(user, CoreAssetCurrency => 0);
			assert_balance_eq!(user, TradeAssetCurrencyA => 10);
		});
	}

	#[test]
	fn buy_fee_asset_from_empty_pool() {
		ExtBuilder::default().build().execute_with(|| {
			let user = with_account!(CoreAssetCurrency => 0, TradeAssetCurrencyA => 10);

			assert_err!(
				<CennzXSpot as BuyFeeAsset>::buy_fee_asset(
					&user,
					51,
					&TestFeeExchange::new_v1(TRADE_ASSET_A_ID, 2_000_000),
				),
				Error::<Test>::EmptyExchangePool
			);

			assert_exchange_balance_eq!(
				CoreAssetCurrency => 0,
				TradeAssetCurrencyA => 0
			);
			assert_exchange_balance_eq!(
				CoreAssetCurrency => 0,
				FeeAssetCurrency => 0
			);
		});
	}
}
