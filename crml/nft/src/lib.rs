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
//!
//! *Collections*:
//!  A namespacing tool for logical grouping of related tokens
//!  Tokens within a collection can have the same royalties fees, metadata base URIs, and owner address
//!
//! *Series*:
//! A grouping of tokens within a collection namespace
//! Tokens in the same series will have exact attributes and royalties.
//! A series of size 1 contains an NFT, while a series with > 1 token is considered semi-fungible
//!
//! *Tokens*:
//!  Individual tokens are uniquely identifiable by a tuple of (collection, series, serial number)
//!

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
	traits::{One, Saturating, Zero},
	DispatchResult,
};
use sp_std::prelude::*;

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
	fn mint_unique() -> Weight;
	fn mint_series() -> Weight;
	fn mint_additional() -> Weight;
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
		<T as frame_system::Trait>::AccountId,
		AssetId = AssetId,
		Balance = Balance,
		Reason = AuctionClosureReason,
		SerialNumber = SerialNumber,
		SeriesId = SeriesId,
		TokenCount = TokenCount,
	{
		/// A new NFT collection was created, (collection, owner)
		CreateCollection(CollectionId, AccountId),
		/// A new set of tokens was created, (collection, series id, quantity, owner)
		CreateSeries(CollectionId, SeriesId, TokenCount, AccountId),
		/// Additional tokens were created, (collection, series id, quantity, owner)
		CreateAdditional(CollectionId, SeriesId, TokenCount, AccountId),
		/// A one off token was created, (collection, series id, serial number, owner)
		CreateToken(CollectionId, SeriesId, SerialNumber, AccountId),
		/// Token(s) were transferred (collection, series id, token(s), new owner)
		Transfer(CollectionId, SeriesId, Vec<SerialNumber>, AccountId),
		/// Tokens were burned (collection, series id, serial number)
		Burn(CollectionId, SeriesId, Vec<SerialNumber>),
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
		/// Map from collection to a base metadata URI for its token's offchain attributes
		pub CollectionMetadatURI get(fn collection_metadata_uri): map hasher(blake2_128_concat) CollectionId => Option<MetadataBaseURI>;
		/// Map from collection to its defacto royalty scheme
		pub CollectionRoyalties get(fn collection_royalties): map hasher(blake2_128_concat) CollectionId => Option<RoyaltiesSchedule<T::AccountId>>;
		/// Map from token to its locked status
		pub TokenLocks get(fn token_locks): map hasher(twox_64_concat) TokenId => bool;
		/// Map from a token to its owner
		/// The token Id is split in this map to allow better indexing (collection, series) + (serial number)
		pub TokenOwner get(fn token_owner): double_map hasher(twox_64_concat) (CollectionId, SeriesId), hasher(twox_64_concat) SerialNumber => T::AccountId;
		/// Map from (collection, set) to its attributes
		pub SeriesAttributes get(fn series_attributes): double_map hasher(blake2_128_concat) CollectionId, hasher(twox_64_concat) SeriesId => Vec<NFTAttributeValue>;
		/// Map from a (collection, set) to its total issuance
		pub SeriesIssuance get(fn series_issuance): double_map hasher(blake2_128_concat) CollectionId, hasher(twox_64_concat) SeriesId =>  TokenCount;
		/// Map from a (collection, set) to its royalty scheme
		pub SeriesRoyalties get(fn series_royalties): double_map hasher(blake2_128_concat) CollectionId, hasher(twox_64_concat) SeriesId => Option<RoyaltiesSchedule<T::AccountId>>;
		/// Map from a token series to its metadata URI path. This should be joined wih the collection base path
		pub SeriesMetadataURI get(fn series_metadata_uri): double_map hasher(blake2_128_concat) CollectionId, hasher(twox_64_concat) SeriesId => Option<Vec<u8>>;
		/// The next group Id within an NFT collection
		/// It is used as material to generate the global `TokenId`
		NextSeriesId get(fn next_series_id): map hasher(blake2_128_concat) CollectionId => SeriesId;
		/// The next available serial number in a given (colleciton, set)
		NextSerialNumber get(fn next_serial_number): double_map hasher(blake2_128_concat) CollectionId, hasher(twox_64_concat) SeriesId => SerialNumber;
		/// The next available listing Id
		pub NextListingId get(fn next_listing_id): ListingId;
		/// NFT sale/auction listings keyed by collection id and token id
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

		/// Create a new token collection
		///
		/// The caller will become the collection owner
		/// `collection_id`- 32 byte utf-8 string
		/// `metdata_base_uri` - Base URI for off-chain metadata for tokens in this collection
		/// `royalties_schedule` - defacto royalties plan for secondary sales, this will apply to all tokens in the collection by default.
		#[weight = T::WeightInfo::create_collection()]
		fn create_collection(
			origin,
			collection_id: CollectionId,
			metadata_base_uri: Option<MetadataBaseURI>,
			royalties_schedule: Option<RoyaltiesSchedule<T::AccountId>>,
		) -> DispatchResult {
			let origin = ensure_signed(origin)?;

			ensure!(!collection_id.is_empty() && collection_id.len() <= MAX_COLLECTION_ID_LENGTH as usize, Error::<T>::CollectionIdInvalid);
			ensure!(core::str::from_utf8(&collection_id).is_ok(), Error::<T>::CollectionIdInvalid);
			ensure!(!<CollectionOwner<T>>::contains_key(&collection_id), Error::<T>::CollectionIdExists);

			// Create the collection, update ownership, and bookkeeping
			if let Some(royalties_schedule) = royalties_schedule {
				ensure!(royalties_schedule.validate(), Error::<T>::RoyaltiesOvercommitment);
				<CollectionRoyalties<T>>::insert(&collection_id, royalties_schedule);
			}
			if let Some(metadata_base_uri) = metadata_base_uri {
				CollectionMetadatURI::insert(&collection_id, metadata_base_uri);
			}
			<CollectionOwner<T>>::insert(&collection_id, &origin);

			Self::deposit_event(RawEvent::CreateCollection(collection_id, origin));

			Ok(())
		}

		/// Mint a single token (NFT)
		///
		/// `owner` - the token owner, defaults to the caller
		/// `attributes` - initial values according to the NFT collection/schema
		/// `royalties_schedule` - optional royalty schedule for secondary sales of _this_ token, defaults to the collection config
		/// `metadata_path` - URI path to the offchain metadata relative to the collection base URI
		/// Caller must be the collection owner
		#[weight = T::WeightInfo::mint_unique()]
		#[transactional]
		fn mint_unique(
			origin,
			collection_id: CollectionId,
			owner: Option<T::AccountId>,
			attributes: Vec<NFTAttributeValue>,
			royalties_schedule: Option<RoyaltiesSchedule<T::AccountId>>,
			metadata_path: Option<Vec<u8>>,
		) -> DispatchResult {
			Self::mint_series(origin, collection_id, One::one(), owner, attributes, royalties_schedule, metadata_path)
		}

		/// Mint a series of tokens distinguishable only by a serial number (SFT)
		///
		/// `quantity` - how many tokens to mint
		/// `owner` - the token owner, defaults to the caller
		/// `is_limited_edition` - signal whether the series is a limited edition or not
		/// `attributes` - all tokens in series will have these values
		/// `royalties_schedule` - optional royalty schedule for secondary sales of _this_ token, defaults to the collection config
		/// `metadata_path` - URI path to token offchain metadata relative to the collection base URI
		/// Caller must be the collection owner
		/// -----------
		/// Performs O(N) writes where N is `quantity`
		#[weight = T::WeightInfo::mint_unique().saturating_mul(*quantity as Weight)]
		#[transactional]
		fn mint_series(
			origin,
			collection_id: CollectionId,
			quantity: TokenCount,
			owner: Option<T::AccountId>,
			attributes: Vec<NFTAttributeValue>,
			royalties_schedule: Option<RoyaltiesSchedule<T::AccountId>>,
			metadata_path: Option<Vec<u8>>,
		) -> DispatchResult {
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
			let series_id = Self::next_series_id(&collection_id);
			ensure!(
				series_id.checked_add(One::one()).is_some(),
				Error::<T>::NoAvailableIds
			);

			// Ok create the token series data
			// All these attributes are the same in a series
			SeriesAttributes::insert(&collection_id, series_id, attributes);
			SeriesIssuance::insert(&collection_id, series_id, quantity);
			if let Some(metadata_path) = metadata_path {
				SeriesMetadataURI::insert(&collection_id, series_id, metadata_path);
			}
			// Add set royalties, if any
			if let Some(royalties_schedule) = royalties_schedule {
				ensure!(royalties_schedule.validate(), Error::<T>::RoyaltiesOvercommitment);
				<SeriesRoyalties<T>>::insert(&collection_id, series_id, royalties_schedule);
			};

			// Now mint the series tokens
			let owner = owner.unwrap_or_else(|| origin.clone());

			// TODO: can we do a lazy mint or similar here to avoid the O(N)...
			for serial_number in 0..quantity as usize {
				<TokenOwner<T>>::insert((&collection_id, series_id), serial_number as SerialNumber, &owner);
			}

			// will not overflow, asserted prior qed.
			NextSeriesId::mutate(&collection_id, |i| *i += SeriesId::one());

			if quantity > One::one() {
				NextSerialNumber::insert(&collection_id, series_id, quantity as SerialNumber);
				Self::deposit_event(RawEvent::CreateSeries(collection_id, series_id, quantity, owner));
			} else {
				Self::deposit_event(RawEvent::CreateToken(collection_id, series_id, 0, owner));
			}

			Ok(())
		}

		/// Mint additional tokens to an SFT series
		///
		/// `quantity` - how many tokens to mint
		/// `owner` - the token owner, defaults to the caller
		/// Caller must be the collection owner
		/// -----------
		/// Weight is O(N) where N is `quantity`
		#[weight = {
			T::WeightInfo::mint_additional().saturating_mul(*quantity as Weight)
		}]
		#[transactional]
		fn mint_additional(
			origin,
			collection_id: CollectionId,
			series_id: SeriesId,
			quantity: TokenCount,
			owner: Option<T::AccountId>,
		) -> DispatchResult {
			let origin = ensure_signed(origin)?;

			ensure!(quantity > Zero::zero(), Error::<T>::NoToken);
			let serial_number = Self::next_serial_number(&collection_id, series_id);
			ensure!(serial_number > Zero::zero(), Error::<T>::NoToken);
			ensure!(serial_number.checked_add(quantity).is_some(), Error::<T>::NoAvailableIds);

			// Permission and existence check
			if let Some(collection_owner) = Self::collection_owner(&collection_id) {
				ensure!(collection_owner == origin, Error::<T>::NoPermission);
			} else {
				return Err(Error::<T>::NoCollection.into());
			}

			// Mint the set tokens
			let owner = owner.unwrap_or_else(|| origin.clone());
			for serial_number in serial_number..serial_number + quantity {
				<TokenOwner<T>>::insert((&collection_id, series_id), serial_number as SerialNumber, &owner);
			}

			NextSerialNumber::mutate(&collection_id, series_id, |q| *q = q.saturating_add(quantity));

			Self::deposit_event(RawEvent::CreateAdditional(collection_id, series_id, quantity, owner));

			Ok(())
		}

		/// Transfer ownership of a batch of NFTs (atomic)
		/// Tokens be in the same collection
		/// Caller must be the token owner
		#[weight = {
			T::WeightInfo::transfer().saturating_mul(serial_numbers.len() as Weight)
		}]
		#[transactional]
		fn transfer_batch(origin, collection_id: CollectionId, series_id: SeriesId, serial_numbers: Vec<SerialNumber>, new_owner: T::AccountId) {
			let origin = ensure_signed(origin)?;

			ensure!(serial_numbers.len() > Zero::zero(), Error::<T>::NoToken);
			Self::do_transfer(&origin, &collection_id, series_id, &serial_numbers, &new_owner)?;

			Self::deposit_event(RawEvent::Transfer(collection_id, series_id, serial_numbers, new_owner));
		}

		/// Transfer ownership of an NFT
		/// Caller must be the token owner
		#[weight = T::WeightInfo::transfer()]
		fn transfer(origin, token_id: TokenId, new_owner: T::AccountId) -> DispatchResult {
			let (collection_id, series_id, serial_number) = token_id;
			Self::transfer_batch(origin, collection_id, series_id, vec![serial_number], new_owner)
		}

		/// Burn some tokens 🔥 (atomic)
		///
		/// Caller must be the token owner
		/// Fails on duplicate serials
		#[weight = T::WeightInfo::burn()]
		#[transactional]
		fn burn(origin, collection_id: CollectionId, series_id: SeriesId, serial_numbers: Vec<SerialNumber>) {
			let origin = ensure_signed(origin)?;

			ensure!(!serial_numbers.is_empty(), Error::<T>::NoToken);

			for serial_number in serial_numbers.iter() {
				ensure!(!Self::token_locks((&collection_id, series_id, serial_number)), Error::<T>::TokenListingProtection);
				ensure!(Self::token_owner((&collection_id, series_id), serial_number) == origin, Error::<T>::NoPermission);
				<TokenOwner<T>>::remove((&collection_id, series_id), serial_number);
			}

			if Self::series_issuance(&collection_id, series_id).saturating_sub(serial_numbers.len() as TokenCount).is_zero() {
				// this is the last of the tokens
				SeriesAttributes::remove(&collection_id, series_id);
				<SeriesRoyalties<T>>::remove(&collection_id, series_id);
				SeriesIssuance::remove(&collection_id, series_id);
				SeriesMetadataURI::remove(&collection_id, series_id);
			} else {
				SeriesIssuance::mutate(&collection_id, series_id, |q| *q = q.saturating_sub(serial_numbers.len() as TokenCount));
			}

			Self::deposit_event(RawEvent::Burn(collection_id, series_id, serial_numbers));
		}

		/// Sell an NFT at a fixed price
		/// Tokens are held in escrow until closure of the sale
		/// `quantity` how many of the token to sell
		/// `buyer` optionally, the account to receive the NFT. If unspecified, then any account may purchase
		/// `asseries_id` fungible asset Id to receive as payment for the NFT
		/// `fixed_price` ask price
		/// `duration` listing duration time in blocks from now
		/// Caller must be the token owner
		#[weight = T::WeightInfo::sell()]
		fn sell(origin, token_id: TokenId, buyer: Option<T::AccountId>, payment_asset: AssetId, fixed_price: Balance, duration: Option<T::BlockNumber>) {
			let origin = ensure_signed(origin)?;

			let (collection_id, series_id, serial_number) = &token_id;
			ensure!(Self::token_owner((collection_id, series_id), serial_number) == origin, Error::<T>::NoToken);
			ensure!(!Self::token_locks(&token_id), Error::<T>::TokenListingProtection);

			let listing_id = Self::next_listing_id();
			ensure!(listing_id.checked_add(One::one()).is_some(), Error::<T>::NoAvailableIds);

			let listing_end_block = <frame_system::Module<T>>::block_number().saturating_add(duration.unwrap_or_else(T::DefaultListingDuration::get));
			ListingEndSchedule::<T>::insert(listing_end_block, listing_id, true);
			let listing = Listing::<T>::FixedPrice(
				FixedPriceListing::<T> {
					payment_asset,
					fixed_price,
					close: listing_end_block,
					token_id: token_id.clone(),
					buyer: buyer.clone(),
					seller: origin.clone(),
				}
			);
			TokenLocks::insert(token_id, true);
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

				let (collection_id, series_id, serial_number) = &listing.token_id;

				// if there are no custom royalties, fallback to default if it exists
				let royalties_schedule = if let Some(royalties_schedule) = Self::series_royalties(collection_id, series_id) {
					royalties_schedule
				} else {
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
				TokenLocks::remove(&listing.token_id);
				Self::do_transfer(&listing.seller, &collection_id, *series_id, &[*serial_number], &origin)?;
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
		fn auction(origin, token_id: TokenId, payment_asset: AssetId, reserve_price: Balance, duration: Option<T::BlockNumber>) {
			let origin = ensure_signed(origin)?;

			let (collection_id, series_id, serial_number) = &token_id;
			ensure!(Self::token_owner((collection_id, series_id), serial_number) == origin, Error::<T>::NoToken);
			ensure!(!Self::token_locks(&token_id), Error::<T>::TokenListingProtection);

			let listing_id = Self::next_listing_id();
			ensure!(listing_id.checked_add(One::one()).is_some(), Error::<T>::NoAvailableIds);

			let listing_end_block =<frame_system::Module<T>>::block_number().saturating_add(duration.unwrap_or_else(T::DefaultListingDuration::get));
			ListingEndSchedule::<T>::insert(listing_end_block, listing_id, true);
			let listing = Listing::<T>::Auction(
				AuctionListing::<T> {
					payment_asset,
					reserve_price,
					close: listing_end_block,
					token_id: token_id.clone(),
					seller: origin.clone(),
				}
			);
			TokenLocks::insert(&token_id, true);
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
					// - `T::MultiCurrency::reserve(origin, asseries_id, amount)` should be checking this internally...
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
					TokenLocks::remove(sale.token_id);

					Self::deposit_event(RawEvent::FixedPriceSaleClosed(listing_id));
				},
				Some(Listing::<T>::Auction(auction)) => {
					ensure!(auction.seller == origin, Error::<T>::NoPermission);
					ensure!(Self::listing_winning_bid(listing_id).is_none(), Error::<T>::TokenListingProtection);
					Listings::<T>::remove(listing_id);
					ListingEndSchedule::<T>::remove(auction.close, listing_id);
					TokenLocks::remove(auction.token_id);

					Self::deposit_event(RawEvent::AuctionClosed(listing_id, AuctionClosureReason::VendorCancelled));
				},
				None => {},
			}
		}
	}
}

impl<T: Trait> Module<T> {
	/// Transfer the given tokens from `current_owner` to `new_owner`
	/// fails on insufficient balance
	fn do_transfer(
		current_owner: &T::AccountId,
		collection_id: &CollectionId,
		series_id: SeriesId,
		serial_numbers: &[SerialNumber],
		new_owner: &T::AccountId,
	) -> DispatchResult {
		for serial_number in serial_numbers.iter() {
			let token_id = (collection_id, series_id, *serial_number);
			ensure!(!Self::token_locks(token_id), Error::<T>::TokenListingProtection);
			ensure!(
				&Self::token_owner((collection_id, series_id), serial_number) == current_owner,
				Error::<T>::NoPermission
			);
			<TokenOwner<T>>::insert((collection_id, series_id), serial_number, new_owner);
		}

		Ok(())
	}
	/// Find the tokens owned by an `address` in the given collection
	pub fn collected_tokens(collection_id: &CollectionId, address: &T::AccountId) -> Vec<SerialNumber> {
		let mut owned_tokens = Vec::<SerialNumber>::default();
		let next_series_id = Self::next_series_id(collection_id);

		// Search each series up until the last known series Id
		for series_id in 0..next_series_id {
			let mut set_tokens: Vec<SerialNumber> = <TokenOwner<T>>::iter_prefix((collection_id, series_id))
				.filter_map(|(serial_number, owner)| if &owner == address { Some(serial_number) } else { None })
				.collect();

			owned_tokens.append(&mut set_tokens);
		}

		return owned_tokens;
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
					TokenLocks::remove(listing.token_id);

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
							TokenLocks::remove(listing.token_id);

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
						TokenLocks::remove(listing.token_id);

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
		let (collection_id, series_id, serial_number) = &listing.token_id;
		// if there are no custom royalties, fallback to default if it exists
		let royalties_schedule = if let Some(royalties_schedule) = Self::series_royalties(collection_id, series_id) {
			royalties_schedule
		} else {
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

		TokenLocks::remove(&listing.token_id);

		Self::do_transfer(&listing.seller, collection_id, *series_id, &[*serial_number], winner)?;

		Ok(())
	}
}
