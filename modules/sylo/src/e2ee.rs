use srml_support::{dispatch::Vec, StorageMap};
use {device, groups, inbox, response, system, system::ensure_signed};

extern crate sr_primitives;
extern crate sr_io;
extern crate substrate_primitives;

const MAX_PKBS: usize = 50;

pub trait Trait: inbox::Trait + response::Trait + device::Trait + groups::Trait {
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

#[cfg(test)]
pub(super) mod tests {
	use super::*;

	use self::sr_io::with_externalities;
	use self::substrate_primitives::{Blake2Hasher, H256};
	// The testing primitives are very useful for avoiding having to work with signatures
	// or public keys. `u64` is used as the `AccountId` and no `Signature`s are requried.
	use self::sr_primitives::{
		testing::{Digest, DigestItem, Header},
		traits::{BlakeTwo256, IdentityLookup},
		BuildStorage,
	};

	impl_outer_origin! {
		pub enum Origin for Test {}
	}

	// For testing the module, we construct most of a mock runtime. This means
	// first constructing a configuration type (`Test`) which `impl`s each of the
	// configuration traits of modules we want to use.
	#[derive(Clone, Eq, PartialEq)]
	pub struct Test;
	impl system::Trait for Test {
		type Origin = Origin;
		type Index = u64;
		type BlockNumber = u64;
		type Hash = H256;
		type Hashing = BlakeTwo256;
		type Digest = Digest;
		type AccountId = H256;
		type Lookup = IdentityLookup<H256>;
		type Header = Header;
		type Event = ();
		type Log = DigestItem;
	}
	impl Trait for Test {
		type Event = ();
	}
	impl device::Trait for Test {
		type Event = ();
	}
	impl inbox::Trait for Test {}
	impl response::Trait for Test {}
	impl groups::Trait for Test{}
	type E2EE = Module<Test>;
	type Device = device::Module<Test>;
	type Response = response::Module<Test>;

	// This function basically just builds a genesis storage key/value store according to
	// our desired mockup.
	fn new_test_ext() -> sr_io::TestExternalities<Blake2Hasher> {
		system::GenesisConfig::<Test>::default()
			.build_storage()
			.unwrap()
			.0
			.into()
	}

	#[test]
	fn should_add_device() {
		with_externalities(&mut new_test_ext(), || {
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
		with_externalities(&mut new_test_ext(), || {
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

			assert_eq!(
				E2EE::pkbs((H256::from_low_u64_be(1), 0)),
				vec![mock_pkb]
			);
		});
	}

	#[test]
	fn should_withdraw_pkbs() {
		with_externalities(&mut new_test_ext(), || {
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

			let req_id = H256::from([3;32]);
			let wanted_pkbs = vec![(H256::from_low_u64_be(1), 0)];

			assert_ok!(E2EE::withdraw_pkbs(
				Origin::signed(H256::from_low_u64_be(2)),
				req_id.clone(),
				wanted_pkbs
			));

			assert_eq!(
				Response::response((H256::from_low_u64_be(2), req_id)),
				response::Response::PreKeyBundles(
					vec![(H256::from_low_u64_be(1), 0, mock_pkb)]
				)
			);
		});
	}

}
