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

//! Attestation benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::{benchmarks, whitelisted_caller};
use frame_system::RawOrigin;

use crate::Module as Cennzx;

benchmarks! {
	_{ }

	add_liquidity {
		let investor: T::AccountId = whitelisted_caller();

		let core_asset_id: T::AssetId = 1u32.into();
		let trade_asset_id: T::AssetId = 2u32.into();

		let _ = T::MultiCurrency::deposit_creating(&investor, Some(core_asset_id), 200u32.into());
		let _ = T::MultiCurrency::deposit_creating(&investor, Some(trade_asset_id), 100u32.into());

		// Create an initial liquidity to force a longer logic path in the benchmarked add_liquidity
		let initial_liquidity = 10u32.into();
		let _ = <Cennzx<T>>::add_liquidity(RawOrigin::Signed(investor.clone()).into(), trade_asset_id, initial_liquidity, 9u32.into(), 20u32.into());

		let top_up = 20u32.into();
	}: _(RawOrigin::Signed(investor.clone()), trade_asset_id, top_up, 20u32.into(), 30u32.into())
	verify {
		assert!(
			<Cennzx<T>>::liquidity_balance((core_asset_id, trade_asset_id), &investor) >= initial_liquidity + top_up
		);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{ExtBuilder, Test};
	use frame_support::assert_ok;

	#[test]
	fn test_benchmarks() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_add_liquidity::<Test>());
		});
	}
}
