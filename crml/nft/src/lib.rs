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

use cennznet_primitives::types::{AssetId, Balance};
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage, ensure,
	traits::{ExistenceRequirement, Get},
	weights::Weight,
	Parameter,
};
use frame_system::{ensure_signed, WeightInfo};
use prml_support::MultiCurrencyAccounting;
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, CheckedAdd, Member, One, Saturating, Zero},
	DispatchResult,
};
use sp_std::prelude::*;

#[cfg(test)]
mod mock;
mod types;
use types::*;

pub trait Trait: frame_system::Trait {
	/// The system event type
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
	/// Type for identifying tokens
	type TokenId: Parameter + Member + AtLeast32BitUnsigned + Default + Copy + One + Into<u64>;
	/// Default auction / sale length in blocks
	type DefaultListingDuration: Get<Self::BlockNumber>;
	/// Handles a multi-currency fungible asset system
	type MultiCurrency: MultiCurrencyAccounting<AccountId = Self::AccountId, CurrencyId = AssetId, Balance = Balance>;
	/// Provides the public call to weight mapping
	type WeightInfo: WeightInfo;
}

decl_event!(
	pub enum Event<T> where CollectionId = CollectionId, <T as Trait>::TokenId, <T as frame_system::Trait>::AccountId, AssetId = AssetId, Balance = Balance {
		/// A new NFT collection was created, (collection, owner)
		CreateCollection(CollectionId, AccountId),
		/// A new NFT was created, (collection, token, owner)
		CreateToken(CollectionId, TokenId, AccountId),
		/// An NFT was transferred (collection, token, new owner)
		Transfer(CollectionId, TokenId, AccountId),
		/// An NFT's data was updated
		Update(CollectionId, TokenId),
		/// An NFT was burned
		Burn(CollectionId, TokenId),
		/// A direct sale has been listed (collection, token, authorised buyer, payment asset, fixed price)
		DirectSaleListed(CollectionId, TokenId, AccountId, AssetId, Balance),
		/// A direct sale has completed (collection, token, new owner, payment asset, fixed price)
		DirectSaleComplete(CollectionId, TokenId, AccountId, AssetId, Balance),
	}
);

decl_error! {
	/// Error for the staking module.
	pub enum Error for Module<T: Trait> {
		/// A collection with the same ID already exists
		CollectionIdExists,
		/// Given collection ID is not valid utf-8
		CollectionIdInvalid,
		/// No more Ids are available, they've been exhausted
		NoAvailableIds,
		/// Max tokens issued
		MaxTokensIssued,
		/// Too many fields in the provided schema or data
		SchemaMaxFields,
		/// Provided fields do not match the collection schema
		SchemaMismatch,
		/// The provided fields or schema cannot be empty
		SchemaEmpty,
		/// The schema contains an invalid type
		SchemaInvalid,
		/// origin does not have permission for the operation
		NoPermission,
		/// The NFT collection does not exist
		NoCollection,
		/// The NFT does not exist
		NoToken,
		/// The NFT is not listed for a direct sale
		NotForDirectSale,
		/// Cannot operate on a listed NFT
		TokenListingProtection,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as Nft {
		/// Map from collection to owner address
		pub CollectionOwner get(fn collection_owner): map hasher(blake2_128_concat) CollectionId => Option<T::AccountId>;
		/// Map from collection to schema definition
		pub CollectionSchema get(fn collection_schema): map hasher(blake2_128_concat) CollectionId => Option<NFTSchema>;
		/// Map from collection to it's defacto royalty scheme
		pub CollectionRoyalties get(fn collection_royalties): map hasher(blake2_128_concat) CollectionId => RoyaltiesPlan<T::AccountId>;
		/// Map from a token to it's royalty scheme
		pub TokenRoyalties get(fn token_royalties): double_map hasher(blake2_128_concat) CollectionId, hasher(twox_64_concat) T::TokenId => Option<RoyaltiesPlan<T::AccountId>>;
		/// Map from (collection, token) to it's encoded value
		pub Tokens get(fn tokens): double_map hasher(blake2_128_concat) CollectionId, hasher(twox_64_concat) T::TokenId => Vec<NFTField>;
		/// The next available token Id for an NFT collection
		pub NextTokenId get(fn next_token_id): map hasher(twox_64_concat) CollectionId => T::TokenId;
		/// The total amount of an NFT collection in existence
		/// Map from (collection, token) to it's owner
		pub TokenOwner get(fn token_owner): double_map hasher(blake2_128_concat) CollectionId, hasher(blake2_128_concat) T::TokenId => T::AccountId;
		/// Map from (collection, account) to the account owned tokens of that collection
		pub CollectedTokens get(fn collected_tokens): double_map hasher(blake2_128_concat) CollectionId, hasher(blake2_128_concat) T::AccountId => Vec<T::TokenId>;
		/// The total amount of an NFT collection in existence
		pub TokenIssuance get(fn token_issuance): map hasher(blake2_128_concat) CollectionId => T::TokenId;
		/// The total amount of an NFT collection burned
		pub TokensBurnt get(fn tokens_burnt): map hasher(blake2_128_concat) CollectionId => T::TokenId;
		/// NFT sale/auction listings. keyed by collection id and token id
		pub Listings get(fn listings): double_map hasher(blake2_128_concat) CollectionId, hasher(twox_64_concat) T::TokenId => Option<Listing<T>>;
		/// Winning bids on open listings. keyed by collection id and token id
		pub ListingWinningBid get(fn winning_bid): double_map hasher(blake2_128_concat) CollectionId, hasher(twox_64_concat) T::TokenId => Option<(T::AccountId, Balance)>;
		/// Map from block numbers to listings scheduled to close
		pub ListingEndSchedule get(fn listing_end_blocks): map hasher(twox_64_concat) T::BlockNumber => Vec<(CollectionId, T::TokenId)>;
	}
}

/// The maximum number of fields in an NFT collection schema
pub const MAX_SCHEMA_FIELDS: u32 = 16;
/// The maximum length of valid collection IDs
pub const MAX_COLLECTION_ID_LEN: u8 = 32;

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin, system = frame_system {
		type Error = Error<T>;

		fn deposit_event() = default;

		fn on_initialize(now: T::BlockNumber) -> Weight {
			if !ListingEndSchedule::<T>::contains_key(now) {
				return Zero::zero();
			}
			let listings = ListingEndSchedule::<T>::take(now);
			Self::close_listings(listings.as_slice());

			// TODO: use benchmarked value
			listings.len() as Weight
		}

		/// Create a new NFT collection
		/// The caller will be come the collection' owner
		/// `collection_id`- 32 byte utf-8 string
		/// `schema` - for the collection
		/// `royalties_plan` - defacto royalties plan for secondary sales
		#[weight = 0]
		fn create_collection(origin, collection_id: CollectionId, schema: NFTSchema, royalties_plan: RoyaltiesPlan<T::AccountId>) -> DispatchResult {
			let origin = ensure_signed(origin)?;

			ensure!(!collection_id.is_empty() && collection_id.len() <= MAX_COLLECTION_ID_LEN as usize, Error::<T>::CollectionIdInvalid);
			ensure!(core::str::from_utf8(&collection_id).is_ok(), Error::<T>::CollectionIdInvalid);
			ensure!(!CollectionSchema::contains_key(&collection_id), Error::<T>::CollectionIdExists);

			ensure!(!schema.is_empty(), Error::<T>::SchemaEmpty);
			ensure!(schema.len() <= MAX_SCHEMA_FIELDS as usize, Error::<T>::SchemaMaxFields);
			// Check the provided field types are valid
			ensure!(
				schema.iter().all(|type_id| NFTField::is_valid_type_id(*type_id)),
				Error::<T>::SchemaInvalid
			);

			// Store schema and owner
			CollectionSchema::insert(&collection_id, schema);
			<CollectionRoyalties<T>>::insert(&collection_id, royalties_plan);
			<CollectionOwner<T>>::insert(&collection_id, origin.clone());

			Self::deposit_event(RawEvent::CreateCollection(collection_id, origin));

			Ok(())
		}

		/// Issue a new NFT
		/// `owner` - the token owner
		/// `fields` - initial values according to the NFT collection/schema, omitted fields will be assigned defaults
		/// `royalties_plan` - optional royalty plan for secondary sales of this token, defaults to the collection plan
		/// Caller must be the collection owner
		#[weight = 0]
		fn create_token(origin, collection_id: CollectionId, owner: T::AccountId, fields: Vec<Option<NFTField>>, royalties_plan: Option<RoyaltiesPlan<T::AccountId>>) -> DispatchResult {
			let origin = ensure_signed(origin)?;

			// Permission and existence check
			let collection_owner = Self::collection_owner(&collection_id);
			ensure!(collection_owner.is_some(), Error::<T>::NoCollection);
			ensure!(collection_owner.unwrap() == origin, Error::<T>::NoPermission);

			// Check we can issue a new token
			let token_id = Self::next_token_id(&collection_id);
			let next_token_id = token_id.checked_add(&One::one()).ok_or(Error::<T>::NoAvailableIds)?;
			Self::token_issuance(&collection_id).checked_add(&One::one()).ok_or(Error::<T>::MaxTokensIssued)?;

			// Quick `fields` sanity checks
			ensure!(!fields.is_empty(), Error::<T>::SchemaEmpty);
			ensure!(fields.len() as u32 <= MAX_SCHEMA_FIELDS, Error::<T>::SchemaMaxFields);
			let schema: NFTSchema = Self::collection_schema(&collection_id).ok_or(Error::<T>::NoCollection)?;
			ensure!(fields.len() == schema.len(), Error::<T>::SchemaMismatch);

			// Build the NFT + schema type level validation
			let token: Vec<NFTField> = schema.iter().zip(fields.iter()).map(|(schema_field_type, maybe_provided_field)| {
				if let Some(provided_field) = maybe_provided_field {
					// caller provided a field, check it's the right type
					if *schema_field_type == provided_field.type_id() {
						Ok(*provided_field)
					} else {
						Err(Error::<T>::SchemaMismatch)
					}
				} else {
					// caller did not provide a field, use the default
					NFTField::default_from_type_id(*schema_field_type).map_err(|_| Error::<T>::SchemaInvalid)
				}
			}).collect::<Result<Vec<NFTField>, Error<T>>>()?;

			// Create the token, update ownership, and bookkeeping
			<Tokens<T>>::insert(&collection_id, token_id, token);
			if let Some(royalties_plan) = royalties_plan {
				<TokenRoyalties<T>>::insert(&collection_id, token_id, royalties_plan);
			}
			<NextTokenId<T>>::insert(&collection_id, next_token_id);
			<TokenIssuance<T>>::mutate(&collection_id, |i| *i += One::one());
			<TokenOwner<T>>::insert(&collection_id, token_id, owner.clone());
			<CollectedTokens<T>>::append(&collection_id, owner.clone(), token_id);

			Self::deposit_event(RawEvent::CreateToken(collection_id, token_id, owner));

			Ok(())
		}

		/// Update an existing NFT
		/// `new_fields` - new values according to the NFT collection/schema, omitted fields will retain their current values.
		/// Caller must be the collection owner
		#[weight = 0]
		fn update_token(origin, collection_id: CollectionId, token_id: T::TokenId, new_fields: Vec<Option<NFTField>>) -> DispatchResult {
			let origin = ensure_signed(origin)?;

			// Permission and existence check
			let collection_owner = Self::collection_owner(&collection_id);
			ensure!(collection_owner.is_some(), Error::<T>::NoCollection);
			ensure!(collection_owner.unwrap() == origin, Error::<T>::NoPermission);
			ensure!(<Tokens<T>>::contains_key(&collection_id, token_id), Error::<T>::NoToken);
			ensure!(!<Listings<T>>::contains_key(&collection_id, token_id), Error::<T>::TokenListingProtection);

			// Quick `new_fields` sanity checks
			ensure!(!new_fields.is_empty(), Error::<T>::SchemaEmpty);
			ensure!(new_fields.len() <= MAX_SCHEMA_FIELDS as usize, Error::<T>::SchemaMaxFields);
			let schema: NFTSchema = Self::collection_schema(&collection_id).ok_or(Error::<T>::NoCollection)?;
			ensure!(new_fields.len() == schema.len(), Error::<T>::SchemaMismatch);

			// Rebuild the NFT inserting new values
			let current_token: Vec<NFTField> = Self::tokens(&collection_id, token_id);
			let token: Vec<NFTField> = current_token.iter().zip(new_fields.iter()).map(|(current_value, maybe_new_value)| {
				if let Some(new_value) = maybe_new_value {
					// existing types are valid
					if current_value.type_id() == new_value.type_id() {
						Ok(*new_value)
					} else {
						Err(Error::<T>::SchemaMismatch)
					}
				} else {
					// caller did not provide a new value, retain the existing value
					Ok(*current_value)
				}
			}).collect::<Result<Vec<NFTField>, Error<T>>>()?;

			<Tokens<T>>::insert(&collection_id, token_id, token);
			Self::deposit_event(RawEvent::Update(collection_id, token_id));

			Ok(())
		}

		/// Transfer ownership of an NFT
		/// Caller must be the token owner
		#[weight = 0]
		fn transfer(origin, collection_id: CollectionId, token_id: T::TokenId, new_owner: T::AccountId) {
			let origin = ensure_signed(origin)?;

			ensure!(CollectionSchema::contains_key(&collection_id), Error::<T>::NoCollection);
			ensure!(<Tokens<T>>::contains_key(&collection_id, token_id), Error::<T>::NoToken);

			let current_owner = Self::token_owner(&collection_id, token_id);
			ensure!(current_owner == origin, Error::<T>::NoPermission);

			ensure!(!<Listings<T>>::contains_key(&collection_id, token_id), Error::<T>::TokenListingProtection);

			Self::transfer_ownership(&collection_id, token_id, &current_owner, &new_owner);
			Self::deposit_event(RawEvent::Transfer(collection_id, token_id, new_owner));
		}

		/// Burn an NFT 🔥
		/// Caller must be the token owner
		#[weight = 0]
		fn burn(origin, collection_id: CollectionId, token_id: T::TokenId) {
			let origin = ensure_signed(origin)?;

			ensure!(CollectionSchema::contains_key(&collection_id), Error::<T>::NoCollection);
			ensure!(<Tokens<T>>::contains_key(&collection_id, token_id), Error::<T>::NoToken);

			let current_owner = Self::token_owner(&collection_id, token_id);
			ensure!(current_owner == origin, Error::<T>::NoPermission);

			ensure!(!<Listings<T>>::contains_key(&collection_id, token_id), Error::<T>::TokenListingProtection);

			// Update token ownership
			<CollectedTokens<T>>::mutate(&collection_id, current_owner, |tokens| {
				tokens.retain(|t| t != &token_id)
			});
			<TokenOwner<T>>::take(&collection_id, token_id);
			<Tokens<T>>::take(&collection_id, token_id);

			// Will not overflow, cannot exceed the amount issued qed.
			let tokens_burnt = Self::tokens_burnt(&collection_id).checked_add(&One::one()).unwrap();
			<TokensBurnt<T>>::insert(&collection_id, tokens_burnt);

			Self::deposit_event(RawEvent::Burn(collection_id, token_id));
		}

		/// Sell an NFT to specific account at a fixed price
		/// `receiver` the account to receive the NFT
		/// `asset_id` fungible asset Id to receive as payment for the NFT
		/// `fixed_price` ask price
		#[weight = 0]
		fn direct_sale(origin, collection_id: CollectionId, token_id: T::TokenId, buyer: T::AccountId, payment_asset: AssetId, fixed_price: Balance) {
			let origin = ensure_signed(origin)?;
			let current_owner = Self::token_owner(&collection_id, token_id);
			ensure!(current_owner == origin, Error::<T>::NoPermission);

			ensure!(!<Listings<T>>::contains_key(&collection_id, token_id), Error::<T>::TokenListingProtection);

			let listing_end_block = <frame_system::Module<T>>::block_number().saturating_add(T::DefaultListingDuration::get());
			ListingEndSchedule::<T>::mutate(listing_end_block, |schedule| schedule.push((collection_id.clone(), token_id)));
			let listing = Listing::<T>::DirectSale(
				DirectSaleListing::<T> {
					payment_asset,
					fixed_price,
					close: listing_end_block,
					buyer: buyer.clone(),
				}
			);
			Listings::insert(&collection_id, token_id, listing);
			Self::deposit_event(RawEvent::DirectSaleListed(collection_id, token_id, buyer, payment_asset, fixed_price));
		}

		/// Buy an NFT for its listed price, must be listed for sale and called by the receiver
		#[weight = 0]
		fn direct_purchase(origin, collection_id: CollectionId, token_id: T::TokenId) {
			let origin = ensure_signed(origin)?;
			ensure!(<Listings<T>>::contains_key(&collection_id, token_id), Error::<T>::NotForDirectSale);

			if let Some(Listing::DirectSale(listing)) = Self::listings(&collection_id, token_id) {
				ensure!(origin == listing.buyer, Error::<T>::NoPermission);
				let current_owner = Self::token_owner(&collection_id, token_id);
				T::MultiCurrency::transfer(&listing.buyer, &current_owner, Some(listing.payment_asset), listing.fixed_price, ExistenceRequirement::AllowDeath)?;
				// must not fail not that payment has been made
				// TODO: use `#[transactional]` in next plug update
				Self::transfer_ownership(&collection_id, token_id, &current_owner, &listing.buyer);
				Self::remove_direct_listing(&collection_id, token_id);
				Self::deposit_event(RawEvent::DirectSaleComplete(collection_id, token_id, listing.buyer, listing.payment_asset, listing.fixed_price));
			} else {
				return Err(Error::<T>::NotForDirectSale.into());
			}
		}

		/// Sell NFT on the open market to the highest bidder
		/// - `reserve_price` winning bid must be over this threshold
		/// - `bid_asset` fungible asset Id to receive bids with
		/// - `duration` length of the auction (in blocks)
		#[weight = 0]
		fn auction(origin, collection_id: CollectionId, token_id: T::TokenId, bid_asset: AssetId, reserve_price: Balance, duration: Option<T::BlockNumber>) {
			// store auction schedule
			// lock NFT
			// map Locks(collection_id, token_id, lock)
			// log listing id
			// let origin = ensure_signed(origin)?;
			// let current_owner = Self::token_owner(collection_id.clone(), token_id);
			// ensure!(current_owner == origin, Error::<T>::NoPermission);

			// ensure!(!Listings::contains_key(collection_id, token_id), Error::<T>::AlreadyListed);
		}

		/// Place a bid on an open auction
		/// - `listing_id` to bid on
		/// - `amount` to bid (in the requested asset)
		#[weight = 0]
		fn bid(origin, collection_id: CollectionId, token_id: T::TokenId, amount: Balance) {
			// ensure!(Listings::contains_key(collection_id, token_id), Error::<T>::NotListed);
			// check highest bid
			// lock funds
			// update listing schedule
			// map Listing(listing_id, listing_schedule)
			// map ListingBids(listing_id, (account, bid))
			// map ListingEndSchedule(block, vec![listing_id])
		}
	}
}

impl<T: Trait> Module<T> {
	/// Transfer ownership of a token. modifies storage, does no verification.
	fn transfer_ownership(
		collection_id: &[u8],
		token_id: T::TokenId,
		current_owner: &T::AccountId,
		new_owner: &T::AccountId,
	) {
		// Update token ownership
		<CollectedTokens<T>>::mutate(collection_id, current_owner, |tokens| tokens.retain(|t| t != &token_id));
		<TokenOwner<T>>::insert(collection_id, token_id, new_owner);
		<CollectedTokens<T>>::append(collection_id, new_owner, token_id);
	}
	/// Remove a single direct listing and all it's metadata
	fn remove_direct_listing(collection_id: &CollectionId, token_id: T::TokenId) {
		let listing_type = Listings::<T>::take(collection_id, token_id);
		ListingWinningBid::<T>::remove(collection_id, token_id);
		if let Some(Listing::<T>::DirectSale(listing)) = listing_type {
			ListingEndSchedule::<T>::mutate(listing.close, |listings| {
				listings.retain(|l| l != &(collection_id.clone(), token_id));
			});
		}
	}
	/// Close all given `listings` ensuring payments are made for the winning bids
	fn close_listings(listings: &[(CollectionId, T::TokenId)]) {
		for (collection_id, token_id) in listings {
			match Listings::<T>::take(collection_id, token_id) {
				Some(Listing::DirectSale(_)) => (), // all the clean ups done
				Some(Listing::Auction(_)) => {
					// TODO: winning bids should be paid out here.
				}
				_ => (),
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{Event, ExtBuilder, Test};
	use frame_support::{assert_noop, assert_ok, parameter_types, traits::OnInitialize};
	use sp_runtime::Percent;

	type Nft = Module<Test>;
	type System = frame_system::Module<Test>;
	type AccountId = u64;
	type TokenId = u32;

	parameter_types! {
		pub const DefaultListingDuration: u64 = 5;
	}
	impl Trait for Test {
		type Event = Event;
		type TokenId = TokenId;
		type MultiCurrency = prml_generic_asset::Module<Test>;
		type DefaultListingDuration = DefaultListingDuration;
		type WeightInfo = ();
	}

	// Check the test system contains an event record `event`
	fn has_event(event: RawEvent<CollectionId, TokenId, AccountId, AssetId, Balance>) -> bool {
		System::events()
			.iter()
			.find(|e| e.event == Event::nft(event.clone()))
			.is_some()
	}

	// Create an NFT collection with schema
	// Returns the created `collection_id`
	fn setup_collection(owner: AccountId, schema: NFTSchema) -> CollectionId {
		let collection_id = b"test-collection".to_vec();
		assert_ok!(Nft::create_collection(
			Some(owner).into(),
			collection_id.clone(),
			schema.clone(),
			RoyaltiesPlan::default()
		));
		collection_id
	}

	#[test]
	fn create_collection() {
		ExtBuilder::default().build().execute_with(|| {
			let owner = 1_u64;
			let schema = vec![
				NFTField::U8(Default::default()).type_id(),
				NFTField::U8(Default::default()).type_id(),
				NFTField::Bytes32(Default::default()).type_id(),
			];

			let collection_id = setup_collection(owner, schema.clone());
			assert!(has_event(RawEvent::CreateCollection(collection_id.clone(), owner)));

			assert_eq!(
				Nft::collection_owner(collection_id.clone()).expect("owner should exist"),
				owner
			);
			assert_eq!(
				Nft::collection_schema(collection_id.clone()).expect("schema should exist"),
				schema
			);
			assert_eq!(Nft::collection_royalties(collection_id), RoyaltiesPlan::default());
		});
	}

	#[test]
	fn create_collection_invalid_schema() {
		ExtBuilder::default().build().execute_with(|| {
			let collection_id = b"test-collection".to_vec();
			assert_noop!(
				Nft::create_collection(
					Some(1_u64).into(),
					collection_id.clone(),
					vec![],
					RoyaltiesPlan::default()
				),
				Error::<Test>::SchemaEmpty
			);

			let too_many_fields = [0_u8; (MAX_SCHEMA_FIELDS + 1_u32) as usize];
			assert_noop!(
				Nft::create_collection(
					Some(1_u64).into(),
					collection_id.clone(),
					too_many_fields.to_owned().to_vec(),
					RoyaltiesPlan::default()
				),
				Error::<Test>::SchemaMaxFields
			);

			let invalid_nft_field_type: NFTFieldTypeId = 200;
			assert_noop!(
				Nft::create_collection(
					Some(1_u64).into(),
					collection_id,
					vec![invalid_nft_field_type],
					RoyaltiesPlan::default()
				),
				Error::<Test>::SchemaInvalid
			);
		});
	}

	#[test]
	fn create_collection_invalid_id() {
		ExtBuilder::default().build().execute_with(|| {
			// too long
			let bad_collection_id = b"someidentifierthatismuchlongerthanthe32bytelimitsoshouldfail".to_vec();
			assert_noop!(
				Nft::create_collection(Some(1_u64).into(), bad_collection_id, vec![], RoyaltiesPlan::default()),
				Error::<Test>::CollectionIdInvalid
			);

			// empty id
			assert_noop!(
				Nft::create_collection(Some(1_u64).into(), vec![], vec![], RoyaltiesPlan::default()),
				Error::<Test>::CollectionIdInvalid
			);

			// non UTF-8 chars
			// kudos: https://www.cl.cam.ac.uk/~mgk25/ucs/examples/UTF-8-test.txt
			let bad_collection_id = vec![0xfe, 0xff];
			assert_noop!(
				Nft::create_collection(Some(1_u64).into(), bad_collection_id, vec![], RoyaltiesPlan::default()),
				Error::<Test>::CollectionIdInvalid
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
			let collection_owner = 1_u64;
			let collection_id = setup_collection(collection_owner, schema);

			let token_owner = 2_u64;
			let token_id = 0;
			let royalties_plan = RoyaltiesPlan {
				total_commission: Percent::from_percent(10),
				charter: vec![],
			};
			assert_eq!(Nft::next_token_id(&collection_id), token_id);
			assert_ok!(Nft::create_token(
				Some(collection_owner).into(),
				collection_id.clone(),
				token_owner,
				vec![Some(NFTField::I32(-33)), None, Some(NFTField::Bytes32([1_u8; 32]))],
				Some(royalties_plan.clone()),
			));
			assert!(has_event(RawEvent::CreateToken(
				collection_id.clone(),
				token_id,
				token_owner.clone()
			)));

			let token = Nft::tokens(&collection_id, token_id);
			assert_eq!(
				token,
				vec![
					NFTField::I32(-33),
					NFTField::U8(Default::default()),
					NFTField::Bytes32([1_u8; 32])
				],
			);

			assert_eq!(Nft::token_owner(&collection_id, token_id), token_owner);
			assert_eq!(
				Nft::token_royalties(&collection_id, token_id).expect("royalties plan set"),
				royalties_plan
			);
			assert_eq!(Nft::collected_tokens(&collection_id, token_owner), vec![token_id]);
			assert_eq!(
				Nft::next_token_id(&collection_id),
				token_id.checked_add(One::one()).unwrap()
			);
			assert_eq!(Nft::token_issuance(&collection_id), 1);
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
			let collection_owner = 1_u64;
			let collection_id = setup_collection(collection_owner, schema);
			let token_owner = 2_u64;

			assert_ok!(Nft::create_token(
				Some(collection_owner).into(),
				collection_id.clone(),
				token_owner,
				vec![Some(NFTField::I32(-33)), None, Some(NFTField::Bytes32([1_u8; 32]))],
				None,
			));

			assert_ok!(Nft::create_token(
				Some(collection_owner).into(),
				collection_id.clone(),
				token_owner,
				vec![Some(NFTField::I32(33)), None, Some(NFTField::Bytes32([2_u8; 32]))],
				None,
			));
			assert!(has_event(RawEvent::CreateToken(
				collection_id.clone(),
				1,
				token_owner.clone()
			)));

			assert_eq!(Nft::token_owner(collection_id.clone(), 1), token_owner);
			assert_eq!(Nft::collected_tokens(collection_id.clone(), token_owner), vec![0, 1]);
			assert_eq!(Nft::next_token_id(&collection_id), 2);
			assert_eq!(Nft::token_issuance(collection_id), 2);
		});
	}

	#[test]
	fn create_token_fails_prechecks() {
		ExtBuilder::default().build().execute_with(|| {
			let schema = vec![NFTField::I32(Default::default()).type_id()];
			let collection_owner = 1_u64;
			let collection_id = setup_collection(collection_owner, schema);

			assert_noop!(
				Nft::create_token(
					Some(2_u64).into(),
					collection_id.clone(),
					collection_owner,
					vec![None],
					None
				),
				Error::<Test>::NoPermission
			);

			assert_noop!(
				Nft::create_token(
					Some(collection_owner).into(),
					b"this-collection-doesn't-exist".to_vec(),
					collection_owner,
					vec![None],
					None
				),
				Error::<Test>::NoCollection
			);

			assert_noop!(
				Nft::create_token(
					Some(collection_owner).into(),
					collection_id.clone(),
					collection_owner,
					vec![],
					None
				),
				Error::<Test>::SchemaEmpty
			);

			// additional field vs. schema
			assert_noop!(
				Nft::create_token(
					Some(collection_owner).into(),
					collection_id.clone(),
					collection_owner,
					vec![None, None],
					None
				),
				Error::<Test>::SchemaMismatch
			);

			// different field type vs. schema
			assert_noop!(
				Nft::create_token(
					Some(collection_owner).into(),
					collection_id.clone(),
					collection_owner,
					vec![Some(NFTField::U32(404))],
					None,
				),
				Error::<Test>::SchemaMismatch
			);

			let too_many_fields: [Option<NFTField>; (MAX_SCHEMA_FIELDS + 1_u32) as usize] = Default::default();
			assert_noop!(
				Nft::create_token(
					Some(collection_owner).into(),
					collection_id,
					collection_owner,
					too_many_fields.to_owned().to_vec(),
					None,
				),
				Error::<Test>::SchemaMaxFields
			);
		});
	}

	#[test]
	fn update_token() {
		ExtBuilder::default().build().execute_with(|| {
			// setup token collection + one token
			let schema = vec![
				NFTField::I32(Default::default()).type_id(),
				NFTField::U8(Default::default()).type_id(),
				NFTField::Bytes32(Default::default()).type_id(),
			];
			let collection_owner = 1_u64;
			let collection_id = setup_collection(collection_owner, schema);
			let token_owner = 2_u64;
			let token_id = Nft::next_token_id(&collection_id);
			let initial_values = vec![
				Some(NFTField::I32(-33)),
				Some(NFTField::U8(12)),
				Some(NFTField::Bytes32([1_u8; 32])),
			];
			assert_ok!(Nft::create_token(
				Some(collection_owner).into(),
				collection_id.clone(),
				token_owner,
				initial_values.clone(),
				None,
			));

			// test
			assert_ok!(Nft::update_token(
				Some(collection_owner).into(),
				collection_id.clone(),
				token_id,
				// only change the bytes32 value
				vec![None, None, Some(NFTField::Bytes32([2_u8; 32]))]
			));
			assert!(has_event(RawEvent::Update(collection_id.clone(), token_id)));

			assert_eq!(
				Nft::tokens(collection_id, token_id),
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
			let collection_owner = 1_u64;
			let collection_id = setup_collection(collection_owner, schema);
			let token_owner = 2_u64;
			let token_id = Nft::next_token_id(&collection_id);
			assert_ok!(Nft::create_token(
				Some(collection_owner).into(),
				collection_id.clone(),
				token_owner,
				vec![None],
				None,
			));

			assert_noop!(
				Nft::update_token(Some(2_u64).into(), collection_id.clone(), token_id, vec![None]),
				Error::<Test>::NoPermission
			);

			assert_noop!(
				Nft::update_token(
					Some(collection_owner).into(),
					b"no-collection".to_vec(),
					token_id,
					vec![None]
				),
				Error::<Test>::NoCollection
			);

			assert_noop!(
				Nft::update_token(Some(collection_owner).into(), collection_id.clone(), token_id, vec![]),
				Error::<Test>::SchemaEmpty
			);

			// additional field vs. schema
			assert_noop!(
				Nft::update_token(
					Some(collection_owner).into(),
					collection_id.clone(),
					token_id,
					vec![None, None]
				),
				Error::<Test>::SchemaMismatch
			);

			// different field type vs. schema
			assert_noop!(
				Nft::update_token(
					Some(collection_owner).into(),
					collection_id.clone(),
					token_id,
					vec![Some(NFTField::U32(404))]
				),
				Error::<Test>::SchemaMismatch
			);

			let too_many_fields: [Option<NFTField>; (MAX_SCHEMA_FIELDS + 1_u32) as usize] = Default::default();
			assert_noop!(
				Nft::update_token(
					Some(collection_owner).into(),
					collection_id.clone(),
					token_id,
					too_many_fields.to_owned().to_vec()
				),
				Error::<Test>::SchemaMaxFields
			);

			assert_ok!(Nft::direct_sale(
				Some(token_owner).into(),
				collection_id.clone(),
				token_id,
				5,
				16_000,
				1_000
			));
			// cannot transfer while listed
			assert_noop!(
				Nft::transfer(Some(token_owner).into(), collection_id, token_id, token_owner),
				Error::<Test>::TokenListingProtection,
			);
		});
	}

	#[test]
	fn transfer() {
		ExtBuilder::default().build().execute_with(|| {
			// setup token collection + one token
			let schema = vec![NFTField::I32(Default::default()).type_id()];
			let collection_owner = 1_u64;
			let collection_id = setup_collection(collection_owner, schema);
			let token_owner = 2_u64;
			let token_id = Nft::next_token_id(&collection_id);
			assert_ok!(Nft::create_token(
				Some(collection_owner).into(),
				collection_id.clone(),
				token_owner,
				vec![Some(NFTField::I32(500))],
				None,
			));

			// test
			let new_owner = 3_u64;
			assert_ok!(Nft::transfer(
				Some(token_owner).into(),
				collection_id.clone(),
				token_id,
				new_owner,
			));
			assert!(has_event(RawEvent::Transfer(
				collection_id.clone(),
				token_id,
				new_owner
			)));

			assert_eq!(Nft::token_owner(&collection_id, token_id), new_owner);
			assert!(Nft::collected_tokens(&collection_id, token_owner).is_empty());
			assert_eq!(Nft::collected_tokens(&collection_id, new_owner), vec![token_id]);
		});
	}

	#[test]
	fn transfer_fails_prechecks() {
		ExtBuilder::default().build().execute_with(|| {
			// setup token collection + one token
			let schema = vec![NFTField::I32(Default::default()).type_id()];
			let collection_owner = 1_u64;

			// no collection yet
			assert_noop!(
				Nft::transfer(
					Some(collection_owner).into(),
					b"no-collection".to_vec(),
					1,
					collection_owner
				),
				Error::<Test>::NoCollection,
			);

			let collection_id = setup_collection(collection_owner, schema);
			let token_owner = 2_u64;
			let token_id = Nft::next_token_id(&collection_id);

			// no token yet
			assert_noop!(
				Nft::transfer(Some(token_owner).into(), collection_id.clone(), token_id, token_owner),
				Error::<Test>::NoToken,
			);

			assert_ok!(Nft::create_token(
				Some(collection_owner).into(),
				collection_id.clone(),
				token_owner,
				vec![Some(NFTField::I32(500))],
				None,
			));

			let not_the_owner = 3_u64;
			assert_noop!(
				Nft::transfer(
					Some(not_the_owner).into(),
					collection_id.clone(),
					token_id,
					not_the_owner
				),
				Error::<Test>::NoPermission,
			);

			assert_ok!(Nft::direct_sale(
				Some(token_owner).into(),
				collection_id.clone(),
				token_id,
				5,
				16_000,
				1_000
			));
			// cannot transfer while listed
			assert_noop!(
				Nft::transfer(Some(token_owner).into(), collection_id.clone(), token_id, token_owner),
				Error::<Test>::TokenListingProtection,
			);
		});
	}

	#[test]
	fn burn() {
		ExtBuilder::default().build().execute_with(|| {
			// setup token collection + one token
			let schema = vec![NFTField::I32(Default::default()).type_id()];
			let collection_owner = 1_u64;
			let collection_id = setup_collection(collection_owner, schema);
			let token_owner = 2_u64;
			let token_id = Nft::next_token_id(&collection_id);
			assert_ok!(Nft::create_token(
				Some(collection_owner).into(),
				collection_id.clone(),
				token_owner,
				vec![Some(NFTField::I32(500))],
				None,
			));

			// test
			assert_ok!(Nft::burn(Some(token_owner).into(), collection_id.clone(), token_id));
			assert!(has_event(RawEvent::Burn(collection_id.clone(), token_id)));

			assert!(!<Tokens<Test>>::contains_key(&collection_id, token_id));
			assert!(!<TokenOwner<Test>>::contains_key(&collection_id, token_id));
			assert!(Nft::collected_tokens(&collection_id, token_owner).is_empty());
			assert_eq!(Nft::tokens_burnt(&collection_id), 1);
		});
	}

	#[test]
	fn burn_fails_prechecks() {
		ExtBuilder::default().build().execute_with(|| {
			// setup token collection + one token
			let schema = vec![NFTField::I32(Default::default()).type_id()];
			let collection_owner = 1_u64;
			assert_noop!(
				Nft::burn(Some(collection_owner).into(), b"no-collection".to_vec(), 0),
				Error::<Test>::NoCollection
			);

			let collection_id = setup_collection(collection_owner, schema);
			let token_owner = 2_u64;
			let token_id = Nft::next_token_id(&collection_id);
			assert_noop!(
				Nft::burn(Some(token_owner).into(), collection_id.clone(), token_id),
				Error::<Test>::NoToken,
			);

			assert_ok!(Nft::create_token(
				Some(collection_owner).into(),
				collection_id.clone(),
				token_owner,
				vec![Some(NFTField::I32(500))],
				None,
			));

			assert_noop!(
				Nft::burn(Some(3_u64).into(), collection_id.clone(), token_id),
				Error::<Test>::NoPermission,
			);

			assert_ok!(Nft::direct_sale(
				Some(token_owner).into(),
				collection_id.clone(),
				token_id,
				5,
				16_000,
				1_000
			));
			// cannot transfer while listed
			assert_noop!(
				Nft::transfer(Some(token_owner).into(), collection_id, token_id, token_owner),
				Error::<Test>::TokenListingProtection,
			);
		});
	}

	/// Setup a token, return collection id, token id, token owner
	fn setup_token() -> (
		CollectionId,
		<Test as Trait>::TokenId,
		<Test as frame_system::Trait>::AccountId,
	) {
		let schema = vec![NFTField::I32(Default::default()).type_id()];
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner, schema);
		let token_owner = 2_u64;
		let token_id = Nft::next_token_id(&collection_id);
		assert_ok!(Nft::create_token(
			Some(collection_owner).into(),
			collection_id.clone(),
			token_owner,
			vec![Some(NFTField::I32(500))],
			None,
		));

		(collection_id, token_id, token_owner)
	}

	#[test]
	fn direct_sale() {
		ExtBuilder::default().build().execute_with(|| {
			let (collection_id, token_id, token_owner) = setup_token();
			assert_ok!(Nft::direct_sale(
				Some(token_owner).into(),
				collection_id.clone(),
				token_id,
				5,
				16_000,
				1_000
			));

			let expected = Listing::<Test>::DirectSale(DirectSaleListing::<Test> {
				payment_asset: 16_000,
				fixed_price: 1_000,
				close: System::block_number() + <Test as Trait>::DefaultListingDuration::get(),
				buyer: 5,
			});

			let listing = Nft::listings(&collection_id, token_id).expect("token is listed");
			assert_eq!(listing, expected);

			// current block is 1 + duration
			assert_eq!(
				Nft::listing_end_blocks(System::block_number() + <Test as Trait>::DefaultListingDuration::get()),
				vec![(collection_id.clone(), token_id)]
			);

			assert!(has_event(RawEvent::DirectSaleListed(
				collection_id,
				token_id,
				5,
				16_000,
				1_000
			)));
		});
	}

	#[test]
	fn direct_sale_prechecks() {
		ExtBuilder::default().build().execute_with(|| {
			let (collection_id, token_id, token_owner) = setup_token();
			// no permission
			assert_noop!(
				Nft::direct_sale(
					Some(token_owner + 1).into(),
					collection_id.clone(),
					token_id,
					5,
					16_000,
					1_000
				),
				Error::<Test>::NoPermission
			);
			// token listed already
			assert_ok!(Nft::direct_sale(
				Some(token_owner).into(),
				collection_id.clone(),
				token_id,
				5,
				16_000,
				1_000
			));
			assert_noop!(
				Nft::direct_sale(
					Some(token_owner).into(),
					collection_id.clone(),
					token_id,
					5,
					16_000,
					1_000
				),
				Error::<Test>::TokenListingProtection
			);
			// TODO: listed for auction should fail here
		});
	}

	#[test]
	fn direct_sale_closes_on_schedule() {
		ExtBuilder::default().build().execute_with(|| {
			let (collection_id, token_id, token_owner) = setup_token();
			assert_ok!(Nft::direct_sale(
				Some(token_owner).into(),
				collection_id.clone(),
				token_id,
				5,
				16_000,
				1_000
			));

			Nft::on_initialize(System::block_number() + <Test as Trait>::DefaultListingDuration::get());

			assert_eq!(Nft::token_owner(&collection_id, token_id), token_owner);
			assert!(!Listings::<Test>::contains_key(&collection_id, token_id));
			assert!(!ListingEndSchedule::<Test>::contains_key(
				System::block_number() + <Test as Trait>::DefaultListingDuration::get()
			));

			// should be free to transfer now
			let new_owner = 8;
			assert_ok!(Nft::transfer(
				Some(token_owner).into(),
				collection_id.clone(),
				token_id,
				new_owner,
			));
		});
	}

	#[test]
	fn direct_purchase() {
		ExtBuilder::default().build().execute_with(|| {
			let (collection_id, token_id, token_owner) = setup_token();
			let buyer = 5;
			let payment_asset = 16_000;
			let price = 1_000;

			assert_ok!(Nft::direct_sale(
				Some(token_owner).into(),
				collection_id.clone(),
				token_id,
				buyer,
				payment_asset,
				price
			));

			let _ = <Test as Trait>::MultiCurrency::deposit_creating(&buyer, Some(payment_asset), price);
			assert_ok!(Nft::direct_purchase(
				Some(buyer).into(),
				collection_id.clone(),
				token_id
			));

			// listing removed
			assert!(!Listings::<Test>::contains_key(&collection_id, token_id));
			assert!(ListingEndSchedule::<Test>::get(
				System::block_number() + <Test as Trait>::DefaultListingDuration::get()
			)
			.is_empty());

			// ownership changed
			assert_eq!(<TokenOwner<Test>>::get(&collection_id, token_id), buyer);
			assert_eq!(<CollectedTokens<Test>>::get(&collection_id, buyer), vec![token_id]);
		});
	}

	#[test]
	fn direct_purchase_fails_prechecks() {
		ExtBuilder::default().build().execute_with(|| {
			let (collection_id, token_id, token_owner) = setup_token();
			let buyer = 5;
			let payment_asset = 16_000;
			let price = 1_000;

			// not for sale
			assert_noop!(
				Nft::direct_purchase(Some(buyer).into(), collection_id.clone(), token_id),
				Error::<Test>::NotForDirectSale,
			);

			assert_ok!(Nft::direct_sale(
				Some(token_owner).into(),
				collection_id.clone(),
				token_id,
				buyer,
				payment_asset,
				price
			));

			// no permission
			assert_noop!(
				Nft::direct_purchase(Some(buyer + 1).into(), collection_id.clone(), token_id),
				Error::<Test>::NoPermission,
			);

			// fund the buyer with not quite enough
			let _ = <Test as Trait>::MultiCurrency::deposit_creating(&buyer, Some(payment_asset), price - 1);
			assert_noop!(
				Nft::direct_purchase(Some(buyer).into(), collection_id.clone(), token_id),
				prml_generic_asset::Error::<Test>::InsufficientBalance,
			);

			// TODO: listed for auction should fail here
		});
	}
}
