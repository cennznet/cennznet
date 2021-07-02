// Copyright 2019-2021 Plug New Zealand Limited and Centrality Investments Ltd.
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

//! # Attestation Pallet
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

mod benchmarking;
mod mock;
mod weights;

use frame_support::sp_std::prelude::*;
use frame_support::{decl_error, decl_event, decl_module, decl_storage, dispatch::DispatchResult, ensure};
use frame_system::ensure_signed;
use sp_core::U256;
use sp_runtime::traits::Zero;
use weights::WeightInfo;

pub trait Config: frame_system::Config {
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
	type WeightInfo: WeightInfo;
}

type AttestationTopic = U256;
type AttestationValue = U256;

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		/// Create or update an existing claim
		/// The `issuer` of the claim comes from the extrinsic `origin`
		/// The `topic` and `value` are both U256 which can hold any 32-byte encoded data.
		#[weight = T::WeightInfo::set_claim()]
		pub fn set_claim(origin, holder: T::AccountId, topic: AttestationTopic, value: AttestationValue) -> DispatchResult {
			let issuer = ensure_signed(origin)?;

			Self::create_or_update_claim(holder, issuer, topic, value);
			Ok(())
		}

		/// Remove a claim, only the original issuer can remove a claim
		/// If the `issuer` has not yet issued a claim of `topic`, this function will return error.
		#[weight = T::WeightInfo::remove_claim()]
		pub fn remove_claim(origin, holder: T::AccountId, topic: AttestationTopic) -> DispatchResult {
			let issuer = ensure_signed(origin)?;

			ensure!(
				<Topics<T>>::get((holder.clone(), issuer.clone())).contains(&topic),
				Error::<T>::TopicNotRegistered
			);

			<Values<T>>::remove((holder.clone(), issuer.clone(), topic));
			<Topics<T>>::mutate((holder.clone(), issuer.clone()),|topics| topics.retain(|vec_topic| *vec_topic != topic));

			let remove_issuer = <Topics<T>>::get((holder.clone(), issuer.clone())).len().is_zero();
			if remove_issuer {
				<Issuers<T>>::mutate(&holder, |issuers| {
					issuers.retain(|vec_issuer| *vec_issuer != issuer.clone())
				});
			}

			Self::deposit_event(RawEvent::ClaimRemoved(holder, issuer, topic));

			Ok(())
		}
	}
}

decl_event!(
	pub enum Event<T> where <T as frame_system::Config>::AccountId {
		ClaimCreated(AccountId, AccountId, AttestationTopic, AttestationValue),
		ClaimRemoved(AccountId, AccountId, AttestationTopic),
		ClaimUpdated(AccountId, AccountId, AttestationTopic, AttestationValue),
	}
);

// The storage maps are layed out to support the nested structure shown below in JSON:
//
// {
//  holder: {
//    issuer: {
//      topic: <value>
//    }
//  }
// }
//
decl_storage! {
	trait Store for Pallet<T: Config> as Attestation {
		/// A map from holders to all their attesting issuers
		Issuers get(fn issuers):
			map hasher(blake2_128_concat) T::AccountId => Vec<T::AccountId>;
		/// A map from (holder, issuer) to attested topics
		Topics get(fn topics):
			map hasher(blake2_128_concat) (T::AccountId, T::AccountId) => Vec<AttestationTopic>;
		/// A map from (holder, issuer, topic) to attested values
		Values get(fn value):
			map hasher(blake2_128_concat) (T::AccountId, T::AccountId, AttestationTopic) => AttestationValue;
	}
}

decl_error! {
	/// Error for the attestation module.
	pub enum Error for Pallet<T: Config> {
		TopicNotRegistered,
	}
}

impl<T: Config> Pallet<T> {
	/// Sets a claim about a `holder` from an `issuer`
	/// If the claim `topic` already exists, then the claim `value` is updated,
	/// Otherwise, a new claim is created for the `holder` by the `issuer`
	fn create_or_update_claim(
		holder: T::AccountId,
		issuer: T::AccountId,
		topic: AttestationTopic,
		value: AttestationValue,
	) {
		<Issuers<T>>::mutate(&holder, |issuers| {
			if !issuers.contains(&issuer) {
				issuers.push(issuer.clone())
			}
		});

		let topic_exists: bool = <Topics<T>>::get((holder.clone(), issuer.clone())).contains(&topic);

		<Topics<T>>::mutate((holder.clone(), issuer.clone()), |topics| {
			if !topic_exists {
				topics.push(topic)
			}
		});

		<Values<T>>::insert((holder.clone(), issuer.clone(), topic), value);

		if topic_exists {
			Self::deposit_event(RawEvent::ClaimUpdated(holder, issuer, topic, value));
		} else {
			Self::deposit_event(RawEvent::ClaimCreated(holder, issuer, topic, value));
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{new_test_ext, Attestation, Event as TestEvent, Origin, System, Test};
	use frame_support::{assert_noop, assert_ok};

	type AccountId = <Test as frame_system::Config>::AccountId;

	#[test]
	fn initialize_holder_has_no_claims() {
		let holder = 0xbaa;
		new_test_ext().execute_with(|| {
			// Note: without any valid issuers, there is no valid input for topics or value
			assert_eq!(Attestation::issuers(holder), <Vec<AccountId>>::new());
		})
	}

	#[test]
	fn adding_claim_to_storage() {
		let issuer = 0xf00;
		let holder = 0xbaa;
		let topic = AttestationTopic::from(0xf00d);
		let value = AttestationValue::from(0xb33f);
		new_test_ext().execute_with(|| {
			let result = Attestation::set_claim(Origin::signed(issuer), holder, topic, value);

			assert_ok!(result);

			assert_eq!(Attestation::issuers(holder), [issuer]);
			assert_eq!(Attestation::topics((holder, issuer)), [topic]);
			assert_eq!(Attestation::value((holder, issuer, topic)), value);
		})
	}

	#[test]
	fn account_can_claim_on_itself() {
		let holder = 0x1d107;
		let topic = AttestationTopic::from(0xf001);
		let value = AttestationValue::from(0xb01);
		new_test_ext().execute_with(|| {
			let result = Attestation::set_claim(Origin::signed(holder), holder, topic, value);

			assert_ok!(result);

			assert_eq!(Attestation::issuers(holder), [holder]);
			assert_eq!(Attestation::topics((holder, holder)), [topic]);
			assert_eq!(Attestation::value((holder, holder, topic)), value);
		})
	}

	#[test]
	fn adding_existing_claim_overwrites_claim() {
		let issuer = 0xf00;
		let holder = 0xbaa;
		let topic = AttestationTopic::from(0xf00d);
		let value_old = AttestationValue::from(0xb33f);
		let value_new = AttestationValue::from(0xcabba93);
		new_test_ext().execute_with(|| {
			let result_old = Attestation::set_claim(Origin::signed(issuer), holder, topic, value_old);

			assert_ok!(result_old);
			assert_eq!(Attestation::value((holder, issuer, topic)), value_old);

			let result_new = Attestation::set_claim(Origin::signed(issuer), holder, topic, value_new);

			assert_ok!(result_new);
			assert_eq!(Attestation::value((holder, issuer, topic)), value_new);
		})
	}

	#[test]
	fn adding_multiple_claims_from_same_issuer() {
		let issuer = 0xf00;
		let holder = 0xbaa;
		let topic_food = AttestationTopic::from(0xf00d);
		let value_food = AttestationValue::from(0xb33f);
		let topic_loot = AttestationTopic::from(0x1007);
		let value_loot = AttestationValue::from(0x901d);
		new_test_ext().execute_with(|| {
			let result_food = Attestation::set_claim(Origin::signed(issuer), holder, topic_food, value_food);
			let result_loot = Attestation::set_claim(Origin::signed(issuer), holder, topic_loot, value_loot);

			assert_ok!(result_food);
			assert_ok!(result_loot);

			assert_eq!(Attestation::issuers(holder), [issuer]);
			assert_eq!(Attestation::topics((holder, issuer)), [topic_food, topic_loot]);
			assert_eq!(Attestation::value((holder, issuer, topic_food)), value_food);
			assert_eq!(Attestation::value((holder, issuer, topic_loot)), value_loot);
		})
	}

	#[test]
	fn adding_claims_from_different_issuers() {
		let issuer_foo = 0xf00;
		let issuer_boa = 0xb0a;
		let holder = 0xbaa;
		let topic_food = AttestationTopic::from(0xf00d);
		let value_food_foo = AttestationValue::from(0xb33f);
		let value_food_boa = AttestationValue::from(0x90a7);
		new_test_ext().execute_with(|| {
			let result_foo = Attestation::set_claim(Origin::signed(issuer_foo), holder, topic_food, value_food_foo);
			let result_boa = Attestation::set_claim(Origin::signed(issuer_boa), holder, topic_food, value_food_boa);

			assert_ok!(result_foo);
			assert_ok!(result_boa);

			assert_eq!(Attestation::issuers(holder), [issuer_foo, issuer_boa]);
			assert_eq!(Attestation::topics((holder, issuer_foo)), [topic_food]);
			assert_eq!(Attestation::topics((holder, issuer_boa)), [topic_food]);
			assert_eq!(Attestation::value((holder, issuer_foo, topic_food)), value_food_foo);
			assert_eq!(Attestation::value((holder, issuer_boa, topic_food)), value_food_boa);
		})
	}

	#[test]
	fn remove_claim_from_storage() {
		let issuer = 0xf00;
		let holder = 0xbaa;
		let topic = AttestationTopic::from(0xf00d);
		let value = AttestationValue::from(0xb33f);
		let invalid_value = AttestationValue::zero();
		new_test_ext().execute_with(|| {
			let result_add = Attestation::set_claim(Origin::signed(issuer), holder, topic, value);

			let result_remove = Attestation::remove_claim(Origin::signed(issuer), holder, topic);

			assert_ok!(result_add);
			assert_ok!(result_remove);

			assert_eq!(Attestation::issuers(holder), <Vec<AccountId>>::new());
			assert_eq!(Attestation::topics((holder, issuer)), []);
			assert_eq!(Attestation::value((holder, issuer, topic)), invalid_value);
		})
	}

	#[test]
	fn remove_claim_from_account_with_multiple_issuers() {
		let issuer_foo = 0xf00;
		let issuer_boa = 0xb0a;
		let holder = 0xbaa;
		let topic_food = AttestationTopic::from(0xf00d);
		let value_food_foo = AttestationValue::from(0xb33f);
		let value_food_boa = AttestationValue::from(0x90a7);
		let invalid_value = AttestationValue::zero();
		new_test_ext().execute_with(|| {
			let result_foo = Attestation::set_claim(Origin::signed(issuer_foo), holder, topic_food, value_food_foo);
			let result_boa = Attestation::set_claim(Origin::signed(issuer_boa), holder, topic_food, value_food_boa);

			let result_remove = Attestation::remove_claim(Origin::signed(issuer_foo), holder, topic_food);

			assert_ok!(result_foo);
			assert_ok!(result_boa);
			assert_ok!(result_remove);

			assert_eq!(Attestation::issuers(holder), [issuer_boa]);
			assert_eq!(Attestation::topics((holder, issuer_foo)), []);
			assert_eq!(Attestation::topics((holder, issuer_boa)), [topic_food]);
			assert_eq!(Attestation::value((holder, issuer_foo, topic_food)), invalid_value);
			assert_eq!(Attestation::value((holder, issuer_boa, topic_food)), value_food_boa);
		})
	}

	#[test]
	fn remove_claim_from_account_with_multiple_claims_from_same_issuer() {
		let issuer = 0xf00;
		let holder = 0xbaa;
		let topic_food = AttestationTopic::from(0xf00d);
		let value_food = AttestationValue::from(0xb33f);
		let topic_loot = AttestationTopic::from(0x1007);
		let value_loot = AttestationValue::from(0x901d);
		let invalid_value = AttestationValue::zero();
		new_test_ext().execute_with(|| {
			let result_food = Attestation::set_claim(Origin::signed(issuer), holder, topic_food, value_food);
			let result_loot = Attestation::set_claim(Origin::signed(issuer), holder, topic_loot, value_loot);

			let result_remove = Attestation::remove_claim(Origin::signed(issuer), holder, topic_food);

			assert_ok!(result_food);
			assert_ok!(result_loot);
			assert_ok!(result_remove);

			assert_eq!(Attestation::issuers(holder), [issuer]);
			assert_eq!(Attestation::topics((holder, issuer)), [topic_loot]);
			assert_eq!(Attestation::value((holder, issuer, topic_food)), invalid_value);
			assert_eq!(Attestation::value((holder, issuer, topic_loot)), value_loot);
		})
	}

	#[test]
	fn issuer_is_removed_if_there_are_no_claims_left() {
		let issuer = 0xf00;
		let holder = 0xbaa;
		let topic_food = AttestationTopic::from(0xf00d);
		let value_food = AttestationValue::from(0xb33f);
		let topic_loot = AttestationTopic::from(0x1007);
		let value_loot = AttestationValue::from(0x901d);
		let invalid_value = AttestationValue::zero();
		new_test_ext().execute_with(|| {
			let result_food = Attestation::set_claim(Origin::signed(issuer), holder, topic_food, value_food);
			let result_loot = Attestation::set_claim(Origin::signed(issuer), holder, topic_loot, value_loot);

			let result_remove_food = Attestation::remove_claim(Origin::signed(issuer), holder, topic_food);
			let result_remove_loot = Attestation::remove_claim(Origin::signed(issuer), holder, topic_loot);

			assert_ok!(result_food);
			assert_ok!(result_loot);
			assert_ok!(result_remove_food);
			assert_ok!(result_remove_loot);

			assert_eq!(Attestation::issuers(holder), <Vec<AccountId>>::new());
			assert_eq!(Attestation::topics((holder, issuer)), []);
			assert_eq!(Attestation::value((holder, issuer, topic_food)), invalid_value);
			assert_eq!(Attestation::value((holder, issuer, topic_loot)), invalid_value);
		})
	}

	#[test]
	fn remove_claim_which_doesnt_exist_fails() {
		let issuer = 0xf00;
		let holder = 0xbaa;
		let topic = AttestationTopic::from(0xf00d);
		new_test_ext().execute_with(|| {
			assert_noop!(
				Attestation::remove_claim(Origin::signed(issuer), holder, topic),
				Error::<Test>::TopicNotRegistered
			);
		})
	}

	#[test]
	fn created_claim_emits_event() {
		let issuer = 0xf00;
		let holder = 0xbaa;
		let topic = AttestationTopic::from(0xf00d);
		let value = AttestationValue::from(0xb33f);
		new_test_ext().execute_with(|| {
			System::set_block_number(1);
			assert_ok!(Attestation::set_claim(Origin::signed(issuer), holder, topic, value));

			let expected_event = TestEvent::crml_attestation(RawEvent::ClaimCreated(holder, issuer, topic, value));
			// Assert
			assert!(System::events().iter().any(|record| record.event == expected_event));
		})
	}

	#[test]
	fn removing_claim_emits_event() {
		let issuer = 0xf00;
		let holder = 0xbaa;
		let topic = AttestationTopic::from(0xf00d);
		let value = AttestationValue::from(0xb33f);
		new_test_ext().execute_with(|| {
			System::set_block_number(1);
			assert_ok!(Attestation::set_claim(Origin::signed(issuer), holder, topic, value));
			assert_ok!(Attestation::remove_claim(Origin::signed(issuer), holder, topic));

			let expected_event = TestEvent::crml_attestation(RawEvent::ClaimRemoved(holder, issuer, topic));
			// Assert
			assert!(System::events().iter().any(|record| record.event == expected_event));
		})
	}

	#[test]
	fn updating_claim_emits_event() {
		let issuer = 0xf00;
		let holder = 0xbaa;
		let topic = AttestationTopic::from(0xf00d);
		let value_old = AttestationValue::from(0xb33f);
		let value_new = AttestationValue::from(0xcabba93);
		new_test_ext().execute_with(|| {
			System::set_block_number(1);
			assert_ok!(Attestation::set_claim(Origin::signed(issuer), holder, topic, value_old));
			assert_ok!(Attestation::set_claim(Origin::signed(issuer), holder, topic, value_new));

			let expected_event = TestEvent::crml_attestation(RawEvent::ClaimUpdated(holder, issuer, topic, value_new));
			// Assert
			assert!(System::events().iter().any(|record| record.event == expected_event));
		})
	}
}
