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
//! Provides the basic creation and management of dynamic NFTs

use frame_support::{decl_error, decl_event, decl_module, decl_storage, Parameter};
use frame_system::{ensure_signed, WeightInfo};
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, CheckedAdd, Member, One},
	DispatchResult,
};
use sp_std::prelude::*;

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
	}
);

decl_error! {
	/// Error for the staking module.
	pub enum Error for Module<T: Trait> {
		/// No more Ids are available, they've been exhausted
		NoAvailableIds,
		/// Too many fields in the provided schema or data
		MaxSchemaFields,
		/// Max tokens issued
		MaxTokensIssued,
		/// Provided fields do not match the class schema
		SchemaMismatch,
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
		pub AccountTokensByClass get(fn account_tokens): double_map hasher(twox_64_concat) T::ClassId, hasher(blake2_128_concat) T::AccountId => Vec<T::TokenId>;
		/// The next available NFT class Id
		pub NextClassId get(fn next_class_id): T::ClassId;
		/// The next available token Id for an NFT class
		pub NextTokenId get(fn next_token_id): map hasher(twox_64_concat) T::ClassId => T::TokenId;
		/// The total amount of an NFT class in existence
		pub TokenIssuance get(fn token_issuance): map hasher(twox_64_concat) T::ClassId => T::TokenId;
	}
}

/// The maximum number of fields in an NFT class schema
const MAX_SCHEMA_FIELDS: u32 = 16;

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin, system = frame_system {
		type Error = Error<T>;

		fn deposit_event() = default;

		/// Create a new NFT class
		#[weight = 0]
		fn create_class(origin, schema: NFTSchema) -> DispatchResult {
			let origin = ensure_signed(origin)?;
			// TODO: require a CENNZ deposit or governance vote to create i.e. to prevent spam.
			let class_id = Self::next_class_id();

			if schema.len() as u32 > MAX_SCHEMA_FIELDS {
				return Err(Error::<T>::MaxSchemaFields)?
			}
			// TODO: disallow empty schema

			// Store schema and owner
			<ClassSchema<T>>::insert(class_id, schema);
			<ClassOwner<T>>::insert(class_id, origin.clone());

			let new_class_id = class_id.checked_add(&One::one()).ok_or(Error::<T>::NoAvailableIds)?;
			<NextClassId<T>>::put(new_class_id);

			Self::deposit_event(RawEvent::CreateClass(class_id.clone(), origin));

			Ok(())
		}

		/// Issue a new NFT
		/// `fields` - initial values according to the NFT class/schema, any missing fields will assigned the defaults
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

			// quick data sanity check
			let schema: NFTSchema = Self::class_schema(class_id);
			if fields.len() as u32 > MAX_SCHEMA_FIELDS || fields.len() > schema.len() {
				return Err(Error::<T>::MaxSchemaFields)?
			} else if fields.len() < schema.len() {
				return Err(Error::<T>::SchemaMismatch)?
			}

			// Build the NFT + schema validation
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
					Ok(NFTField::default_from_type_id(*schema_field_type))
				}
			}).collect::<Result<Vec<NFTField>, Error<T>>>()?;

			// TODO: Add unique ID hash as first field
			// token.push_front(T::Hasher::hash((class_id.encode(), token_id.encode())));

			// Create the token, update ownership, and bookkeeping
			<Tokens<T>>::insert(class_id, token_id, token);
			<NextTokenId<T>>::insert(class_id, next_token_id);
			<TokenIssuance<T>>::mutate(class_id, |i| *i += One::one());
			<TokenOwner<T>>::insert(class_id, token_id, owner.clone());
			<AccountTokensByClass<T>>::append(class_id, owner.clone(), token_id);

			Self::deposit_event(RawEvent::CreateToken(class_id.clone(), token_id.clone(), owner));

			Ok(())
		}

		/// Update an existing NFT
		/// `new_fields` - new values according to the NFT class/schema, omitted fields will retain their current values.
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

			// quick data sanity check
			let schema: NFTSchema = Self::class_schema(class_id);
			if new_fields.len() as u32 > MAX_SCHEMA_FIELDS || new_fields.len() > schema.len() {
				return Err(Error::<T>::MaxSchemaFields)?
			} else if new_fields.len() < schema.len() {
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
			Self::deposit_event(RawEvent::Update(class_id.clone(), token_id.clone()));

			Ok(())
		}

		/// Transfer ownership of an NFT
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
			<AccountTokensByClass<T>>::append(class_id, new_owner, token_id);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{ExtBuilder, Origin, Test};
	use frame_support::assert_ok;
	use frame_system::RawOrigin;
	use sp_runtime::DispatchError::Other;

	type Nft = Module<Test>;

	impl Trait for Test {
		type Event = ();
		type ClassId = u32;
		type TokenId = u32;
		type WeightInfo = ();
	}

	#[test]
	fn create_class() {}

	#[test]
	fn create_token() {}

	#[test]
	fn update_token() {}

	#[test]
	fn transfer() {}
}
