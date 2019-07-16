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
use system::ensure_signed;

extern crate sr_io;
extern crate sr_std;
extern crate substrate_primitives;

// Needed for various traits. In our case, `OnFinalise`.
extern crate sr_primitives;

// `system` module provides us with all sorts of useful stuff and macros
// depend on it being around.
extern crate srml_system as system;

// type String = Vec<u8>;

pub trait Trait: system::Trait {
	// add code here
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn add_value(origin, peer_id: T::AccountId, value: Vec<u8>) -> Result {
			ensure_signed(origin)?;

			Self::add(peer_id, value)
		}

		fn delete_values(origin, value_ids: Vec<u32>) -> Result {
			let user_id = ensure_signed(origin)?;

			Self::delete(user_id, value_ids)
		}
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as SyloInbox {
		NextIndexes: map(T::AccountId) => u32;
		AccountValues: map(T::AccountId) => Vec<(T::AccountId, u32)>;
		Values get(values): map T::AccountId => Vec<(u32, Vec<u8>)>;
	}
}

impl<T: Trait> Module<T> {
	pub fn inbox(who: T::AccountId) -> Vec<Vec<u8>> {
		<Values<T>>::get(who).into_iter().map(|(_, value)| value).collect()
	}

	pub fn add(peer_id: T::AccountId, value: Vec<u8>) -> Result {
		// Get required data
		let next_index = <NextIndexes<T>>::get(&peer_id);
		let mut account_values = <AccountValues<T>>::get(&peer_id);

		// Add new mapping to account values
		account_values.push((peer_id.clone(), next_index));

		// Store data
		let mut values = <Values<T>>::get(&peer_id);
		if let Some((i, _)) = values.iter().enumerate().find(|(_, item)| item.0 == next_index) {
			values[i] = (next_index, value);
		} else {
			values.push((next_index, value));
		}
		<Values<T>>::insert(peer_id.clone(), values);
		<AccountValues<T>>::insert(&peer_id, account_values);

		// Update next_index
		<NextIndexes<T>>::insert(&peer_id, next_index + 1);

		Ok(())
	}

	pub fn delete(user_id: T::AccountId, value_ids: Vec<u32>) -> Result {
		let account_values = <AccountValues<T>>::get(&user_id);

		// Remove reference to value
		let account_values: Vec<(T::AccountId, u32)> = account_values
			.into_iter()
			.filter(|account_value| !value_ids.contains(&account_value.1))
			.collect();

		let mut values = <Values<T>>::get(&user_id);
		for id in value_ids {
			// Remove value from storage
			if let Some(index) = values.iter().position(|(x, _)| *x == id) {
				values.remove(index);
			}
		}
		<Values<T>>::insert(user_id.clone(), values);

		// Update account reference values
		<AccountValues<T>>::insert(&user_id, account_values);

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	use codec::{Decode, Encode};
	use serde::{Deserialize, Serialize};
	use primitives::traits::{Verify, Lazy};

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

	#[derive(Encode, Decode, Serialize, Deserialize, Debug)]
	pub struct Signature;

	impl Verify for Signature {
		type Signer = H256;
		fn verify<L: Lazy<[u8]>>(&self, _msg: L, _signer: &Self::Signer) -> bool {
			true
		}
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
		type Signature = Signature;
	}
	impl Trait for Test {}
	type Inbox = Module<Test>;

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
	fn it_works_adding_values_to_an_inbox() {
		with_externalities(&mut new_test_ext(), || {
			// Add a value to an empty inbox
			assert_ok!(Inbox::add_value(
				Origin::signed(H256::from_low_u64_be(1)),
				H256::from_low_u64_be(2),
				b"hello, world".to_vec()
			));
			assert_eq!(Inbox::inbox(H256::from_low_u64_be(2)), vec![b"hello, world".to_vec()]);

			// Add another value
			assert_ok!(Inbox::add_value(
				Origin::signed(H256::from_low_u64_be(1)),
				H256::from_low_u64_be(2),
				b"sylo".to_vec()
			));
			assert_eq!(
				Inbox::inbox(H256::from_low_u64_be(2)),
				vec![b"hello, world".to_vec(), b"sylo".to_vec()]
			);
		});
	}

	#[test]
	fn it_works_removing_values_from_an_inbox() {
		with_externalities(&mut new_test_ext(), || {
			// Add values to an empty inbox
			assert_ok!(Inbox::add_value(
				Origin::signed(H256::from_low_u64_be(1)),
				H256::from_low_u64_be(2),
				b"hello, world".to_vec()
			));
			assert_ok!(Inbox::add_value(
				Origin::signed(H256::from_low_u64_be(1)),
				H256::from_low_u64_be(2),
				b"sylo".to_vec()
			));
			assert_ok!(Inbox::add_value(
				Origin::signed(H256::from_low_u64_be(1)),
				H256::from_low_u64_be(2),
				b"foo".to_vec()
			));
			assert_ok!(Inbox::add_value(
				Origin::signed(H256::from_low_u64_be(1)),
				H256::from_low_u64_be(2),
				b"bar".to_vec()
			));

			// Remove a single value
			assert_ok!(Inbox::delete_values(Origin::signed(H256::from_low_u64_be(2)), vec![0]));
			assert_eq!(
				Inbox::inbox(H256::from_low_u64_be(2)),
				vec![b"sylo".to_vec(), b"foo".to_vec(), b"bar".to_vec()]
			);

			assert_ok!(Inbox::delete_values(
				Origin::signed(H256::from_low_u64_be(2)),
				vec![2, 3]
			));
			assert_eq!(Inbox::inbox(H256::from_low_u64_be(2)), vec![b"sylo".to_vec()]);
		});
	}

	#[test]
	fn it_works_removing_values_from_an_empty_inbox() {
		with_externalities(&mut new_test_ext(), || {
			// Remove a value that doesn't exist
			assert_ok!(Inbox::delete_values(Origin::signed(H256::from_low_u64_be(2)), vec![0]));
		});
	}
}
