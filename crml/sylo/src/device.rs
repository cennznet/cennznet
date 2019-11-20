// Copyright 2019 Centrality Investments Limited
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

use system;
use support::{decl_module, decl_storage, decl_event, ensure, dispatch::Result, dispatch::Vec};

const MAX_DEVICES: usize = 1000;

pub trait Trait: system::Trait {
	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;
	}
}

// The data that is stored
decl_storage! {
	trait Store for Module<T: Trait> as SyloDevice {
		pub Devices get(devices): map T::AccountId => Vec<u32>;
	}
}

decl_event!(
	pub enum Event<T> where <T as system::Trait>::Hash, <T as system::Trait>::AccountId {
		DeviceAdded(AccountId, Hash, u32),
	}
);

impl<T: Trait> Module<T> {
	pub fn append_device(user_id: &T::AccountId, device_id: u32) -> Result {
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
