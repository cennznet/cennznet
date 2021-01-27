/* Copyright 2019-2021 Centrality Investments Limited
*
* Licensed under the LGPL, Version 3.0 (the "License");
* you may not use this file except in compliance with the License.
* Unless required by applicable law or agreed to in writing, software
* distributed under the License is distributed on an "AS IS" BASIS,
* WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
* See the License for the specific language governing permissions and
* limitations under the License.
* You may obtain a copy of the License at the root of this project source code,
* or at:
*     https://centrality.ai/licenses/gplv3.txt
*     https://centrality.ai/licenses/lgplv3.txt
*/
#![cfg_attr(not(feature = "std"), no_std)]

//! # NFT Module
//!
//! Provides the basic creation and management of dynamic NFTs (created at runtime).
//!
//! Intended to be used "as is" by dapps and provide basic NFT feature set for smart contracts
//! to extend.

use frame_support::{decl_error, decl_event, decl_module, decl_storage, Parameter};
use frame_system::{ensure_signed, WeightInfo};
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, CheckedAdd, Member, One},
	DispatchResult,
};
use sp_std::prelude::*;

#[cfg(test)]
mod mock;
mod types;
use types::{NFTField, NFTSchema};

pub trait Trait: frame_system::Trait {
	/// The system event type
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
	/// Type for identifying token classes
	type ClassId: Parameter + Member + AtLeast32BitUnsigned + Default + Copy + One + Into<u64>;
	/// Type for identifying tokens
	type TokenId: Parameter + Member + AtLeast32BitUnsigned + Default + Copy + One + Into<u64>;
	/// Provides the public call to weight mapping
	type WeightInfo: WeightInfo;
}

decl_event!(
	pub enum Event<T> where ClassId = <T as Trait>::ClassId, TokenId = <T as Trait>::TokenId, <T as frame_system::Trait>::AccountId {
		/// A new NFT class was created, (class Id, owner)
		CreateClass(ClassId, AccountId),
		/// A new NFT was created, (class Id, token Id owner)
		CreateToken(ClassId, TokenId, AccountId),
		/// An NFT was transferred (class Id, token Id, new owner)
		Transfer(ClassId, TokenId, AccountId),
		/// An NFT's data was updated
		Update(ClassId, TokenId),
		/// An NFT was burned
		Burn(ClassId, TokenId),
	}
);

decl_error! {
	/// Error for the staking module.
	pub enum Error for Module<T: Trait> {
		/// No more Ids are available, they've been exhausted
		NoAvailableIds,
		/// Max tokens issued
		MaxTokensIssued,
		/// Too many fields in the provided schema or data
		SchemaMaxFields,
		/// Provided fields do not match the class schema
		SchemaMismatch,
		/// The provided fields or schema cannot be empty
		SchemaEmpty,
		/// The schema contains an invalid type
		SchemaInvalid,
		/// origin does not have permission for the operation
		NoPermission,
		/// The NFT class does not exist
		NoClass,
		/// The NFT does not exist
		NoToken,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as Nft {
		/// Map from class to owner address
		pub ClassOwner get(fn class_owner): map hasher(twox_64_concat) T::ClassId => T::AccountId;
		/// Map from class to schema definition
		pub ClassSchema get(fn class_schema): map hasher(twox_64_concat) T::ClassId => NFTSchema;
		/// Map from (class, token) to it's encoded value
		pub Tokens get(fn tokens): double_map hasher(twox_64_concat) T::ClassId, hasher(twox_64_concat) T::TokenId => Vec<NFTField>;
		/// Map from (class, token) to it's owner
		pub TokenOwner get(fn token_owner): double_map hasher(twox_64_concat) T::ClassId, hasher(twox_64_concat) T::TokenId => T::AccountId;
		/// Map from (class, account) to it's owned tokens of that class
		pub AccountTokensByClass get(fn account_tokens_by_class): double_map hasher(twox_64_concat) T::ClassId, hasher(blake2_128_concat) T::AccountId => Vec<T::TokenId>;
		/// The next available NFT class Id
		pub NextClassId get(fn next_class_id): T::ClassId;
		/// The next available token Id for an NFT class
		pub NextTokenId get(fn next_token_id): map hasher(twox_64_concat) T::ClassId => T::TokenId;
		/// The total amount of an NFT class in existence
		pub TokenIssuance get(fn token_issuance): map hasher(twox_64_concat) T::ClassId => T::TokenId;
		/// The total amount of an NFT class burned
		pub TokensBurnt get(fn tokens_burnt): map hasher(twox_64_concat) T::ClassId => T::TokenId;
	}
}

/// The maximum number of fields in an NFT class schema
// TODO: use onchain modifiable value
const MAX_SCHEMA_FIELDS: u32 = 16;

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin, system = frame_system {
		type Error = Error<T>;

		fn deposit_event() = default;

		/// Create a new NFT class
		/// The caller will be come the class' owner
		#[weight = 0]
		fn create_class(origin, schema: NFTSchema) -> DispatchResult {
			let origin = ensure_signed(origin)?;
			// TODO: require a CENNZ deposit or governance vote to create i.e. to prevent spam.
			let class_id = Self::next_class_id();

			if schema.is_empty() {
				return Err(Error::<T>::SchemaEmpty)?
			} else if schema.len() as u32 > MAX_SCHEMA_FIELDS {
				return Err(Error::<T>::SchemaMaxFields)?
			}

			// Check the provided field types are valid
			if !schema.iter().any(|type_id| NFTField::is_valid_type_id(*type_id)) {
				return Err(Error::<T>::SchemaInvalid)?
			}

			// Store schema and owner
			<ClassSchema<T>>::insert(class_id, schema);
			<ClassOwner<T>>::insert(class_id, origin.clone());

			let new_class_id = class_id.checked_add(&One::one()).ok_or(Error::<T>::NoAvailableIds)?;
			<NextClassId<T>>::put(new_class_id);

			Self::deposit_event(RawEvent::CreateClass(class_id, origin));

			Ok(())
		}

		/// Issue a new NFT
		/// `fields` - initial values according to the NFT class/schema, omitted fields will be assigned defaults
		/// Caller must be the class owner
		#[weight = 0]
		fn create_token(origin, class_id: T::ClassId, owner: T::AccountId, fields: Vec<Option<NFTField>>) -> DispatchResult {
			let origin = ensure_signed(origin)?;

			// Permission and existence check
			let class_owner = Self::class_owner(class_id);
			if class_owner == T::AccountId::default() {
				return Err(Error::<T>::NoClass)?
			} else if class_owner != origin {
				// restricted to class owner
				return Err(Error::<T>::NoPermission)?
			}

			// Check we can issue a new token
			let token_id = Self::next_token_id(class_id);
			let next_token_id = token_id.checked_add(&One::one()).ok_or(Error::<T>::NoAvailableIds)?;
			Self::token_issuance(class_id).checked_add(&One::one()).ok_or(Error::<T>::MaxTokensIssued)?;

			// Quick `fields` sanity checks
			if fields.is_empty() {
				return Err(Error::<T>::SchemaEmpty)?
			}
			if fields.len() as u32 > MAX_SCHEMA_FIELDS {
				return Err(Error::<T>::SchemaMaxFields)?
			}
			let schema: NFTSchema = Self::class_schema(class_id);
			if fields.len() != schema.len() {
				return Err(Error::<T>::SchemaMismatch)?
			}

			// Build the NFT + schema type level validation
			let token: Vec<NFTField> = schema.iter().zip(fields.iter()).map(|(schema_field_type, maybe_provided_field)| {
				if let Some(provided_field) = maybe_provided_field {
					// caller provided a field, check it's the right type
					if *schema_field_type == provided_field.type_id() {
						Ok(provided_field.clone())
					} else {
						Err(Error::<T>::SchemaMismatch)?
					}
				} else {
					// caller did not provide a field, use the default
					NFTField::default_from_type_id(*schema_field_type).map_err(|_| Error::<T>::SchemaInvalid)
				}
			}).collect::<Result<Vec<NFTField>, Error<T>>>()?;

			// TODO: Add unique ID hash as first field

			// Create the token, update ownership, and bookkeeping
			<Tokens<T>>::insert(class_id, token_id, token);
			<NextTokenId<T>>::insert(class_id, next_token_id);
			<TokenIssuance<T>>::mutate(class_id, |i| *i += One::one());
			<TokenOwner<T>>::insert(class_id, token_id, owner.clone());
			<AccountTokensByClass<T>>::append(class_id, owner.clone(), token_id);

			Self::deposit_event(RawEvent::CreateToken(class_id, token_id, owner));

			Ok(())
		}

		/// Update an existing NFT
		/// `new_fields` - new values according to the NFT class/schema, omitted fields will retain their current values.
		/// Caller must be the class owner
		#[weight = 0]
		fn update_token(origin, class_id: T::ClassId, token_id: T::TokenId, new_fields: Vec<Option<NFTField>>) -> DispatchResult {
			let origin = ensure_signed(origin)?;

			// Permission and existence check
			let class_owner = Self::class_owner(class_id);
			if class_owner == T::AccountId::default() {
				return Err(Error::<T>::NoClass)?
			} else if class_owner != origin {
				// restricted to class owner
				return Err(Error::<T>::NoPermission)?
			}

			if !<Tokens<T>>::contains_key(class_id, token_id) {
				return Err(Error::<T>::NoToken)?
			}

			// Quick `new_fields` sanity checks
			if new_fields.is_empty() {
				return Err(Error::<T>::SchemaEmpty)?
			}
			if new_fields.len() as u32 > MAX_SCHEMA_FIELDS {
				return Err(Error::<T>::SchemaMaxFields)?
			}
			let schema: NFTSchema = Self::class_schema(class_id);
			if new_fields.len() != schema.len() {
				return Err(Error::<T>::SchemaMismatch)?
			}

			// Rebuild the NFT inserting new values
			let current_token: Vec<NFTField> = Self::tokens(class_id, token_id);
			let token: Vec<NFTField> = current_token.iter().zip(new_fields.iter()).map(|(current_value, maybe_new_value)| {
				if let Some(new_value) = maybe_new_value {
					// existing types are valid
					if current_value.type_id() == new_value.type_id() {
						Ok(new_value.clone())
					} else {
						Err(Error::<T>::SchemaMismatch)?
					}
				} else {
					// caller did not provide a new value, retain the existing value
					Ok(current_value.clone())
				}
			}).collect::<Result<Vec<NFTField>, Error<T>>>()?;

			<Tokens<T>>::insert(class_id, token_id, token);
			Self::deposit_event(RawEvent::Update(class_id, token_id));

			Ok(())
		}

		/// Transfer ownership of an NFT
		/// Caller must be the token owner
		#[weight = 0]
		fn transfer(origin, class_id: T::ClassId, token_id: T::TokenId, new_owner: T::AccountId) {
			let origin = ensure_signed(origin)?;

			if !<ClassOwner<T>>::contains_key(class_id) {
				return Err(Error::<T>::NoClass)?
			}

			if !<Tokens<T>>::contains_key(class_id, token_id) {
				return Err(Error::<T>::NoToken)?
			}

			let current_owner = Self::token_owner(class_id, token_id);
			if origin != current_owner {
				// restricted to token owner
				return Err(Error::<T>::NoPermission)?
			}

			// Update token ownership
			<AccountTokensByClass<T>>::mutate(class_id, current_owner, |tokens| {
				tokens.retain(|t| t != &token_id)
			});
			<TokenOwner<T>>::insert(class_id, token_id, new_owner.clone());
			<AccountTokensByClass<T>>::append(class_id, new_owner.clone(), token_id);

			Self::deposit_event(RawEvent::Transfer(class_id, token_id, new_owner));
		}

		/// Burn an NFT 🔥
		/// Caller must be the token owner
		#[weight = 0]
		fn burn(origin, class_id: T::ClassId, token_id: T::TokenId) {
			let origin = ensure_signed(origin)?;

			if !<ClassOwner<T>>::contains_key(class_id) {
				return Err(Error::<T>::NoClass)?
			}

			if !<Tokens<T>>::contains_key(class_id, token_id) {
				return Err(Error::<T>::NoToken)?
			}

			let current_owner = Self::token_owner(class_id, token_id);
			if origin != current_owner {
				// restricted to token owner
				return Err(Error::<T>::NoPermission)?
			}

			// Update token ownership
			<AccountTokensByClass<T>>::mutate(class_id, current_owner, |tokens| {
				tokens.retain(|t| t != &token_id)
			});
			<TokenOwner<T>>::take(class_id, token_id);
			<Tokens<T>>::take(class_id, token_id);

			// Will not overflow, cannot exceed the amount issued qed.
			let tokens_burnt = Self::tokens_burnt(class_id).checked_add(&One::one()).unwrap();
			<TokensBurnt<T>>::insert(class_id, tokens_burnt);

			Self::deposit_event(RawEvent::Burn(class_id, token_id));
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{Event, ExtBuilder, Test};
	use crate::types::*;
	use frame_support::{assert_noop, assert_ok};

	type Nft = Module<Test>;
	type System = frame_system::Module<Test>;
	type AccountId = u64;
	type ClassId = u32;
	type TokenId = u32;

	impl Trait for Test {
		type Event = Event;
		type ClassId = ClassId;
		type TokenId = TokenId;
		type WeightInfo = ();
	}

	// Check the test system contains an event record `event`
	fn has_event(event: RawEvent<ClassId, TokenId, AccountId>) -> bool {
		System::events()
			.iter()
			.find(|e| e.event == Event::nft(event.clone()))
			.is_some()
	}

	// Create an NFT class with schema
	// Returns the created `class_id`
	fn setup_class(owner: AccountId, schema: NFTSchema) -> ClassId {
		let class_id = Nft::next_class_id();
		assert_ok!(Nft::create_class(Some(owner).into(), schema.clone()));
		class_id
	}

	#[test]
	fn create_class() {
		ExtBuilder::default().build().execute_with(|| {
			let class_id = Nft::next_class_id();
			assert_eq!(Nft::next_class_id(), 0);
			let owner = 1_u64;
			let schema = vec![
				NFTField::U8(Default::default()).type_id(),
				NFTField::U8(Default::default()).type_id(),
				NFTField::Bytes32(Default::default()).type_id(),
			];

			assert_ok!(Nft::create_class(Some(owner).into(), schema.clone()));
			assert!(has_event(RawEvent::CreateClass(class_id, owner)));

			assert_eq!(Nft::next_class_id(), class_id.checked_add(One::one()).unwrap());
			assert_eq!(Nft::class_owner(class_id), owner);
			assert_eq!(Nft::class_schema(class_id), schema);
		});
	}

	#[test]
	fn create_class_invalid_schema() {
		ExtBuilder::default().build().execute_with(|| {
			assert_noop!(
				Nft::create_class(Some(1_u64).into(), vec![]),
				Error::<Test>::SchemaEmpty
			);

			let too_many_fields = [0_u8; (MAX_SCHEMA_FIELDS + 1_u32) as usize];
			assert_noop!(
				Nft::create_class(Some(1_u64).into(), too_many_fields.to_owned().to_vec()),
				Error::<Test>::SchemaMaxFields
			);

			let invalid_nft_field_type: NFTFieldTypeId = 200;
			assert_noop!(
				Nft::create_class(Some(1_u64).into(), vec![invalid_nft_field_type]),
				Error::<Test>::SchemaInvalid
			);
		});
	}

	#[test]
	fn create_token() {
		ExtBuilder::default().build().execute_with(|| {
			let schema = vec![
				NFTField::I32(Default::default()).type_id(),
				NFTField::U8(Default::default()).type_id(),
				NFTField::Bytes32(Default::default()).type_id(),
			];
			let class_owner = 1_u64;
			let class_id = setup_class(class_owner, schema);

			let token_owner = 2_u64;
			let token_id = 0;
			assert_eq!(Nft::next_token_id(class_id), token_id);
			assert_ok!(Nft::create_token(
				Some(class_owner).into(),
				class_id,
				token_owner,
				vec![Some(NFTField::I32(-33)), None, Some(NFTField::Bytes32([1_u8; 32]))]
			));
			assert!(has_event(RawEvent::CreateToken(
				class_id,
				token_id,
				token_owner.clone()
			)));

			let token = Nft::tokens(class_id, token_id);
			assert_eq!(
				token,
				vec![
					NFTField::I32(-33),
					NFTField::U8(Default::default()),
					NFTField::Bytes32([1_u8; 32])
				],
			);

			assert_eq!(Nft::token_owner(class_id, token_id), token_owner);
			assert_eq!(Nft::account_tokens_by_class(class_id, token_owner), vec![token_id]);
			assert_eq!(Nft::next_token_id(class_id), token_id.checked_add(One::one()).unwrap());
			assert_eq!(Nft::token_issuance(class_id), 1);
		});
	}

	#[test]
	fn create_multiple_tokens() {
		ExtBuilder::default().build().execute_with(|| {
			let schema = vec![
				NFTField::I32(Default::default()).type_id(),
				NFTField::U8(Default::default()).type_id(),
				NFTField::Bytes32(Default::default()).type_id(),
			];
			let class_owner = 1_u64;
			let class_id = setup_class(class_owner, schema);
			let token_owner = 2_u64;

			assert_ok!(Nft::create_token(
				Some(class_owner).into(),
				class_id,
				token_owner,
				vec![Some(NFTField::I32(-33)), None, Some(NFTField::Bytes32([1_u8; 32]))]
			));

			assert_ok!(Nft::create_token(
				Some(class_owner).into(),
				class_id,
				token_owner,
				vec![Some(NFTField::I32(33)), None, Some(NFTField::Bytes32([2_u8; 32]))]
			));
			assert!(has_event(RawEvent::CreateToken(class_id, 1, token_owner.clone())));

			assert_eq!(Nft::token_owner(class_id, 1), token_owner);
			assert_eq!(Nft::account_tokens_by_class(class_id, token_owner), vec![0, 1]);
			assert_eq!(Nft::next_token_id(class_id), 2);
			assert_eq!(Nft::token_issuance(class_id), 2);
		});
	}

	#[test]
	fn create_token_fails_prechecks() {
		ExtBuilder::default().build().execute_with(|| {
			let schema = vec![NFTField::I32(Default::default()).type_id()];
			let class_owner = 1_u64;
			let class_id = setup_class(class_owner, schema);

			assert_noop!(
				Nft::create_token(Some(2_u64).into(), class_id, class_owner, vec![None]),
				Error::<Test>::NoPermission
			);

			assert_noop!(
				Nft::create_token(Some(class_owner).into(), class_id + 1, class_owner, vec![None]),
				Error::<Test>::NoClass
			);

			assert_noop!(
				Nft::create_token(Some(class_owner).into(), class_id, class_owner, vec![]),
				Error::<Test>::SchemaEmpty
			);

			// additional field vs. schema
			assert_noop!(
				Nft::create_token(Some(class_owner).into(), class_id, class_owner, vec![None, None]),
				Error::<Test>::SchemaMismatch
			);

			// different field type vs. schema
			assert_noop!(
				Nft::create_token(
					Some(class_owner).into(),
					class_id,
					class_owner,
					vec![Some(NFTField::U32(404))]
				),
				Error::<Test>::SchemaMismatch
			);

			let too_many_fields: [Option<NFTField>; (MAX_SCHEMA_FIELDS + 1_u32) as usize] = Default::default();
			assert_noop!(
				Nft::create_token(
					Some(class_owner).into(),
					class_id,
					class_owner,
					too_many_fields.to_owned().to_vec()
				),
				Error::<Test>::SchemaMaxFields
			);
		});
	}

	#[test]
	fn update_token() {
		ExtBuilder::default().build().execute_with(|| {
			// setup token class + one token
			let schema = vec![
				NFTField::I32(Default::default()).type_id(),
				NFTField::U8(Default::default()).type_id(),
				NFTField::Bytes32(Default::default()).type_id(),
			];
			let class_owner = 1_u64;
			let class_id = setup_class(class_owner, schema);
			let token_owner = 2_u64;
			let token_id = Nft::next_token_id(class_id);
			let initial_values = vec![
				Some(NFTField::I32(-33)),
				Some(NFTField::U8(12)),
				Some(NFTField::Bytes32([1_u8; 32])),
			];
			assert_ok!(Nft::create_token(
				Some(class_owner).into(),
				class_id,
				token_owner,
				initial_values.clone(),
			));

			// test
			assert_ok!(Nft::update_token(
				Some(class_owner).into(),
				class_id,
				token_id,
				// only change the bytes32 value
				vec![None, None, Some(NFTField::Bytes32([2_u8; 32]))]
			));
			assert!(has_event(RawEvent::Update(class_id, token_id)));

			assert_eq!(
				Nft::tokens(class_id, token_id),
				// original values retained, bytes32 updated
				vec![
					initial_values[0].unwrap(),
					initial_values[1].unwrap(),
					NFTField::Bytes32([2_u8; 32]),
				],
			);
		});
	}

	#[test]
	fn update_token_fails_prechecks() {
		ExtBuilder::default().build().execute_with(|| {
			let schema = vec![NFTField::I32(Default::default()).type_id()];
			let class_owner = 1_u64;
			let class_id = setup_class(class_owner, schema);
			let token_owner = 2_u64;
			let token_id = Nft::next_token_id(class_id);
			assert_ok!(Nft::create_token(
				Some(class_owner).into(),
				class_id,
				token_owner,
				vec![None],
			));

			assert_noop!(
				Nft::update_token(Some(2_u64).into(), class_id, token_id, vec![None]),
				Error::<Test>::NoPermission
			);

			assert_noop!(
				Nft::update_token(Some(class_owner).into(), class_id + 1, token_id, vec![None]),
				Error::<Test>::NoClass
			);

			assert_noop!(
				Nft::update_token(Some(class_owner).into(), class_id, token_id, vec![]),
				Error::<Test>::SchemaEmpty
			);

			// additional field vs. schema
			assert_noop!(
				Nft::update_token(Some(class_owner).into(), class_id, token_id, vec![None, None]),
				Error::<Test>::SchemaMismatch
			);

			// different field type vs. schema
			assert_noop!(
				Nft::update_token(
					Some(class_owner).into(),
					class_id,
					token_id,
					vec![Some(NFTField::U32(404))]
				),
				Error::<Test>::SchemaMismatch
			);

			let too_many_fields: [Option<NFTField>; (MAX_SCHEMA_FIELDS + 1_u32) as usize] = Default::default();
			assert_noop!(
				Nft::update_token(
					Some(class_owner).into(),
					class_id,
					token_id,
					too_many_fields.to_owned().to_vec()
				),
				Error::<Test>::SchemaMaxFields
			);
		});
	}

	#[test]
	fn transfer() {
		ExtBuilder::default().build().execute_with(|| {
			// setup token class + one token
			let schema = vec![NFTField::I32(Default::default()).type_id()];
			let class_owner = 1_u64;
			let class_id = setup_class(class_owner, schema);
			let token_owner = 2_u64;
			let token_id = Nft::next_token_id(class_id);
			assert_ok!(Nft::create_token(
				Some(class_owner).into(),
				class_id,
				token_owner,
				vec![Some(NFTField::I32(500))],
			));

			// test
			let new_owner = 3_u64;
			assert_ok!(Nft::transfer(Some(token_owner).into(), class_id, token_id, new_owner,));
			assert!(has_event(RawEvent::Transfer(class_id, token_id, new_owner)));

			assert_eq!(Nft::token_owner(class_id, token_id), new_owner);
			assert!(Nft::account_tokens_by_class(class_id, token_owner).is_empty());
			assert_eq!(Nft::account_tokens_by_class(class_id, new_owner), vec![token_id]);
		});
	}

	#[test]
	fn transfer_fails_prechecks() {
		ExtBuilder::default().build().execute_with(|| {
			// setup token class + one token
			let schema = vec![NFTField::I32(Default::default()).type_id()];
			let class_owner = 1_u64;

			// no class yet
			assert_noop!(
				Nft::transfer(Some(class_owner).into(), 0, 1, class_owner),
				Error::<Test>::NoClass,
			);

			let class_id = setup_class(class_owner, schema);
			let token_owner = 2_u64;
			let token_id = Nft::next_token_id(class_id);

			// no token yet
			assert_noop!(
				Nft::transfer(Some(token_owner).into(), class_id, token_id, token_owner),
				Error::<Test>::NoToken,
			);

			assert_ok!(Nft::create_token(
				Some(class_owner).into(),
				class_id,
				token_owner,
				vec![Some(NFTField::I32(500))],
			));

			let not_the_owner = 3_u64;
			assert_noop!(
				Nft::transfer(Some(not_the_owner).into(), class_id, token_id, not_the_owner),
				Error::<Test>::NoPermission,
			);
		});
	}

	#[test]
	fn burn() {
		ExtBuilder::default().build().execute_with(|| {
			// setup token class + one token
			let schema = vec![NFTField::I32(Default::default()).type_id()];
			let class_owner = 1_u64;
			let class_id = setup_class(class_owner, schema);
			let token_owner = 2_u64;
			let token_id = Nft::next_token_id(class_id);
			assert_ok!(Nft::create_token(
				Some(class_owner).into(),
				class_id,
				token_owner,
				vec![Some(NFTField::I32(500))],
			));

			// test
			assert_ok!(Nft::burn(Some(token_owner).into(), class_id, token_id));
			assert!(has_event(RawEvent::Burn(class_id, token_id)));

			assert!(!<Tokens<Test>>::contains_key(class_id, token_id));
			assert!(!<TokenOwner<Test>>::contains_key(class_id, token_id));
			assert!(Nft::account_tokens_by_class(class_id, token_owner).is_empty());
			assert_eq!(Nft::tokens_burnt(class_id), 1);
		});
	}

	#[test]
	fn burn_fails_prechecks() {
		ExtBuilder::default().build().execute_with(|| {
			// setup token class + one token
			let schema = vec![NFTField::I32(Default::default()).type_id()];
			let class_owner = 1_u64;
			assert_noop!(Nft::burn(Some(class_owner).into(), 0, 0), Error::<Test>::NoClass,);

			let class_id = setup_class(class_owner, schema);
			let token_owner = 2_u64;
			let token_id = Nft::next_token_id(class_id);
			assert_noop!(
				Nft::burn(Some(token_owner).into(), class_id, token_id),
				Error::<Test>::NoToken,
			);

			assert_ok!(Nft::create_token(
				Some(class_owner).into(),
				class_id,
				token_owner,
				vec![Some(NFTField::I32(500))],
			));

			assert_noop!(
				Nft::burn(Some(3_u64).into(), class_id, token_id),
				Error::<Test>::NoPermission,
			);
		});
	}
}
