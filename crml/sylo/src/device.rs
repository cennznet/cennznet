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

use frame_support::{decl_event, decl_module, decl_storage, dispatch::DispatchResult, dispatch::Vec, ensure};
use frame_system;

const MAX_DEVICES: usize = 1000;

pub trait Trait: frame_system::Trait {
	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin, system = frame_system {
		fn deposit_event() = default;
	}
}

// The data that is stored
decl_storage! {
	trait Store for Module<T: Trait> as SyloDevice {
		pub Devices get(devices): map hasher(blake2_256) T::AccountId => Vec<u32>;
	}
}

decl_event!(
	pub enum Event<T> where <T as frame_system::Trait>::Hash, <T as frame_system::Trait>::AccountId {
		DeviceAdded(AccountId, Hash, u32),
	}
);

impl<T: Trait> Module<T> {
	pub fn append_device(user_id: &T::AccountId, device_id: u32) -> DispatchResult {
		let mut devices = <Devices<T>>::get(user_id);

		ensure!(!devices.contains(&device_id), "Device Id already in use");
		ensure!(
			devices.len() <= MAX_DEVICES,
			"User has registered up to the maximum number of devices"
		);

		devices.push(device_id);

		<Devices<T>>::insert(user_id, devices);

		Ok(())
	}
}
