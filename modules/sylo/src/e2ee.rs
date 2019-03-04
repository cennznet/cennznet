use srml_support::{dispatch::Vec, StorageMap};
use {balances, inbox, response, groups, device, system::ensure_signed};

const MAX_PKBS: usize = 50;

pub trait Trait: balances::Trait + inbox::Trait + response::Trait + device::Trait + groups::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

type DeviceId = u32;

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

		fn register_device(origin, device_id: u32, pkbs: Vec<PreKeyBundle>) {
			let sender = ensure_signed(origin)?;

			let current_pkbs = <PreKeyBundles<T>>::get((sender.clone(), device_id));
			ensure!((current_pkbs.len() + pkbs.len()) <= MAX_PKBS, "User can not store more than maximum number of pkbs");

			<device::Module<T>>::append_device(&sender, device_id)?;

			let user_groups = <groups::Memberships<T>>::get(&sender);
			for group_id in user_groups {
				<groups::Module<T>>::append_member_device(&group_id, sender.clone(), device_id);
			}

			<PreKeyBundles<T>>::mutate((sender, device_id), |current_pkbs| current_pkbs.extend(pkbs));
		}

		fn replenish_pkbs(origin, device_id: u32, pkbs: Vec<PreKeyBundle>) {
			let sender = ensure_signed(origin)?;

			let current_pkbs = <PreKeyBundles<T>>::get((sender.clone(), device_id));
			ensure!((current_pkbs.len() + pkbs.len()) <= MAX_PKBS, "User can not store more than maximum number of pkbs");

			<PreKeyBundles<T>>::mutate((sender, device_id), |current_pkbs| current_pkbs.extend(pkbs));
		}

		fn withdraw_pkbs(origin, request_id: T::Hash, wanted_pkbs: Vec<(T::AccountId, DeviceId)>) {
			let sender = ensure_signed(origin)?;

			let acquired_pkbs: Vec<(T::AccountId, DeviceId, PreKeyBundle)> = wanted_pkbs
				.into_iter()
				.filter_map(|wanted_pkb| {
					let mut pkbs = <PreKeyBundles<T>>::get(&wanted_pkb);

					pkbs.pop().map(|retrieved_pkb| {
						<PreKeyBundles<T>>::insert(&wanted_pkb, pkbs);
						(wanted_pkb.0, wanted_pkb.1, retrieved_pkb)
					})
				})
				.collect();

			<response::Module<T>>::set_response(sender, request_id, response::Response::PreKeyBundles(acquired_pkbs));
		}
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as SyloE2EE {
		PreKeyBundles get(pkbs): map (T::AccountId, DeviceId) => Vec<PreKeyBundle>;
	}
}

impl<T: Trait> Module<T> {}
