use srml_support::{dispatch::Vec, StorageMap};
use {system, system::ensure_signed};

extern crate sr_io;
extern crate sr_primitives;
extern crate substrate_primitives;

pub const KEYS_MAX: usize = 100;

pub trait Trait: system::Trait {}

pub type Key = Vec<u8>;
pub type Val = Vec<u8>;

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn add_value(origin, key: Key, value: Val) {
			let user_id = ensure_signed(origin)?;

			ensure!(<Vault<T>>::get(&user_id).len() < KEYS_MAX, "Can not store more than maximum amount of keys");

			Self::add(user_id, key, value);
		}

		fn delete_values(origin, keys: Vec<Key>) {
			let user_id = ensure_signed(origin)?;

			Self::delete(user_id, keys);
		}
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as SyloVault {
		pub Vault get(values): map T::AccountId => Vec<(Key, Val)>;
	}
}

impl<T: Trait> Module<T> {
	pub fn add(user_id: T::AccountId, key: Key, value: Val) {
		let mut values = <Vault<T>>::get(&user_id);

		match values.iter().enumerate().find(|(_, item)| item.0 == key) {
			None => values.push((key, value)),
			Some((i, _)) => { values[i] = (key, value) }
		}

		<Vault<T>>::insert(user_id, values)
	}

	pub fn delete(user_id: T::AccountId, keys: Vec<Key>) {
		let remaining_values: Vec<(Key, Val)> =
			<Vault<T>>::get(&user_id)
				.into_iter()
				.filter(|item| {
					keys.iter().find(|key_to_remove| &&item.0 == key_to_remove).is_none()
				})
				.collect();

		<Vault<T>>::insert(user_id, remaining_values)
	}
}

#[cfg(test)]
mod tests {
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
	impl Trait for Test {}
	type Vault = Module<Test>;

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
	fn should_add_values() {
		with_externalities(&mut new_test_ext(), || {
			let key_0 = b"0".to_vec();
			let value_0 = b"1".to_vec();

			assert_ok!(Vault::add_value(
				Origin::signed(H256::from_low_u64_be(1)),
				key_0.clone(),
				value_0.clone()
			));

			assert_eq!(
				Vault::values(H256::from_low_u64_be(1)),
				vec![(key_0.clone(), value_0.clone())]
			);

			let key_1 = b"01".to_vec();
			let value_1 = b"10".to_vec();

			assert_ok!(Vault::add_value(
				Origin::signed(H256::from_low_u64_be(1)),
				key_1.clone(),
				value_1.clone()
			));

			assert_eq!(
				Vault::values(H256::from_low_u64_be(1)),
				vec![(key_0, value_0), (key_1, value_1)]
			);
		})
	}

	#[test]
	fn should_replace_existing_keys() {
		with_externalities(&mut new_test_ext(), || {
			let key_0 = b"0".to_vec();
			let value_0 = b"1".to_vec();
			let value_1 = b"01".to_vec();

			assert_ok!(Vault::add_value(
				Origin::signed(H256::from_low_u64_be(1)),
				key_0.clone(),
				value_0.clone()
			));

			assert_eq!(
				Vault::values(H256::from_low_u64_be(1)),
				vec![(key_0.clone(), value_0)]
			);

			assert_ok!(Vault::add_value(
				Origin::signed(H256::from_low_u64_be(1)),
				key_0.clone(),
				value_1.clone()
			));

			assert_eq!(
				Vault::values(H256::from_low_u64_be(1)),
				vec![(key_0, value_1)]
			);
		}
	}

	#[test]
	fn should_delete_keys() {
		with_externalities(&mut new_test_ext(), || {
			let key_0 = b"0".to_vec();
			let key_1 = b"1".to_vec();
			let value_0 = b"01".to_vec();

			assert_ok!(Vault::add_value(
				Origin::signed(H256::from_low_u64_be(1)),
				key_0.clone(),
				value_0.clone()
			));

			assert_ok!(Vault::add_value(
				Origin::signed(H256::from_low_u64_be(1)),
				key_1.clone(),
				value_0.clone()
			));

			assert_eq!(
				Vault::values(H256::from_low_u64_be(1)),
				vec![(key_0.clone(), value_0.clone()), (key_1.clone(), value_0)]
			);

			assert_ok!(Vault::delete_values(
				Origin::signed(H256::from_low_u64_be(1)),
				vec![key_0, key_1]
			));

			assert_eq!(
				Vault::values(H256::from_low_u64_be(1)),
				vec![]
			);
		}
	}
}
