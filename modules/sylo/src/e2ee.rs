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

const MAX_PKBS: usize = 50;

pub trait Trait: balances::Trait + inbox::Trait + response::Trait + device::Trait + groups::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

// Serialized pre key bundle used to establish one to one e2ee
pub type PreKeyBundle = Vec<u8>;

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
		fn register_device(origin, device_id: u32, pkbs: Vec<PreKeyBundle>) -> Result {
			let sender = ensure_signed(origin)?;

			let result = <device::Module<T>>::append_device(&sender, device_id);

			match result {
				Ok(()) => {
					let user_groups = <groups::Module<T>>::get_users_groups(&sender);
					for group_id in user_groups {
						<groups::Module<T>>::append_member_device(&group_id, sender.clone(), device_id);
					}
					Self::store_pkbs(sender.clone(), device_id, pkbs)
				},
				Err(error) => Err(error)
			}
		}

		fn replenish_pkbs(origin, device_id: u32, pkbs: Vec<PreKeyBundle>) -> Result {
			let sender = ensure_signed(origin)?;

			Self::store_pkbs(sender, device_id, pkbs)
		}

		fn withdraw_pkbs(origin, request_id: T::Hash, wanted_pkbs: Vec<(T::AccountId, u32 /* device id */)>) -> Result {
			let sender = ensure_signed(origin)?;

			let acquired_pkbs: Vec<(T::AccountId, u32, PreKeyBundle)> = wanted_pkbs
				.into_iter()
				.filter_map(|wanted_pkb| {
					// retrieve set of pre key bundles for (user, deviceId)
					let mut pkbs = <PreKeyBundles<T>>::get(&wanted_pkb);

					match pkbs.pop() {
						Some(retrieved_pkb) => {
							<PreKeyBundles<T>>::insert(&wanted_pkb, pkbs);
							return Some((wanted_pkb.0, wanted_pkb.1, retrieved_pkb))
						}
						None => None
					}
				})
				.collect();

			<response::Module<T>>::set_response(sender, request_id, response::Response::PreKeyBundles(acquired_pkbs));
			Ok(())
		}
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as SyloE2EE {
		/* PreKeyBundles */
		PreKeyBundles get(pkbs): map (T::AccountId, u32 /* device_id */) => Vec<PreKeyBundle>;
	}
}

impl<T: Trait> Module<T> {
	fn store_pkbs(account_id: T::AccountId, device_id: u32, pkbs: Vec<PreKeyBundle>) -> Result {
		let mut current_pkbs = <PreKeyBundles<T>>::get((account_id.clone(), device_id));

		ensure!((current_pkbs.len() + pkbs.len()) <= MAX_PKBS, "User can not store more than maximum number of pkbs");

		current_pkbs.extend(pkbs);

		<PreKeyBundles<T>>::insert((account_id, device_id), current_pkbs);

		Ok(())
	}
}
