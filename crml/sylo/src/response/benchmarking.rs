// Copyright 2019-2020 Plug New Zealand Limited
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

//! Sylo response benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use crate::response::Module as SyloResponse;
use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_system::RawOrigin;
use sp_std::boxed::Box;
use sp_std::{vec, vec::Vec};
const SEED: u32 = 0;

benchmarks! {
	_{ }

	remove_response {
		let sender: T::AccountId = whitelisted_caller();
		let recipient: T::AccountId = account("recipient", 0, SEED);
		let request_id: T::Hash = Default::default();

		let mut bundles = Vec::<(T::AccountId, u32, Vec<u8>)>::new();
		bundles.push((recipient, 2, Vec::<u8>::new()));

		let resp_pkb = Response::<T::AccountId>::PreKeyBundles(bundles);

		let _ = <SyloResponse<T>>::set_response(sender.clone(), request_id, resp_pkb);
	}: _(RawOrigin::Signed(sender.clone()), request_id)
	verify {
		assert!(<SyloResponse<T>>::response((sender, request_id)) == Response::None);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{ExtBuilder, Test};
	use frame_support::assert_ok;

	#[test]
	fn remove_response() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_remove_response::<Test>());
		});
	}
}
