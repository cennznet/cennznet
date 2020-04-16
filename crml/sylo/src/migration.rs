/* Copyright 2020 Centrality Investments Limited
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

use crate::{
	device::{self, DeviceId},
	groups,
	inbox::{self, Message, MessageId},
	vault,
};
use frame_support::{decl_error, decl_module, decl_storage, dispatch::Vec, ensure, weights::SimpleDispatchInfo};
use frame_system::{ensure_root, ensure_signed};
use sp_runtime::{DispatchError::BadOrigin, DispatchResult};

pub trait Trait: device::Trait + groups::Trait + inbox::Trait + vault::Trait {}

decl_error! {
	pub enum Error for Module<T: Trait> {
		MaxDeviceLimitReached,
		MaxInboxLimitReached,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as SyloMigration {
		MigrationAccount: T::AccountId;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin, system = frame_system {
		type Error = Error<T>;

		#[weight = SimpleDispatchInfo::FixedOperational(0)]
		pub fn set_migrator_account(origin, account_id: T::AccountId) -> DispatchResult {
			ensure_root(origin)?;
			MigrationAccount::<T>::put(account_id);
			Ok(())
		}

		#[weight = SimpleDispatchInfo::FixedOperational(0)]
		pub fn self_destruct(origin) -> DispatchResult {
			Self::ensure_sylo_migrator(origin)?;
			MigrationAccount::<T>::kill();
			Ok(())
		}

		#[weight = SimpleDispatchInfo::FixedOperational(0)]
		fn migrate_devices(origin, user_id: T::AccountId, device_ids: Vec<DeviceId>) -> DispatchResult {
			Self::ensure_sylo_migrator(origin)?;
			ensure!(device_ids.len() <= device::MAX_DEVICES, Error::<T>::MaxDeviceLimitReached);

			let mut devices = <device::Devices<T>>::get(user_id.clone());
			ensure!(devices.len() + device_ids.len() <= device::MAX_DEVICES, Error::<T>::MaxDeviceLimitReached);

			for device_id in device_ids {
				if !devices.contains(&device_id) {
					devices.push(device_id);
				}
			}

			<device::Devices<T>>::insert(user_id, devices);

			Ok(())
		}

		#[weight = SimpleDispatchInfo::FixedOperational(0)]
		fn migrate_inbox(origin, user_id: T::AccountId, next_index: MessageId, new_messages: Vec<(MessageId, Message)>) -> DispatchResult {
			Self::ensure_sylo_migrator(origin)?;

			let mut existing_messages = <inbox::Values<T>>::get(&user_id).clone();
			let (existing_indexes, _): (Vec<MessageId>, Vec<_>) = existing_messages.clone().into_iter().unzip();

			// For repeatability, we update the existing messages that are assumed to be migrated already.
			for (new_index, new_message) in new_messages {
				if !existing_indexes.contains(&new_index) {
					existing_messages.push((new_index, new_message));
				}
			}

			ensure!(existing_messages.len() as u32 <= u32::max_value(), Error::<T>::MaxInboxLimitReached);

			<inbox::Values<T>>::insert(&user_id, existing_messages);
			<inbox::NextIndexes<T>>::insert(&user_id, next_index);
			Ok(())
		}
	}
}

impl<T: Trait> Module<T> {
	pub fn ensure_sylo_migrator(origin: T::Origin) -> DispatchResult {
		let account_id = ensure_signed(origin)?;
		ensure!(MigrationAccount::<T>::get() == account_id, BadOrigin);
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{ExtBuilder, Origin, Test};
	use frame_support::assert_ok;
	use sp_core::H256;
	use sp_runtime::DispatchError::BadOrigin;

	impl Trait for Test {}
	type Migration = Module<Test>;
	type Device = device::Module<Test>;
	type Inbox = inbox::Module<Test>;

	#[test]
	fn set_migration_account_works() {
		ExtBuilder::default().build().execute_with(|| {
			let migration_account = H256::from_low_u64_be(2);

			assert_ok!(Migration::set_migrator_account(Origin::ROOT, migration_account));

			assert_ok!(Migration::ensure_sylo_migrator(Origin::signed(migration_account)));
		});
	}

	#[test]
	fn wrong_migration_account_fails_ensure() {
		ExtBuilder::default().build().execute_with(|| {
			let migration_account = H256::from_low_u64_be(2);
			let invalid_account = H256::from_low_u64_be(3);

			assert_ok!(Migration::set_migrator_account(Origin::ROOT, migration_account));

			assert_eq!(
				Migration::ensure_sylo_migrator(Origin::signed(invalid_account)),
				Err(BadOrigin)
			);
		});
	}

	#[test]
	fn no_migration_account_fails_ensure() {
		ExtBuilder::default().build().execute_with(|| {
			let migration_account = H256::from_low_u64_be(2);

			assert_eq!(
				Migration::ensure_sylo_migrator(Origin::signed(migration_account)),
				Err(BadOrigin)
			);
		});
	}

	#[test]
	fn remove_migration_account_works() {
		ExtBuilder::default().build().execute_with(|| {
			let migration_account = H256::from_low_u64_be(2);

			assert_ok!(Migration::set_migrator_account(Origin::ROOT, migration_account));

			assert_ok!(Migration::self_destruct(Origin::signed(migration_account)));

			assert_eq!(
				Migration::ensure_sylo_migrator(Origin::signed(migration_account)),
				Err(BadOrigin)
			);
		});
	}

	#[test]
	fn remove_migration_account_with_invalid_account_fails() {
		ExtBuilder::default().build().execute_with(|| {
			let migration_account = H256::from_low_u64_be(2);
			let invalid_account = H256::from_low_u64_be(3);

			assert_ok!(Migration::set_migrator_account(Origin::ROOT, migration_account));

			assert_eq!(
				Migration::self_destruct(Origin::signed(invalid_account)),
				Err(BadOrigin)
			);

			assert_ok!(Migration::ensure_sylo_migrator(Origin::signed(migration_account)));
		});
	}

	#[test]
	fn migrate_devices_works() {
		ExtBuilder::default().build().execute_with(|| {
			let user_id = H256::from_low_u64_be(1);
			let devices = vec![1, 2, 3, 4];
			let migration_account = H256::from_low_u64_be(2);

			assert_ok!(Migration::set_migrator_account(Origin::ROOT, migration_account));

			assert_ok!(Migration::migrate_devices(
				Origin::signed(migration_account),
				user_id.clone(),
				devices.clone()
			));

			assert_eq!(Device::devices(user_id), [1, 2, 3, 4]);
		});
	}

	#[test]
	fn migrate_devices_does_not_double_up() {
		ExtBuilder::default().build().execute_with(|| {
			let user_id = H256::from_low_u64_be(1);
			let devices = vec![1, 2, 3, 4];
			let migration_account = H256::from_low_u64_be(2);

			assert_ok!(Migration::set_migrator_account(Origin::ROOT, migration_account));

			assert_ok!(Device::append_device(&user_id, 3));

			assert_ok!(Migration::migrate_devices(
				Origin::signed(migration_account),
				user_id.clone(),
				devices.clone()
			));

			assert_eq!(Device::devices(user_id), [3, 1, 2, 4]);
		});
	}

	#[test]
	fn migrate_devices_fails_with_bad_account() {
		ExtBuilder::default().build().execute_with(|| {
			let user_id = H256::from_low_u64_be(1);
			let devices = vec![1, 2, 3, 4];
			let migration_account = H256::from_low_u64_be(2);
			let invalid_account = H256::from_low_u64_be(3);

			assert_ok!(Migration::set_migrator_account(Origin::ROOT, migration_account));

			assert_eq!(
				Migration::migrate_devices(Origin::signed(invalid_account), user_id.clone(), devices.clone()),
				Err(BadOrigin)
			);

			assert_eq!(Device::devices(user_id), []);
		});
	}

	#[test]
	fn migrate_inbox_works() {
		ExtBuilder::default().build().execute_with(|| {
			let user_id = H256::from_low_u64_be(1);
			let messages = vec![
				(0, b"test0".to_vec()),
				(1, b"test1".to_vec()),
				(2, b"test2".to_vec()),
				(3, b"test3".to_vec()),
			];
			let next_index = 7357;
			let migration_account = H256::from_low_u64_be(2);

			assert_ok!(Migration::set_migrator_account(Origin::ROOT, migration_account));
			assert_ok!(Migration::migrate_inbox(
				Origin::signed(migration_account),
				user_id.clone(),
				next_index.clone(),
				messages.clone()
			));

			assert_eq!(Inbox::values(user_id), messages);
			assert_eq!(inbox::NextIndexes::<Test>::get(user_id), next_index);
		});
	}

	#[test]
	fn migrate_inbox_works_with_existing_messages() {
		ExtBuilder::default().build().execute_with(|| {
			let user_id = H256::from_low_u64_be(1);
			let next_index = 7357;
			let existing_messages = vec![
				b"test0".to_vec(),
				b"test1".to_vec(),
				b"test2".to_vec(),
				b"test3".to_vec(),
			];
			for message in existing_messages {
				assert_ok!(Inbox::add(user_id, message));
			}

			let migration_account = H256::from_low_u64_be(2);
			let new_messages = vec![
				(0, b"different_test_message_0".to_vec()),
				(1, b"different_test_message_1".to_vec()),
				(2, b"different_test_message_2".to_vec()),
				(3, b"test3".to_vec()),
				(4, b"test4".to_vec()),
				(5, b"test4".to_vec()),
			];

			assert_ok!(Migration::set_migrator_account(Origin::ROOT, migration_account));
			assert_ok!(Migration::migrate_inbox(
				Origin::signed(migration_account),
				user_id.clone(),
				next_index.clone(),
				new_messages.clone()
			));

			let current_messages = vec![
				(0, b"test0".to_vec()), // existing data untouched
				(1, b"test1".to_vec()), // existing data untouched
				(2, b"test2".to_vec()), // existing data untouched
				(3, b"test3".to_vec()),
				(4, b"test4".to_vec()),
				(5, b"test4".to_vec()),
			];
			assert_eq!(Inbox::values(user_id), current_messages);
			assert_eq!(inbox::NextIndexes::<Test>::get(user_id), next_index);
		});
	}
}
