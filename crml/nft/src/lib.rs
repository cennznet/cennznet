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
//! *Series*:
//! Series are a grouping of tokens- equivalent to an ERC721 contract
//!
//! *Tokens*:
//!  Individual tokens within a series. Globally identifiable by a tuple of (series, serial number)
//!

use cennznet_primitives::types::{AssetId, Balance, SerialNumber, SeriesId};
use crml_support::{log, IsTokenOwner, MultiCurrency, OnTransferSubscriber};
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

mod types;
pub use types::*;

/// The maximum number of attributes in an NFT series schema
pub const MAX_SCHEMA_FIELDS: u32 = 16;
/// The maximum length of valid series IDs
pub const MAX_SERIES_NAME_LENGTH: u8 = 32;
/// The maximum amount of listings to return
pub const MAX_SERIES_LISTING_LIMIT: u16 = 100;
/// The logging target for this module
pub(crate) const LOG_TARGET: &str = "nft";

/// Unique token identifier
pub type TokenId = (SeriesId, SerialNumber);

// Interface for determining ownership of an NFT from some account
impl<T: Config> IsTokenOwner for Module<T> {
	type AccountId = T::AccountId;

	fn check_ownership(account: &Self::AccountId, token_id: &TokenId) -> bool {
		if let Some(owner) = Self::token_owner(token_id.0, token_id.1) {
			&owner == account
		} else {
			false
		}
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
		<T as frame_system::Config>::AccountId,
		AssetId = AssetId,
		Balance = Balance,
		Reason = AuctionClosureReason,
		SeriesId = SeriesId,
		SerialNumber = SerialNumber,
		TokenCount = TokenCount,
		Permill = Permill,
		MarketplaceId = MarketplaceId,
		OfferId = OfferId,
	{
		/// A new series of tokens was created (series id, quantity, owner)
		CreateSeries(SeriesId, TokenCount, AccountId),
		/// Token(s) were created (series id, quantity, owner)
		CreateTokens(SeriesId, TokenCount, AccountId),
		/// A token was transferred (previous owner, token Id, new owner)
		Transfer(AccountId, SeriesId, SerialNumber, AccountId),
		/// A token was burned (series id, serial number)
		Burn(SeriesId, SerialNumber),
		/// A fixed price sale has been listed (series, listing, marketplace_id)
		FixedPriceSaleListed(SeriesId, ListingId, Option<MarketplaceId>),
		/// A fixed price sale has completed (series, listing, buyer))
		FixedPriceSaleComplete(SeriesId, ListingId, AccountId),
		/// A fixed price sale has closed without selling (series, listing)
		FixedPriceSaleClosed(SeriesId, ListingId),
		///A fixed price sale has had its price updated (series, listing)
		FixedPriceSalePriceUpdated(SeriesId, ListingId),
		/// An auction has opened (series, listing, marketplace_id)
		AuctionOpen(SeriesId, ListingId, Option<MarketplaceId>),
		/// An auction has sold (series, listing, payment asset, bid, new owner)
		AuctionSold(SeriesId, ListingId, AssetId, Balance, AccountId),
		/// An auction has closed without selling (series, listing, reason)
		AuctionClosed(SeriesId, ListingId, Reason),
		/// A new highest bid was placed (series, listing, amount)
		Bid(SeriesId, ListingId, Balance),
		/// An account has been registered as a marketplace (account, entitlement, marketplace_id)
		RegisteredMarketplace(AccountId, Permill, MarketplaceId),
		/// An offer has been made on an NFT (offer_id, amount, asset_id, marketplace_id, buyer)
		OfferMade(OfferId, Balance, AssetId, Option<MarketplaceId>, AccountId),
		/// An offer has been cancelled (offer_id)
		OfferCancelled(OfferId),
		/// An offer has been cancelled (offer_id)
		OfferAccepted(OfferId, Balance),
		/// The token balances for a series have been updated (series_id)
		TokenBalancesUpdated(SeriesId),
	}
);

decl_error! {
	/// Error for the staking module.
	pub enum Error for Module<T: Config> {
		/// Given series name is invalid (invalid utf-8, too long, empty)
		SeriesNameInvalid,
		/// No more Ids are available, they've been exhausted
		NoAvailableIds,
		/// Too many attributes in the provided schema or data
		SchemaMaxAttributes,
		/// Given attribute value is larger than the configured max.
		MaxAttributeLength,
		/// origin does not have permission for the operation (the token may not exist)
		NoPermission,
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
		/// Selling tokens from different series is not allowed
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
		/// Map from series to its information
		pub SeriesInfo get(fn series_info): map hasher(twox_64_concat) SeriesId => Option<SeriesInformation<T::AccountId>>;
		/// Map from a series to its total issuance
		pub SeriesIssuance get(fn series_issuance): map hasher(twox_64_concat) SeriesId =>  TokenCount;
		/// The next available series Id
		NextSeriesId get(fn next_series_id): SeriesId;
		/// The next available serial number in a given series
		NextSerialNumber get(fn next_serial_number): map hasher(twox_64_concat) SeriesId => SerialNumber;
		/// Map from a token to lock status if any
		pub TokenLocks get(fn token_locks): map hasher(twox_64_concat) TokenId => Option<TokenLockReason>;
		/// Map from a token to its owner
		pub TokenOwner get(fn token_owner): double_map hasher(twox_64_concat) SeriesId, hasher(twox_64_concat) SerialNumber => Option<T::AccountId>;
		/// Count of tokens owned by an address, supports ERC721 `balanceOf`
		pub TokenBalance get(fn token_balance): map hasher(blake2_128_concat) T::AccountId => BTreeMap<SeriesId, TokenCount>;
		/// The next available marketplace id
		pub NextMarketplaceId get(fn next_marketplace_id): MarketplaceId;
		/// Map from marketplace account_id to royalties schedule
		pub RegisteredMarketplaces get(fn registered_marketplaces): map hasher(twox_64_concat) MarketplaceId => Option<Marketplace<T::AccountId>>;
		/// NFT sale/auction listings keyed by listing id
		pub Listings get(fn listings): map hasher(twox_64_concat) ListingId => Option<Listing<T>>;
		/// The next available listing Id
		pub NextListingId get(fn next_listing_id): ListingId;
		/// Map from series to any open listings
		pub OpenSeriesListings get(fn open_series_listings): double_map hasher(twox_64_concat) SeriesId, hasher(twox_64_concat) ListingId => bool;
		/// Winning bids on open listings.
		pub ListingWinningBid get(fn listing_winning_bid): map hasher(twox_64_concat) ListingId => Option<(T::AccountId, Balance)>;
		/// Block numbers where listings will close. Value is `true` if at block number `listing_id` is scheduled to close.
		pub ListingEndSchedule get(fn listing_end_schedule): double_map hasher(twox_64_concat) T::BlockNumber, hasher(twox_64_concat) ListingId => bool;
		/// Map from offer_id to the information related to the offer
		pub Offers get(fn offers): map hasher(twox_64_concat) OfferId => Option<OfferType<T::AccountId>>;
		/// Maps from token_id to a vector of offer_ids on that token
		pub TokenOffers get(fn token_offers): map hasher(twox_64_concat) TokenId => Vec<OfferId>;
		/// The next available offer_id
		pub NextOfferId get(fn next_offer_id): OfferId;
		/// Version of this module's storage schema
		StorageVersion build(|_: &GenesisConfig| Releases::V2 as u32): u32;
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin, system = frame_system {
		type Error = Error<T>;

		fn deposit_event() = default;

		/// Check and close all expired listings
		fn on_initialize(now: T::BlockNumber) -> Weight {
			// TODO: this is unbounded and could become costly
			// https://github.com/cennznet/cennznet/issues/444
			let removed_count = Self::close_listings_at(now);
			// 'buy' weight is comparable to successful closure of an auction
			T::WeightInfo::buy() * removed_count as Weight
		}

		/// Set the owner of a series
		/// Caller must be the current series owner
		#[weight = T::WeightInfo::set_owner()]
		fn set_owner(origin, series_id: SeriesId, new_owner: T::AccountId) -> DispatchResult {
			let origin = ensure_signed(origin)?;
			if let Some(mut series_info) = Self::series_info(series_id) {
				ensure!(series_info.owner == origin, Error::<T>::NoPermission);
				series_info.owner = new_owner;
				<SeriesInfo<T>>::insert(series_id, series_info);
				Ok(())
			} else {
				Err(Error::<T>::NoSeries.into())
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

		/// Create a new series
		/// Additional tokens can be minted via `mint_additional`
		///
		/// `name` - the name of the series
		/// `quantity` - number of tokens to mint now
		/// `owner` - the token owner, defaults to the caller
		/// `metadata_scheme` - The off-chain metadata referencing scheme for tokens in this series
		/// `royalties_schedule` - defacto royalties plan for secondary sales, this will apply to all tokens in the series by default.
		#[weight = T::WeightInfo::mint_series(*quantity)]
		#[transactional]
		pub fn create_series(
			origin,
			name: SeriesNameType,
			quantity: TokenCount,
			owner: Option<T::AccountId>,
			metadata_scheme: MetadataScheme,
			royalties_schedule: Option<RoyaltiesSchedule<T::AccountId>>,
		) -> DispatchResult {
			let origin = ensure_signed(origin)?;

			// Check we can issue the new tokens
			let series_id = Self::next_series_id();
			ensure!(
				series_id.checked_add(One::one()).is_some(),
				Error::<T>::NoAvailableIds
			);


			// Validate series attributes
			ensure!(!name.is_empty() && name.len() <= MAX_SERIES_NAME_LENGTH as usize, Error::<T>::SeriesNameInvalid);
			ensure!(core::str::from_utf8(&name).is_ok(), Error::<T>::SeriesNameInvalid);
			let metadata_scheme = metadata_scheme.sanitize().map_err(|_| Error::<T>::InvalidMetadataPath)?;
			if let Some(royalties_schedule) = royalties_schedule.clone() {
				ensure!(royalties_schedule.validate(), Error::<T>::RoyaltiesInvalid);
			}

			let owner = owner.unwrap_or(origin);
			<SeriesInfo<T>>::insert(series_id, SeriesInformation {
				owner: owner.clone(),
				name,
				metadata_scheme,
				royalties_schedule,
			});

			// Now mint the series tokens
			if quantity > Zero::zero() {
				Self::do_mint(&owner, series_id, 0 as SerialNumber, quantity)?;
			}
			// will not overflow, asserted prior qed.
			NextSeriesId::mutate(|i| *i += SeriesId::one());

			Self::deposit_event(RawEvent::CreateSeries(series_id, quantity, owner));

			Ok(())
		}

		/// Mint tokens for an existing series
		///
		/// `series_id` - the series to mint tokens in
		/// `quantity` - how many tokens to mint
		/// `owner` - the token owner, defaults to the caller if unspecified
		/// Caller must be the series owner
		/// -----------
		/// Weight is O(N) where N is `quantity`
		#[weight = T::WeightInfo::mint_additional(*quantity)]
		#[transactional]
		fn mint(
			origin,
			series_id: SeriesId,
			quantity: TokenCount,
			owner: Option<T::AccountId>,
		) -> DispatchResult {
			let origin = ensure_signed(origin)?;

			// Permission and existence check
			if let Some(series_info) = Self::series_info(series_id) {
				ensure!(series_info.owner == origin, Error::<T>::NoPermission);
			} else {
				return Err(Error::<T>::NoSeries.into());
			}

			let serial_number = Self::next_serial_number(series_id);
			ensure!(
				serial_number.checked_add(quantity).is_some(),
				Error::<T>::NoAvailableIds
			);
			let owner = owner.unwrap_or(origin);

			Self::do_mint(&owner, series_id, serial_number, quantity)?;
			Self::deposit_event(RawEvent::CreateTokens(series_id, quantity, owner));

			Ok(())
		}

		/// Transfer ownership of an NFT
		/// Caller must be the token owner
		#[weight = {
			T::WeightInfo::transfer()
		}]
		#[transactional]
		fn transfer(origin, token_id: TokenId, new_owner: T::AccountId) {
			let origin = ensure_signed(origin)?;
			ensure!(!TokenLocks::contains_key(token_id), Error::<T>::TokenListingProtection);
			ensure!(
				Self::token_owner(token_id.0, token_id.1) == Some(origin.clone()),
				Error::<T>::NoPermission
			);
			let _ = Self::do_transfer_unchecked(token_id.clone(), &origin, &new_owner)?;

			Self::deposit_event(RawEvent::Transfer(origin, token_id.0, token_id.1, new_owner));
		}

		/// Burn a token 🔥
		///
		/// Caller must be the token owner
		#[weight = T::WeightInfo::burn()]
		#[transactional]
		fn burn(origin, token_id: TokenId) {
			let origin = ensure_signed(origin)?;

			let (series_id, serial_number) = token_id;

			ensure!(!TokenLocks::contains_key((series_id, serial_number)), Error::<T>::TokenListingProtection);
			ensure!(Self::token_owner(series_id, serial_number) == Some(origin.clone()), Error::<T>::NoPermission);
			<TokenOwner<T>>::remove(series_id, serial_number);

			let _ = <TokenBalance<T>>::try_mutate::<_, (), Error<T>, _>(&origin, |balances| {
				match (*balances).get_mut(&series_id) {
					Some(balance) => {
						let new_balance = balance.saturating_sub(1);
						if new_balance.is_zero() {
							balances.remove(&series_id);
						} else {
							*balance = new_balance;
						}
						Ok(())
					},
					None => Err(Error::<T>::NoToken.into()),
				}
			})?;

			if Self::series_issuance(series_id).saturating_sub(1).is_zero() {
				// this is the last of the tokens
				<SeriesInfo<T>>::remove(series_id);
				SeriesIssuance::remove(series_id);
			} else {
				SeriesIssuance::mutate(series_id, |q| *q = q.saturating_sub(1));
			}

			Self::deposit_event(RawEvent::Burn(series_id, serial_number));
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
		fn sell(
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

			// use the first token's series as representative of the bundle
			let (bundle_series_id, _serial_number) = tokens[0];
			for (series_id, serial_number) in tokens.iter() {
				ensure!(!TokenLocks::contains_key((series_id, serial_number)), Error::<T>::TokenListingProtection);
				ensure!(Self::token_owner(series_id, serial_number) == Some(origin.clone()), Error::<T>::NoPermission);
				TokenLocks::insert((series_id, serial_number), TokenLockReason::Listed(listing_id));
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

			OpenSeriesListings::insert(bundle_series_id, listing_id, true);
			Listings::insert(listing_id, listing);
			NextListingId::mutate(|i| *i += 1);

			Self::deposit_event(RawEvent::FixedPriceSaleListed(bundle_series_id, listing_id, marketplace_id));
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

				let series_id = listing.tokens.get(0).ok_or_else(|| Error::<T>::NoToken)?.0;

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

				OpenSeriesListings::remove(series_id, listing_id);

				for token_id in listing.tokens.clone() {
					TokenLocks::remove(token_id);
					let _ = Self::do_transfer_unchecked(token_id, &listing.seller, &origin)?;
				}
				Self::remove_fixed_price_listing(listing_id);

				Self::deposit_event(RawEvent::FixedPriceSaleComplete(series_id, listing_id, origin));
			} else {
				return Err(Error::<T>::NotForFixedPriceSale.into());
			}
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
		fn auction(
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

			// use the first token's series as representative of the bundle
			let (bundle_series_id, _serial_number) = tokens[0];
			for (series_id, serial_number) in tokens.iter() {
				ensure!(!TokenLocks::contains_key((series_id, serial_number)), Error::<T>::TokenListingProtection);
				ensure!(Self::token_owner(series_id, serial_number) == Some(origin.clone()), Error::<T>::NoPermission);
				TokenLocks::insert((series_id, serial_number), TokenLockReason::Listed(listing_id));
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

			OpenSeriesListings::insert(bundle_series_id, listing_id, true);
			Listings::insert(listing_id, listing);
			NextListingId::mutate(|i| *i += 1);

			Self::deposit_event(RawEvent::AuctionOpen(bundle_series_id, listing_id, marketplace_id));
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

				let listing_series_id = listing.tokens[0].0;
				Self::deposit_event(RawEvent::Bid(listing_series_id, listing_id, amount));
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
					let series_id = sale.tokens[0].0;
					OpenSeriesListings::remove(series_id, listing_id);

					Self::deposit_event(RawEvent::FixedPriceSaleClosed(series_id, listing_id));
				},
				Some(Listing::<T>::Auction(auction)) => {
					ensure!(auction.seller == origin, Error::<T>::NoPermission);
					ensure!(Self::listing_winning_bid(listing_id).is_none(), Error::<T>::TokenListingProtection);
					Listings::<T>::remove(listing_id);
					ListingEndSchedule::<T>::remove(auction.close, listing_id);
					for token_id in auction.tokens.iter() {
						TokenLocks::remove(token_id);
					}
					let series_id = auction.tokens[0].0;
					OpenSeriesListings::remove(series_id, listing_id);

					Self::deposit_event(RawEvent::AuctionClosed(series_id, listing_id, AuctionClosureReason::VendorCancelled));
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
					let series_id = sale.tokens[0].0;

					Listings::insert(listing_id, Listing::<T>::FixedPrice(sale));
					Self::deposit_event(RawEvent::FixedPriceSalePriceUpdated(series_id, listing_id));
					Ok(())
				},
				Some(Listing::<T>::Auction(_)) => Err(Error::<T>::NotForFixedPriceSale.into()),
				None => Err(Error::<T>::NotForFixedPriceSale.into()),
			}
		}

		/// Create an offer on a token
		/// Locks funds until offer is accepted, rejected or cancelled
		#[weight = T::WeightInfo::make_simple_offer()]
		#[transactional]
		fn make_simple_offer (
			origin,
			token_id: TokenId,
			amount: Balance,
			asset_id: AssetId,
			marketplace_id: Option<MarketplaceId>,
		) -> DispatchResult {
			let origin = ensure_signed(origin)?;
			ensure!(!amount.is_zero(), Error::<T>::ZeroOffer);
			ensure!(Self::token_owner(token_id.0, token_id.1) != Some(origin.clone()), Error::<T>::IsTokenOwner);
			let offer_id = Self::next_offer_id();
			ensure!(offer_id.checked_add(One::one()).is_some(), Error::<T>::NoAvailableIds);

			// ensure the token_id is not currently in an auction
			if let Some(TokenLockReason::Listed(listing_id)) = Self::token_locks(token_id) {
				match Self::listings(listing_id) {
					Some(Listing::<T>::Auction(_)) => return Err(Error::<T>::TokenOnAuction.into()),
					None | Some(Listing::<T>::FixedPrice(_)) => (),
				}
			}
			// check user has the required funds to make this offer
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
			TokenOffers::append(token_id, offer_id);
			let new_offer = OfferType::<T::AccountId>::Simple(
				SimpleOffer{
					token_id,
					asset_id,
					amount,
					buyer: origin.clone(),
					marketplace_id,
				}
			);
			Offers::<T>::insert(offer_id, new_offer);
			NextOfferId::mutate(|i| *i += 1);

			Self::deposit_event(RawEvent::OfferMade(offer_id, amount, asset_id, marketplace_id, origin));
			Ok(())
		}

		/// Cancels an offer on a token
		/// Caller must be the offer buyer
		#[weight = T::WeightInfo::cancel_offer()]
		fn cancel_offer (
			origin,
			offer_id: OfferId,
		) -> DispatchResult {
			let origin = ensure_signed(origin)?;
			if let Some(offer_type) = Self::offers(offer_id) {
				match offer_type {
					OfferType::Simple(offer) => {
						ensure!(offer.buyer == origin, Error::<T>::NotBuyer);
						T::MultiCurrency::unreserve(&origin, offer.asset_id, offer.amount);
						Offers::<T>::remove(offer_id);
						TokenOffers::mutate(offer.token_id, |offers| offers.binary_search(&offer_id).map(|idx| offers.remove(idx)).unwrap());
						Self::deposit_event(RawEvent::OfferCancelled(offer_id));
						Ok(())
					}
				}
			} else {
				Err(Error::<T>::InvalidOffer.into())
			}
		}

		/// Accepts an offer on a token
		/// Caller must be token owner
		#[weight = T::WeightInfo::accept_offer()]
		#[transactional]
		fn accept_offer (
			origin,
			offer_id: OfferId,
		) -> DispatchResult {
			let origin = ensure_signed(origin)?;
			if let Some(offer_type) = Self::offers(offer_id) {
				match offer_type {
					OfferType::Simple(offer) => {
						let token_id = offer.token_id;
						ensure!(Self::token_owner(token_id.0, token_id.1) == Some(origin.clone()), Error::<T>::NoPermission);

						let royalties_schedule = Self::check_bundle_royalties(&vec![token_id], offer.marketplace_id)?;
						Self::process_payment_and_transfer(
							&offer.buyer,
							&origin,
							offer.asset_id,
							vec![offer.token_id],
							offer.amount,
							royalties_schedule,
						)?;

						// Clean storage
						Offers::<T>::remove(offer_id);
						TokenOffers::mutate(token_id, |offers| offers.binary_search(&offer_id).map(|idx| offers.remove(idx)).unwrap());
						Self::deposit_event(RawEvent::OfferAccepted(offer_id, offer.amount));
						Ok(())
					}
				}
			} else {
				Err(Error::<T>::InvalidOffer.into())
			}
		}
	}
}

impl<T: Config> Module<T> {
	/// Return whether the series exists or not
	pub fn series_exists(series_id: SeriesId) -> bool {
		<SeriesInfo<T>>::contains_key(series_id)
	}

	/// Construct & return the full metadata URI for a given `token_id` (analogous to ERC721 metadata token_uri)
	pub fn token_uri(token_id: TokenId) -> Vec<u8> {
		use core::fmt::Write;
		if let Some(series_info) = Self::series_info(token_id.0) {
			let scheme = series_info.metadata_scheme;
			let mut token_uri = sp_std::Writer::default();
			match scheme {
				MetadataScheme::Http(path) => {
					let path = core::str::from_utf8(&path).unwrap_or("");
					write!(&mut token_uri, "http://{}/{}.json", path, token_id.1).expect("Not written");
				}
				MetadataScheme::Https(path) => {
					let path = core::str::from_utf8(&path).unwrap_or("");
					write!(&mut token_uri, "https://{}/{}.json", path, token_id.1).expect("Not written");
				}
				MetadataScheme::IpfsDir(dir_cid) => {
					write!(
						&mut token_uri,
						"ipfs://{}/{}.json",
						core::str::from_utf8(&dir_cid).unwrap_or(""),
						token_id.1
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
			return Default::default();
		}
	}

	/// Check royalties will be respected on all tokens if placed into a bundle sale.
	/// We're ok iff, all tokens in the bundle are from the:
	/// 1) same series
	/// Although possible, we do not support:
	/// 3) different series, no royalties allowed
	fn check_bundle_royalties(
		tokens: &[TokenId],
		marketplace_id: Option<MarketplaceId>,
	) -> Result<RoyaltiesSchedule<T::AccountId>, Error<T>> {
		// use the first token's series as representative of the bundle
		let (bundle_series_id, _serial_number) = tokens[0];

		for (series_id, _serial_number) in tokens.iter() {
			ensure!(*series_id == bundle_series_id, Error::<T>::MixedBundleSale);
		}

		let series_info = Self::series_info(bundle_series_id);
		ensure!(series_info.is_some(), Error::<T>::NoSeries);
		let series_royalties = series_info.unwrap().royalties_schedule;

		let mut royalties: RoyaltiesSchedule<T::AccountId> =
			series_royalties.unwrap_or_else(|| RoyaltiesSchedule { entitlements: vec![] });

		let royalties = match marketplace_id {
			Some(marketplace_id) => {
				ensure!(
					<RegisteredMarketplaces<T>>::contains_key(marketplace_id),
					Error::<T>::MarketplaceNotRegistered
				);
				if let Some(marketplace) = Self::registered_marketplaces(marketplace_id) {
					royalties
						.entitlements
						.push((marketplace.account, marketplace.entitlement));
				}
				ensure!(royalties.validate(), Error::<T>::RoyaltiesInvalid);
				royalties
			}
			None => royalties,
		};
		Ok(royalties)
	}

	/// Transfer the given token from `current_owner` to `new_owner`
	/// Does no verification
	fn do_transfer_unchecked(
		token_id: TokenId,
		current_owner: &T::AccountId,
		new_owner: &T::AccountId,
	) -> DispatchResult {
		let (series_id, serial_number) = token_id;

		<TokenOwner<T>>::insert(series_id, serial_number, new_owner);
		T::OnTransferSubscription::on_nft_transfer(&token_id);

		let quantity = 1 as TokenCount;
		let _ = <TokenBalance<T>>::try_mutate::<_, (), Error<T>, _>(&current_owner, |balances| {
			match (*balances).get_mut(&series_id) {
				Some(balance) => {
					let new_balance = balance.saturating_sub(quantity);
					if new_balance.is_zero() {
						balances.remove(&series_id);
					} else {
						*balance = new_balance;
					}
					Ok(())
				}
				None => return Err(Error::NoToken.into()), // should not happen
			}
		});
		<TokenBalance<T>>::mutate(&new_owner, |balances| {
			*balances.entry(series_id).or_default() += quantity
		});

		Ok(())
	}

	/// Mint additional tokens in a series
	fn do_mint(
		owner: &T::AccountId,
		series_id: SeriesId,
		serial_number: SerialNumber,
		quantity: TokenCount,
	) -> DispatchResult {
		ensure!(quantity > Zero::zero(), Error::<T>::NoToken);

		// Mint the set tokens
		for serial_number in serial_number..serial_number + quantity {
			<TokenOwner<T>>::insert(series_id, serial_number as SerialNumber, &owner);
		}

		// update token balances
		<TokenBalance<T>>::mutate(&owner, |balances| *balances.entry(series_id).or_default() += quantity);
		SeriesIssuance::mutate(series_id, |q| *q = q.saturating_add(quantity));
		NextSerialNumber::mutate(series_id, |q| *q = q.saturating_add(quantity));

		Ok(())
	}

	/// Find the tokens owned by an `address` in the given series
	pub fn collected_tokens(series_id: SeriesId, address: &T::AccountId) -> Vec<TokenId> {
		let mut owned_tokens = Vec::<TokenId>::default();

		let mut owned_in_series: Vec<TokenId> = <TokenOwner<T>>::iter_prefix(series_id)
			.filter_map(|(serial_number, owner)| {
				if &owner == address {
					Some((series_id, serial_number))
				} else {
					None
				}
			})
			.collect();

		if !owned_in_series.is_empty() {
			owned_in_series.sort_unstable();
			owned_tokens.append(&mut owned_in_series);
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
					let listing_series_id: SeriesId = listing.tokens[0].0;
					OpenSeriesListings::remove(listing_series_id, listing_id);

					Self::deposit_event(RawEvent::FixedPriceSaleClosed(listing_series_id, listing_id));
				}
				Some(Listing::Auction(listing)) => {
					// release listed tokens
					for token_id in listing.tokens.iter() {
						TokenLocks::remove(token_id);
					}
					let listing_series_id: SeriesId = listing.tokens[0].0;
					OpenSeriesListings::remove(listing_series_id, listing_id);

					if let Some((winner, hammer_price)) = ListingWinningBid::<T>::take(listing_id) {
						if let Err(err) = Self::process_payment_and_transfer(
							&winner,
							&listing.seller,
							listing.payment_asset,
							listing.tokens,
							hammer_price,
							listing.royalties_schedule,
						) {
							// auction settlement failed despite our prior validations.
							// release winning bid funds
							log!(error, "🃏 auction settlement failed: {:?}", err);
							T::MultiCurrency::unreserve(&winner, listing.payment_asset, hammer_price);

							// listing metadata is removed by now.
							Self::deposit_event(RawEvent::AuctionClosed(
								listing_series_id,
								listing_id,
								AuctionClosureReason::SettlementFailed,
							));
						} else {
							// auction settlement success
							Self::deposit_event(RawEvent::AuctionSold(
								listing_series_id,
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
							listing_series_id,
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

	/// Settle an auction listing or accepted offer
	/// (guaranteed to be atomic).
	/// - transfer funds from winning bidder to entitled royalty accounts and seller
	/// - transfer ownership to the winning bidder
	#[transactional]
	fn process_payment_and_transfer(
		buyer: &T::AccountId,
		seller: &T::AccountId,
		asset_id: AssetId,
		token_ids: Vec<TokenId>,
		amount: Balance,
		royalties_schedule: RoyaltiesSchedule<T::AccountId>,
	) -> DispatchResult {
		let for_royalties = royalties_schedule.calculate_total_entitlement() * amount;
		let mut for_seller = amount;

		// do royalty payments
		if !for_royalties.is_zero() {
			let entitlements = royalties_schedule.entitlements.clone();
			for (who, entitlement) in entitlements.into_iter() {
				let royalty = entitlement * amount;
				let _ = T::MultiCurrency::repatriate_reserved(buyer, asset_id, &who, royalty)?;
				for_seller -= royalty;
			}
		}

		let seller_balance = T::MultiCurrency::free_balance(seller, asset_id);
		let _ = T::MultiCurrency::repatriate_reserved(buyer, asset_id, seller, for_seller)?;

		// The implementation of `repatriate_reserved` may take less than the required amount and succeed
		// this should not happen but could for reasons outside the control of this module
		ensure!(
			T::MultiCurrency::free_balance(seller, asset_id) >= seller_balance.saturating_add(for_seller),
			Error::<T>::InternalPayment
		);

		// Transfer each token
		for token_id in token_ids {
			let _ = Self::do_transfer_unchecked(token_id, seller, buyer)?;
		}
		Ok(())
	}

	/// Find the royalties and owner of a token
	pub fn token_info(series_id: SeriesId, serial_number: SerialNumber) -> Option<TokenInfo<T::AccountId>> {
		let series_info = Self::series_info(series_id);
		if let Some(series_info) = series_info {
			if let Some(owner) = Self::token_owner(series_id, serial_number) {
				let royalties = match series_info.royalties_schedule {
					Some(r) => r.entitlements,
					None => Vec::new(),
				};

				return Some(TokenInfo { owner, royalties });
			}
		}
		None
	}

	/// Get list of all NFT listings within a range
	pub fn series_listings(
		series_id: SeriesId,
		cursor: u128,
		limit: u16,
	) -> (Option<u128>, Vec<(ListingId, Listing<T>)>) {
		let mut listing_ids = OpenSeriesListings::iter_prefix(series_id)
			.map(|(listing_id, _)| listing_id)
			.collect::<Vec<u128>>();
		listing_ids.sort();
		let last_id = listing_ids.last().copied();
		let mut highest_cursor: u128 = 0;

		let response: Vec<(ListingId, Listing<T>)> = listing_ids
			.into_iter()
			.filter(|listing_id| listing_id >= &cursor)
			.take(sp_std::cmp::min(limit, MAX_SERIES_LISTING_LIMIT).into())
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
