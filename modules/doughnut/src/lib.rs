// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]
// #![cfg_attr(not(feature = "std"), feature(alloc))]

extern crate parity_codec as codec;
// Needed for deriving `Encode` and `Decode` for `RawEvent`.
extern crate parity_codec_derive;
extern crate sr_io as io;
extern crate sr_primitives as primitives;
// Needed for type-safe access to storage DB.
extern crate srml_support as runtime_support;
// `system` module provides us with all sorts of useful stuff and macros
// depend on it being around.
extern crate srml_system as system;
extern crate substrate_primitives;

use std::time::SystemTime;

use codec::Encode;
use primitives::traits::Verify;
use runtime_support::{dispatch::Result};
use runtime_support::rstd::prelude::*;
use sr_std::result;

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

// derive Debug to meet the requirement of deposit_event

#[derive(Clone, Eq, PartialEq, Default)]
pub struct Certificate<AccountId> {
	pub expires: u64,
	pub version: u32,
	pub holder: AccountId,
	pub not_before: Option<u64>,
	//	use vec of tuple to work as a key value map
	pub permissions: Vec<(Vec<u8>, Vec<u8>)>,
	pub issuer: AccountId,
}

#[derive(Clone, Eq, PartialEq, Default)]
pub struct Doughnut<AccountId, Signature> {
	pub certificate: Certificate<AccountId>,
	pub signature: Signature,
	pub compact: Vec<u8>,
}

impl<AccountId> Encode for Certificate<AccountId> where
	AccountId: Encode {
	fn encode(&self) -> Vec<u8> {
		let mut r = Vec::new();
		self.version.encode_to(&mut r);
		self.expires.encode_to(&mut r);
		self.holder.encode_to(&mut r);
		self.issuer.encode_to(&mut r);
		self.not_before.encode_to(&mut r);
		self.permissions.encode_to(&mut r);
		r
	}
}

impl<AccountId, Signature> Encode for Doughnut<AccountId, Signature> where
	AccountId: Encode,
	Signature: Encode {
	fn encode(&self) -> Vec<u8> {
		let mut r = Vec::new();
		self.certificate.encode_to(&mut r);
		self.signature.encode_to(&mut r);
		self.compact.encode_to(&mut r);
		r
	}
}


impl<AccountId, Signature> Doughnut<AccountId, Signature> where
	Signature: Verify<Signer=AccountId> + Encode,
	AccountId: Encode {
	pub fn validate(self) -> result::Result<Self, &'static str> {
		let now = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
			Ok(n) => n.as_secs(),
			Err(_) => return Err("SystemTime before UNIX EPOCH!")
		};
		if self.certificate.expires > now {
			let valid = match self.certificate.not_before {
				Some(not_before) => not_before <= now,
				None => true
			};
			if valid {
				if self.signature.verify(self.certificate.encode().as_slice(), &self.certificate.issuer) {
					// TODO: ensure doughnut hasn't been revoked
//						Self::deposit_event(RawEvent::Validated(doughnut.certificate.issuer, doughnut.compact));
					return Ok(self);
				} else {
					return Err("invalid signature");
				}
			}
		}
		return Err("invalid doughnut");
	}
	pub fn validate_permission(self) -> Result {
		// not efficient, optimize later
		for permission_pair in &self.certificate.permissions {
			if permission_pair.0 == "cennznet".encode() {
				return Ok(());
			}
		}
		return Err("no permission");
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {

		fn deposit_event<T>() = default;

	}
}

decl_event!(
	pub enum Event<T> where <T as system::Trait>::AccountId  {
		Validated(AccountId, Vec<u8>),
	}
);
