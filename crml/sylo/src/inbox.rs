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

use frame_support::{decl_module, decl_storage, dispatch::DispatchResult, dispatch::Vec};
use frame_system::{self, ensure_signed};

pub trait Trait: frame_system::Trait {
	// add code here
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin, system = frame_system {
		fn add_value(origin, peer_id: T::AccountId, value: Vec<u8>) -> DispatchResult {
			ensure_signed(origin)?;

			Self::add(peer_id, value)
		}

		fn delete_values(origin, value_ids: Vec<u32>) -> DispatchResult {
			let user_id = ensure_signed(origin)?;

			Self::delete(user_id, value_ids)
		}
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as SyloInbox {
		NextIndexes: map hasher(blake2_128_concat) T::AccountId => u32;
		AccountValues: map hasher(blake2_128_concat) T::AccountId => Vec<(T::AccountId, u32)>;
		Values get(values): map hasher(blake2_128_concat) T::AccountId => Vec<(u32, Vec<u8>)>;
	}
}

impl<T: Trait> Module<T> {
	pub fn inbox(who: T::AccountId) -> Vec<Vec<u8>> {
		<Values<T>>::get(who).into_iter().map(|(_, value)| value).collect()
	}

	pub fn add(peer_id: T::AccountId, value: Vec<u8>) -> DispatchResult {
		// Get required data
		let next_index = <NextIndexes<T>>::get(&peer_id);
		let mut account_values = <AccountValues<T>>::get(&peer_id);

		// Add new mapping to account values
		account_values.push((peer_id.clone(), next_index));

		// Store data
		let mut values = <Values<T>>::get(&peer_id);
		if let Some((i, _)) = values.iter().enumerate().find(|(_, item)| item.0 == next_index) {
			values[i] = (next_index, value);
		} else {
			values.push((next_index, value));
		}
		<Values<T>>::insert(peer_id.clone(), values);
		<AccountValues<T>>::insert(&peer_id, account_values);

		// Update next_index
		<NextIndexes<T>>::insert(&peer_id, next_index + 1);

		Ok(())
	}

	pub fn delete(user_id: T::AccountId, value_ids: Vec<u32>) -> DispatchResult {
		let account_values = <AccountValues<T>>::get(&user_id);

		// Remove reference to value
		let account_values: Vec<(T::AccountId, u32)> = account_values
			.into_iter()
			.filter(|account_value| !value_ids.contains(&account_value.1))
			.collect();

		let mut values = <Values<T>>::get(&user_id);
		for id in value_ids {
			// Remove value from storage
			if let Some(index) = values.iter().position(|(x, _)| *x == id) {
				values.remove(index);
			}
		}
		<Values<T>>::insert(user_id.clone(), values);

		// Update account reference values
		<AccountValues<T>>::insert(&user_id, account_values);

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{ExtBuilder, Origin, Test};
	use frame_support::assert_ok;
	use sp_core::H256;

	type Inbox = Module<Test>;

	#[test]
	fn it_works_adding_values_to_an_inbox() {
		ExtBuilder.build().execute_with(|| {
			// Add a value to an empty inbox
			assert_ok!(Inbox::add_value(
				Origin::signed(H256::from_low_u64_be(1)),
				H256::from_low_u64_be(2),
				b"hello, world".to_vec()
			));
			assert_eq!(Inbox::inbox(H256::from_low_u64_be(2)), vec![b"hello, world".to_vec()]);

			// Add another value
			assert_ok!(Inbox::add_value(
				Origin::signed(H256::from_low_u64_be(1)),
				H256::from_low_u64_be(2),
				b"sylo".to_vec()
			));
			assert_eq!(
				Inbox::inbox(H256::from_low_u64_be(2)),
				vec![b"hello, world".to_vec(), b"sylo".to_vec()]
			);
		});
	}

	#[test]
	fn it_works_removing_values_from_an_inbox() {
		ExtBuilder.build().execute_with(|| {
			// Add values to an empty inbox
			assert_ok!(Inbox::add_value(
				Origin::signed(H256::from_low_u64_be(1)),
				H256::from_low_u64_be(2),
				b"hello, world".to_vec()
			));
			assert_ok!(Inbox::add_value(
				Origin::signed(H256::from_low_u64_be(1)),
				H256::from_low_u64_be(2),
				b"sylo".to_vec()
			));
			assert_ok!(Inbox::add_value(
				Origin::signed(H256::from_low_u64_be(1)),
				H256::from_low_u64_be(2),
				b"foo".to_vec()
			));
			assert_ok!(Inbox::add_value(
				Origin::signed(H256::from_low_u64_be(1)),
				H256::from_low_u64_be(2),
				b"bar".to_vec()
			));

			// Remove a single value
			assert_ok!(Inbox::delete_values(Origin::signed(H256::from_low_u64_be(2)), vec![0]));
			assert_eq!(
				Inbox::inbox(H256::from_low_u64_be(2)),
				vec![b"sylo".to_vec(), b"foo".to_vec(), b"bar".to_vec()]
			);

			assert_ok!(Inbox::delete_values(
				Origin::signed(H256::from_low_u64_be(2)),
				vec![2, 3]
			));
			assert_eq!(Inbox::inbox(H256::from_low_u64_be(2)), vec![b"sylo".to_vec()]);
		});
	}

	#[test]
	fn it_works_removing_values_from_an_empty_inbox() {
		ExtBuilder.build().execute_with(|| {
			// Remove a value that doesn't exist
			assert_ok!(Inbox::delete_values(Origin::signed(H256::from_low_u64_be(2)), vec![0]));
		});
	}
}
