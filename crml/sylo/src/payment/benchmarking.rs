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

//! Sylo payment benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::{account, benchmarks};
use frame_system::RawOrigin;

use crate::payment::Module as SyloPayment;

const SEED: u32 = 0;

benchmarks! {
	_{ }

	set_payment_account {
		let recipient: T::AccountId = account("recipient", 0, SEED);
	}: _(RawOrigin::Root, recipient.clone())
	verify {
		assert!(<SyloPayment<T>>::authorised_payers().contains(&recipient));
	}

	revoke_payment_account_self {
		let recipient: T::AccountId = account("recipient", 0, SEED);
		let _ = <SyloPayment<T>>::set_payment_account(RawOrigin::Root.into(), recipient.clone());
	}: _(RawOrigin::Signed(recipient.clone()))
	verify {
		assert!(!<SyloPayment<T>>::authorised_payers().contains(&recipient));
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{ExtBuilder, Test};
	use frame_support::assert_ok;

	#[test]
	fn set_payment_account() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_set_payment_account::<Test>());
		});
	}

	#[test]
	fn revoke_payment_account_self() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_revoke_payment_account_self::<Test>());
		});
	}
}
