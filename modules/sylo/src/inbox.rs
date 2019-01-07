use srml_support::{dispatch::Result, dispatch::Vec, StorageMap};
use {balances, system::ensure_signed};

// Assert macros used in tests.
extern crate sr_std;

// Needed for tests (`with_externalities`).
#[cfg(test)]
extern crate sr_io;

// Needed for the set of mock primitives used in our tests.
#[cfg(test)]
extern crate substrate_primitives;
// Needed for various traits. In our case, `OnFinalise`.
extern crate sr_primitives;

// `system` module provides us with all sorts of useful stuff and macros
// depend on it being around.
extern crate srml_system as system;

// type String = Vec<u8>;

pub trait Trait: balances::Trait {
    // add code here
}

decl_module! {
  pub struct Module<T: Trait> for enum Call where origin: T::Origin {

    fn add_value(origin, peer_id: T::AccountId, value: Vec<u8>) -> Result {
      ensure_signed(origin)?;

      // Get required data
      let next_index = <NextIndexes<T>>::get(&peer_id);
      let mut account_values = <AccountValues<T>>::get(&peer_id);

      // Add new mapping to account values
      account_values.push((peer_id.clone(), next_index));

      // Store data
      <Values<T>>::insert((peer_id.clone(), next_index), value);
      <AccountValues<T>>::insert(&peer_id, account_values);

      // Update next_index
      <NextIndexes<T>>::insert(&peer_id, next_index + 1);


      Ok(())
    }

    fn delete_values(origin, value_ids: Vec<u32>) -> Result {
      let sender = ensure_signed(origin)?;

      let account_values = <AccountValues<T>>::get(&sender);

      // Remove reference to value
      let account_values: Vec<(T::AccountId, u32)> = account_values
        .into_iter()
        .filter(|account_value| !value_ids.contains(&account_value.1))
        .collect();

      for id in value_ids {
        // Remove value from storage
        <Values<T>>::remove((sender.clone(), id));
      }

      // Update account reference values
      <AccountValues<T>>::insert(&sender, account_values);

      Ok(())
    }
  }
}

decl_storage! {
  trait Store for Module<T: Trait> as Inbox {
    NextIndexes: map(T::AccountId) => u32;
    AccountValues: map(T::AccountId) => Vec<(T::AccountId, u32)>;
    Values: map(T::AccountId, u32) => Vec<u8>;
  }
}

impl<T: Trait> Module<T> {
    pub fn inbox(who: T::AccountId) -> Vec<Vec<u8>> {
        let inboxes = <AccountValues<T>>::get(who);

        inboxes
            .iter()
            .map(|entry| <Values<T>>::get(entry))
            .collect()
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
        traits::BlakeTwo256,
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
        type AccountId = u64;
        type Header = Header;
        type Event = ();
        type Log = DigestItem;
    }
    impl balances::Trait for Test {
        type Balance = u64;
        type AccountIndex = u64;
        type OnFreeBalanceZero = ();
        type EnsureAccountLiquid = ();
        type Event = ();
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
                Origin::signed(1),
                2,
                b"hello, world".to_vec()
            ));
            assert_eq!(Inbox::inbox(2), vec![b"hello, world".to_vec()]);

            // Add another value
            assert_ok!(Inbox::add_value(Origin::signed(1), 2, b"sylo".to_vec()));
            assert_eq!(
                Inbox::inbox(2),
                vec![b"hello, world".to_vec(), b"sylo".to_vec()]
            );
        });
    }

    #[test]
    fn it_works_removing_values_from_an_inbox() {
        with_externalities(&mut new_test_ext(), || {
            // Add a values to an empty inbox
            assert_ok!(Inbox::add_value(
                Origin::signed(1),
                2,
                b"hello, world".to_vec()
            ));
            assert_ok!(Inbox::add_value(Origin::signed(1), 2, b"sylo".to_vec()));
            assert_ok!(Inbox::add_value(Origin::signed(1), 2, b"foo".to_vec()));
            assert_ok!(Inbox::add_value(Origin::signed(1), 2, b"bar".to_vec()));

            // Remove a single value
            assert_ok!(Inbox::delete_values(Origin::signed(2), vec![0]));
            assert_eq!(
                Inbox::inbox(2),
                vec![b"sylo".to_vec(), b"foo".to_vec(), b"bar".to_vec()]
            );

            assert_ok!(Inbox::delete_values(Origin::signed(2), vec![2, 3]));
            assert_eq!(Inbox::inbox(2), vec![b"sylo".to_vec()]);
        });
    }

    #[test]
    fn it_works_removing_values_from_an_empty_inbox() {
        with_externalities(&mut new_test_ext(), || {
            // Remove a value that doesn't exist
            assert_ok!(Inbox::delete_values(Origin::signed(2), vec![0]));
        });
    }
}
