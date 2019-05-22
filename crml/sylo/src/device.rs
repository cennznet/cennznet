// Copyright (C) 2019 Centrality Investments Limited
// This file is part of CENNZnet.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.
use srml_support::{dispatch::Result, dispatch::Vec, StorageMap};
extern crate srml_system as system;

const MAX_DEVICES: usize = 1000;

pub trait Trait: system::Trait {
	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event<T>() = default;
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
