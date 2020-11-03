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

//! Cennzx benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_system::RawOrigin;

use crate::Module as Cennzx;

const TRADE_ASSET_A_ID: u32 = 2;
const TRADE_ASSET_B_ID: u32 = 3;

benchmarks! {
	_{ }

	buy_asset {
		let investor: T::AccountId = whitelisted_caller();
		let buyer: T::AccountId = account("buyer", 0, 0);

		let core_asset_id = <Cennzx<T>>::core_asset_id();
		let asset_a: T::AssetId = TRADE_ASSET_A_ID.into();
		let asset_b: T::AssetId = TRADE_ASSET_B_ID.into();

		let _ = T::MultiCurrency::deposit_creating(&investor, Some(core_asset_id), 1000u32.into());
		let _ = T::MultiCurrency::deposit_creating(&investor, Some(asset_a), 200u32.into());
		let _ = T::MultiCurrency::deposit_creating(&investor, Some(asset_b), 300u32.into());
		let _ = T::MultiCurrency::deposit_creating(&buyer, Some(asset_a), 100u32.into());

		let _ = <Cennzx<T>>::add_liquidity(RawOrigin::Signed(investor.clone()).into(), asset_a, 20u32.into(), 20u32.into(), 100u32.into());
		let _ = <Cennzx<T>>::add_liquidity(RawOrigin::Signed(investor.clone()).into(), asset_b, 30u32.into(), 30u32.into(), 100u32.into());

	}: _(RawOrigin::Signed(buyer.clone()), None, asset_a, asset_b, 10u32.into(), 50u32.into())
	verify {
		assert_eq!(T::MultiCurrency::free_balance(&buyer, Some(asset_a)), 79u32.into());
	}

	sell_asset {
		let investor: T::AccountId = whitelisted_caller();
		let seller: T::AccountId = account("seller", 0, 0);

		let core_asset_id = <Cennzx<T>>::core_asset_id();
		let asset_a: T::AssetId = TRADE_ASSET_A_ID.into();
		let asset_b: T::AssetId = TRADE_ASSET_B_ID.into();

		let _ = T::MultiCurrency::deposit_creating(&investor, Some(core_asset_id), 1000u32.into());
		let _ = T::MultiCurrency::deposit_creating(&investor, Some(asset_a), 200u32.into());
		let _ = T::MultiCurrency::deposit_creating(&investor, Some(asset_b), 300u32.into());
		let _ = T::MultiCurrency::deposit_creating(&seller, Some(asset_a), 100u32.into());

		let _ = <Cennzx<T>>::add_liquidity(RawOrigin::Signed(investor.clone()).into(), asset_a, 20u32.into(), 20u32.into(), 100u32.into());
		let _ = <Cennzx<T>>::add_liquidity(RawOrigin::Signed(investor.clone()).into(), asset_b, 30u32.into(), 30u32.into(), 100u32.into());

	}: _(RawOrigin::Signed(seller.clone()), None, asset_a, asset_b, 20u32.into(), 5u32.into())
	verify {
		assert_eq!(T::MultiCurrency::free_balance(&seller, Some(asset_a)), 80u32.into());
	}

	add_liquidity {
		let investor: T::AccountId = whitelisted_caller();

		let core_asset_id = <Cennzx<T>>::core_asset_id();
		let trade_asset_id: T::AssetId = TRADE_ASSET_A_ID.into();

		let _ = T::MultiCurrency::deposit_creating(&investor, Some(core_asset_id), 200u32.into());
		let _ = T::MultiCurrency::deposit_creating(&investor, Some(trade_asset_id), 100u32.into());

		// Create an initial liquidity to force a longer logic path in the benchmarked add_liquidity
		let initial_liquidity = 10u32.into();
		let _ = <Cennzx<T>>::add_liquidity(RawOrigin::Signed(investor.clone()).into(), trade_asset_id, initial_liquidity, 9u32.into(), 30u32.into())?;

		let top_up = 20u32.into();
	}: _(RawOrigin::Signed(investor.clone()), trade_asset_id, top_up, 20u32.into(), 30u32.into())
	verify {
		assert_eq!(
			<Cennzx<T>>::liquidity_balance((core_asset_id, trade_asset_id), &investor), 60u32.into()
		);
	}

	remove_liquidity {
		let investor: T::AccountId = whitelisted_caller();

		let core_asset_id = <Cennzx<T>>::core_asset_id();
		let trade_asset_id: T::AssetId = TRADE_ASSET_A_ID.into();

		let _ = T::MultiCurrency::deposit_creating(&investor, Some(core_asset_id), 200u32.into());
		let _ = T::MultiCurrency::deposit_creating(&investor, Some(trade_asset_id), 100u32.into());

		let initial_liquidity = 10u32.into();
		let _ = <Cennzx<T>>::add_liquidity(RawOrigin::Signed(investor.clone()).into(), trade_asset_id, initial_liquidity, 9u32.into(), 20u32.into());

	}: _(RawOrigin::Signed(investor.clone()), trade_asset_id, initial_liquidity, 4u32.into(), 4u32.into())
	verify {
		assert_eq!(
			<Cennzx<T>>::liquidity_balance((core_asset_id, trade_asset_id), &investor), 10u32.into()
		);
	}

	set_fee_rate {
		let rate = FeeRate::<PerMillion>::from(1234u128);
	}: _(RawOrigin::Root, rate)
	verify {
		assert_eq!(<Cennzx<T>>::fee_rate(), rate);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{ExtBuilder, Test};
	use frame_support::assert_ok;

	#[test]
	fn buy_asset() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_buy_asset::<Test>());
		});
	}

	#[test]
	fn sell_asset() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_sell_asset::<Test>());
		});
	}

	#[test]
	fn add_liquidity() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_add_liquidity::<Test>());
		});
	}

	#[test]
	fn remove_liquidity() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_remove_liquidity::<Test>());
		});
	}

	#[test]
	fn set_fee_rate() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_set_fee_rate::<Test>());
		});
	}
}
