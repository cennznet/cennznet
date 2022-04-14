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
#![recursion_limit = "256"]
//! # NFT Module
//!
//! Provides the basic creation and management of dynamic NFTs (created at runtime).
//!
//! Intended to be used "as is" by dapps and provide basic NFT feature set for smart contracts
//! to extend.
//!
//! *Collections*:
//!  A name spacing construct for grouping related series
//!  Series within a collection can share the same royalties fee schedules and owner address
//!
//! *Series*:
//! Series are a grouping of tokens- equivalent to an ERC721 contract
//!
//! *Tokens*:
//!  Individual tokens within a series. Globally identifiable by a tuple of (collection, series, serial number)
//!

use cennznet_primitives::types::{AssetId, Balance, CollectionId, SerialNumber, SeriesId, TokenId};
use crml_support::{IsTokenOwner, MultiCurrency, OnTransferSubscriber};
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage,
	pallet_prelude::*,
	storage::IterableStorageDoubleMap,
	traits::{ExistenceRequirement, Imbalance, SameOrOther, WithdrawReasons},
	transactional,
};
use frame_system::pallet_prelude::*;
use sp_runtime::{
	traits::{One, Saturating, Zero},
	DispatchResult, PerThing, Permill,
};
use sp_std::{collections::btree_map::BTreeMap, prelude::*};

mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
mod weights;
use weights::WeightInfo;

mod migration;
mod types;
pub use types::*;

// Interface for determining ownership of an NFT from some account
impl<T: Config> IsTokenOwner for Module<T> {
	type AccountId = T::AccountId;

	fn check_ownership(account: &Self::AccountId, token_id: &TokenId) -> bool {
		&Self::token_owner((token_id.0, token_id.1), token_id.2) == account
	}
}

pub trait Config: frame_system::Config {
	/// The system event type
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
	/// Default auction / sale length in blocks
	type DefaultListingDuration: Get<Self::BlockNumber>;
	/// Maximum byte length of an NFT attribute
	type MaxAttributeLength: Get<u8>;
	/// Handles a multi-currency fungible asset system
	type MultiCurrency: MultiCurrency<AccountId = Self::AccountId, CurrencyId = AssetId, Balance = Balance>;
	/// Provides the public call to weight mapping
	type WeightInfo: WeightInfo;
	/// Handler for when an NFT has been transferred
	type OnTransferSubscription: OnTransferSubscriber;
}

decl_event!(
	pub enum Event<T> where
		CollectionId = CollectionId,
		<T as frame_system::Config>::AccountId,
		AssetId = AssetId,
		Balance = Balance,
		Reason = AuctionClosureReason,
		SeriesId = SeriesId,
		SerialNumber = SerialNumber,
		TokenCount = TokenCount,
		CollectionNameType = CollectionNameType,
		Permill = Permill,
		MarketplaceId = MarketplaceId,
		OfferId = OfferId,
	{
		/// A new token collection was created (collection, name, owner)
		CreateCollection(CollectionId, CollectionNameType, AccountId),
		/// A new series of tokens was created (collection, series id, quantity, owner)
		CreateSeries(CollectionId, SeriesId, TokenCount, AccountId),
		/// Token(s) were created (collection, series id, quantity, owner)
		CreateTokens(CollectionId, SeriesId, TokenCount, AccountId),
		/// Token(s) were transferred (previous owner, token Ids, new owner)
		Transfer(AccountId, CollectionId, SeriesId, Vec<SerialNumber>, AccountId),
		/// Tokens were burned (collection, series id, serial numbers)
		Burn(CollectionId, SeriesId, Vec<SerialNumber>),
		/// A fixed price sale has been listed (collection, listing, marketplace_id)
		FixedPriceSaleListed(CollectionId, ListingId, Option<MarketplaceId>),
		/// A fixed price sale has completed (collection, listing, buyer))
		FixedPriceSaleComplete(CollectionId, ListingId, AccountId),
		/// A fixed price sale has closed without selling (collection, listing)
		FixedPriceSaleClosed(CollectionId, ListingId),
		///A fixed price sale has had its price updated (collection, listing)
		FixedPriceSalePriceUpdated(CollectionId, ListingId),
		/// An auction has opened (collection, listing, marketplace_id)
		AuctionOpen(CollectionId, ListingId, Option<MarketplaceId>),
		/// An auction has sold (collection, listing, payment asset, bid, new owner)
		AuctionSold(CollectionId, ListingId, AssetId, Balance, AccountId),
		/// An auction has closed without selling (collection, listing, reason)
		AuctionClosed(CollectionId, ListingId, Reason),
		/// A new highest bid was placed (collection, listing, amount)
		Bid(CollectionId, ListingId, Balance),
		/// An account has been registered as a marketplace (account, entitlement, marketplace_id)
		RegisteredMarketplace(AccountId, Permill, MarketplaceId),
		/// An offer has been made on an NFT (offer_id, amount, asset_id, marketplace_id, buyer)
		OfferMade(OfferId, Balance, AssetId, Option<MarketplaceId>, AccountId),
		/// An offer has been accepted (offer_id)
		OfferAccepted(OfferId),
		/// An offer has been rejected (offer_id)
		OfferRejected(OfferId),
		/// An offer has been cancelled (offer_id)
		OfferCancelled(OfferId),
	}
);

decl_error! {
	/// Error for the staking module.
	pub enum Error for Module<T: Config> {
		/// A collection with the same ID already exists
		CollectionIdExists,
		/// Given collection name is invalid (invalid utf-8, too long, empty)
		CollectionNameInvalid,
		/// No more Ids are available, they've been exhausted
		NoAvailableIds,
		/// Too many attributes in the provided schema or data
		SchemaMaxAttributes,
		/// Given attribute value is larger than the configured max.
		MaxAttributeLength,
		/// origin does not have permission for the operation (the token may not exist)
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
		/// Total royalties would exceed 100% of sale or an empty vec is supplied
		RoyaltiesInvalid,
		/// Auction bid was lower than reserve or current highest bid
		BidTooLow,
		/// Selling tokens from different collections is not allowed
		MixedBundleSale,
		/// Tokens with different individual royalties cannot be sold together
		RoyaltiesProtection,
		/// The account_id hasn't been registered as a marketplace
		MarketplaceNotRegistered,
		/// The series does not exist
		NoSeries,
		/// The Series name has been set
		NameAlreadySet,
		/// The metadata path is invalid (non-utf8 or empty)
		InvalidMetadataPath,
		/// No offer exists for the given OfferId
		InvalidOffer,
		/// The caller is not the buyer
		NotBuyer,
		/// The caller owns the token and can't make an offer
		IsTokenOwner,
		/// Offer amount needs to be greater than 0
		ZeroOffer,
		/// Cannot make an offer on a token up for auction
		TokenOnAuction,
	}
}

decl_storage! {
	trait Store for Module<T: Config> as Nft {
		/// Map from collection to owner address
		pub CollectionOwner get(fn collection_owner): map hasher(twox_64_concat) CollectionId => Option<T::AccountId>;
		/// Map from collection to its human friendly name
		pub CollectionName get(fn collection_name): map hasher(twox_64_concat) CollectionId => CollectionNameType;
		/// Map from collection to its defacto royalty scheme
		pub CollectionRoyalties get(fn collection_royalties): map hasher(twox_64_concat) CollectionId => Option<RoyaltiesSchedule<T::AccountId>>;
		/// Map from a token to lock status if any
		pub TokenLocks get(fn token_locks): map hasher(twox_64_concat) TokenId => Option<TokenLockReason>;
		/// Map from a token to its owner
		/// The token Id is split in this map to allow better indexing (collection, series) + (serial number)
		pub TokenOwner get(fn token_owner): double_map hasher(twox_64_concat) (CollectionId, SeriesId), hasher(twox_64_concat) SerialNumber => T::AccountId;
		/// Count of tokens owned by an address, supports ERC721 `balanceOf`
		pub TokenBalance get(fn token_balance): map hasher(blake2_128_concat) T::AccountId => BTreeMap<(CollectionId, SeriesId), TokenCount>;
		/// The next available marketplace id
		pub NextMarketplaceId get(fn next_marketplace_id): MarketplaceId;
		/// Map from marketplace account_id to royalties schedule
		pub RegisteredMarketplaces get(fn registered_marketplaces): map hasher(twox_64_concat) MarketplaceId => Marketplace<T::AccountId>;
		/// Map from (collection, series) to its attributes (deprecated)
		pub SeriesAttributes get(fn series_attributes): double_map hasher(twox_64_concat) CollectionId, hasher(twox_64_concat) SeriesId => Vec<NFTAttributeValue>;
		/// Map from series to its human friendly name
		pub SeriesName get(fn series_name): map hasher(twox_64_concat) (CollectionId, SeriesId) => CollectionNameType;
		/// Map from (collection, series) to configured royalties schedule
		pub SeriesRoyalties get(fn series_royalties): double_map hasher(twox_64_concat) CollectionId, hasher(twox_64_concat) SeriesId => Option<RoyaltiesSchedule<T::AccountId>>;
		/// Map from a (collection, series) to its total issuance
		pub SeriesIssuance get(fn series_issuance): double_map hasher(twox_64_concat) CollectionId, hasher(twox_64_concat) SeriesId =>  TokenCount;
		/// Map from a token series to its metadata reference scheme
		pub SeriesMetadataScheme get(fn series_metadata_scheme): double_map hasher(twox_64_concat) CollectionId, hasher(twox_64_concat) SeriesId => Option<MetadataScheme>;
		/// DEPRECATED: Migrate to seriesMetadataScheme. Read-only for NFTs created in v46
		pub SeriesMetadataURI get(fn series_metadata_uri): double_map hasher(twox_64_concat) CollectionId, hasher(twox_64_concat) SeriesId => Option<Vec<u8>>;
		/// The next available collection Id
		NextCollectionId get(fn next_collection_id): CollectionId;
		/// The next group Id within an NFT collection
		/// It is used as material to generate the global `TokenId`
		NextSeriesId get(fn next_series_id): map hasher(twox_64_concat) CollectionId => SeriesId;
		/// The next available serial number in a given (collection, series)
		NextSerialNumber get(fn next_serial_number): double_map hasher(twox_64_concat) CollectionId, hasher(twox_64_concat) SeriesId => SerialNumber;
		/// The next available listing Id
		pub NextListingId get(fn next_listing_id): ListingId;
		/// NFT sale/auction listings keyed by collection id and token id
		pub Listings get(fn listings): map hasher(twox_64_concat) ListingId => Option<Listing<T>>;
		/// Map from collection to any open listings
		pub OpenCollectionListings get(fn open_collection_listings): double_map hasher(twox_64_concat) CollectionId, hasher(twox_64_concat) ListingId => bool;
		/// Winning bids on open listings. keyed by collection id and token id
		pub ListingWinningBid get(fn listing_winning_bid): map hasher(twox_64_concat) ListingId => Option<(T::AccountId, Balance)>;
		/// Block numbers where listings will close. Value is `true` if at block number `listing_id` is scheduled to close.
		pub ListingEndSchedule get(fn listing_end_schedule): double_map hasher(twox_64_concat) T::BlockNumber, hasher(twox_64_concat) ListingId => bool;
		/// Map from offer_id to the information related to the offer
		pub Offers get(fn offers): map hasher(twox_64_concat) OfferId => Option<Offer<T::AccountId>>;
		/// Maps from token_id to a vector of offer_ids on that token
		pub TokenOffers get(fn token_offers): map hasher(twox_64_concat) TokenId => Vec<OfferId>;
		/// The next available offer_id
		pub NextOfferId get(fn next_offer_id): OfferId;
		/// Version of this module's storage schema
		StorageVersion build(|_: &GenesisConfig| Releases::V2 as u32): u32;
	}
}

/// The maximum number of attributes in an NFT collection schema
pub const MAX_SCHEMA_FIELDS: u32 = 16;
/// The maximum length of valid collection IDs
pub const MAX_COLLECTION_NAME_LENGTH: u8 = 32;
/// The maximum amount of listings to return
pub const MAX_COLLECTION_LISTING_LIMIT: u16 = 100;
/// The logging target for this module
pub(crate) const LOG_TARGET: &str = "nft";

// syntactic sugar for logging.
#[macro_export]
macro_rules! log {
	($level:tt, $patter:expr $(, $values:expr)* $(,)?) => {
		log::$level!(
			target: crate::LOG_TARGET,
			$patter $(, $values)*
		)
	};
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin, system = frame_system {
		type Error = Error<T>;

		fn deposit_event() = default;

		fn on_runtime_upgrade() -> Weight {
			use migration::v1_storage;
			use frame_support::IterableStorageMap;

			if StorageVersion::get() == Releases::V1 as u32 {
				StorageVersion::put(Releases::V2 as u32);
				v1_storage::CollectionMetadataURI::remove_all(None);
				v1_storage::IsSingleIssue::remove_all(None);

				let listings: Vec<(ListingId, v1_storage::Listing<T>)> = v1_storage::Listings::<T>::iter().collect();
				let weight = listings.len() as Weight;
				for (listing_id, listing) in listings {
					let listing_migrated = match listing {
						v1_storage::Listing::<T>::FixedPrice(v1_storage::FixedPriceListing {
							fixed_price,
							close,
							payment_asset,
							seller,
							buyer,
							tokens,
							royalties_schedule,
						}) => types::Listing::<T>::FixedPrice(types::FixedPriceListing {
							fixed_price,
							close,
							payment_asset,
							seller,
							buyer,
							tokens,
							royalties_schedule,
							marketplace_id: None,
						}),
						v1_storage::Listing::<T>::Auction(v1_storage::AuctionListing {
							reserve_price,
							close,
							payment_asset,
							seller,
							tokens,
							royalties_schedule,
						}) => types::Listing::<T>::Auction(types::AuctionListing {
							reserve_price,
							close,
							payment_asset,
							seller,
							tokens,
							royalties_schedule,
							marketplace_id: None,
						}),
					};
					Listings::insert(listing_id, listing_migrated);
				}

				log!(warn, "🃏 listings migrated");
				return 6_000_000 as Weight + weight * 100_000;
			} else {
				Zero::zero()
			}
		}

		/// Check and close all expired listings
		fn on_initialize(now: T::BlockNumber) -> Weight {
			// TODO: this is unbounded and could become costly
			// https://github.com/cennznet/cennznet/issues/444
			let removed_count = Self::close_listings_at(now);
			// 'buy' weight is comparable to successful closure of an auction
			T::WeightInfo::buy() * removed_count as Weight
		}

		/// Set the owner of a collection
		/// Caller must be the current collection owner
		#[weight = T::WeightInfo::set_owner()]
		fn migrate_to_metadata_scheme(origin, collection_id: CollectionId, series_id: SeriesId, scheme: MetadataScheme) -> DispatchResult {
			let origin = ensure_signed(origin)?;
			if let Some(owner) = Self::collection_owner(collection_id) {
				ensure!(owner == origin, Error::<T>::NoPermission);
				// anti-rug
				ensure!(SeriesMetadataScheme::get(collection_id, series_id).is_none(), Error::<T>::NoPermission);
				let sanitized_scheme = scheme.sanitize().map_err(|_| Error::<T>::InvalidMetadataPath)?;
				SeriesMetadataScheme::insert(collection_id, series_id, sanitized_scheme);
				Ok(())
			} else {
				Err(Error::<T>::NoCollection.into())
			}
		}

		/// Set the owner of a collection
		/// Caller must be the current collection owner
		#[weight = T::WeightInfo::set_owner()]
		fn set_owner(origin, collection_id: CollectionId, new_owner: T::AccountId) -> DispatchResult {
			let origin = ensure_signed(origin)?;
			if let Some(owner) = Self::collection_owner(collection_id) {
				ensure!(owner == origin, Error::<T>::NoPermission);
				<CollectionOwner<T>>::insert(collection_id, new_owner);
				Ok(())
			} else {
				Err(Error::<T>::NoCollection.into())
			}
		}

		/// Set the name of a series
		/// Caller must be the current collection owner
		#[weight = T::WeightInfo::set_owner()]
		fn set_series_name(origin, collection_id: CollectionId, series_id: SeriesId, name: CollectionNameType) -> DispatchResult {
			let origin = ensure_signed(origin)?;
			if let Some(owner) = Self::collection_owner(collection_id) {
				ensure!(owner == origin, Error::<T>::NoPermission);
				ensure!(SeriesMetadataScheme::contains_key(collection_id, series_id), Error::<T>::NoSeries);
				ensure!(!SeriesName::contains_key((collection_id, series_id)), Error::<T>::NameAlreadySet);
				ensure!(!name.is_empty() && name.len() <= MAX_COLLECTION_NAME_LENGTH as usize, Error::<T>::CollectionNameInvalid);
				ensure!(core::str::from_utf8(&name).is_ok(), Error::<T>::CollectionNameInvalid);

				SeriesName::insert((collection_id, series_id), name);
				Ok(())
			} else {
				Err(Error::<T>::NoCollection.into())
			}
		}

		/// Flag an account as a marketplace
		///
		/// `marketplace_account` - if specified, this account will be registered
		/// `entitlement` - Permill, percentage of sales to go to the marketplace
		/// If no marketplace is specified the caller will be registered
		#[weight = 16_000_000]
		fn register_marketplace(
			origin,
			marketplace_account: Option<T::AccountId>,
			entitlement: Permill
		) -> DispatchResult {
			let origin = ensure_signed(origin)?;
			ensure!(entitlement.deconstruct() as u32 <= Permill::ACCURACY, Error::<T>::RoyaltiesInvalid);
			let marketplace_account = marketplace_account.unwrap_or(origin);
			let marketplace_id = Self::next_marketplace_id();
			let marketplace = Marketplace {
				account: marketplace_account.clone(),
				entitlement
			};
			let next_marketplace_id = NextMarketplaceId::get();
			ensure!(next_marketplace_id.checked_add(One::one()).is_some(), Error::<T>::NoAvailableIds);
			<RegisteredMarketplaces<T>>::insert(&marketplace_id, marketplace);
			Self::deposit_event(RawEvent::RegisteredMarketplace(marketplace_account, entitlement, marketplace_id));
			NextMarketplaceId::mutate(|i| *i += 1);
			Ok(())
		}

		/// Create a new token collection
		///
		/// The caller will become the collection owner
		/// `collection_id`- 32 byte utf-8 string
		/// `metadata_base_uri` - Base URI for off-chain metadata for tokens in this collection
		/// `royalties_schedule` - defacto royalties plan for secondary sales, this will apply to all tokens in the collection by default.
		#[weight = T::WeightInfo::create_collection()]
		pub fn create_collection(
			origin,
			name: CollectionNameType,
			royalties_schedule: Option<RoyaltiesSchedule<T::AccountId>>,
		) -> DispatchResult {
			let origin = ensure_signed(origin)?;

			ensure!(!name.is_empty() && name.len() <= MAX_COLLECTION_NAME_LENGTH as usize, Error::<T>::CollectionNameInvalid);
			ensure!(core::str::from_utf8(&name).is_ok(), Error::<T>::CollectionNameInvalid);

			let collection_id = NextCollectionId::get();
			ensure!(collection_id.checked_add(One::one()).is_some(), Error::<T>::NoAvailableIds);

			// Create the collection, update ownership, and bookkeeping
			if let Some(royalties_schedule) = royalties_schedule {
				ensure!(royalties_schedule.validate(), Error::<T>::RoyaltiesInvalid);
				<CollectionRoyalties<T>>::insert(collection_id, royalties_schedule);
			}
			<CollectionOwner<T>>::insert(collection_id, &origin);
			CollectionName::insert(collection_id, &name);
			NextCollectionId::mutate(|c| *c += 1);

			Self::deposit_event(RawEvent::CreateCollection(collection_id, name, origin));

			Ok(())
		}

		/// Create a new series
		/// Additional tokens can be minted via `mint_additional`
		///
		/// `quantity` - number of tokens to mint now
		/// `owner` - the token owner, defaults to the caller
		/// `metadata_scheme` - The off-chain metadata referencing scheme for tokens in this series
		/// Caller must be the collection owner
		#[weight = T::WeightInfo::mint_series(*quantity)]
		#[transactional]
		pub fn mint_series(
			origin,
			collection_id: CollectionId,
			quantity: TokenCount,
			owner: Option<T::AccountId>,
			metadata_scheme: MetadataScheme,
			royalties_schedule: Option<RoyaltiesSchedule<T::AccountId>>,
		) -> DispatchResult {
			let origin = ensure_signed(origin)?;

			// Permission and existence check
			if let Some(collection_owner) = Self::collection_owner(collection_id) {
				ensure!(collection_owner == origin, Error::<T>::NoPermission);
			} else {
				return Err(Error::<T>::NoCollection.into());
			}

			// Check we can issue the new tokens
			let series_id = Self::next_series_id(collection_id);
			ensure!(
				series_id.checked_add(One::one()).is_some(),
				Error::<T>::NoAvailableIds
			);

			let metadata_scheme = metadata_scheme.sanitize().map_err(|_| Error::<T>::InvalidMetadataPath)?;
			SeriesMetadataScheme::insert(collection_id, series_id, metadata_scheme);

			// Setup royalties
			if let Some(royalties_schedule) = royalties_schedule {
				ensure!(royalties_schedule.validate(), Error::<T>::RoyaltiesInvalid);
				<SeriesRoyalties<T>>::insert(collection_id, series_id, royalties_schedule);
			}

			// Now mint the series tokens
			let owner = owner.unwrap_or(origin);
			Self::do_mint(&owner, collection_id, series_id, 0 as SerialNumber, quantity)?;

			// will not overflow, asserted prior qed.
			NextSeriesId::mutate(collection_id, |i| *i += SeriesId::one());

			Self::deposit_event(RawEvent::CreateSeries(collection_id, series_id, quantity, owner));

			Ok(())
		}

		/// Mint tokens for an existing series
		///
		/// `quantity` - how many tokens to mint
		/// `owner` - the token owner, defaults to the caller if unspecified
		/// Caller must be the collection owner
		/// -----------
		/// Weight is O(N) where N is `quantity`
		#[weight = T::WeightInfo::mint_additional(*quantity)]
		#[transactional]
		fn mint_additional(
			origin,
			collection_id: CollectionId,
			series_id: SeriesId,
			quantity: TokenCount,
			owner: Option<T::AccountId>,
		) -> DispatchResult {
			let origin = ensure_signed(origin)?;

			// Permission and existence check
			if let Some(collection_owner) = Self::collection_owner(collection_id) {
				ensure!(collection_owner == origin, Error::<T>::NoPermission);
			} else {
				return Err(Error::<T>::NoCollection.into());
			}

			let serial_number = Self::next_serial_number(collection_id, series_id);
			ensure!(serial_number > Zero::zero(), Error::<T>::NoToken);
			ensure!(
				serial_number.checked_add(quantity).is_some(),
				Error::<T>::NoAvailableIds
			);
			let owner = owner.unwrap_or(origin);

			Self::do_mint(&owner, collection_id, series_id, serial_number, quantity)?;
			Self::deposit_event(RawEvent::CreateTokens(collection_id, series_id, quantity, owner));

			Ok(())
		}

		/// Transfer ownership of an NFT
		/// Caller must be the token owner
		#[weight = T::WeightInfo::transfer()]
		fn transfer(origin, token_id: TokenId, new_owner: T::AccountId) -> DispatchResult {
			let (collection_id, series_id, serial_number) = token_id;
			Self::transfer_batch(origin, collection_id, series_id, vec![serial_number], new_owner)
		}

		/// Transfer ownership of a batch of NFTs (atomic)
		/// Tokens must be from the same series
		/// Caller must be the token owner
		#[weight = {
			T::WeightInfo::transfer().saturating_mul(serial_numbers.len() as Weight)
		}]
		#[transactional]
		fn transfer_batch(origin, collection_id: CollectionId, series_id: SeriesId, serial_numbers: Vec<SerialNumber>, new_owner: T::AccountId) {
			let origin = ensure_signed(origin)?;

			ensure!(serial_numbers.len() > Zero::zero(), Error::<T>::NoToken);
			for serial_number in serial_numbers.iter() {
				ensure!(!TokenLocks::contains_key((collection_id, series_id, serial_number)), Error::<T>::TokenListingProtection);
				ensure!(
					Self::token_owner((collection_id, series_id), serial_number) == origin,
					Error::<T>::NoPermission
				);
			}
			let _ = Self::do_transfer_unchecked(collection_id, series_id, &serial_numbers, &origin, &new_owner)?;

			Self::deposit_event(RawEvent::Transfer(origin, collection_id, series_id, serial_numbers, new_owner));
		}

		/// Burn a token 🔥
		///
		/// Caller must be the token owner
		#[weight = T::WeightInfo::burn()]
		fn burn(origin, token_id: TokenId) -> DispatchResult {
			let (collection_id, series_id, serial_number) = token_id;
			Self::burn_batch(origin, collection_id, series_id, vec![serial_number])
		}

		/// Burn some tokens 🔥
		/// Tokens must be from the same series
		///
		/// Caller must be the token owner
		/// Fails on duplicate serials
		#[weight = {
			T::WeightInfo::burn()
				.saturating_add(
					T::DbWeight::get().reads_writes(2, 1).saturating_mul(serial_numbers.len() as Weight)
				)
		}]
		#[transactional]
		fn burn_batch(origin, collection_id: CollectionId, series_id: SeriesId, serial_numbers: Vec<SerialNumber>) {
			let origin = ensure_signed(origin)?;

			ensure!(!serial_numbers.is_empty(), Error::<T>::NoToken);

			for serial_number in serial_numbers.iter() {
				ensure!(!TokenLocks::contains_key((collection_id, series_id, serial_number)), Error::<T>::TokenListingProtection);
				ensure!(Self::token_owner((collection_id, series_id), serial_number) == origin, Error::<T>::NoPermission);
				<TokenOwner<T>>::remove((collection_id, series_id), serial_number);
			}

			let quantity = serial_numbers.len() as TokenCount;
			let _ = <TokenBalance<T>>::try_mutate::<_, (), Error<T>, _>(&origin, |balances| {
				match (*balances).get_mut(&(collection_id, series_id)) {
					Some(balance) => {
						let new_balance = balance.saturating_sub(quantity);
						if new_balance.is_zero() {
							balances.remove(&(collection_id, series_id));
						} else {
							*balance = new_balance;
						}
						Ok(())
					},
					None => Err(Error::<T>::NoToken.into()),
				}
			})?;

			if Self::series_issuance(collection_id, series_id).saturating_sub(serial_numbers.len() as TokenCount).is_zero() {
				// this is the last of the tokens
				SeriesAttributes::remove(collection_id, series_id);
				SeriesIssuance::remove(collection_id, series_id);
				SeriesMetadataScheme::remove(collection_id, series_id);
				<SeriesRoyalties<T>>::remove(collection_id, series_id);
			} else {
				SeriesIssuance::mutate(collection_id, series_id, |q| *q = q.saturating_sub(quantity));
			}

			Self::deposit_event(RawEvent::Burn(collection_id, series_id, serial_numbers));
		}

		/// Sell a single token at a fixed price
		///
		/// `buyer` optionally, the account to receive the NFT. If unspecified, then any account may purchase
		/// `asset_id` fungible asset Id to receive as payment for the NFT
		/// `fixed_price` ask price
		/// `duration` listing duration time in blocks from now
		/// `marketplace` optionally, the marketplace that the NFT is being sold on
		/// Caller must be the token owner
		#[weight = T::WeightInfo::sell()]
		#[transactional]
		fn sell(
			origin,
			token_id: TokenId,
			buyer: Option<T::AccountId>,
			payment_asset: AssetId,
			fixed_price: Balance,
			duration: Option<T::BlockNumber>,
			marketplace_id: Option<MarketplaceId>,
		) {
			Self::sell_bundle(
				origin,
				vec![token_id],
				buyer,
				payment_asset,
				fixed_price,
				duration,
				marketplace_id,
			)?;
		}

		/// Sell a bundle of tokens at a fixed price
		/// - Tokens must be from the same series
		/// - Tokens with individual royalties schedules cannot be sold with this method
		///
		/// `buyer` optionally, the account to receive the NFT. If unspecified, then any account may purchase
		/// `asset_id` fungible asset Id to receive as payment for the NFT
		/// `fixed_price` ask price
		/// `duration` listing duration time in blocks from now
		/// Caller must be the token owner
		#[weight = {
			T::WeightInfo::sell()
				.saturating_add(
					T::DbWeight::get().reads_writes(2, 1).saturating_mul(tokens.len() as Weight)
				)
		}]
		#[transactional]
		fn sell_bundle(
			origin,
			tokens: Vec<TokenId>,
			buyer: Option<T::AccountId>,
			payment_asset: AssetId,
			fixed_price: Balance,
			duration: Option<T::BlockNumber>,
			marketplace_id: Option<MarketplaceId>
		) {
			let origin = ensure_signed(origin)?;

			if tokens.is_empty() {
				return Err(Error::<T>::NoToken.into());
			}

			let royalties_schedule = Self::check_bundle_royalties(&tokens, marketplace_id)?;

			let listing_id = Self::next_listing_id();
			ensure!(listing_id.checked_add(One::one()).is_some(), Error::<T>::NoAvailableIds);

			// use the first token's collection as representative of the bundle
			let (bundle_collection_id, _series_id, _serial_number) = tokens[0];
			for (collection_id, series_id, serial_number) in tokens.iter() {
				ensure!(!TokenLocks::contains_key((collection_id, series_id, serial_number)), Error::<T>::TokenListingProtection);
				ensure!(Self::token_owner((collection_id, series_id), serial_number) == origin, Error::<T>::NoPermission);
				TokenLocks::insert((collection_id, series_id, serial_number), TokenLockReason::Listed(listing_id));
			}

			let listing_end_block = <frame_system::Pallet<T>>::block_number().saturating_add(duration.unwrap_or_else(T::DefaultListingDuration::get));
			ListingEndSchedule::<T>::insert(listing_end_block, listing_id, true);
			let listing = Listing::<T>::FixedPrice(
				FixedPriceListing::<T> {
					payment_asset,
					fixed_price,
					close: listing_end_block,
					tokens: tokens.clone(),
					buyer: buyer.clone(),
					seller: origin.clone(),
					royalties_schedule,
					marketplace_id,
				}
			);

			OpenCollectionListings::insert(bundle_collection_id, listing_id, true);
			Listings::insert(listing_id, listing);
			NextListingId::mutate(|i| *i += 1);

			Self::deposit_event(RawEvent::FixedPriceSaleListed(bundle_collection_id, listing_id, marketplace_id));
		}

		/// Buy a token listing for its specified price
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

				let (collection_id, series_id, _serial_number) = listing.tokens.get(0).ok_or_else(|| Error::<T>::NoToken)?;

				let royalty_fees = listing.royalties_schedule.calculate_total_entitlement();
				if royalty_fees.is_zero() {
					// full proceeds to seller/`current_owner`
					T::MultiCurrency::transfer(&origin, &listing.seller, listing.payment_asset, listing.fixed_price, ExistenceRequirement::AllowDeath)?;
				} else {
					// withdraw funds from buyer, split between royalty payments and seller
					let mut for_seller = listing.fixed_price;
					let mut imbalance = T::MultiCurrency::withdraw(&origin, listing.payment_asset, listing.fixed_price, WithdrawReasons::TRANSFER, ExistenceRequirement::AllowDeath)?;
					for (who, entitlement) in listing.royalties_schedule.entitlements.into_iter() {
						let royalty = entitlement * listing.fixed_price;
						for_seller -= royalty;
						imbalance = match imbalance.offset(T::MultiCurrency::deposit_into_existing(&who, listing.payment_asset, royalty)?) {
							SameOrOther::Same(value) => value,
							SameOrOther::Other(_) | SameOrOther::None => return Err(Error::<T>::InternalPayment.into()),
						}
					}
					match imbalance.offset(T::MultiCurrency::deposit_into_existing(&listing.seller, listing.payment_asset, for_seller)?) {
						SameOrOther::Same(_) => (),
						SameOrOther::Other(_) | SameOrOther::None => return Err(Error::<T>::InternalPayment.into()),
					}
				}

				let serial_numbers: Vec<SerialNumber> = listing.tokens.iter().map(|token_id| {
					TokenLocks::remove(token_id);
					token_id.2
				}).collect();
				OpenCollectionListings::remove(collection_id, listing_id);

				let _ = Self::do_transfer_unchecked(*collection_id, *series_id, &serial_numbers, &listing.seller, &origin)?;
				Self::remove_fixed_price_listing(listing_id);

				Self::deposit_event(RawEvent::FixedPriceSaleComplete(*collection_id, listing_id, origin));
			} else {
				return Err(Error::<T>::NotForFixedPriceSale.into());
			}
		}

		/// Auction a token on the open market to the highest bidder
		///
		/// Caller must be the token owner
		/// - `payment_asset` fungible asset Id to receive payment with
		/// - `reserve_price` winning bid must be over this threshold
		/// - `duration` length of the auction (in blocks), uses default duration if unspecified
		#[weight = T::WeightInfo::sell()]
		fn auction(
			origin,
			token_id: TokenId,
			payment_asset: AssetId,
			reserve_price: Balance,
			duration: Option<T::BlockNumber>,
			marketplace_id: Option<MarketplaceId>
		) -> DispatchResult {
			Self::auction_bundle(
				origin,
				vec![token_id],
				payment_asset,
				reserve_price,
				duration,
				marketplace_id
			)
		}

		/// Auction a bundle of tokens on the open market to the highest bidder
		/// - Tokens must be from the same series
		/// - Tokens with individual royalties schedules cannot be sold in bundles
		///
		/// Caller must be the token owner
		/// - `payment_asset` fungible asset Id to receive payment with
		/// - `reserve_price` winning bid must be over this threshold
		/// - `duration` length of the auction (in blocks), uses default duration if unspecified
		#[weight = {
			T::WeightInfo::sell()
				.saturating_add(
					T::DbWeight::get().reads_writes(2, 1).saturating_mul(tokens.len() as Weight)
				)
		}]
		#[transactional]
		fn auction_bundle(
			origin,
			tokens: Vec<TokenId>,
			payment_asset: AssetId,
			reserve_price: Balance,
			duration: Option<T::BlockNumber>,
			marketplace_id: Option<MarketplaceId>
		) {
			let origin = ensure_signed(origin)?;

			if tokens.is_empty() {
				return Err(Error::<T>::NoToken.into());
			}

			let royalties_schedule = Self::check_bundle_royalties(&tokens, marketplace_id)?;

			let listing_id = Self::next_listing_id();
			ensure!(listing_id.checked_add(One::one()).is_some(), Error::<T>::NoAvailableIds);

			// use the first token's collection as representative of the bundle
			let (bundle_collection_id, _series_id, _serial_number) = tokens[0];
			for (collection_id, series_id, serial_number) in tokens.iter() {
				ensure!(!TokenLocks::contains_key((collection_id, series_id, serial_number)), Error::<T>::TokenListingProtection);
				ensure!(Self::token_owner((collection_id, series_id), serial_number) == origin, Error::<T>::NoPermission);
				TokenLocks::insert((collection_id, series_id, serial_number), TokenLockReason::Listed(listing_id));
			}

			let listing_end_block =<frame_system::Pallet<T>>::block_number().saturating_add(duration.unwrap_or_else(T::DefaultListingDuration::get));
			ListingEndSchedule::<T>::insert(listing_end_block, listing_id, true);
			let listing = Listing::<T>::Auction(
				AuctionListing::<T> {
					payment_asset,
					reserve_price,
					close: listing_end_block,
					tokens: tokens.clone(),
					seller: origin.clone(),
					royalties_schedule,
					marketplace_id,
				}
			);

			OpenCollectionListings::insert(bundle_collection_id, listing_id, true);
			Listings::insert(listing_id, listing);
			NextListingId::mutate(|i| *i += 1);

			Self::deposit_event(RawEvent::AuctionOpen(bundle_collection_id, listing_id, marketplace_id));
		}

		/// Place a bid on an open auction
		/// - `amount` to bid (in the seller's requested payment asset)
		#[weight = T::WeightInfo::bid()]
		#[transactional]
		fn bid(origin, listing_id: ListingId, amount: Balance) {
			let origin = ensure_signed(origin)?;

			if let Some(Listing::Auction(mut listing)) = Self::listings(listing_id) {
				if let Some(current_bid) = Self::listing_winning_bid(listing_id) {
					ensure!(amount > current_bid.1, Error::<T>::BidTooLow);
				} else {
					// first bid
					ensure!(amount >= listing.reserve_price, Error::<T>::BidTooLow);
				}

				// check user has the requisite funds to make this bid
				let balance = T::MultiCurrency::free_balance(&origin, listing.payment_asset);
				if let Some(balance_after_bid) = balance.checked_sub(amount) {
					// TODO: review behaviour with 3.0 upgrade: https://github.com/cennznet/cennznet/issues/414
					// - `amount` is unused
					// - if there are multiple locks on user asset this could return true inaccurately
					// - `T::MultiCurrency::reserve(origin, asset_id, amount)` should be checking this internally...
					let _ = T::MultiCurrency::ensure_can_withdraw(&origin, listing.payment_asset, amount, WithdrawReasons::RESERVE, balance_after_bid)?;
				}

				// try lock funds
				T::MultiCurrency::reserve(&origin, listing.payment_asset, amount)?;

				ListingWinningBid::<T>::mutate(listing_id, |maybe_current_bid| {
					if let Some(current_bid) = maybe_current_bid {
						// replace old bid
						T::MultiCurrency::unreserve(&current_bid.0, listing.payment_asset, current_bid.1);
					}
					*maybe_current_bid = Some((origin, amount))
				});

				// Auto extend auction if bid is made within certain amount of time of auction duration
				let listing_end_block = listing.close;
				let current_block = <frame_system::Pallet<T>>::block_number();
				let blocks_till_close = listing_end_block - current_block;
				let new_closing_block = current_block + T::BlockNumber::from(AUCTION_EXTENSION_PERIOD);
				if blocks_till_close <= T::BlockNumber::from(AUCTION_EXTENSION_PERIOD) {
					ListingEndSchedule::<T>::remove(listing_end_block, listing_id);
					ListingEndSchedule::<T>::insert(new_closing_block, listing_id, true);
					listing.close = new_closing_block;
					Listings::<T>::insert(listing_id, Listing::Auction(listing.clone()));
				}

				let listing_collection_id = listing.tokens[0].0;
				Self::deposit_event(RawEvent::Bid(listing_collection_id, listing_id, amount));
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
					for token_id in sale.tokens.iter() {
						TokenLocks::remove(token_id);
					}
					let collection_id = sale.tokens[0].0;
					OpenCollectionListings::remove(collection_id, listing_id);

					Self::deposit_event(RawEvent::FixedPriceSaleClosed(collection_id, listing_id));
				},
				Some(Listing::<T>::Auction(auction)) => {
					ensure!(auction.seller == origin, Error::<T>::NoPermission);
					ensure!(Self::listing_winning_bid(listing_id).is_none(), Error::<T>::TokenListingProtection);
					Listings::<T>::remove(listing_id);
					ListingEndSchedule::<T>::remove(auction.close, listing_id);
					for token_id in auction.tokens.iter() {
						TokenLocks::remove(token_id);
					}
					let collection_id = auction.tokens[0].0;
					OpenCollectionListings::remove(collection_id, listing_id);

					Self::deposit_event(RawEvent::AuctionClosed(collection_id, listing_id, AuctionClosureReason::VendorCancelled));
				},
				None => {},
			}
		}

		/// Update fixed price for a single token sale
		///
		/// `listing_id` id of the fixed price listing
		/// `new_price` new fixed price
		/// Caller must be the token owner
		#[weight = T::WeightInfo::update_fixed_price()]
		fn update_fixed_price (
			origin,
			listing_id: ListingId,
			new_price: Balance
		) -> DispatchResult {
			let origin = ensure_signed(origin)?;

			match Self::listings(listing_id) {
				Some(Listing::<T>::FixedPrice(mut sale)) => {
					ensure!(sale.seller == origin, Error::<T>::NoPermission);

					sale.fixed_price = new_price;
					let collection_id = sale.tokens[0].0;

					Listings::insert(listing_id, Listing::<T>::FixedPrice(sale));
					Self::deposit_event(RawEvent::FixedPriceSalePriceUpdated(collection_id, listing_id));
					Ok(())
				},
				Some(Listing::<T>::Auction(_)) => Err(Error::<T>::NotForFixedPriceSale.into()),
				None => Err(Error::<T>::NotForFixedPriceSale.into()),
			}
		}

		/// Create an offer on a token
		/// Locks funds until offer is accepted, rejected or cancelled
		#[weight = 0]
		fn make_offer (
			origin,
			token_id: TokenId,
			amount: Balance,
			asset_id: AssetId,
			marketplace_id: Option<MarketplaceId>,
		) -> DispatchResult {
			let origin = ensure_signed(origin)?;
			ensure!(!amount.is_zero(), Error::<T>::ZeroOffer);
			ensure!(Self::token_owner((token_id.0, token_id.1), token_id.2) != origin, Error::<T>::IsTokenOwner);
			let offer_id = Self::next_offer_id();
			ensure!(offer_id.checked_add(One::one()).is_some(), Error::<T>::NoAvailableIds);

			// ensure the token_id is not currently in an auction
			if let Some(TokenLockReason::Listed(listing_id)) = Self::token_locks(token_id) {
				match Self::listings(listing_id) {
					Some(Listing::<T>::Auction(_)) => return Err(Error::<T>::TokenOnAuction.into()),
					None | Some(Listing::<T>::FixedPrice(_)) => (),
				}
			}
			// check user has the requisite funds to make this offer
			let balance = T::MultiCurrency::free_balance(&origin, asset_id);
			if let Some(balance_after_bid) = balance.checked_sub(amount) {
				// TODO: review behaviour with 3.0 upgrade: https://github.com/cennznet/cennznet/issues/414
				// - `amount` is unused
				// - if there are multiple locks on user asset this could return true inaccurately
				// - `T::MultiCurrency::reserve(origin, asset_id, amount)` should be checking this internally...
				let _ = T::MultiCurrency::ensure_can_withdraw(&origin, asset_id, amount, WithdrawReasons::RESERVE, balance_after_bid)?;
			}

			// try lock funds
			T::MultiCurrency::reserve(&origin, asset_id, amount)?;

			let mut token_offers = Self::token_offers(token_id);
			token_offers.push(offer_id);
			TokenOffers::insert(token_id, token_offers);

			let new_offer = Offer {
				token_id,
				asset_id,
				amount,
				buyer: origin.clone(),
				marketplace_id,
			};
			Offers::<T>::insert(offer_id, new_offer);
			NextOfferId::mutate(|i| *i += 1);

			Self::deposit_event(RawEvent::OfferMade(offer_id, amount, asset_id, marketplace_id, origin));
			Ok(())
		}

		/// Cancels an offer on a token
		/// Caller must be the offer buyer
		#[weight = 0]
		fn cancel_offer (
			origin,
			offer_id: OfferId,
		) -> DispatchResult {
			let origin = ensure_signed(origin)?;
			if let Some(offer) = Self::offers(offer_id) {
				ensure!(offer.buyer == origin, Error::<T>::NotBuyer);
				T::MultiCurrency::unreserve(&origin, offer.asset_id, offer.amount);
				Offers::<T>::remove(offer_id);
				TokenOffers::remove(offer.token_id);
				Self::deposit_event(RawEvent::OfferCancelled(offer_id));
				Ok(())
			} else {
				Err(Error::<T>::InvalidOffer.into())
			}
		}

		/// Accepts an offer on a token
		/// Caller must be token owner
		#[weight = 0]
		fn accept_offer (
			origin,
			offer_id: OfferId,
		) -> DispatchResult {
			let origin = ensure_signed(origin)?;
			if let Some(offer) = Self::offers(offer_id) {
				let token_id = offer.token_id;
				ensure!(Self::token_owner((token_id.0, token_id.1), token_id.2) == origin, Error::<T>::NoPermission);

				let royalties_schedule = Self::check_bundle_royalties(&vec![token_id], offer.marketplace_id)?;
				let for_royalties = royalties_schedule.calculate_total_entitlement() * offer.amount;
				let mut for_seller = offer.amount;

				// do royalty payments
				if !for_royalties.is_zero() {
					let entitlements = royalties_schedule.entitlements.clone();
					for (who, entitlement) in entitlements.into_iter() {
						let royalty = entitlement * offer.amount;
						let _ = T::MultiCurrency::repatriate_reserved(&offer.buyer, offer.asset_id, &who, royalty)?;
						for_seller -= royalty;
					}
				}

				let seller_balance = T::MultiCurrency::free_balance(&origin, offer.asset_id);
				let _ = T::MultiCurrency::repatriate_reserved(&offer.buyer, offer.asset_id, &origin, for_seller)?;

				// The implementation of `repatriate_reserved` may take less than the required amount and succeed
				// this should not happen but could for reasons outside the control of this module
				ensure!(
					T::MultiCurrency::free_balance(&origin, offer.asset_id)
						>= seller_balance.saturating_add(for_seller),
					Error::<T>::InternalPayment
				);
				Self::do_transfer_unchecked(token_id.0, token_id.1, &vec![token_id.2], &origin, &offer.buyer)?;

				// Clean storage
				Offers::<T>::remove(offer_id);
				let mut token_offers = Self::token_offers(token_id);
				token_offers.retain(|&x| x != offer_id);
				TokenOffers::insert(token_id, token_offers);

				Self::deposit_event(RawEvent::OfferAccepted(offer_id));
				Ok(())
			} else {
				Err(Error::<T>::InvalidOffer.into())
			}
		}

		/// Rejects an offer on a token
		/// Caller must be token owner
		#[weight = 0]
		fn reject_offer (
			origin,
			offer_id: OfferId,
		) -> DispatchResult {
			let origin = ensure_signed(origin)?;
			if let Some(offer) = Self::offers(offer_id) {
				let token_id = offer.token_id;
				ensure!(Self::token_owner((token_id.0, token_id.1), token_id.2) == origin, Error::<T>::NoPermission);
				T::MultiCurrency::unreserve(&offer.buyer, offer.asset_id, offer.amount);
				Offers::<T>::remove(offer_id);
				TokenOffers::remove(offer.token_id);
				Self::deposit_event(RawEvent::OfferRejected(offer_id));
				Ok(())
			} else {
				Err(Error::<T>::InvalidOffer.into())
			}
		}
	}
}

impl<T: Config> Module<T> {
	/// Return whether the series exists or not
	pub fn series_exists(collection_id: CollectionId, series_id: SeriesId) -> bool {
		SeriesMetadataScheme::contains_key(collection_id, series_id)
	}
	/// Construct & return the full metadata URI for a given `token_id` (analogous to ERC721 metadata token_uri)
	pub fn token_uri(token_id: TokenId) -> Vec<u8> {
		use core::fmt::Write;
		if let Some(scheme) = Self::series_metadata_scheme(token_id.0, token_id.1) {
			let mut token_uri = sp_std::Writer::default();
			match scheme {
				MetadataScheme::Http(path) => {
					let path = core::str::from_utf8(&path).unwrap_or("");
					write!(&mut token_uri, "http://{}/{}.json", path, token_id.2).expect("Not written");
				}
				MetadataScheme::Https(path) => {
					let path = core::str::from_utf8(&path).unwrap_or("");
					write!(&mut token_uri, "https://{}/{}.json", path, token_id.2).expect("Not written");
				}
				MetadataScheme::IpfsDir(dir_cid) => {
					write!(
						&mut token_uri,
						"ipfs://{}/{}.json",
						core::str::from_utf8(&dir_cid).unwrap_or(""),
						token_id.2
					)
					.expect("Not written");
				}
				MetadataScheme::IpfsShared(shared_cid) => {
					write!(
						&mut token_uri,
						"ipfs://{}.json",
						core::str::from_utf8(&shared_cid).unwrap_or("")
					)
					.expect("Not written");
				}
			}
			token_uri.inner().clone()
		} else {
			// should not happen
			log!(warn, "🃏 Unexpected empty metadata scheme: {:?}", token_id);
			Default::default()
		}
	}
	/// Check royalties will be respected on all tokens if placed into a bundle sale.
	/// We're ok iff, all tokens in the bundle are from the:
	/// 1) same collection and same series
	/// 2) same collection and different series, no series royalties set (could extend to iff royalties equal)
	/// Although possible, we do not support:
	/// 3) different collections, no royalties allowed
	fn check_bundle_royalties(
		tokens: &[TokenId],
		marketplace_id: Option<MarketplaceId>,
	) -> Result<RoyaltiesSchedule<T::AccountId>, Error<T>> {
		// use the first token's collection as representative of the bundle
		let (bundle_collection_id, bundle_series_id, _serial_number) = tokens[0];

		for (collection_id, series_id, _serial_number) in tokens.iter() {
			ensure!(*collection_id == bundle_collection_id, Error::<T>::MixedBundleSale);
			if *series_id != bundle_series_id {
				ensure!(
					!<SeriesRoyalties<T>>::contains_key(collection_id, series_id),
					Error::<T>::RoyaltiesProtection
				);
			}
		}
		// series schedule takes priority if it exists
		let mut royalties = Self::series_royalties(bundle_collection_id, bundle_series_id)
			.unwrap_or_else(|| Self::collection_royalties(bundle_collection_id).unwrap_or_else(Default::default));
		let royalties = match marketplace_id {
			Some(marketplace_id) => {
				ensure!(
					<RegisteredMarketplaces<T>>::contains_key(marketplace_id),
					Error::<T>::MarketplaceNotRegistered
				);
				let marketplace = Self::registered_marketplaces(marketplace_id);
				royalties
					.entitlements
					.push((marketplace.account, marketplace.entitlement));
				ensure!(royalties.validate(), Error::<T>::RoyaltiesInvalid);
				royalties
			}
			None => royalties,
		};
		Ok(royalties)
	}
	/// Transfer the given tokens from `current_owner` to `new_owner`
	/// Does no verification
	fn do_transfer_unchecked(
		collection_id: CollectionId,
		series_id: SeriesId,
		serial_numbers: &[SerialNumber],
		current_owner: &T::AccountId,
		new_owner: &T::AccountId,
	) -> DispatchResult {
		for serial_number in serial_numbers.iter() {
			<TokenOwner<T>>::insert((collection_id, series_id), serial_number, new_owner);
			let token_id: TokenId = (collection_id, series_id, *serial_number);
			T::OnTransferSubscription::on_nft_transfer(&token_id);
		}
		let quantity = serial_numbers.len() as TokenCount;
		let _ = <TokenBalance<T>>::try_mutate::<_, (), Error<T>, _>(&current_owner, |balances| {
			match (*balances).get_mut(&(collection_id, series_id)) {
				Some(balance) => {
					let new_balance = balance.saturating_sub(quantity);
					if new_balance.is_zero() {
						balances.remove(&(collection_id, series_id));
					} else {
						*balance = new_balance;
					}
					Ok(())
				}
				None => return Err(Error::NoToken.into()), // should not happen
			}
		});
		<TokenBalance<T>>::mutate(&new_owner, |balances| {
			*balances.entry((collection_id, series_id)).or_default() += quantity
		});

		Ok(())
	}
	/// Mint additional tokens in a series
	fn do_mint(
		owner: &T::AccountId,
		collection_id: CollectionId,
		series_id: SeriesId,
		serial_number: SerialNumber,
		quantity: TokenCount,
	) -> DispatchResult {
		ensure!(quantity > Zero::zero(), Error::<T>::NoToken);

		// Mint the set tokens
		for serial_number in serial_number..serial_number + quantity {
			<TokenOwner<T>>::insert((collection_id, series_id), serial_number as SerialNumber, &owner);
		}

		// update token balances
		<TokenBalance<T>>::mutate(&owner, |balances| {
			*balances.entry((collection_id, series_id)).or_default() += quantity
		});
		SeriesIssuance::mutate(collection_id, series_id, |q| *q = q.saturating_add(quantity));
		NextSerialNumber::mutate(collection_id, series_id, |q| *q = q.saturating_add(quantity));

		Ok(())
	}

	/// Find the tokens owned by an `address` in the given collection
	pub fn collected_tokens(collection_id: CollectionId, address: &T::AccountId) -> Vec<TokenId> {
		let next_series_id = Self::next_series_id(collection_id);
		let mut owned_tokens = Vec::<TokenId>::default();

		// Search each series up until the last known series Id
		for series_id in 0..next_series_id {
			let mut owned_in_series: Vec<TokenId> = <TokenOwner<T>>::iter_prefix((collection_id, series_id))
				.filter_map(|(serial_number, owner)| {
					if &owner == address {
						Some((collection_id, series_id, serial_number))
					} else {
						None
					}
				})
				.collect();
			if !owned_in_series.is_empty() {
				owned_in_series.sort_unstable();
				owned_tokens.append(&mut owned_in_series);
			}
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
					// release listed tokens
					for token_id in listing.tokens.iter() {
						TokenLocks::remove(token_id);
					}
					let listing_collection_id = listing.tokens[0].0;
					OpenCollectionListings::remove(listing_collection_id, listing_id);

					Self::deposit_event(RawEvent::FixedPriceSaleClosed(listing_collection_id, listing_id));
				}
				Some(Listing::Auction(listing)) => {
					// release listed tokens
					for token_id in listing.tokens.iter() {
						TokenLocks::remove(token_id);
					}
					let listing_collection_id = listing.tokens[0].0;
					OpenCollectionListings::remove(listing_collection_id, listing_id);

					if let Some((winner, hammer_price)) = ListingWinningBid::<T>::take(listing_id) {
						if let Err(err) = Self::settle_auction(&listing, &winner, hammer_price) {
							// auction settlement failed despite our prior validations.
							// release winning bid funds
							log!(error, "🃏 auction settlement failed: {:?}", err);
							T::MultiCurrency::unreserve(&winner, listing.payment_asset, hammer_price);

							// listing metadata is removed by now.
							Self::deposit_event(RawEvent::AuctionClosed(
								listing_collection_id,
								listing_id,
								AuctionClosureReason::SettlementFailed,
							));
						} else {
							// auction settlement success
							Self::deposit_event(RawEvent::AuctionSold(
								listing_collection_id,
								listing_id,
								listing.payment_asset,
								hammer_price,
								winner,
							));
						}
					} else {
						// normal closure, no acceptable bids
						// listing metadata is removed by now.
						Self::deposit_event(RawEvent::AuctionClosed(
							listing_collection_id,
							listing_id,
							AuctionClosureReason::ExpiredNoBids,
						));
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
		let for_royalties = listing.royalties_schedule.calculate_total_entitlement() * hammer_price;
		let mut for_seller = hammer_price;

		// do royalty payments
		if !for_royalties.is_zero() {
			let entitlements = listing.royalties_schedule.entitlements.clone();
			for (who, entitlement) in entitlements.into_iter() {
				let royalty = entitlement * hammer_price;
				let _ = T::MultiCurrency::repatriate_reserved(&winner, listing.payment_asset, &who, royalty)?;
				for_seller -= royalty;
			}
		}

		let seller_balance = T::MultiCurrency::free_balance(&listing.seller, listing.payment_asset);
		let _ = T::MultiCurrency::repatriate_reserved(&winner, listing.payment_asset, &listing.seller, for_seller)?;

		// The implementation of `repatriate_reserved` may take less than the required amount and succeed
		// this should not happen but could for reasons outside the control of this module
		ensure!(
			T::MultiCurrency::free_balance(&listing.seller, listing.payment_asset)
				>= seller_balance.saturating_add(for_seller),
			Error::<T>::InternalPayment
		);
		// all tokens have the same collection and series id
		let (collection_id, series_id, _) = listing.tokens[0];
		let serial_numbers: Vec<SerialNumber> = listing.tokens.iter().map(|id| id.2).collect();
		Self::do_transfer_unchecked(collection_id, series_id, &serial_numbers, &listing.seller, &winner)
	}
	/// Get collection information from given collection_id
	pub fn collection_info<AccountId>(collection_id: CollectionId) -> Option<CollectionInfo<T::AccountId>> {
		let name = Self::collection_name(&collection_id);
		let owner = Self::collection_owner(&collection_id).unwrap_or(Default::default());

		if name.is_empty() {
			None
		} else {
			let royalties = match <CollectionRoyalties<T>>::get(&collection_id) {
				Some(r) => r.entitlements,
				None => Vec::new(),
			};
			Some(CollectionInfo { name, owner, royalties })
		}
	}
	/// Find the attributes and owner from a series
	pub fn token_info(
		collection_id: CollectionId,
		series_id: SeriesId,
		serial_number: SerialNumber,
	) -> TokenInfo<T::AccountId> {
		let attributes = Self::series_attributes(collection_id, series_id);
		let owner = Self::token_owner((collection_id, series_id), serial_number);
		let royalties = match <SeriesRoyalties<T>>::get(collection_id, series_id) {
			Some(r) => r.entitlements,
			None => match <CollectionRoyalties<T>>::get(&collection_id) {
				Some(r) => r.entitlements,
				None => Vec::new(),
			},
		};
		TokenInfo {
			attributes,
			owner,
			royalties,
		}
	}
	/// Get list of all NFT listings within a range
	pub fn collection_listings(
		collection_id: CollectionId,
		cursor: u128,
		limit: u16,
	) -> (Option<u128>, Vec<(ListingId, Listing<T>)>) {
		let mut listing_ids = OpenCollectionListings::iter_prefix(collection_id)
			.map(|(listing_id, _)| listing_id)
			.collect::<Vec<u128>>();
		listing_ids.sort();
		let last_id = listing_ids.last().copied();
		let mut highest_cursor: u128 = 0;

		let response: Vec<(ListingId, Listing<T>)> = listing_ids
			.into_iter()
			.filter(|listing_id| listing_id >= &cursor)
			.take(sp_std::cmp::min(limit, MAX_COLLECTION_LISTING_LIMIT).into())
			.map(|listing_id| {
				highest_cursor = listing_id;
				match Self::listings(listing_id) {
					Some(listing) => Some((listing_id, listing)),
					None => {
						log!(error, "🃏 Unexpected empty listing: {:?}", listing_id);
						None
					}
				}
			})
			.flatten()
			.collect();

		let new_cursor = match last_id {
			Some(id) => {
				if highest_cursor != id {
					Some(highest_cursor + 1)
				} else {
					None
				}
			}
			None => None,
		};
		(new_cursor, response)
	}
}
