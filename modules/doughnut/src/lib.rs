//! Centrality's Doughnut.

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]
// #![cfg_attr(not(feature = "std"), feature(alloc))]

// Needed for deriving `Encode` and `Decode` for `RawEvent`.
#[macro_use]
extern crate parity_codec_derive;
extern crate parity_codec as codec;

// Needed for type-safe access to storage DB.
#[macro_use]
extern crate srml_support as runtime_support;

extern crate sr_io as io;
extern crate sr_primitives as primitives;
extern crate substrate_primitives;
// `system` module provides us with all sorts of useful stuff and macros
// depend on it being around.
extern crate srml_system as system;

use runtime_support::rstd::prelude::*;
use runtime_support::{dispatch::Result, StorageMap};
use substrate_primitives::uint::U256;
use system::ensure_signed;

/*

Find a way to allow other kinds of permission domains (other keys in permissions)
Might look like: a wild card that accepts cennznet + w/e
If cennznet is present, success otherwise fail
*/

#[derive(Debug, Encode, Decode, Clone, Eq, PartialEq, Default)]
struct Permissions {
	cennznet: bool,
}

#[derive(Debug, Encode, Decode, Clone, Eq, PartialEq, Default)]
struct Certificate {
	Expires: Vec<u32>,
	Version: u32,
	Holder: Vec<u8>,
	Permissions: Permissions,
	Issuer: Vec<u8>,
}

#[derive(Debug, Encode, Decode, Clone, Eq, PartialEq, Default)]
pub struct Doughnut {
	certificate: Option<Certificate>,
	signature: Option<Vec<u32>>,
	compact: Option<Vec<u32>>,
}

pub trait Trait: system::Trait {
	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

decl_module! {
		pub struct Module<T: Trait> for enum Call where origin: T::Origin {

			fn validate(doughnut: Doughnut) {
					// validation for a doughnut
					println!("Hello world");
			}

			// fn validate_certificate(certificate: Certificate) {
			// 	println!("{}", certificate)
			// }

			// fn validate_signature(signature: Vec<u32>) {
			// 	println!("{}", signature)
			// }

}

}

/// An event in this module. Events are simple means of reporting specific conditions and
/// circumstances that have happened that users, Dapps and/or chain explorers would find
/// interesting and otherwise difficult to detect.
decl_event!(
	pub enum Event<T> where <T as system::Trait>::AccountId  {
		Validate(AccountId, Doughnut),
	}
);

decl_storage! {
	trait Store for Module<T: Trait> as Doughnut {
	}
}
