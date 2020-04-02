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

use frame_support::{decl_error, decl_module, decl_storage, dispatch::Vec, ensure};
use frame_system::ensure_signed;

pub const MAX_KEYS: usize = 100;
const MAX_VALUE_LENGTH: usize = 100_000;
const MAX_DELETE_KEYS: usize = 100;

pub trait Trait: frame_system::Trait {}

pub type VaultKey = Vec<u8>;
pub type VaultValue = Vec<u8>;

decl_error! {
	pub enum Error for Module<T: Trait> {
		/// Cannot store more than MAX_KEYS
		MaxKeys,
		/// Cannot store value larger than MAX_VALUE_LENGTH
		MaxValueLength,
		/// Cannot delete more than MAX_DELETE_KEYS at a time
		MaxDeleteKeys,
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin, system = frame_system {
		type Error = Error<T>;

		fn upsert_value(origin, key: VaultKey, value: VaultValue) {
			let user_id = ensure_signed(origin)?;
			ensure!(value.len() <= MAX_VALUE_LENGTH, Error::<T>::MaxValueLength);
			ensure!(<Vault<T>>::get(&user_id).len() < MAX_KEYS, Error::<T>::MaxKeys);
			Self::upsert(user_id, key, value);
		}

		fn delete_values(origin, keys: Vec<VaultKey>) {
			let user_id = ensure_signed(origin)?;
			ensure!(keys.len() <= MAX_DELETE_KEYS, Error::<T>::MaxDeleteKeys);
			Self::delete(user_id, keys);
		}
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as SyloVault {
		pub Vault get(values): map hasher(blake2_128_concat) T::AccountId => Vec<(VaultKey, VaultValue)>;
	}
}

impl<T: Trait> Module<T> {
	pub fn upsert(user_id: T::AccountId, key: VaultKey, value: VaultValue) {
		let mut values = <Vault<T>>::get(&user_id);

		match values.iter().enumerate().find(|(_, item)| item.0 == key) {
			None => values.push((key, value)),
			Some((i, _)) => values[i] = (key, value),
		}

		<Vault<T>>::insert(user_id, values)
	}

	pub fn delete(user_id: T::AccountId, keys: Vec<VaultKey>) {
		let remaining_values: Vec<(VaultKey, VaultValue)> = <Vault<T>>::get(&user_id)
			.into_iter()
			.filter(|item| keys.iter().find(|key_to_remove| &&item.0 == key_to_remove).is_none())
			.collect();

		<Vault<T>>::insert(user_id, remaining_values)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{ExtBuilder, Origin, Test};
	use frame_support::{assert_noop, assert_ok};
	use sp_core::H256;

	impl Trait for Test {}
	type Vault = Module<Test>;

	#[test]
	fn should_upsert_values() {
		ExtBuilder.build().execute_with(|| {
			let key_0 = b"0".to_vec();
			let value_0 = b"1".to_vec();

			assert_ok!(Vault::upsert_value(
				Origin::signed(H256::from_low_u64_be(1)),
				key_0.clone(),
				value_0.clone()
			));

			assert_eq!(
				Vault::values(H256::from_low_u64_be(1)),
				vec![(key_0.clone(), value_0.clone())]
			);

			let key_1 = b"01".to_vec();
			let value_1 = b"10".to_vec();

			assert_ok!(Vault::upsert_value(
				Origin::signed(H256::from_low_u64_be(1)),
				key_1.clone(),
				value_1.clone()
			));

			assert_eq!(
				Vault::values(H256::from_low_u64_be(1)),
				vec![(key_0, value_0), (key_1, value_1)]
			);
		});
	}

	#[test]
	fn should_replace_existing_keys() {
		ExtBuilder.build().execute_with(|| {
			let key_0 = b"0".to_vec();
			let value_0 = b"1".to_vec();
			let value_1 = b"01".to_vec();

			assert_ok!(Vault::upsert_value(
				Origin::signed(H256::from_low_u64_be(1)),
				key_0.clone(),
				value_0.clone()
			));

			assert_eq!(Vault::values(H256::from_low_u64_be(1)), vec![(key_0.clone(), value_0)]);

			assert_ok!(Vault::upsert_value(
				Origin::signed(H256::from_low_u64_be(1)),
				key_0.clone(),
				value_1.clone()
			));

			assert_eq!(Vault::values(H256::from_low_u64_be(1)), vec![(key_0, value_1)]);
		});
	}

	#[test]
	fn should_delete_keys() {
		ExtBuilder.build().execute_with(|| {
			let key_0 = b"0".to_vec();
			let key_1 = b"1".to_vec();
			let value_0 = b"01".to_vec();

			assert_ok!(Vault::upsert_value(
				Origin::signed(H256::from_low_u64_be(1)),
				key_0.clone(),
				value_0.clone()
			));

			assert_ok!(Vault::upsert_value(
				Origin::signed(H256::from_low_u64_be(1)),
				key_1.clone(),
				value_0.clone()
			));

			assert_eq!(
				Vault::values(H256::from_low_u64_be(1)),
				vec![(key_0.clone(), value_0.clone()), (key_1.clone(), value_0)]
			);

			assert_ok!(Vault::delete_values(
				Origin::signed(H256::from_low_u64_be(1)),
				vec![key_0, key_1]
			));

			assert_eq!(Vault::values(H256::from_low_u64_be(1)), vec![]);
		});
	}

	#[test]
	fn should_not_add_more_than_max_keys() {
		ExtBuilder.build().execute_with(|| {
			let user_id = H256::from_low_u64_be(1);
			for i in 0..MAX_KEYS {
				let key = format!("key_{}", i).into_bytes();
				let value = format!("value_{}", i).into_bytes();
				assert_ok!(Vault::upsert_value(Origin::signed(user_id), key, value));
			}
			assert_eq!(Vault::values(user_id).len(), 100);

			// an attempt to add another item to Vault should fail
			assert_noop!(
				Vault::upsert_value(Origin::signed(user_id), b"new_key".to_vec(), b"new_value".to_vec()),
				Error::<Test>::MaxKeys,
			);
		});
	}
}
