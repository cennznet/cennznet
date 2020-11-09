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

//! Sylo vault benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::{benchmarks, whitelisted_caller};
use frame_system::RawOrigin;
use sp_std::boxed::Box;
use sp_std::{vec, vec::Vec};

use crate::vault::Module as SyloVault;

benchmarks! {
	_{ }

	upsert_value {
		let owner: T::AccountId = whitelisted_caller();
		let key = VaultKey::from(*b"Averylittlekeyopensaheavydoor");
		let value = VaultValue::from(*b"Ourvalueisthesumofourvalues");
	}: _(RawOrigin::Signed(owner.clone()), key.clone(), value.clone())
	verify {
		assert!(<SyloVault<T>>::values(owner).contains(&(key, value)));
	}

	delete_values {
		let owner: T::AccountId = whitelisted_caller();
		let key0 = VaultKey::from(*b"Averylittlekeyopensaheavydoor");
		let value0 = VaultValue::from(*b"Ourvalueisthesumofourvalues");
		let key1 = VaultKey::from(*b"Alittlekeyopensaheavydoor");
		let value1 = VaultValue::from(*b"Yourvalueisthesumofyourvalues");
		let key2 = VaultKey::from(*b"Averylightkeyopensaheavydoor");
		let value2 = VaultValue::from(*b"Theirvalueisthesumoftheirvalues");
		let key3 = VaultKey::from(*b"Alightkeyopensaheavydoor");
		let value3 = VaultValue::from(*b"Myvalueisthesumofmyvalues");
		let _ = <SyloVault<T>>::upsert_value(RawOrigin::Signed(owner.clone()).into(), key0.clone(), value0.clone());
		let _ = <SyloVault<T>>::upsert_value(RawOrigin::Signed(owner.clone()).into(), key1.clone(), value1.clone());
		let _ = <SyloVault<T>>::upsert_value(RawOrigin::Signed(owner.clone()).into(), key2.clone(), value2.clone());
		let _ = <SyloVault<T>>::upsert_value(RawOrigin::Signed(owner.clone()).into(), key3.clone(), value3.clone());
	}: _(RawOrigin::Signed(owner.clone()), Vec::<VaultKey>::from([key0.clone(), key2.clone()]))
	verify {
		assert!(!<SyloVault<T>>::values(owner.clone()).contains(&(key0, value0)));
		assert!(<SyloVault<T>>::values(owner.clone()).contains(&(key1, value1)));
		assert!(!<SyloVault<T>>::values(owner.clone()).contains(&(key2, value2)));
		assert!(<SyloVault<T>>::values(owner.clone()).contains(&(key3, value3)));
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{ExtBuilder, Test};
	use frame_support::assert_ok;

	#[test]
	fn upsert_value() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_upsert_value::<Test>());
		});
	}

	#[test]
	fn delete_values() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_delete_values::<Test>());
		});
	}
}
