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
use crate::traits::{Exchange, ExchangePrice, ManageLiquidity};
use crate::{Error, Module};
use cennznet_primitives::{traits::BuyFeeAsset, types::FeeExchange};
use frame_support::{dispatch::DispatchError, storage::StorageMap};
use frame_system::Origin;
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
			<pallet_generic_asset::TotalIssuance<T>>::exists(&fee_exchange_asset_id),
			Error::<T>::InvalidAssetId,
		);

		// TODO: Hard coded to use spending asset ID
		let fee_asset_id = <pallet_generic_asset::Module<T>>::spending_asset_id();

		Self::make_asset_swap_output(
			&who,
			&who,
			&fee_exchange_asset_id,
			&fee_asset_id,
			amount,
			exchange_op.max_payment(),
			Self::fee_rate(),
		)
	}
}

impl<T: Trait> Exchange<T> for Module<T> {
	fn asset_swap_output(
		origin: T::Origin,
		recipient: Option<T::AccountId>,
		#[compact] asset_sold: T::AssetId,
		#[compact] asset_bought: T::AssetId,
		#[compact] buy_amount: T::Balance,
		#[compact] max_paying_amount: T::Balance,
	) -> Result {
		let buyer = ensure_signed(origin)?;
		let amount_paid = Self::make_asset_swap_output(
			&buyer,
			&recipient.unwrap_or_else(|| buyer.clone()),
			&asset_sold,
			&asset_bought,
			buy_amount,
			max_paying_amount,
			Self::fee_rate(),
		)?;
		Ok(amount_paid)
	}

	fn asset_swap_input(
		origin: T::Origin,
		recipient: Option<T::AccountId>,
		#[compact] asset_sold: T::AssetId,
		#[compact] asset_bought: T::AssetId,
		#[compact] sell_amount: T::Balance,
		#[compact] min_receive: T::Balance,
	) -> Result {
		let seller = ensure_signed(origin)?;
		let amount_received = Self::make_asset_swap_input(
			&seller,
			&recipient.unwrap_or_else(|| seller.clone()),
			&asset_sold,
			&asset_bought,
			sell_amount,
			min_receive,
			Self::fee_rate(),
		)?;
		Ok(amount_received)
	}
}

impl<T: Trait> ManageLiquidity<T> for Module<T> {
	fn add_liquidity(
		origin: T::Origin,
		#[compact] asset_id: T::AssetId,
		#[compact] min_liquidity: T::Balance,
		#[compact] max_asset_amount: T::Balance,
		#[compact] core_amount: T::Balance,
	) -> Result {
		let from_account = ensure_signed(origin)?;
		let core_asset_id = Self::core_asset_id();
		ensure!(
			!max_asset_amount.is_zero(),
			Error::<T>::TradeAssetToAddLiquidityNotAboveZero
		);
		ensure!(!core_amount.is_zero(), Error::<T>::CoreAssetToAddLiquidityNotAboveZero);
		ensure!(
			<pallet_generic_asset::Module<T>>::free_balance(&core_asset_id, &from_account) >= core_amount,
			Error::<T>::CoreAssetBalanceToAddLiquidityTooLow
		);
		ensure!(
			<pallet_generic_asset::Module<T>>::free_balance(&asset_id, &from_account) >= max_asset_amount,
			Error::<T>::TradeAssetBalanceToAddLiquidityTooLow
		);
		let exchange_key = ExchangeKey::<T> {
			core_asset: core_asset_id,
			asset: asset_id,
		};
		let total_liquidity = Self::get_total_supply(&exchange_key);
		let exchange_address = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, asset_id);

		if total_liquidity.is_zero() {
			// new exchange pool
			<pallet_generic_asset::Module<T>>::make_transfer(
				&core_asset_id,
				&from_account,
				&exchange_address,
				core_amount,
			)?;
			<pallet_generic_asset::Module<T>>::make_transfer(
				&asset_id,
				&from_account,
				&exchange_address,
				max_asset_amount,
			)?;
			let trade_asset_amount = max_asset_amount;
			let initial_liquidity = core_amount;
			Self::set_liquidity(&exchange_key, &from_account, initial_liquidity);
			Self::mint_total_supply(&exchange_key, initial_liquidity);
			Self::deposit_event(RawEvent::AddLiquidity(
				from_account,
				initial_liquidity,
				asset_id,
				trade_asset_amount,
			));
		} else {
			// TODO: shall i use total_balance instead? in which case the exchange address will have reserve balance?
			let trade_asset_reserve = <pallet_generic_asset::Module<T>>::free_balance(&asset_id, &exchange_address);
			let core_asset_reserve = <pallet_generic_asset::Module<T>>::free_balance(&core_asset_id, &exchange_address);
			let trade_asset_amount = core_amount * trade_asset_reserve / core_asset_reserve + One::one();
			let liquidity_minted = core_amount * total_liquidity / core_asset_reserve;
			ensure!(
				liquidity_minted >= min_liquidity,
				Error::<T>::LiquidityMintableLowerThanRequired
			);
			ensure!(
				max_asset_amount >= trade_asset_amount,
				Error::<T>::TradeAssetToAddLiquidityAboveMaxAmount
			);

			<pallet_generic_asset::Module<T>>::make_transfer(
				&core_asset_id,
				&from_account,
				&exchange_address,
				core_amount,
			)?;
			<pallet_generic_asset::Module<T>>::make_transfer(
				&asset_id,
				&from_account,
				&exchange_address,
				trade_asset_amount,
			)?;

			Self::set_liquidity(
				&exchange_key,
				&from_account,
				Self::get_liquidity(&exchange_key, &from_account) + liquidity_minted,
			);
			Self::mint_total_supply(&exchange_key, liquidity_minted);
			Self::deposit_event(RawEvent::AddLiquidity(
				from_account,
				core_amount,
				asset_id,
				trade_asset_amount,
			));
		}

		Ok(Self::get_liquidity(&exchange_key, &from_account))
	}

	fn remove_liquidity(
		origin: T::Origin,
		#[compact] asset_id: T::AssetId,
		#[compact] liquidity_withdrawn: T::Balance,
		#[compact] min_asset_withdraw: T::Balance,
		#[compact] min_core_withdraw: T::Balance,
	) -> Result {
		let from_account = ensure_signed(origin)?;
		ensure!(
			liquidity_withdrawn > Zero::zero(),
			Error::<T>::LiquidityToWithdrawNotAboveZero
		);
		ensure!(
			min_asset_withdraw > Zero::zero() && min_core_withdraw > Zero::zero(),
			Error::<T>::AssetToWithdrawNotAboveZero
		);

		let core_asset_id = Self::core_asset_id();
		let exchange_key = (core_asset_id, asset_id);
		let account_liquidity = Self::get_liquidity(&exchange_key, &from_account);
		ensure!(account_liquidity >= liquidity_withdrawn, Error::<T>::LiquidityTooLow);

		let total_liquidity = Self::get_total_supply(&exchange_key);
		let exchange_address = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, asset_id);
		ensure!(total_liquidity > Zero::zero(), Error::<T>::NoLiquidityToRemove);

		let trade_asset_reserve = <pallet_generic_asset::Module<T>>::free_balance(&asset_id, &exchange_address);
		let core_asset_reserve = <pallet_generic_asset::Module<T>>::free_balance(&core_asset_id, &exchange_address);
		let core_asset_amount = liquidity_withdrawn * core_asset_reserve / total_liquidity;
		let trade_asset_amount = liquidity_withdrawn * trade_asset_reserve / total_liquidity;
		ensure!(
			core_asset_amount >= min_core_withdraw,
			Error::<T>::MinimumCoreAssetIsRequired
		);
		ensure!(
			trade_asset_amount >= min_asset_withdraw,
			Error::<T>::MinimumTradeAssetIsRequired
		);

		<pallet_generic_asset::Module<T>>::make_transfer(
			&core_asset_id,
			&exchange_address,
			&from_account,
			core_asset_amount,
		)?;
		<pallet_generic_asset::Module<T>>::make_transfer(
			&asset_id,
			&exchange_address,
			&from_account,
			trade_asset_amount,
		)?;
		Self::set_liquidity(&exchange_key, &from_account, account_liquidity - liquidity_withdrawn);
		Self::burn_total_supply(&exchange_key, liquidity_withdrawn);
		Self::deposit_event(RawEvent::RemoveLiquidity(
			from_account,
			core_asset_amount,
			asset_id,
			trade_asset_amount,
		));
		Ok(Self::get_liquidity(&exchange_key, &from_account))
	}
}

impl<T: Trait> ExchangePrice<T> for Module<T> {
	/// Set the spot exchange wide fee rate (root only)
	fn set_fee_rate(origin: T::Origin, new_fee_rate: FeeRate<PerMillion>) -> Result {
		ensure_root(origin)?;
		DefaultFeeRate::mutate(|fee_rate| *fee_rate = new_fee_rate);
		Ok(DefaultFeeRate::get())
	}

	fn get_core_to_asset_output_price(
		asset_id: &T::AssetId,
		buy_amount: T::Balance,
		fee_rate: FeeRate<PerMillion>,
	) -> Result {
		ensure!(buy_amount > Zero::zero(), Error::<T>::BuyAmountNotPositive);

		let core_asset_id = Self::core_asset_id();
		let exchange_address = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, *asset_id);

		let asset_reserve = <pallet_generic_asset::Module<T>>::free_balance(asset_id, &exchange_address);
		ensure!(asset_reserve > buy_amount, Error::<T>::InsufficientTradeAssetReserve);

		let core_reserve = <pallet_generic_asset::Module<T>>::free_balance(&core_asset_id, &exchange_address);

		Self::get_output_price(buy_amount, core_reserve, asset_reserve, fee_rate)
	}

	fn get_asset_to_core_input_price(
		asset_id: &T::AssetId,
		sell_amount: T::Balance,
		fee_rate: FeeRate<PerMillion>,
	) -> Result {
		ensure!(
			sell_amount > Zero::zero(),
			Error::<T>::AssetToCoreSellAmountNotAboveZero
		);

		let core_asset_id = Self::core_asset_id();
		let exchange_address = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, *asset_id);

		let asset_reserve = <pallet_generic_asset::Module<T>>::free_balance(asset_id, &exchange_address);
		let core_reserve = <pallet_generic_asset::Module<T>>::free_balance(&core_asset_id, &exchange_address);
		Self::get_input_price(sell_amount, asset_reserve, core_reserve, fee_rate)
	}

	fn get_asset_to_core_output_price(
		asset_id: &T::AssetId,
		buy_amount: T::Balance,
		fee_rate: FeeRate<PerMillion>,
	) -> Result {
		ensure!(buy_amount > Zero::zero(), Error::<T>::BuyAmountNotPositive);

		let core_asset_id = Self::core_asset_id();
		let exchange_address = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, *asset_id);

		let core_asset_reserve = <pallet_generic_asset::Module<T>>::free_balance(&core_asset_id, &exchange_address);
		ensure!(
			core_asset_reserve > buy_amount,
			Error::<T>::InsufficientCoreAssetReserve
		);

		let trade_asset_reserve = <pallet_generic_asset::Module<T>>::free_balance(&asset_id, &exchange_address);

		Self::get_output_price(buy_amount, trade_asset_reserve, core_asset_reserve, fee_rate)
	}

	fn get_core_to_asset_input_price(
		asset_id: &T::AssetId,
		sell_amount: T::Balance,
		fee_rate: FeeRate<PerMillion>,
	) -> Result {
		ensure!(
			sell_amount > Zero::zero(),
			Error::<T>::CoreToAssetSellAmountNotAboveZero
		);

		let core_asset_id = Self::core_asset_id();
		let exchange_address = T::ExchangeAddressGenerator::exchange_address_for(core_asset_id, *asset_id);
		let core_asset_reserve = <pallet_generic_asset::Module<T>>::free_balance(&core_asset_id, &exchange_address);
		let trade_asset_reserve = <pallet_generic_asset::Module<T>>::free_balance(asset_id, &exchange_address);

		let output_amount = Self::get_input_price(sell_amount, core_asset_reserve, trade_asset_reserve, fee_rate)?;

		ensure!(
			trade_asset_reserve > output_amount,
			Error::<T>::InsufficientTradeAssetReserve
		);

		Ok(output_amount)
	}
}

#[cfg(test)]
pub(crate) mod impl_tests {
	use super::*;
	use crate::{
		mock::{self, CORE_ASSET_ID, FEE_ASSET_ID, TRADE_ASSET_A_ID},
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
				Error::<Test>::InsufficientTradeAssetReserve
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
