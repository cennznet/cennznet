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
use crate::{weights::WeightInfo, Config, Module};
use cennznet_primitives::{traits::BuyFeeAsset, types::FeeExchange};
use crml_support::MultiCurrency;
use frame_support::{dispatch::DispatchError, weights::Weight};
use sp_core::crypto::{UncheckedFrom, UncheckedInto};
use sp_runtime::traits::Hash;
use sp_std::{marker::PhantomData, prelude::*};

/// A function that generates an `AccountId` for a CENNZX exchange / (core, asset) pair
pub trait ExchangeAddressFor {
	/// The Account Id type
	type AccountId;
	/// The Asset Id type
	type AssetId;
	/// Create and exchange address given `asset_id`
	fn exchange_address_for(asset_id: Self::AssetId) -> Self::AccountId;
}

/// A CENNZX exchange address generator implementation
pub struct ExchangeAddressGenerator<T: Config>(PhantomData<T>);

impl<T: Config> ExchangeAddressFor for ExchangeAddressGenerator<T>
where
	T::AccountId: UncheckedFrom<T::Hash> + AsRef<[u8]>,
	T::AssetId: Into<u64>,
{
	type AccountId = T::AccountId;
	type AssetId = T::AssetId;

	/// Generates a unique, deterministic exchange address for the given `core_asset_id`, `asset_id` pair
	/// It's uniqueness and collision resistance is determined by the `T::Hashing` implementation
	fn exchange_address_for(asset_id: T::AssetId) -> T::AccountId {
		let core_asset_id = Module::<T>::core_asset_id();
		// 13 + 64 + 64
		let mut buf = Vec::<u8>::with_capacity(141);
		buf.extend_from_slice(b"cennz-x-spot:");
		buf.extend_from_slice(&core_asset_id.into().to_le_bytes());
		buf.extend_from_slice(&asset_id.into().to_le_bytes());

		T::Hashing::hash(&buf).unchecked_into()
	}
}

impl<T: Config> BuyFeeAsset for Module<T> {
	type AccountId = T::AccountId;
	type Balance = T::Balance;
	type FeeExchange = FeeExchange<T::AssetId, T::Balance>;

	/// Use CENNZX to seamlessly buy fee asset
	fn buy_fee_asset(
		who: &Self::AccountId,
		amount: Self::Balance,
		exchange_op: &Self::FeeExchange,
	) -> Result<Self::Balance, DispatchError> {
		let fee_exchange_asset_id = exchange_op.asset_id();
		let fee_asset_id = <T::MultiCurrency as MultiCurrency>::fee_currency();

		Self::execute_buy(
			&who,
			&who,
			fee_exchange_asset_id,
			fee_asset_id,
			amount,
			exchange_op.max_payment(),
		)
	}

	fn buy_fee_weight() -> Weight {
		T::WeightInfo::sell_asset()
	}
}

#[cfg(test)]
pub(crate) mod impl_tests {
	use super::*;
	use crate::{
		mock::{Cennzx, ExtBuilder, Test, CORE_ASSET_ID, FEE_ASSET_ID, TRADE_ASSET_A_ID},
		Error,
	};
	use cennznet_primitives::types::FeeExchange;
	use crml_support::MultiCurrency;
	use frame_support::{assert_err, assert_ok};

	#[test]
	fn it_generates_an_exchange_address() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ne!(
				ExchangeAddressGenerator::<Test>::exchange_address_for(1),
				ExchangeAddressGenerator::<Test>::exchange_address_for(2)
			);
		});
	}

	#[test]
	fn buy_fee_asset() {
		ExtBuilder::default().build().execute_with(|| {
			with_exchange!(CORE_ASSET_ID => 10_000, TRADE_ASSET_A_ID => 10_000);
			with_exchange!(CORE_ASSET_ID => 10_000, FEE_ASSET_ID => 10_000);

			let user = with_account!(CORE_ASSET_ID => 0, TRADE_ASSET_A_ID => 1_000);
			let target_fee = 510;
			let scale_factor = 1_000_000;
			let fee_rate = 3_000; // fee is 0.3%
			let fee_rate_factor = scale_factor + fee_rate; // 1_000_000 + 3_000

			assert_ok!(
				<Cennzx as BuyFeeAsset>::buy_fee_asset(
					&user,
					target_fee,
					&FeeExchange::new_v1(TRADE_ASSET_A_ID, 2_000_000)
				),
				571
			);

			// For more detail, see `fn get_output_price` in lib.rs
			let core_asset_price = {
				let output_amount = target_fee;
				let input_reserve = 10_000; // CORE_ASSET_ID reserve
				let output_reserve = 10_000; // FEE_ASSET_ID reserve
				let denom = output_reserve - output_amount; // 10000 - 510 = 9490
				let res = (input_reserve * output_amount) / denom; // 537 (decimals truncated)
				let price = res + 1; // 537 + 1 = 538
				(price * fee_rate_factor) / scale_factor // price adjusted with fee
			};

			let trade_asset_price = {
				let output_amount = core_asset_price;
				let input_reserve = 10_000; // TRADE_ASSET_A_ID reserve
				let output_reserve = 10_000; // CORE_ASSET_ID reserve
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
				CORE_ASSET_ID => exchange1_core,
				TRADE_ASSET_A_ID => exchange1_trade
			);
			assert_exchange_balance_eq!(
				CORE_ASSET_ID => exchange2_core,
				FEE_ASSET_ID => exchange2_fee
			);

			let trade_asset_remainder = 1_000 - trade_asset_price;
			assert_balance_eq!(user, CORE_ASSET_ID => 0);
			assert_balance_eq!(user, FEE_ASSET_ID => target_fee);
			assert_balance_eq!(user, TRADE_ASSET_A_ID => trade_asset_remainder);
		});
	}

	#[test]
	fn buy_fee_asset_insufficient_trade_asset() {
		ExtBuilder::default().build().execute_with(|| {
			with_exchange!(CORE_ASSET_ID => 0, TRADE_ASSET_A_ID => 100);
			with_exchange!(CORE_ASSET_ID => 0, FEE_ASSET_ID => 100);
			let user = with_account!(CORE_ASSET_ID => 0, TRADE_ASSET_A_ID => 10);

			assert_err!(
				<Cennzx as BuyFeeAsset>::buy_fee_asset(&user, 51, &FeeExchange::new_v1(TRADE_ASSET_A_ID, 2_000_000),),
				Error::<Test>::EmptyExchangePool
			);

			assert_balance_eq!(user, CORE_ASSET_ID => 0);
			assert_balance_eq!(user, TRADE_ASSET_A_ID => 10);
		});
	}

	#[test]
	fn buy_fee_asset_from_empty_pool() {
		ExtBuilder::default().build().execute_with(|| {
			let user = with_account!(CORE_ASSET_ID => 0, TRADE_ASSET_A_ID => 10);

			assert_err!(
				<Cennzx as BuyFeeAsset>::buy_fee_asset(&user, 51, &FeeExchange::new_v1(TRADE_ASSET_A_ID, 2_000_000),),
				Error::<Test>::EmptyExchangePool
			);

			assert_exchange_balance_eq!(
				CORE_ASSET_ID => 0,
				TRADE_ASSET_A_ID => 0
			);
			assert_exchange_balance_eq!(
				CORE_ASSET_ID => 0,
				FEE_ASSET_ID => 0
			);
		});
	}
}
