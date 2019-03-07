extern crate parity_codec;

use self::parity_codec::{Decode, Encode};
use srml_support::{dispatch::Result, dispatch::Vec, StorageMap};
use system::ensure_signed;
extern crate srml_system as system;

extern crate sr_primitives;
extern crate sr_io;
extern crate substrate_primitives;

pub trait Trait: system::Trait {}

#[derive(Encode, Decode, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum Response<T: Encode + Decode> {
	DeviceId(u32),
	PreKeyBundles(Vec<(T, u32, Vec<u8>)>),
	None,
}

impl<T: Encode + Decode> Default for Response<T> {
	fn default() -> Response<T> {
		Response::None
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn remove_response(origin, request_id: T::Hash) -> Result {
			let sender = ensure_signed(origin)?;
			<Responses<T>>::remove((sender, request_id));
			Ok(())
		}
	}
}

// The data that is stored
decl_storage! {
	trait Store for Module<T: Trait> as SyloResponse {
		Responses get(response): map (T::AccountId, T::Hash /* request_id */) => Response<T::AccountId>;
	}
}

impl<T: Trait> Module<T> {
	pub(super) fn set_response(sender: T::AccountId, request_id: T::Hash, response: Response<T::AccountId>) {
		if response != Response::None {
			<Responses<T>>::insert((sender, request_id), response);
		}
	}
}

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
	impl Trait for Test {}
	type Responses = Module<Test>;

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
	fn should_set_response() {
		with_externalities(&mut new_test_ext(), || {
			let request_id = H256::from([1; 32]);
			let resp_number = Response::DeviceId(111);

			// setting number
			Responses::set_response(H256::from_low_u64_be(1), request_id.clone(), resp_number.clone());
			assert_eq!(
				Responses::response((H256::from_low_u64_be(1), request_id.clone())),
				resp_number.clone()
			);

			// // setting pkb type
			let resp_pkb = Response::PreKeyBundles(vec![(H256::from_low_u64_be(1), 2, b"test data".to_vec())]);
			Responses::set_response(H256::from_low_u64_be(1), request_id.clone(), resp_pkb.clone());
			assert_eq!(
				Responses::response((H256::from_low_u64_be(1), request_id.clone())),
				resp_pkb.clone()
			);

			// // remove response
			assert_ok!(Responses::remove_response(
				Origin::signed(H256::from_low_u64_be(1)),
				request_id.clone()
			));
			assert_eq!(
				Responses::response((H256::from_low_u64_be(1), request_id.clone())),
				Response::None
			);
		});
	}
}
