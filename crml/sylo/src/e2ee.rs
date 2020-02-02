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

use frame_support::{decl_event, decl_module, decl_storage, dispatch::Vec, ensure};
use frame_system::{self, ensure_signed};

use crate::{device, groups, inbox, response};

const MAX_PKBS: usize = 50;

pub trait Trait: inbox::Trait + response::Trait + device::Trait + groups::Trait {
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
}

type DeviceId = u32;

// Serialized pre key bundle used to establish one to one e2ee
pub type PreKeyBundle = Vec<u8>;

decl_event!(
	pub enum Event<T> where <T as frame_system::Trait>::AccountId {
		DeviceAdded(AccountId, u32),
	}
);

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin, system = frame_system {
		fn deposit_event() = default;

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

#[cfg(test)]
pub(super) mod tests {
	use super::*;
	use crate::mock::{new_test_ext, Origin, Test};
	use frame_support::assert_ok;
	use sp_core::H256;

	impl Trait for Test {
		type Event = ();
	}
	impl device::Trait for Test {
		type Event = ();
	}
	impl inbox::Trait for Test {}
	impl response::Trait for Test {}
	impl groups::Trait for Test {}
	type E2EE = Module<Test>;
	type Device = device::Module<Test>;
	type Response = response::Module<Test>;

	#[test]
	fn should_add_device() {
		new_test_ext().execute_with(|| {
			assert_ok!(E2EE::register_device(
				Origin::signed(H256::from_low_u64_be(1)),
				0,
				vec![]
			));
			assert_eq!(Device::devices(H256::from_low_u64_be(1)).len(), 1);

			assert_ok!(E2EE::register_device(
				Origin::signed(H256::from_low_u64_be(1)),
				1,
				vec![]
			));
			assert_eq!(Device::devices(H256::from_low_u64_be(1)).len(), 2);
			assert_eq!(Device::devices(H256::from_low_u64_be(1))[1], 1);
		});
	}

	#[test]
	fn should_replenish_pkbs() {
		new_test_ext().execute_with(|| {
			assert_ok!(E2EE::register_device(
				Origin::signed(H256::from_low_u64_be(1)),
				0,
				vec![]
			));

			let mock_pkb = b"10".to_vec();

			assert_ok!(E2EE::replenish_pkbs(
				Origin::signed(H256::from_low_u64_be(1)),
				0,
				vec![mock_pkb.clone()]
			));

			assert_eq!(E2EE::pkbs((H256::from_low_u64_be(1), 0)), vec![mock_pkb]);
		});
	}

	#[test]
	fn should_withdraw_pkbs() {
		new_test_ext().execute_with(|| {
			assert_ok!(E2EE::register_device(
				Origin::signed(H256::from_low_u64_be(1)),
				0,
				vec![]
			));

			let mock_pkb = b"10".to_vec();

			assert_ok!(E2EE::replenish_pkbs(
				Origin::signed(H256::from_low_u64_be(1)),
				0,
				vec![mock_pkb.clone()]
			));

			let req_id = H256::from([3; 32]);
			let wanted_pkbs = vec![(H256::from_low_u64_be(1), 0)];

			assert_ok!(E2EE::withdraw_pkbs(
				Origin::signed(H256::from_low_u64_be(2)),
				req_id.clone(),
				wanted_pkbs
			));

			assert_eq!(
				Response::response((H256::from_low_u64_be(2), req_id)),
				response::Response::PreKeyBundles(vec![(H256::from_low_u64_be(1), 0, mock_pkb)])
			);
		});
	}
}
