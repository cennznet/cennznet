//! A simple, secure module for dealing with fungible assets.

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]
// #![cfg_attr(not(feature = "std"), feature(alloc))]

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

pub trait Trait: system::Trait {
	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

type AttestationTopic = U256;
type AttestationValue = U256;

decl_module! {
	// Simple declaration of the `Module` type. Lets the macro know what its working on.
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event<T>() = default;

		pub fn set_claim(origin, holder: T::AccountId, topic: AttestationTopic, value: AttestationValue) -> Result {
			let issuer = ensure_signed(origin)?;

			Self::create_claim(holder, issuer, topic, value)?;
			Ok(())
		}

		pub fn set_self_claim(origin, topic: AttestationTopic, value: AttestationValue) -> Result {
			let holder_and_issuer = ensure_signed(origin)?;

			Self::create_claim(holder_and_issuer.clone(), holder_and_issuer, topic, value)?;
			Ok(())
		}

		pub fn remove_claim(origin, holder: T::AccountId, topic: AttestationTopic) -> Result {
			let issuer = ensure_signed(origin)?;
			<Issuers<T>>::mutate(&holder,|issuers| issuers.retain(|vec_issuer| *vec_issuer != issuer));
			<Topics<T>>::mutate((holder.clone(), issuer.clone()),|topics| topics.retain(|vec_topic| *vec_topic != topic));
			<Values<T>>::remove((holder.clone(), issuer.clone(), topic));

			Self::deposit_event(RawEvent::ClaimRemoved(holder, issuer, topic));

			Ok(())
		}
	}
}

/// An event in this module. Events are simple means of reporting specific conditions and
/// circumstances that have happened that users, Dapps and/or chain explorers would find
/// interesting and otherwise difficult to detect.
decl_event!(
	pub enum Event<T> where <T as system::Trait>::AccountId {
		ClaimSet(AccountId, AccountId, AttestationTopic, AttestationValue),
		ClaimRemoved(AccountId, AccountId, AttestationTopic),
	}
);

decl_storage! {
	trait Store for Module<T: Trait> as Attestation {
		Issuers: map T::AccountId => Vec<T::AccountId>;
		Topics: map (T::AccountId, T::AccountId) => Vec<AttestationTopic>;
		Values: map (T::AccountId, T::AccountId, AttestationTopic) => AttestationValue;
	}
}

// The main implementation block for the module.
impl<T: Trait> Module<T> {
	fn create_claim(
		holder: T::AccountId,
		issuer: T::AccountId,
		topic: AttestationTopic,
		value: AttestationValue,
	) -> Result {
		<Issuers<T>>::mutate(&holder, |issuers| {
			if !issuers.contains(&issuer) {
				issuers.push(issuer.clone())
			}
		});

		<Topics<T>>::mutate((holder.clone(), issuer.clone()), |topics| {
			if !topics.contains(&topic) {
				topics.push(topic)
			}
		});

		<Values<T>>::insert((holder.clone(), issuer.clone(), topic), value);
		Self::deposit_event(RawEvent::ClaimSet(holder, issuer, topic, value));
		Ok(())
	}
}
