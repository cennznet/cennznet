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

use frame_support::{decl_error, decl_module, decl_storage, dispatch::DispatchResult, dispatch::Vec, ensure};

const MAX_DEVICES: usize = 1000;

type DeviceId = u32;

pub trait Trait: frame_system::Trait {}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin, system = frame_system {
		type Error = Error<T>;
	}
}

// The data that is stored
decl_storage! {
	trait Store for Module<T: Trait> as SyloDevice {
		pub Devices get(devices): map hasher(blake2_128_concat) T::AccountId => Vec<DeviceId>;
	}
}

decl_error! {
	pub enum Error for Module<T: Trait> {
		/// There are no devices registered for user (missing user_id in Devices)
		UserIdNotRegistered,
		/// Device is already registered to user (device_id is already in use)
		DeviceIdExists,
		/// A user can't have more than MAX_DEVICES registered devices
		MaxDeviceLimitReached,
	}
}

impl<T: Trait> Module<T> {
	pub fn append_device(user_id: &T::AccountId, device_id: DeviceId) -> DispatchResult {
		let mut devices = <Devices<T>>::get(user_id);

		ensure!(!devices.contains(&device_id), Error::<T>::DeviceIdExists);
		ensure!(devices.len() < MAX_DEVICES, Error::<T>::MaxDeviceLimitReached);

		devices.push(device_id);

		<Devices<T>>::insert(user_id, devices);

		Ok(())
	}

	pub fn delete_device(user_id: &T::AccountId, device_id: DeviceId) -> DispatchResult {
		ensure!(<Devices<T>>::contains_key(user_id), Error::<T>::UserIdNotRegistered);
		let mut devices = <Devices<T>>::take(user_id);
		devices.retain(|device| *device != device_id);
		<Devices<T>>::insert(user_id, devices);
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{ExtBuilder, Test};
	use frame_support::{assert_noop, assert_ok};
	use sp_core::H256;

	type Device = Module<Test>;

	#[test]
	fn append_device_works() {
		ExtBuilder::default().build().execute_with(|| {
			let user_id = H256::from_low_u64_be(1);
			let device_id = 7357;

			assert_ok!(Device::append_device(&user_id, device_id));
			assert_eq!(Device::devices(user_id), vec![device_id]);
		});
	}

	#[test]
	fn append_duplicate_device_works() {
		ExtBuilder::default().build().execute_with(|| {
			let user_id = H256::from_low_u64_be(1);
			let device_id = 7357;

			assert_ok!(Device::append_device(&user_id, device_id));
			assert_eq!(Device::devices(user_id), vec![device_id]);

			// adding the same device should return error
			assert_noop!(
				Device::append_device(&user_id, device_id),
				Error::<Test>::DeviceIdExists
			);
		});
	}

	#[test]
	fn append_up_to_max_device_works() {
		ExtBuilder::default().build().execute_with(|| {
			let user_id = H256::from_low_u64_be(1);
			let device_id = 7357;

			// add up to MAX_DEVICES many devices for user
			let new_devices: Vec<_> = (device_id..(device_id + MAX_DEVICES as DeviceId)).collect();
			assert_eq!(new_devices.len(), 1000); // length assert is here in case MAX_DEVICES changes
			for new_device in new_devices.clone() {
				assert_ok!(Device::append_device(&user_id, new_device));
			}
			assert_eq!(Device::devices(user_id).len(), 1000);

			// adding more than MAX_DEVICES is not allowed
			assert_noop!(
				Device::append_device(&user_id, 123),
				Error::<Test>::MaxDeviceLimitReached
			);
		});
	}

	#[test]
	fn delete_device_works() {
		ExtBuilder::default().build().execute_with(|| {
			let user_id = H256::from_low_u64_be(1);
			let mut devices = vec![1, 2, 3, 4, 5];
			for device in devices.clone() {
				assert_ok!(Device::append_device(&user_id, device));
			}
			for device in devices.clone().into_iter().rev() {
				assert_ok!(Device::delete_device(&user_id, device));
				devices.pop().unwrap();
				assert_eq!(Device::devices(user_id), devices);
			}
			assert!(Device::devices(user_id).is_empty());
		});
	}

	#[test]
	fn delete_non_existing_device_works() {
		ExtBuilder::default().build().execute_with(|| {
			let user_id = H256::from_low_u64_be(1);
			let devices = vec![1, 2, 3, 4];
			for device in devices.clone() {
				assert_ok!(Device::append_device(&user_id, device));
			}
			// deleting a non-existing device should pass through without an error
			assert_ok!(Device::delete_device(&user_id, 5));
			assert_eq!(Device::devices(user_id), [1, 2, 3, 4]);
		});
	}
}
