use srml_support::{dispatch::Result, dispatch::Vec, StorageMap};
use {balances, response, system::ensure_signed};
extern crate srml_system as system;

#[cfg(test)]
extern crate sr_primitives;

#[cfg(test)]
extern crate sr_io;

#[cfg(test)]
extern crate substrate_primitives;

pub trait Trait: balances::Trait + response::Trait {
    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

decl_module! {
  pub struct Module<T: Trait> for enum Call where origin: T::Origin {
    fn deposit_event<T>() = default;

    // Registers a new device for e2ee
    // request_id is used to identify the assigned device id
    fn register_device(origin, request_id: T::Hash) -> Result {
      let sender = ensure_signed(origin)?;

      let mut devices = <Devices<T>>::get(&sender);

      let len = devices.len() as u32;

      devices.push(len);

      <Devices<T>>::insert(&sender, devices);
      Self::deposit_event(RawEvent::DeviceAdded(sender.clone(), request_id, len));
      <response::Module<T>>::set_response(sender, request_id, response::Response::DeviceId(len));
      Ok(())
    }
  }
}

// The data that is stored
decl_storage! {
  trait Store for Module<T: Trait> as Device {
    Devices get(devices): map T::AccountId => Vec<u32>;
  }
}

decl_event!(
  pub enum Event<T> where <T as system::Trait>::Hash, <T as system::Trait>::AccountId {
    DeviceAdded(AccountId, Hash, u32),
  }
);

impl<T: Trait> Module<T> {}

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
    impl Trait for Test {
        type Event = ();
    }
    impl response::Trait for Test {}

    type Devices = Module<Test>;
    type Responses = response::Module<response::tests::Test>;

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
            let request_id = H256::from(23);
            assert_ok!(Devices::register_device(
                Origin::signed(1),
                request_id.clone()
            ));
            assert_eq!(Devices::devices(1).len(), 1);

            assert_ok!(Devices::register_device(
                Origin::signed(1),
                request_id.clone()
            ));
            assert_eq!(Devices::devices(1).len(), 2);
            assert_eq!(Devices::devices(1)[1], 1);

            // check saved response
            assert_eq!(
                Responses::response((1, request_id)),
                response::Response::DeviceId(1)
            );
        });
    }
}
