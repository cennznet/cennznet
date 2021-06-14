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

//! Attestation benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::{account, benchmarks, whitelisted_caller, impl_benchmark_test_suite};
use frame_system::RawOrigin;

use crate::Module as Attestation;

const SEED: u32 = 0;

benchmarks! {
	set_claim {
		let issuer: T::AccountId = whitelisted_caller();
		let holder: T::AccountId = account("holder", 0, SEED);
		let topic = AttestationTopic::from(0xf00d);
		let value = AttestationValue::from(0xb33f);
	}: set_claim(RawOrigin::Signed(issuer.clone()), holder.clone(), topic.clone(), value.clone())
	verify {
		let issuers: Vec<<T as frame_system::Config>::AccountId> = vec![issuer.clone()];
		assert_eq!(Attestation::<T>::issuers(holder.clone()), issuers);
		assert_eq!(Attestation::<T>::topics((holder.clone(), issuer.clone())), [topic.clone()]);
		assert_eq!(Attestation::<T>::value((holder, issuer, topic)), value);
	}

	remove_claim {
		let issuer1: T::AccountId = whitelisted_caller();
		let issuer2: T::AccountId = account("issuer2", 0, SEED);
		let issuer3: T::AccountId = account("issuer3", 0, SEED);

		let holder: T::AccountId = account("holder", 0, SEED);

		let topic1 = AttestationTopic::from(0xf00d);
		let topic2 = AttestationTopic::from(0xf00e);
		let topic3 = AttestationTopic::from(0xf00f);

		let value = AttestationValue::from(0xb33f);

		let _ = Attestation::<T>::set_claim(RawOrigin::Signed(issuer2.clone()).into(), holder.clone(), topic2.clone(), value.clone());
		let _ = Attestation::<T>::set_claim(RawOrigin::Signed(issuer1.clone()).into(), holder.clone(), topic1.clone(), value.clone());
		let _ = Attestation::<T>::set_claim(RawOrigin::Signed(issuer3.clone()).into(), holder.clone(), topic3.clone(), value.clone());

	}: remove_claim(RawOrigin::Signed(issuer1.clone()), holder.clone(), topic1.clone())
	verify {
		let issuers: Vec<<T as frame_system::Config>::AccountId> = vec![issuer2.clone(), issuer3.clone()];
		assert_eq!(Attestation::<T>::issuers(holder.clone()), issuers);
		assert_ne!(Attestation::<T>::value((holder, issuer1, topic1)), value);
	}
}

impl_benchmark_test_suite!(
	Attestation,
	crate::mock::new_test_ext(),
	crate::mock::Test,
);
