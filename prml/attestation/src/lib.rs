// Copyright 2019 Centrality Investments Limited
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//! # Attestation Module
//!
//! The Attestation module provides functionality for entities to create attestation claims about one another.
//!
//! This module borrows heavily from ERC 780 https://github.com/ethereum/EIPs/issues/780
//!
//! ## Terminology
//!
//! Issuer: the entity creating the claim
//! Holder: the entity that the claim is about
//! Topic: the topic which the claim is about ie isOver18
//! Value: any value pertaining to the claim
//!
//! ## Usage
//!
//! Topic and Value are U256 integers. This means that Topic and Value can technically store any value that can be represented in 256 bits.
//!
//! The user of the module must convert whatever value that they would like to store into a value that can be stored as a U256.
//!
//! It is recommended that Topic be a string value converted to hex and stored on the blockchain as a U256.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate parity_codec as codec;

#[macro_use]
extern crate srml_support as runtime_support;

extern crate primitives;
extern crate runtime_primitives;
extern crate sr_io as io;
extern crate srml_system as system;

use primitives::uint::U256;
use primitives::uint::U256;
use runtime_support::rstd::prelude::*;
use runtime_support::{dispatch::Result, StorageMap};
use system::ensure_signed;

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

type AttestationTopic = U256;
type AttestationValue = U256;

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event<T>() = default;

		/// Create a new claim
		pub fn set_claim(origin, holder: T::AccountId, topic: AttestationTopic, value: AttestationValue) -> Result {
			let issuer = ensure_signed(origin)?;

			Self::create_claim(holder, issuer, topic, value)?;
			Ok(())
		}

		/// Create a new claim where the holder and issuer are the same person
		pub fn set_self_claim(origin, topic: AttestationTopic, value: AttestationValue) -> Result {
			let holder_and_issuer = ensure_signed(origin)?;

			Self::create_claim(holder_and_issuer.clone(), holder_and_issuer, topic, value)?;
			Ok(())
		}

		/// Remove a claim, only the original issuer can remove a claim
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

decl_event!(
	pub enum Event<T> where <T as system::Trait>::AccountId {
		ClaimSet(AccountId, AccountId, AttestationTopic, AttestationValue),
		ClaimRemoved(AccountId, AccountId, AttestationTopic),
	}
);

decl_storage! {
	trait Store for Module<T: Trait> as Attestation {
		/// The maps are layed out to support the nested structure shown below in JSON, will look to optimise later.
		///
		/// {
		///   holder: {
		///     issuer: {
		///       topic: value
		///     }
		///   }
		/// }
		///

		/// A map of HolderId => Vec<IssuerId>
		Issuers: map T::AccountId => Vec<T::AccountId>;
		/// A map of (HolderId, IssuerId) => Vec<AttestationTopic>
		Topics: map (T::AccountId, T::AccountId) => Vec<AttestationTopic>;
		/// A map of (HolderId, IssuerId, AttestationTopic) => AttestationValue
		Values: map (T::AccountId, T::AccountId, AttestationTopic) => AttestationValue;
	}
}

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
