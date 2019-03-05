use srml_support::{dispatch::Result, dispatch::Vec, StorageMap};
extern crate srml_system as system;

#[cfg(test)]
extern crate sr_primitives;

#[cfg(test)]
extern crate sr_io;

#[cfg(test)]
extern crate substrate_primitives;

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
