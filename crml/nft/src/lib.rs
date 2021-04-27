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
	traits::{ExistenceRequirement, Get, Imbalance, WithdrawReason},
	transactional,
	weights::Weight,
	Parameter,
};
use frame_system::ensure_signed;
use prml_support::MultiCurrencyAccounting;
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, CheckedAdd, Member, One, Saturating, Zero},
	DispatchResult,
};
use sp_std::{collections::btree_set::BTreeSet, iter::FromIterator, prelude::*};

mod benchmarking;
mod default_weights;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

mod types;
use types::*;

pub trait Trait: frame_system::Trait {
	/// The system event type
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
	/// Type for identifying tokens
	type TokenId: Parameter + Member + AtLeast32BitUnsigned + Default + Copy + One + Into<u64>;
	/// Default auction / sale length in blocks
	type DefaultListingDuration: Get<Self::BlockNumber>;
	/// Maximum byte length of an NFT attribute
	type MaxAttributeLength: Get<u8>;
	/// Handles a multi-currency fungible asset system
	type MultiCurrency: MultiCurrencyAccounting<AccountId = Self::AccountId, CurrencyId = AssetId, Balance = Balance>;
	/// Provides the public call to weight mapping
	type WeightInfo: WeightInfo;
}

/// NFT module weights
pub trait WeightInfo {
	fn create_collection() -> Weight;
	fn create_token() -> Weight;
	fn transfer() -> Weight;
	fn burn() -> Weight;
	fn direct_sale() -> Weight;
	fn direct_purchase() -> Weight;
	fn auction() -> Weight;
	fn bid() -> Weight;
	fn cancel_sale() -> Weight;
}

decl_event!(
	pub enum Event<T> where CollectionId = CollectionId, <T as Trait>::TokenId, <T as frame_system::Trait>::AccountId, AssetId = AssetId, Balance = Balance, Reason = AuctionClosureReason{
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
		DirectSaleListed(CollectionId, TokenId, Option<AccountId>, AssetId, Balance),
		/// A direct sale has completed (collection, token, new owner, payment asset, fixed price)
		DirectSaleComplete(CollectionId, TokenId, AccountId, AssetId, Balance),
		/// A direct sale has closed without selling
		DirectSaleClosed(CollectionId, TokenId),
		/// An auction has opened (collection, token, payment asset, reserve price)
		AuctionOpen(CollectionId, TokenId, AssetId, Balance),
		/// An auction has sold (collection, token, payment asset, bid, new owner)
		AuctionSold(CollectionId, TokenId, AssetId, Balance, AccountId),
		/// An auction has closed without selling (collection, token, reason)
		AuctionClosed(CollectionId, TokenId, Reason),
		/// A new highest bid was placed (collection, token, amount)
		Bid(CollectionId, TokenId, Balance),
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
		/// Too many attributes in the provided schema or data
		SchemaMaxAttributes,
		/// Provided attributes do not match the collection schema
		SchemaMismatch,
		/// Provided attribute is not in the collection schema
		UnknownAttribute,
		/// The provided attributes or schema cannot be empty
		SchemaEmpty,
		/// The schema contains an invalid type
		SchemaInvalid,
		/// The schema contains a duplicate attribute name
		SchemaDuplicateAttribute,
		/// Given attirbute value is larger than the configured max.
		MaxAttributeLength,
		/// origin does not have permission for the operation
		NoPermission,
		/// The NFT collection does not exist
		NoCollection,
		/// The NFT does not exist
		NoToken,
		/// The NFT is not listed for a direct sale
		NotForDirectSale,
		/// The NFT is not listed for auction sale
		NotForAuction,
		/// Cannot operate on a listed NFT
		TokenListingProtection,
		/// Internal error during payment
		InternalPayment,
		/// Total royalties would exceed 100% of sale
		RoyaltiesOvercommitment,
		/// Auction bid was lower than reserve or current highest bid
		BidTooLow
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as Nft {
		/// Map from collection to owner address
		pub CollectionOwner get(fn collection_owner): map hasher(blake2_128_concat) CollectionId => Option<T::AccountId>;
		/// Map from collection to its schema definition
		pub CollectionSchema get(fn collection_schema): map hasher(blake2_128_concat) CollectionId => Option<NFTSchema>;
		/// Map from collection to it's defacto royalty scheme
		pub CollectionRoyalties get(fn collection_royalties): map hasher(blake2_128_concat) CollectionId => Option<RoyaltiesSchedule<T::AccountId>>;
		/// Map from a token to it's royalty scheme
		pub TokenRoyalties get(fn token_royalties): double_map hasher(blake2_128_concat) CollectionId, hasher(twox_64_concat) T::TokenId => Option<RoyaltiesSchedule<T::AccountId>>;
		/// Map from (collection, token) to it's attributes (as defined by schema)
		pub Tokens get(fn tokens): double_map hasher(blake2_128_concat) CollectionId, hasher(twox_64_concat) T::TokenId => Vec<NFTAttributeValue>;
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
		pub ListingWinningBid get(fn listing_winning_bid): double_map hasher(blake2_128_concat) CollectionId, hasher(twox_64_concat) T::TokenId => Option<(T::AccountId, Balance)>;
		/// Map from block numbers to listings scheduled to close
		pub ListingEndSchedule get(fn listing_end_schedule): map hasher(twox_64_concat) T::BlockNumber => Vec<(CollectionId, T::TokenId)>;
	}
}

/// The maximum number of attributes in an NFT collection schema
pub const MAX_SCHEMA_FIELDS: u32 = 16;
/// The maximum length of valid collection IDs
pub const MAX_COLLECTION_ID_LENGTH: u8 = 32;

pub(crate) const LOG_TARGET: &'static str = "nft";

// syntactic sugar for logging.
#[macro_export]
macro_rules! log {
	($level:tt, $patter:expr $(, $values:expr)* $(,)?) => {
		frame_support::debug::$level!(
			target: crate::LOG_TARGET,
			$patter $(, $values)*
		)
	};
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin, system = frame_system {
		type Error = Error<T>;

		fn deposit_event() = default;

		/// Check and close all expired listings
		fn on_initialize(now: T::BlockNumber) -> Weight {
			if ListingEndSchedule::<T>::contains_key(now) {
				let removed_count = Self::close_listings_at(now);
				// 'direct_purchase' weight is comparable to succesful closure of an auction
				T::WeightInfo::direct_purchase() * removed_count as Weight
			} else {
				Zero::zero()
			}
		}

		/// Create a new NFT collection
		/// The caller will be come the collection' owner
		/// `collection_id`- 32 byte utf-8 string
		/// `schema` - for the collection
		/// `royalties_schedule` - defacto royalties plan for secondary sales, this will apply to all tokens in the collection by default.
		#[weight = T::WeightInfo::create_collection()]
		fn create_collection(origin, collection_id: CollectionId, schema: NFTSchema, royalties_schedule: Option<RoyaltiesSchedule<T::AccountId>>) -> DispatchResult {
			let origin = ensure_signed(origin)?;

			ensure!(!collection_id.is_empty() && collection_id.len() <= MAX_COLLECTION_ID_LENGTH as usize, Error::<T>::CollectionIdInvalid);
			ensure!(core::str::from_utf8(&collection_id).is_ok(), Error::<T>::CollectionIdInvalid);
			ensure!(!<CollectionOwner<T>>::contains_key(&collection_id), Error::<T>::CollectionIdExists);

			ensure!(!schema.is_empty(), Error::<T>::SchemaEmpty);
			ensure!(schema.len() <= MAX_SCHEMA_FIELDS as usize, Error::<T>::SchemaMaxAttributes);

			// Check the provided attribute types are valid
			ensure!(
				schema.iter().all(|(_name, type_id)| NFTAttributeValue::is_valid_type_id(*type_id)),
				Error::<T>::SchemaInvalid
			);

			// Attribute names must be unique (future proofing for map lookups etc.)
			let (attribute_names, _): (Vec<NFTAttributeName>, Vec<NFTAttributeTypeId>) = schema.iter().cloned().unzip();
			let deduped = BTreeSet::from_iter(attribute_names);
			ensure!(deduped.len() == schema.len(), Error::<T>::SchemaDuplicateAttribute);

			// Create the collection, update ownership, and bookkeeping
			if let Some(royalties_schedule) = royalties_schedule {
				ensure!(royalties_schedule.validate(), Error::<T>::RoyaltiesOvercommitment);
				<CollectionRoyalties<T>>::insert(&collection_id, royalties_schedule);
			}
			CollectionSchema::insert(&collection_id, schema);
			<CollectionOwner<T>>::insert(&collection_id, origin.clone());

			Self::deposit_event(RawEvent::CreateCollection(collection_id, origin));

			Ok(())
		}

		/// Issue a new NFT
		/// `owner` - the token owner
		/// `attributes` - initial values according to the NFT collection/schema
		/// `royalties_schedule` - optional royalty schedule for secondary sales of _this_ token, defaults to the collection config
		/// Caller must be the collection owner
		#[weight = T::WeightInfo::create_token()]
		#[transactional]
		fn create_token(origin, collection_id: CollectionId, owner: T::AccountId, attributes: Vec<NFTAttributeValue>, royalties_schedule: Option<RoyaltiesSchedule<T::AccountId>>) -> DispatchResult {
			let origin = ensure_signed(origin)?;

			// Permission and existence check
			let collection_owner = Self::collection_owner(&collection_id);
			ensure!(collection_owner.is_some(), Error::<T>::NoCollection);
			ensure!(collection_owner.unwrap() == origin, Error::<T>::NoPermission);

			// Check we can issue a new token
			let token_id = Self::next_token_id(&collection_id);
			let next_token_id = token_id.checked_add(&One::one()).ok_or(Error::<T>::NoAvailableIds)?;
			Self::token_issuance(&collection_id).checked_add(&One::one()).ok_or(Error::<T>::MaxTokensIssued)?;

			// Quick `attributes` sanity checks
			ensure!(!attributes.is_empty(), Error::<T>::SchemaEmpty);
			ensure!(attributes.len() as u32 <= MAX_SCHEMA_FIELDS, Error::<T>::SchemaMaxAttributes);
			let schema: NFTSchema = Self::collection_schema(&collection_id).ok_or(Error::<T>::NoCollection)?;
			ensure!(attributes.len() == schema.len(), Error::<T>::SchemaMismatch);

			// Build the NFT + schema type level validation
			let token: Vec<NFTAttributeValue> = schema.iter().zip(attributes.iter()).map(|((_schema_attribute_name, schema_attribute_type), provided_attribute)| {
				// check provided attribute has the correct type
				if *schema_attribute_type == provided_attribute.type_id() {
					ensure!(provided_attribute.len() <= T::MaxAttributeLength::get() as usize, Error::<T>::MaxAttributeLength);
					Ok(provided_attribute.clone())
				} else {
					Err(Error::<T>::SchemaMismatch)
				}
			}).collect::<Result<_, Error<T>>>()?;

			// Create the token, update ownership, and bookkeeping
			if let Some(royalties_schedule) = royalties_schedule {
				ensure!(royalties_schedule.validate(), Error::<T>::RoyaltiesOvercommitment);
				<TokenRoyalties<T>>::insert(&collection_id, token_id, royalties_schedule);
			}
			<Tokens<T>>::insert(&collection_id, token_id, token);
			<NextTokenId<T>>::insert(&collection_id, next_token_id);
			<TokenIssuance<T>>::mutate(&collection_id, |i| *i += One::one());
			<TokenOwner<T>>::insert(&collection_id, token_id, owner.clone());
			<CollectedTokens<T>>::append(&collection_id, owner.clone(), token_id);

			Self::deposit_event(RawEvent::CreateToken(collection_id, token_id, owner));

			Ok(())
		}

		/// Transfer ownership of an NFT
		/// Caller must be the token owner
		#[weight = T::WeightInfo::transfer()]
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
		#[weight = T::WeightInfo::burn()]
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
		/// `buyer` optionally, the account to receive the NFT. If unspecified, then any account may purchase
		/// `asset_id` fungible asset Id to receive as payment for the NFT
		/// `fixed_price` ask price
		/// `duration` listing duration time in blocks
		/// Caller must be the token owner
		#[weight = T::WeightInfo::direct_sale()]
		fn direct_sale(origin, collection_id: CollectionId, token_id: T::TokenId, buyer: Option<T::AccountId>, payment_asset: AssetId, fixed_price: Balance, duration: Option<T::BlockNumber>) {
			let origin = ensure_signed(origin)?;
			let current_owner = Self::token_owner(&collection_id, token_id);
			ensure!(current_owner == origin, Error::<T>::NoPermission);

			ensure!(!<Listings<T>>::contains_key(&collection_id, token_id), Error::<T>::TokenListingProtection);

			let listing_end_block = <frame_system::Module<T>>::block_number().saturating_add(duration.unwrap_or_else(T::DefaultListingDuration::get));
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

		/// Buy an NFT for its listed price, must be listed for sale
		#[weight = T::WeightInfo::direct_purchase()]
		#[transactional]
		fn direct_purchase(origin, collection_id: CollectionId, token_id: T::TokenId) {
			let origin = ensure_signed(origin)?;
			ensure!(<Listings<T>>::contains_key(&collection_id, token_id), Error::<T>::NotForDirectSale);

			if let Some(Listing::DirectSale(listing)) = Self::listings(&collection_id, token_id) {

				match listing.buyer {
					// if buyer is specified in the listing, then `origin` must be buyer
					Some(buyer) => ensure!(origin == buyer, Error::<T>::NoPermission),
					None => (),
				};

				let current_owner = Self::token_owner(&collection_id, token_id);

				// if there are no custom royalties, fallback to default if it exists
				let royalties_schedule = if let Some(royalties_schedule) = Self::token_royalties(&collection_id, token_id) {
					royalties_schedule
				} else {
					Self::collection_royalties(&collection_id).unwrap_or_else(Default::default)
				};

				let royalty_fees = royalties_schedule.calculate_total_entitlement();
				if royalty_fees.is_zero() {
					// full proceeds to seller/`current_owner`
					T::MultiCurrency::transfer(&origin, &current_owner, Some(listing.payment_asset), listing.fixed_price, ExistenceRequirement::AllowDeath)?;
				} else {
					// withdraw funds from buyer, split between royalty payments and seller
					let mut for_seller = listing.fixed_price;
					let mut imbalance = T::MultiCurrency::withdraw(&origin, Some(listing.payment_asset), listing.fixed_price, WithdrawReason::Transfer.into(), ExistenceRequirement::AllowDeath)?;
					for (who, entitlement) in royalties_schedule.entitlements.into_iter() {
						let royalty = entitlement * listing.fixed_price;
						for_seller -= royalty;
						imbalance = imbalance.offset(T::MultiCurrency::deposit_into_existing(&who, Some(listing.payment_asset), royalty)?).map_err(|_| Error::<T>::InternalPayment)?;
					}
					imbalance.offset(T::MultiCurrency::deposit_into_existing(&current_owner, Some(listing.payment_asset), for_seller)?).map_err(|_| Error::<T>::InternalPayment)?;
				}

				// must not fail now that payment has been made
				Self::transfer_ownership(&collection_id, token_id, &current_owner, &origin);
				Self::remove_direct_listing(&collection_id, token_id);
				Self::deposit_event(RawEvent::DirectSaleComplete(collection_id, token_id, origin, listing.payment_asset, listing.fixed_price));
			} else {
				return Err(Error::<T>::NotForDirectSale.into());
			}
		}

		/// Sell NFT on the open market to the highest bidder
		/// Caller must be the token owner
		/// - `reserve_price` winning bid must be over this threshold
		/// - `payment_asset` fungible asset Id to receive payment with
		/// - `duration` length of the auction (in blocks), uses default duration if unspecified
		#[weight = T::WeightInfo::auction()]
		fn auction(origin, collection_id: CollectionId, token_id: T::TokenId, payment_asset: AssetId, reserve_price: Balance, duration: Option<T::BlockNumber>) {
			let origin = ensure_signed(origin)?;
			let current_owner = Self::token_owner(&collection_id, token_id);
			ensure!(current_owner == origin, Error::<T>::NoPermission);

			ensure!(!<Listings<T>>::contains_key(&collection_id, token_id), Error::<T>::TokenListingProtection);

			let listing_end_block =<frame_system::Module<T>>::block_number().saturating_add(duration.unwrap_or_else(T::DefaultListingDuration::get));
			ListingEndSchedule::<T>::mutate(listing_end_block, |schedule| schedule.push((collection_id.clone(), token_id)));
			let listing = Listing::<T>::Auction(
				AuctionListing::<T> {
					payment_asset,
					reserve_price,
					close: listing_end_block,
				}
			);
			Listings::insert(&collection_id, token_id, listing);

			Self::deposit_event(RawEvent::AuctionOpen(collection_id, token_id, payment_asset, reserve_price));
		}

		/// Place a bid on an open auction
		/// - `amount` to bid (in the seller's requested payment asset)
		#[weight = T::WeightInfo::bid()]
		#[transactional]
		fn bid(origin, collection_id: CollectionId, token_id: T::TokenId, amount: Balance) {
			let origin = ensure_signed(origin)?;
			ensure!(<Listings<T>>::contains_key(&collection_id, token_id), Error::<T>::NotForAuction);

			if let Some(Listing::Auction(listing)) = Self::listings(&collection_id, token_id) {
				if let Some(current_bid) = Self::listing_winning_bid(&collection_id, token_id) {
					ensure!(amount > current_bid.1, Error::<T>::BidTooLow);
				} else {
					// first bid
					ensure!(amount >= listing.reserve_price, Error::<T>::BidTooLow);
				}

				// check user has the requisite funds to make this bid
				let balance = T::MultiCurrency::free_balance(&origin, Some(listing.payment_asset));
				if let Some(balance_after_bid) = balance.checked_sub(amount) {
					// TODO: review this during 3.0 upgrade
					// - `amount` is unused
					// - if there are multiple locks on user asset this could return true inaccurately
					// - `T::MultiCurrency::reserve(origin, asset_id, amount)` should be checking this internally...
					let _ = T::MultiCurrency::ensure_can_withdraw(&origin, Some(listing.payment_asset), amount, WithdrawReason::Reserve.into(), balance_after_bid)?;
				}

				// try lock funds
				T::MultiCurrency::reserve(&origin, Some(listing.payment_asset), amount)?;

				ListingWinningBid::<T>::mutate(&collection_id, token_id, |maybe_current_bid| {
					if let Some(current_bid) = maybe_current_bid {
						// replace old bid
						T::MultiCurrency::unreserve(&current_bid.0, Some(listing.payment_asset), current_bid.1);
					}
					*maybe_current_bid = Some((origin, amount))
				});

				Self::deposit_event(RawEvent::Bid(collection_id, token_id, amount));
			} else {
				return Err(Error::<T>::NotForAuction.into());
			}
		}

		/// Close a sale or auction
		/// Requires no successful bids have been made for the auction.
		/// Caller must be the token owner
		#[weight = T::WeightInfo::cancel_sale()]
		fn cancel_sale(origin, collection_id: CollectionId, token_id: T::TokenId) {
			let origin = ensure_signed(origin)?;
			let current_owner = Self::token_owner(&collection_id, token_id);
			ensure!(current_owner == origin, Error::<T>::NoPermission);

			match Self::listings(&collection_id, token_id) {
				Some(Listing::<T>::DirectSale(sale)) => {
					ListingEndSchedule::<T>::mutate(sale.close, |schedule| schedule.retain(|(c, t)| (c, t) != (&collection_id, &token_id)));
					Listings::<T>::remove(&collection_id, token_id);
					Self::deposit_event(RawEvent::DirectSaleClosed(collection_id, token_id));
				},
				Some(Listing::<T>::Auction(auction)) => {
					ensure!(Self::listing_winning_bid(&collection_id, token_id).is_none(), Error::<T>::TokenListingProtection);
					Listings::<T>::remove(&collection_id, token_id);
					ListingEndSchedule::<T>::mutate(auction.close, |schedule| schedule.retain(|(c, t)| (c, t) != (&collection_id, &token_id)));
					Self::deposit_event(RawEvent::AuctionClosed(collection_id, token_id, AuctionClosureReason::VendorCancelled));
				},
				None => {},
			}
		}

	}
}

impl<T: Trait> Module<T> {
	/// Transfer ownership of a token. modifies storage, does no verification, infallible.
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
	/// Close all listings scheduled to close at this block `now`, ensuring payments and ownerships changes are made for winning bids
	/// Metadata for listings will be removed from storage
	/// Returns the number of listings removed
	fn close_listings_at(now: T::BlockNumber) -> u32 {
		let mut removed = 0_u32;
		for (collection_id, token_id) in ListingEndSchedule::<T>::take(now).into_iter() {
			match Listings::<T>::take(&collection_id, token_id) {
				Some(Listing::DirectSale(_)) => {
					Self::deposit_event(RawEvent::DirectSaleClosed(collection_id.clone(), token_id));
				}
				Some(Listing::Auction(listing)) => {
					if let Some((winner, hammer_price)) = ListingWinningBid::<T>::take(&collection_id, token_id) {
						if let Err(err) =
							Self::settle_auction(&collection_id, token_id, &listing, &winner, hammer_price)
						{
							// auction settlement failed despite our prior validations.
							// release winning bid tokens. listing metadadta is removed by now.
							log!(error, "🃏 auction settlement failed: {:?}", err);
							T::MultiCurrency::unreserve(&winner, Some(listing.payment_asset), hammer_price);

							Self::deposit_event(RawEvent::AuctionClosed(
								collection_id.clone(),
								token_id,
								AuctionClosureReason::SettlementFailed,
							));
						} else {
							// auction settlement success
							Self::deposit_event(RawEvent::AuctionSold(
								collection_id.clone(),
								token_id,
								listing.payment_asset,
								hammer_price,
								winner,
							));
						}
					} else {
						// normal closure, no acceptable bids
						Self::deposit_event(RawEvent::AuctionClosed(
							collection_id.clone(),
							token_id,
							AuctionClosureReason::ExpiredNoBids,
						));
					}
				}
				_ => (),
			}
			removed += 1;
		}
		return removed;
	}
	/// Settle an auction listing (guaranteed to be atomic).
	/// - transfer funds from winning bidder to entitled royalty accounts and seller
	/// - transfer ownership to the winning bidder
	#[transactional]
	fn settle_auction(
		collection_id: &CollectionId,
		token_id: T::TokenId,
		listing: &AuctionListing<T>,
		winner: &T::AccountId,
		hammer_price: Balance,
	) -> DispatchResult {
		// if there are no custom royalties, fallback to default if it exists
		let royalties_schedule = if let Some(royalties_schedule) = Self::token_royalties(&collection_id, token_id) {
			royalties_schedule
		} else {
			Self::collection_royalties(&collection_id).unwrap_or_else(Default::default)
		};

		let for_royalties = royalties_schedule.calculate_total_entitlement() * hammer_price;
		let mut for_seller = hammer_price;

		// do royalty payments
		if !for_royalties.is_zero() {
			for (who, entitlement) in royalties_schedule.entitlements.into_iter() {
				let royalty = entitlement * hammer_price;
				let _ = T::MultiCurrency::repatriate_reserved(&winner, Some(listing.payment_asset), &who, royalty)?;
				for_seller -= royalty;
			}
		}

		let current_owner = Self::token_owner(&collection_id, token_id);
		let _ =
			T::MultiCurrency::repatriate_reserved(&winner, Some(listing.payment_asset), &current_owner, for_seller)?;

		// The implementation of `repatriate_reserved` may take less than the required amount and succeed
		// this should not happen but could for reasons outside the control of this module
		ensure!(
			T::MultiCurrency::free_balance(&current_owner, Some(listing.payment_asset)) >= for_seller,
			Error::<T>::InternalPayment
		);

		Self::transfer_ownership(&collection_id, token_id, &current_owner, &winner);

		Ok(())
	}
}
