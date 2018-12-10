//! A simple, secure module for dealing with fungible assets.

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

// Assert macros used in tests.
// extern crate sr_std;

// Needed for tests (`with_externalities`).
// #[cfg(test)]
// extern crate sr_io as runtime_io;

// Needed for the set of mock primitives used in our tests.
// #[cfg(test)]
// extern crate substrate_primitives;

// Needed for deriving `Encode` and `Decode` for `RawEvent`.
#[macro_use]
extern crate parity_codec_derive;
extern crate parity_codec as codec;

// Needed for type-safe access to storage DB.
#[macro_use]
extern crate srml_support as runtime_support;

// Needed for various traits. In our case, `OnFinalise`.
extern crate sr_primitives as primitives;
// `system` module provides us with all sorts of useful stuff and macros
// depend on it being around.
extern crate srml_system as system;

use primitives::traits::{Member, SimpleArithmetic, Zero};
use primitives::traits::MaybeDecode;
use primitives::RuntimeString;
use runtime_support::{dispatch::Result, Parameter, StorageMap, StorageValue};
use system::ensure_signed;

pub trait Trait: system::Trait {
    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;

    /// The units in which we record balances.
    type Balance: Member + Parameter + SimpleArithmetic + Default + Copy;
    // type Creator: system::Trait::AccountId;
}

type AssetId = u32;

decl_module! {
    // Simple declaration of the `Module` type. Lets the macro know what its working on.
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event() = default;
        pub fn create(origin, total: T::Balance) -> Result {
            let origin = ensure_signed(origin)?;

            let asset_id = Self::next_asset_id();
            <NextAssetId<T>>::mutate(|id| *id += 1);

            <FreeBalance<T>>::insert((asset_id, origin.clone()), total);
            Self::deposit_event(RawEvent::Created(asset_id, origin, total));
            Ok(())
        }

        // Move some assets from one holder to another.
        fn transfer(origin, asset_id: AssetId, dest: T::AccountId, amount: T::Balance) -> Result {
        	let origin = ensure_signed(origin)?;
        	let origin_account = (asset_id, origin.clone());
        	let origin_balance = <FreeBalance<T>>::get(&origin_account);
        	ensure!(origin_balance >= amount, "origin account balance must be greater than amount");

        	Self::deposit_event(RawEvent::Transfered(asset_id, origin, dest.clone(), amount));
        	<FreeBalance<T>>::insert(origin_account, origin_balance - amount);
        	<FreeBalance<T>>::mutate((asset_id, dest), |balance| *balance += amount);

        	Ok(())
        }
    }
}

/// An event in this module. Events are simple means of reporting specific conditions and
/// circumstances that have happened that users, Dapps and/or chain explorers would find
/// interesting and otherwise difficult to detect.
decl_event!(
	pub enum Event<T> where <T as system::Trait>::AccountId, <T as Trait>::Balance {
		// An asset was created.
        Created(AssetId, AccountId, Balance),
        
		// Some assets were transfered.
		Transfered(AssetId, AccountId, AccountId, Balance),
	}
);

decl_storage! {
    trait Store for Module<T: Trait> as gat {
        /// The number of units of assets held by any given account.
        pub FreeBalance get(free_balance) build(|config: &GenesisConfig<T>| config.balances.clone()): map (AssetId, T::AccountId) => T::Balance;

        /// The next asset identifier up for grabs.
        NextAssetId get(next_asset_id): AssetId;
    }
}

// The main implementation block for the module.
impl<T: Trait> Module<T> {
    // Public immutables

    /// Get the asset `id` balance of `who`.
    pub fn balance(asset_id: AssetId, who: T::AccountId) -> T::Balance {
        <FreeBalance<T>>::get((asset_id, who))
    }
}

// #[cfg(test)]
// mod tests {
// 	use super::*;

// 	use runtime_io::with_externalities;
// 	use substrate_primitives::{H256, Blake2Hasher};
// 	// The testing primitives are very useful for avoiding having to work with signatures
// 	// or public keys. `u64` is used as the `AccountId` and no `Signature`s are requried.
// 	use primitives::{BuildStorage, traits::{BlakeTwo256}, testing::{Digest, DigestItem, Header}};

// 	impl_outer_origin! {
// 		pub enum Origin for Test {}
// 	}

// 	// For testing the module, we construct most of a mock runtime. This means
// 	// first constructing a configuration type (`Test`) which `impl`s each of the
// 	// configuration traits of modules we want to use.
// 	#[derive(Clone, Eq, PartialEq)]
// 	pub struct Test;
// 	impl system::Trait for Test {
// 		type Origin = Origin;
// 		type Index = u64;
// 		type BlockNumber = u64;
// 		type Hash = H256;
// 		type Hashing = BlakeTwo256;
// 		type Digest = Digest;
// 		type AccountId = u64;
// 		type Header = Header;
// 		type Event = ();
// 		type Log = DigestItem;
// 	}
// 	impl Trait for Test {
// 		type Event = ();
// 		type Balance = u64;
// 	}
// 	type Assets = Module<Test>;

// 	// This function basically just builds a genesis storage key/value store according to
// 	// our desired mockup.
// 	fn new_test_ext() -> runtime_io::TestExternalities<Blake2Hasher> {
// 		system::GenesisConfig::<Test>::default().build_storage().unwrap().0.into()
// 	}

// 	#[test]
// 	fn it_works() {
// 		with_externalities(&mut new_test_ext(), || {
// 			assert_ok!(Assets::issue(Origin::signed(1), 100));
// 			assert_eq!(Assets::balance(0, 1), 100);
// 			assert_ok!(Assets::transfer(Origin::signed(1), 0, 2, 50));
// 			assert_eq!(Assets::balance(0, 1), 50);
// 			assert_eq!(Assets::balance(0, 2), 50);
// 			assert_ok!(Assets::destroy(Origin::signed(2), 0));
// 			assert_eq!(Assets::balance(0, 2), 0);
// 			assert_noop!(Assets::transfer(Origin::signed(2), 0, 1, 50), "origin account balance must be greater than amount");
// 		});
// 	}
// }
