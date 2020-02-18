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
//!
//! Extra CENNZX-Spot traits + implementations
//!
use super::Trait;
use crate::Module;
use cennznet_primitives::traits::{BuyFeeAsset, FeeExchange};
use frame_support::dispatch::DispatchError;
use sp_core::crypto::{UncheckedFrom, UncheckedInto};
use sp_runtime::traits::Hash;
use sp_std::{marker::PhantomData, prelude::*};

/// A function that generates an `AccountId` for a CENNZX-SPOT exchange / (core, asset) pair
pub trait ExchangeAddressFor<AssetId: Sized, AccountId: Sized> {
	fn exchange_address_for(core_asset_id: AssetId, asset_id: AssetId) -> AccountId;
}

// A CENNZX-Spot exchange address generator implementation
pub struct ExchangeAddressGenerator<T: Trait>(PhantomData<T>);

impl<T: Trait> ExchangeAddressFor<T::AssetId, T::AccountId> for ExchangeAddressGenerator<T>
where
	T::AccountId: UncheckedFrom<T::Hash> + AsRef<[u8]>,
	T::AssetId: Into<u64>,
{
	/// Generates an exchange address for the given core / asset pair
	fn exchange_address_for(core_asset_id: T::AssetId, asset_id: T::AssetId) -> T::AccountId {
		let mut buf = Vec::new();
		buf.extend_from_slice(b"cennz-x-spot:");
		buf.extend_from_slice(&u64_to_bytes(core_asset_id.into()));
		buf.extend_from_slice(&u64_to_bytes(asset_id.into()));

		T::Hashing::hash(&buf[..]).unchecked_into()
	}
}

fn u64_to_bytes(x: u64) -> [u8; 8] {
	x.to_le_bytes()
}

impl<T: Trait> BuyFeeAsset for Module<T> {
	type AccountId = T::AccountId;
	type AssetId = T::AssetId;
	type Balance = T::Balance;

	/// Use the CENNZX-Spot exchange to seamlessly buy fee asset
	fn buy_fee_asset(
		who: &T::AccountId,
		amount: T::Balance,
		exchange_op: &FeeExchange<Self>,
	) -> Result<T::Balance, DispatchError> {
		// TODO: Hard coded to use spending asset ID
		let fee_asset_id = <pallet_generic_asset::Module<T>>::spending_asset_id();

		Self::make_asset_swap_output(
			&who,
			&who,
			&exchange_op.get_asset_id(),
			&fee_asset_id,
			amount,
			exchange_op.get_balance(),
			Self::fee_rate(),
		)
		.map_err(|_| DispatchError::Other("Failed to charge transaction fees during conversion"))
	}
}

#[cfg(test)]
pub(crate) mod impl_tests {
	use super::*;
	use crate::{
		mock::{self, CORE_ASSET_ID, FEE_ASSET_ID, TRADE_ASSET_A_ID},
		tests::{CennzXSpot, ExtBuilder, Test},
	};
	use frame_support::traits::Currency;
	use sp_core::H256;

	type CoreAssetCurrency = mock::CoreAssetCurrency<Test>;
	type TradeAssetCurrencyA = mock::TradeAssetCurrencyA<Test>;
	type FeeAssetCurrency = mock::FeeAssetCurrency<Test>;
	type TestFeeExchange = FeeExchange<CennzXSpot>;

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

			assert_ok!(<CennzXSpot as BuyFeeAsset>::buy_fee_asset(
				&user,
				target_fee,
				&TestFeeExchange::new_v1(TRADE_ASSET_A_ID, 2_000_000)
			));

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

			assert_eq!(core_asset_price, 539);
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
			let user = with_account!(CoreAssetCurrency => 0, TradeAssetCurrencyA => 10);

			assert_err!(
				<CennzXSpot as BuyFeeAsset>::buy_fee_asset(
					&user,
					51,
					&TestFeeExchange::new_v1(TRADE_ASSET_A_ID, 2_000_000),
				),
				"Failed to charge transaction fees during conversion"
			);

			assert_balance_eq!(user, CoreAssetCurrency => 0);
			assert_balance_eq!(user, TradeAssetCurrencyA => 10);
		});
	}

	#[test]
	fn u64_to_bytes_works() {
		assert_eq!(u64_to_bytes(80_000), [128, 56, 1, 0, 0, 0, 0, 0]);
	}
}
