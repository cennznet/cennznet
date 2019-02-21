// Needed for tests (`with_externalities`).
#[cfg(test)]
extern crate sr_io;

extern crate substrate_primitives;
// Needed for various traits. In our case, `OnFinalise`.
extern crate sr_primitives;

// `system` module provides us with all sorts of useful stuff and macros
// depend on it being around.
extern crate srml_system as system;

extern crate parity_codec;

use srml_support::{dispatch::Result, dispatch::Vec, StorageMap};
use {balances, inbox, response, groups, device, system::ensure_signed};

pub trait Trait: balances::Trait + inbox::Trait + response::Trait + device::Trait + groups::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

// Serialized pre key bundle used to establish one to one e2ee
pub type PKB = Vec<u8>;

decl_event!(
	pub enum Event<T> where <T as system::Trait>::AccountId {
		DeviceAdded(AccountId, u32),
	}
);

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event<T>() = default;

		// Registers a new device id for e2ee
		// request_id is used to identify the assigned device id
		fn register_device(origin, device_id: u32, pkbs: Vec<PKB>) -> Result {
			let sender = ensure_signed(origin)?;

			let result = <device::Module<T>>::append_device(sender.clone(), device_id);

			match result {
				Ok(()) => {
					Self::store_pkbs(sender, device_id, pkbs);
					Ok(())
				},
				Err(error) => Err(error)
			}
		}

		fn replenish_pkbs(origin, device_id: u32, pkbs: Vec<PKB>) -> Result {
			let sender = ensure_signed(origin)?;

			Self::store_pkbs(sender, device_id, pkbs);

			Ok(())
		}

		fn withdraw_pkbs(origin, request_id: T::Hash, wanted_pkbs: Vec<(T::AccountId, u32)>) -> Result {
			let sender = ensure_signed(origin)?;

			let acquired_pkbs: Vec<(T::AccountId, u32, PKB)> = wanted_pkbs
				.into_iter()
				.filter_map(|wanted_pkb| {
					// retrieve set of pre key bundles for (user, deviceId)
					let mut pkbs = <PKBs<T>>::get(wanted_pkb.clone());

					match pkbs.pop() {
						Some(retrieved_pkb) => {
							<PKBs<T>>::insert(wanted_pkb.clone(), pkbs);
							return Some((wanted_pkb.0, wanted_pkb.1, retrieved_pkb))
						}
						None => None
					}
				})
				.collect();

			<response::Module<T>>::set_response(sender, request_id, response::Response::Pkb(acquired_pkbs));
			Ok(())
		}
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as SyloE2EE {
		/* PKBs */
		PKBs get(pkbs): map (T::AccountId, u32 /* device_id */) => Vec<PKB>;
	}
}

impl<T: Trait> Module<T> {
	fn store_pkbs(account_id: T::AccountId, device_id: u32, pkbs: Vec<PKB>) {
		let mut current_pkbs = <PKBs<T>>::get((account_id.clone(), device_id.clone()));

		current_pkbs.extend(pkbs);

		<PKBs<T>>::insert((account_id, device_id), current_pkbs);
	}
}
