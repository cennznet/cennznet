// Copyright 2019-2020 Centrality Investments Limited
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Staking module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::{account, benchmarks};

use crate::rewards::Module as Rewards;

const SEED: u32 = 0;

benchmarks! {
	where_clause {  where BalanceOf<T>: FixedPointOperand }

	_{ }

	process_reward_payouts {
		let p in 1..T::PayoutSplitThreshold::get() as u32;
		for i in 0..p {
			let payout: BalanceOf<T> = 7u32.into();
			EraRemainingPayouts::<T>::mutate(|p| p.push((account("payee", i, SEED), payout)));
		}
	}: { Rewards::<T>::process_reward_payouts(0u32.into()) }
	verify {
		assert_eq!(EraRemainingPayouts::<T>::get().len(), 0);
	}

	process_zero_payouts {
	}: { Rewards::<T>::process_reward_payouts(0u32.into()) }
	verify {
		assert_eq!(EraRemainingPayouts::<T>::get().len(), 0);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{ExtBuilder, Test};
	use frame_support::assert_ok;

	#[test]
	fn process_reward_payouts() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_process_reward_payouts::<Test>());
		});
	}

	#[test]
	fn process_zero_payouts() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_process_zero_payouts::<Test>());
		});
	}
}
