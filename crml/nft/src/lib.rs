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

use cennznet_primitives::types::{AssetId, Balance, CollectionId};
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage, ensure,
	traits::{ExistenceRequirement, Get, Imbalance, WithdrawReason},
	transactional,
	weights::Weight,
};
use frame_system::ensure_signed;
use prml_support::MultiCurrencyAccounting;
use sp_runtime::{
	traits::{Hash, One, Saturating, Zero},
	DispatchResult,
};
use sp_std::{collections::btree_set::BTreeSet, prelude::*};

mod benchmarking;
mod default_weights;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

mod types;
pub use types::*;

pub trait Trait: frame_system::Trait {
	/// The system event type
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
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
	fn set_owner() -> Weight;
	fn create_collection() -> Weight;
	fn create_token() -> Weight;
	fn transfer() -> Weight;
	fn burn() -> Weight;
	fn sell() -> Weight;
	fn buy() -> Weight;
	fn auction() -> Weight;
	fn bid() -> Weight;
	fn cancel_sale() -> Weight;
}

decl_event!(
	pub enum Event<T> where
		CollectionId = CollectionId,
		TokenId = TokenId<T>,
		<T as frame_system::Trait>::AccountId,
		AssetId = AssetId,
		Balance = Balance,
		Reason = AuctionClosureReason,
		TokenCount = TokenCount,
	{
		/// A new NFT collection was created, (collection, owner)
		CreateCollection(CollectionId, AccountId),
		/// A new NFT was created, (collection, token, quantity, owner)
		CreateToken(CollectionId, TokenId, TokenCount, AccountId),
		/// Token(s) were transferred (token(s), new owner)
		Transfer(Vec<(TokenId, TokenCount)>, AccountId),
		/// An token was burned
		Burn(TokenId, TokenCount),
		/// A fixed price sale has been listed (collection, token, authorised buyer, payment asset, fixed price)
		FixedPriceSaleListed(ListingId, Option<AccountId>, AssetId, Balance),
		/// A fixed price sale has completed (collection, token, new owner, payment asset, fixed price)
		FixedPriceSaleComplete(ListingId, AccountId, AssetId, Balance),
		/// A fixed price sale has closed without selling
		FixedPriceSaleClosed(ListingId),
		/// An auction has opened (collection, token, payment asset, reserve price)
		AuctionOpen(ListingId, AssetId, Balance),
		/// An auction has sold (collection, token, payment asset, bid, new owner)
		AuctionSold(ListingId, AssetId, Balance, AccountId),
		/// An auction has closed without selling (collection, token, reason)
		AuctionClosed(ListingId, Reason),
		/// A new highest bid was placed (collection, token, amount)
		Bid(ListingId, Balance),
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
		/// Too many attributes in the provided schema or data
		SchemaMaxAttributes,
		/// Provided attributes do not match the collection schema
		SchemaMismatch,
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
		/// The token does not exist
		NoToken,
		/// The token is not listed for fixed price sale
		NotForFixedPriceSale,
		/// The token is not listed for auction sale
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
		/// Map from collection to its onchain schema definition
		pub CollectionSchema get(fn collection_schema): map hasher(blake2_128_concat) CollectionId => Option<NFTSchema>;
		/// Map from collection to a base metadata URI for its token's offchain attributes
		pub CollectionMetadataURI get(fn collection_metadata_uri): map hasher(blake2_128_concat) CollectionId => MetadataURI;
		/// Map from collection to its defacto royalty scheme
		pub CollectionRoyalties get(fn collection_royalties): map hasher(blake2_128_concat) CollectionId => Option<RoyaltiesSchedule<T::AccountId>>;
		/// Map from collection to all of its tokens (value is meaningless)
		pub CollectionTokens get(fn collection_tokens): double_map hasher(blake2_128_concat) CollectionId, hasher(identity) TokenId<T> => bool;
		/// Map from token to its collection
		pub TokenCollection get(fn token_collection): map hasher(identity) TokenId<T> => CollectionId;
		/// Map from token to its total issuance
		pub TokenIssuance get(fn token_issuance): map hasher(identity) TokenId<T> => TokenCount;
		/// Map from a token to it's royalty scheme
		pub TokenRoyalties get(fn token_royalties): map hasher(identity) TokenId<T> => Option<RoyaltiesSchedule<T::AccountId>>;
		/// Map from (collection, token) to it's attributes (as defined by schema)
		pub TokenAttributes get(fn token_attributes): map hasher(identity) TokenId<T> => Vec<NFTAttributeValue>;
		/// The next sequential integer token Id within an NFT collection
		/// It is used as material to generate the global `TokenId`
		NextInnerTokenId get(fn next_inner_token_id): map hasher(twox_64_concat) CollectionId => InnerId;
		/// The next available listing Id
		pub NextListingId get(fn next_listing_id): ListingId;
		/// Map from (token id, address) to balance
		pub BalanceOf get(fn balance_of): double_map hasher(identity) TokenId<T>, hasher(blake2_128_concat) T::AccountId => TokenCount;
		/// Map of locks on a balance of tokens (token, owner) to locked amount
		pub TokenLocks get(fn token_locks): double_map hasher(twox_64_concat) TokenId<T>, hasher(blake2_128_concat) T::AccountId => TokenCount;
		/// NFT sale/auction listings. keyed by collection id and token id
		pub Listings get(fn listings): map hasher(twox_64_concat) ListingId => Option<Listing<T>>;
		/// Winning bids on open listings. keyed by collection id and token id
		pub ListingWinningBid get(fn listing_winning_bid): map hasher(twox_64_concat) ListingId => Option<(T::AccountId, Balance)>;
		/// Block numbers where listings will close. It is `Some` if at block number, (collection id, token id) is listed and scheduled to close.
		pub ListingEndSchedule get(fn listing_end_schedule): double_map hasher(twox_64_concat) T::BlockNumber, hasher(twox_64_concat) ListingId => bool;
	}
}

/// The maximum number of attributes in an NFT collection schema
pub const MAX_SCHEMA_FIELDS: u32 = 16;
/// The maximum length of valid collection IDs
pub const MAX_COLLECTION_ID_LENGTH: u8 = 32;
/// The logging target for this module
pub(crate) const LOG_TARGET: &str = "nft";

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

/// Creates a bloke2 hash of (collection_id, inner_token_id)
fn generate_token_id<T: Trait>(collection_id: &[u8], inner_token_id: InnerId) -> TokenId<T> {
	let mut buf = Vec::with_capacity(collection_id.len() + inner_token_id.to_le_bytes().len());
	buf.extend_from_slice(collection_id);
	buf.extend_from_slice(&inner_token_id.to_le_bytes());
	T::Hashing::hash(&buf)
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin, system = frame_system {
		type Error = Error<T>;

		fn deposit_event() = default;

		/// Check and close all expired listings
		fn on_initialize(now: T::BlockNumber) -> Weight {
			// TODO: this is unbounded and could become costly
			let removed_count = Self::close_listings_at(now);
			// 'buy' weight is comparable to succesful closure of an auction
			T::WeightInfo::buy() * removed_count as Weight
		}

		/// Set the owner of a collection
		/// Caller must be the current collection owner
		#[weight = T::WeightInfo::set_owner()]
		fn set_owner(origin, collection_id: CollectionId, new_owner: T::AccountId) -> DispatchResult {
			let origin = ensure_signed(origin)?;
			if let Some(owner) = Self::collection_owner(&collection_id) {
				ensure!(owner == origin, Error::<T>::NoPermission);
				<CollectionOwner<T>>::insert(&collection_id, new_owner);
				Ok(())
			} else {
				Err(Error::<T>::NoCollection.into())
			}
		}

		/// Create a new NFT collection
		/// The caller will be come the collection' owner
		/// `collection_id`- 32 byte utf-8 string
		/// `schema` - onchain attributes for tokens in this collection
		/// `metdata_uri` - offchain metadata uri for tokens in this collection
		/// `royalties_schedule` - defacto royalties plan for secondary sales, this will apply to all tokens in the collection by default.
		#[weight = T::WeightInfo::create_collection()]
		fn create_collection(
			origin,
			collection_id: CollectionId,
			schema: NFTSchema,
			metadata_uri: Option<MetadataURI>,
			royalties_schedule: Option<RoyaltiesSchedule<T::AccountId>>,
		) -> DispatchResult {
			let origin = ensure_signed(origin)?;

			ensure!(!collection_id.is_empty() && collection_id.len() <= MAX_COLLECTION_ID_LENGTH as usize, Error::<T>::CollectionIdInvalid);
			ensure!(core::str::from_utf8(&collection_id).is_ok(), Error::<T>::CollectionIdInvalid);
			ensure!(!<CollectionOwner<T>>::contains_key(&collection_id), Error::<T>::CollectionIdExists);

			ensure!(schema.len() <= MAX_SCHEMA_FIELDS as usize, Error::<T>::SchemaMaxAttributes);

			let mut set = BTreeSet::new();
			for (name, type_id) in schema.iter() {
				// Check the provided attribute types are valid
				ensure!(NFTAttributeValue::is_valid_type_id(*type_id), Error::<T>::SchemaInvalid);
				// Attribute names must be unique (future proofing for map lookups etc.)
				ensure!(set.insert(name), Error::<T>::SchemaDuplicateAttribute);
			}

			// Create the collection, update ownership, and bookkeeping
			if let Some(royalties_schedule) = royalties_schedule {
				ensure!(royalties_schedule.validate(), Error::<T>::RoyaltiesOvercommitment);
				<CollectionRoyalties<T>>::insert(&collection_id, royalties_schedule);
			}
			CollectionSchema::insert(&collection_id, schema);
			if let Some(metadata_uri) = metadata_uri {
				CollectionMetadataURI::insert(&collection_id, metadata_uri);
			}
			<CollectionOwner<T>>::insert(&collection_id, &origin);

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
			Self::batch_create_token(origin, collection_id, One::one(), owner, attributes, royalties_schedule)
		}

		/// Issue a batch of NFTs with the same attributes
		/// `quantity` - how many tokens to mint
		/// `owner` - the token owner
		/// `attributes` - initial values according to the NFT collection/schema
		/// `royalties_schedule` - optional royalty schedule for secondary sales of _this_ token, defaults to the collection config
		/// Caller must be the collection owner
		/// -----------
		/// Weight is O(1) regardless of quantity
		#[weight = T::WeightInfo::create_token()]
		#[transactional]
		fn batch_create_token(origin, collection_id: CollectionId, quantity: TokenCount, owner: T::AccountId, attributes: Vec<NFTAttributeValue>, royalties_schedule: Option<RoyaltiesSchedule<T::AccountId>>) -> DispatchResult {
			let origin = ensure_signed(origin)?;

			ensure!(quantity > Zero::zero(), Error::<T>::NoToken);
			// Permission and existence check
			if let Some(collection_owner) = Self::collection_owner(&collection_id) {
				ensure!(collection_owner == origin, Error::<T>::NoPermission);
			} else {
				return Err(Error::<T>::NoCollection.into());
			}

			ensure!(attributes.len() as u32 <= MAX_SCHEMA_FIELDS, Error::<T>::SchemaMaxAttributes);

			// Check we can issue the new tokens
			let inner_token_id = Self::next_inner_token_id(&collection_id);
			ensure!(
				inner_token_id.checked_add(One::one()).is_some(),
				Error::<T>::NoAvailableIds
			);

			let schema: NFTSchema = Self::collection_schema(&collection_id).ok_or(Error::<T>::NoCollection)?;
			ensure!(attributes.len() == schema.len(), Error::<T>::SchemaMismatch);

			// Build the NFT + schema type level validation
			let token_attributes: Vec<NFTAttributeValue> = schema.iter().zip(attributes.iter()).map(|((_schema_attribute_name, schema_attribute_type), provided_attribute)| {
				// check provided attribute has the correct type
				if *schema_attribute_type == provided_attribute.type_id() {
					ensure!(provided_attribute.len() <= T::MaxAttributeLength::get() as usize, Error::<T>::MaxAttributeLength);
					Ok(provided_attribute.clone())
				} else {
					Err(Error::<T>::SchemaMismatch)
				}
			}).collect::<Result<_, Error<T>>>()?;

			// Ok create tokens
			let token_id = generate_token_id::<T>(&collection_id, inner_token_id);
			<TokenAttributes<T>>::insert(token_id, token_attributes);
			<BalanceOf<T>>::insert(token_id, &owner, quantity);
			<TokenCollection<T>>::insert(token_id, &collection_id);
			<CollectionTokens<T>>::insert(&collection_id, token_id, true);
			<TokenIssuance<T>>::insert(token_id, quantity);
			// will not overflow, asserted prior qed.
			NextInnerTokenId::mutate(&collection_id, |i| *i += InnerId::one());

			// Add royalties, if any
			if let Some(royalties_schedule) = royalties_schedule {
				ensure!(royalties_schedule.validate(), Error::<T>::RoyaltiesOvercommitment);
				<TokenRoyalties<T>>::insert(token_id, royalties_schedule);
			};

			Self::deposit_event(RawEvent::CreateToken(collection_id, token_id, quantity, owner));

			Ok(())
		}

		/// Transfer ownership of a batch of NFTs (atomic)
		/// Tokens be in the same collection
		/// Caller must be the token owner
		#[weight = {
			T::WeightInfo::transfer().saturating_mul(tokens.len() as u64)
		}]
		#[transactional]
		fn batch_transfer(origin, tokens: Vec<(T::Hash, TokenCount)>, new_owner: T::AccountId) {
			let origin = ensure_signed(origin)?;

			ensure!(tokens.len() > Zero::zero(), Error::<T>::NoToken);
			Self::do_transfer(&origin, &tokens, &new_owner)?;

			Self::deposit_event(RawEvent::Transfer(tokens, new_owner));
		}

		/// Transfer ownership of an NFT
		/// Caller must be the token owner
		#[weight = T::WeightInfo::transfer()]
		fn transfer(origin, token_id: TokenId<T>, new_owner: T::AccountId) -> DispatchResult {
			Self::batch_transfer(origin, vec![(token_id, One::one())], new_owner)
		}

		/// Burn an NFT 🔥
		/// Caller must be the token owner
		#[weight = T::WeightInfo::burn()]
		fn burn(origin, token_id: TokenId<T>, quantity: TokenCount) {
			let origin = ensure_signed(origin)?;

			ensure!(quantity > Zero::zero(), Error::<T>::NoToken);

			let owned = Self::balance_of(token_id, &origin);
			ensure!(owned >= quantity, Error::<T>::NoToken);
			ensure!(owned.saturating_sub(Self::token_locks(token_id, &origin)) >= quantity, Error::<T>::TokenListingProtection);

			// Update token ownership.
			if Self::token_issuance(token_id).saturating_sub(quantity).is_zero() {
				// this is the last of the tokens
				<BalanceOf<T>>::remove(token_id, &origin);
				<TokenAttributes<T>>::remove(token_id);
				let collection_id = <TokenCollection<T>>::take(token_id);
				<CollectionTokens<T>>::remove(collection_id, token_id);
				<TokenRoyalties<T>>::remove(token_id);
				<TokenIssuance<T>>::remove(token_id);
			} else {
				<BalanceOf<T>>::mutate(token_id, &origin, |q| *q = q.saturating_sub(quantity));
				<TokenIssuance<T>>::mutate(token_id, |q| *q = q.saturating_sub(quantity));
			}

			Self::deposit_event(RawEvent::Burn(token_id, quantity));
		}

		/// Sell an NFT at a fixed price
		/// Tokens are held in escrow until closure of the sale
		/// `quantity` how many of the token to sell
		/// `buyer` optionally, the account to receive the NFT. If unspecified, then any account may purchase
		/// `asset_id` fungible asset Id to receive as payment for the NFT
		/// `fixed_price` ask price
		/// `duration` listing duration time in blocks from now
		/// Caller must be the token owner
		#[weight = T::WeightInfo::sell()]
		fn sell(origin, token_id: TokenId<T>, quantity: TokenCount, buyer: Option<T::AccountId>, payment_asset: AssetId, fixed_price: Balance, duration: Option<T::BlockNumber>) {
			let origin = ensure_signed(origin)?;
			ensure!(quantity > Zero::zero(), Error::<T>::NoToken);

			let owned = Self::balance_of(token_id, &origin);
			ensure!(owned >= quantity, Error::<T>::NoToken);
			let locked = Self::token_locks(token_id, &origin);
			ensure!(owned.saturating_sub(locked) >= quantity, Error::<T>::TokenListingProtection);

			let listing_id = Self::next_listing_id();
			ensure!(listing_id.checked_add(One::one()).is_some(), Error::<T>::NoAvailableIds);

			let listing_end_block = <frame_system::Module<T>>::block_number().saturating_add(duration.unwrap_or_else(T::DefaultListingDuration::get));
			ListingEndSchedule::<T>::insert(listing_end_block, listing_id, true);
			let listing = Listing::<T>::FixedPrice(
				FixedPriceListing::<T> {
					payment_asset,
					fixed_price,
					close: listing_end_block,
					buyer: buyer.clone(),
					token_id,
					quantity,
					seller: origin.clone(),
				}
			);
			<TokenLocks::<T>>::insert(token_id, &origin, locked + quantity);
			Listings::insert(listing_id, listing);
			NextListingId::mutate(|i| *i += 1);

			Self::deposit_event(RawEvent::FixedPriceSaleListed(listing_id, buyer, payment_asset, fixed_price));
		}

		/// Buy an NFT for its listed price, must be listed for sale
		#[weight = T::WeightInfo::buy()]
		#[transactional]
		fn buy(origin, listing_id: ListingId) {
			let origin = ensure_signed(origin)?;
			ensure!(<Listings<T>>::contains_key(listing_id), Error::<T>::NotForFixedPriceSale);

			if let Some(Listing::FixedPrice(listing)) = Self::listings(listing_id) {

				// if buyer is specified in the listing, then `origin` must be buyer
				if let Some(buyer) = &listing.buyer {
					ensure!(&origin == buyer, Error::<T>::NoPermission);
				}

				// if there are no custom royalties, fallback to default if it exists
				let royalties_schedule = if let Some(royalties_schedule) = Self::token_royalties(listing.token_id) {
					royalties_schedule
				} else {
					let collection_id = Self::token_collection(listing.token_id);
					Self::collection_royalties(&collection_id).unwrap_or_else(Default::default)
				};

				let royalty_fees = royalties_schedule.calculate_total_entitlement();
				if royalty_fees.is_zero() {
					// full proceeds to seller/`current_owner`
					T::MultiCurrency::transfer(&origin, &listing.seller, Some(listing.payment_asset), listing.fixed_price, ExistenceRequirement::AllowDeath)?;
				} else {
					// withdraw funds from buyer, split between royalty payments and seller
					let mut for_seller = listing.fixed_price;
					let mut imbalance = T::MultiCurrency::withdraw(&origin, Some(listing.payment_asset), listing.fixed_price, WithdrawReason::Transfer.into(), ExistenceRequirement::AllowDeath)?;
					for (who, entitlement) in royalties_schedule.entitlements.into_iter() {
						let royalty = entitlement * listing.fixed_price;
						for_seller -= royalty;
						imbalance = imbalance.offset(T::MultiCurrency::deposit_into_existing(&who, Some(listing.payment_asset), royalty)?).map_err(|_| Error::<T>::InternalPayment)?;
					}
					imbalance.offset(T::MultiCurrency::deposit_into_existing(&listing.seller, Some(listing.payment_asset), for_seller)?).map_err(|_| Error::<T>::InternalPayment)?;
				}

				// must not fail now that payment has been made
				<TokenLocks<T>>::mutate(listing.token_id, &listing.seller, |q| *q = q.saturating_sub(listing.quantity));
				Self::do_transfer(&listing.seller, &[(listing.token_id, listing.quantity)], &origin)?;
				Self::remove_fixed_price_listing(listing_id);

				Self::deposit_event(RawEvent::FixedPriceSaleComplete(listing_id, origin, listing.payment_asset, listing.fixed_price));
			} else {
				return Err(Error::<T>::NotForFixedPriceSale.into());
			}
		}

		/// Sell NFT on the open market to the highest bidder
		/// Tokens are held in escrow until closure of the auction
		/// Caller must be the token owner
		/// - `quantity` how many of the token to sell
		/// - `payment_asset` fungible asset Id to receive payment with
		/// - `reserve_price` winning bid must be over this threshold
		/// - `duration` length of the auction (in blocks), uses default duration if unspecified
		#[weight = T::WeightInfo::auction()]
		fn auction(origin, token_id: TokenId<T>, quantity: TokenCount, payment_asset: AssetId, reserve_price: Balance, duration: Option<T::BlockNumber>) {
			let origin = ensure_signed(origin)?;
			ensure!(quantity > Zero::zero(), Error::<T>::NoToken);

			let owned = Self::balance_of(token_id, &origin);
			ensure!(owned >= quantity, Error::<T>::NoToken);
			let locked = Self::token_locks(token_id, &origin);
			ensure!(owned.saturating_sub(locked) >= quantity, Error::<T>::TokenListingProtection);

			let listing_id = Self::next_listing_id();
			ensure!(listing_id.checked_add(One::one()).is_some(), Error::<T>::NoAvailableIds);

			let listing_end_block =<frame_system::Module<T>>::block_number().saturating_add(duration.unwrap_or_else(T::DefaultListingDuration::get));
			ListingEndSchedule::<T>::insert(listing_end_block, listing_id, true);
			let listing = Listing::<T>::Auction(
				AuctionListing::<T> {
					payment_asset,
					reserve_price,
					close: listing_end_block,
					token_id,
					quantity,
					seller: origin.clone(),
				}
			);
			<TokenLocks::<T>>::insert(token_id, &origin, locked + quantity);
			Listings::insert(listing_id, listing);
			NextListingId::mutate(|i| *i += 1);

			Self::deposit_event(RawEvent::AuctionOpen(listing_id, payment_asset, reserve_price));
		}

		/// Place a bid on an open auction
		/// - `amount` to bid (in the seller's requested payment asset)
		#[weight = T::WeightInfo::bid()]
		#[transactional]
		fn bid(origin, listing_id: ListingId, amount: Balance) {
			let origin = ensure_signed(origin)?;

			if let Some(Listing::Auction(listing)) = Self::listings(listing_id) {
				if let Some(current_bid) = Self::listing_winning_bid(listing_id) {
					ensure!(amount > current_bid.1, Error::<T>::BidTooLow);
				} else {
					// first bid
					ensure!(amount >= listing.reserve_price, Error::<T>::BidTooLow);
				}

				// check user has the requisite funds to make this bid
				let balance = T::MultiCurrency::free_balance(&origin, Some(listing.payment_asset));
				if let Some(balance_after_bid) = balance.checked_sub(amount) {
					// TODO: review behaviour with 3.0 upgrade: https://github.com/cennznet/cennznet/issues/414
					// - `amount` is unused
					// - if there are multiple locks on user asset this could return true inaccurately
					// - `T::MultiCurrency::reserve(origin, asset_id, amount)` should be checking this internally...
					let _ = T::MultiCurrency::ensure_can_withdraw(&origin, Some(listing.payment_asset), amount, WithdrawReason::Reserve.into(), balance_after_bid)?;
				}

				// try lock funds
				T::MultiCurrency::reserve(&origin, Some(listing.payment_asset), amount)?;

				ListingWinningBid::<T>::mutate(listing_id, |maybe_current_bid| {
					if let Some(current_bid) = maybe_current_bid {
						// replace old bid
						T::MultiCurrency::unreserve(&current_bid.0, Some(listing.payment_asset), current_bid.1);
					}
					*maybe_current_bid = Some((origin, amount))
				});

				Self::deposit_event(RawEvent::Bid(listing_id, amount));
			} else {
				return Err(Error::<T>::NotForAuction.into());
			}
		}

		/// Close a sale or auction returning tokens
		/// Requires no successful bids have been made for an auction.
		/// Caller must be the listed seller
		#[weight = T::WeightInfo::cancel_sale()]
		fn cancel_sale(origin, listing_id: ListingId) {
			let origin = ensure_signed(origin)?;

			match Self::listings(listing_id) {
				Some(Listing::<T>::FixedPrice(sale)) => {
					ensure!(sale.seller == origin, Error::<T>::NoPermission);
					Listings::<T>::remove(listing_id);
					ListingEndSchedule::<T>::remove(sale.close, listing_id);
					<TokenLocks<T>>::mutate(sale.token_id, &origin, |q| *q = q.saturating_sub(sale.quantity));

					Self::deposit_event(RawEvent::FixedPriceSaleClosed(listing_id));
				},
				Some(Listing::<T>::Auction(auction)) => {
					ensure!(auction.seller == origin, Error::<T>::NoPermission);
					ensure!(Self::listing_winning_bid(listing_id).is_none(), Error::<T>::TokenListingProtection);
					Listings::<T>::remove(listing_id);
					ListingEndSchedule::<T>::remove(auction.close, listing_id);
					<TokenLocks<T>>::mutate(auction.token_id, &origin, |q| *q = q.saturating_sub(auction.quantity));
					Self::deposit_event(RawEvent::AuctionClosed(listing_id, AuctionClosureReason::VendorCancelled));
				},
				None => {},
			}
		}
	}
}

impl<T: Trait> Module<T> {
	/// Transfer amounts of tokens from `current_owner` to `new_owner`
	/// fails on insufficient balance
	fn do_transfer(
		current_owner: &T::AccountId,
		tokens: &[(TokenId<T>, TokenCount)],
		new_owner: &T::AccountId,
	) -> DispatchResult {
		for (token_id, quantity) in tokens.iter() {
			let owned = Self::balance_of(token_id, current_owner);
			ensure!(owned >= *quantity, Error::<T>::NoToken);
			ensure!(
				owned.saturating_sub(Self::token_locks(token_id, &current_owner)) >= *quantity,
				Error::<T>::TokenListingProtection
			);
			if owned - quantity == 0 {
				// down to 0, free the storage key
				<BalanceOf<T>>::take(token_id, current_owner);
			} else {
				<BalanceOf<T>>::mutate(token_id, current_owner, |q| *q -= quantity);
			}
			<BalanceOf<T>>::mutate(token_id, new_owner, |q| *q += quantity);
		}

		Ok(())
	}
	/// Find the tokens owned by an `address` in the given collection
	pub fn collected_tokens(collection_id: &CollectionId, address: &T::AccountId) -> Vec<TokenId<T>> {
		<CollectionTokens<T>>::iter_prefix(collection_id)
			.into_iter()
			.filter_map(|(token_id, _)| {
				if Self::balance_of(token_id, address).is_zero() {
					None
				} else {
					Some(token_id)
				}
			})
			.collect()
	}
	/// Remove a single fixed price listing and all it's metadata
	fn remove_fixed_price_listing(listing_id: ListingId) {
		let listing_type = Listings::<T>::take(listing_id);
		ListingWinningBid::<T>::remove(listing_id);
		if let Some(Listing::<T>::FixedPrice(listing)) = listing_type {
			ListingEndSchedule::<T>::remove(listing.close, listing_id);
		}
	}
	/// Close all listings scheduled to close at this block `now`, ensuring payments and ownerships changes are made for winning bids
	/// Metadata for listings will be removed from storage
	/// Returns the number of listings removed
	fn close_listings_at(now: T::BlockNumber) -> u32 {
		let mut removed = 0_u32;
		for (listing_id, _) in ListingEndSchedule::<T>::drain_prefix(now).into_iter() {
			match Listings::<T>::take(listing_id) {
				Some(Listing::FixedPrice(listing)) => {
					<TokenLocks<T>>::mutate(listing.token_id, &listing.seller, |q| {
						*q = q.saturating_sub(listing.quantity)
					});
					Self::deposit_event(RawEvent::FixedPriceSaleClosed(listing_id));
				}
				Some(Listing::Auction(listing)) => {
					if let Some((winner, hammer_price)) = ListingWinningBid::<T>::take(listing_id) {
						if let Err(err) = Self::settle_auction(&listing, &winner, hammer_price) {
							// auction settlement failed despite our prior validations.
							// release winning bid funds
							log!(error, "🃏 auction settlement failed: {:?}", err);
							T::MultiCurrency::unreserve(&winner, Some(listing.payment_asset), hammer_price);
							// release listing tokens
							<TokenLocks<T>>::mutate(listing.token_id, &listing.seller, |q| {
								*q = q.saturating_sub(listing.quantity)
							});
							// listing metadadta is removed by now.
							Self::deposit_event(RawEvent::AuctionClosed(
								listing_id,
								AuctionClosureReason::SettlementFailed,
							));
						} else {
							// auction settlement success
							Self::deposit_event(RawEvent::AuctionSold(
								listing_id,
								listing.payment_asset,
								hammer_price,
								winner,
							));
						}
					} else {
						// normal closure, no acceptable bids
						// release listed tokens
						<TokenLocks<T>>::mutate(listing.token_id, &listing.seller, |q| {
							*q = q.saturating_sub(listing.quantity)
						});
						// listing metadadta is removed by now.
						Self::deposit_event(RawEvent::AuctionClosed(listing_id, AuctionClosureReason::ExpiredNoBids));
					}
				}
				None => (),
			}
			removed += 1;
		}

		removed
	}
	/// Settle an auction listing (guaranteed to be atomic).
	/// - transfer funds from winning bidder to entitled royalty accounts and seller
	/// - transfer ownership to the winning bidder
	#[transactional]
	fn settle_auction(listing: &AuctionListing<T>, winner: &T::AccountId, hammer_price: Balance) -> DispatchResult {
		// if there are no custom royalties, fallback to default if it exists
		let royalties_schedule = if let Some(royalties_schedule) = Self::token_royalties(listing.token_id) {
			royalties_schedule
		} else {
			let collection_id = Self::token_collection(listing.token_id);
			Self::collection_royalties(collection_id).unwrap_or_else(Default::default)
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

		let seller_balance = T::MultiCurrency::free_balance(&listing.seller, Some(listing.payment_asset));
		let _ =
			T::MultiCurrency::repatriate_reserved(&winner, Some(listing.payment_asset), &listing.seller, for_seller)?;

		// The implementation of `repatriate_reserved` may take less than the required amount and succeed
		// this should not happen but could for reasons outside the control of this module
		ensure!(
			T::MultiCurrency::free_balance(&listing.seller, Some(listing.payment_asset))
				>= seller_balance.saturating_add(for_seller),
			Error::<T>::InternalPayment
		);
		<TokenLocks<T>>::mutate(listing.token_id, &listing.seller, |q| {
			*q = q.saturating_sub(listing.quantity)
		});
		Self::do_transfer(&listing.seller, &[(listing.token_id, listing.quantity)], winner)?;

		Ok(())
	}
}
