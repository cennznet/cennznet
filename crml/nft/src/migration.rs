#[allow(dead_code)]
use crate::pallet::{AuctionListing, FixedPriceListing, Listing, Listings, StorageVersion, CollectionId, Config, SeriesId, log};
use crate::types::{ListingId, MultiCurrency, Releases, RoyaltiesSchedule};
use cennznet_primitives::types::TokenId;
use frame_support::{IterableStorageMap, StoragePrefixedMap, weights::Weight};
use sp_runtime::traits::Zero;
use sp_std::prelude::*;

pub mod v1_storage {
	use super::*;

	#[derive(codec::Encode, codec::Decode, Debug, Clone, PartialEq, scale_info::TypeInfo)]
	pub enum MetadataBaseURI {
		Ipfs,
		Https(Vec<u8>),
	}

	/// A type of NFT sale listing
	#[derive(Debug, Clone, codec::Encode, codec::Decode, PartialEq, Eq, scale_info::TypeInfo)]
	#[scale_info(skip_type_params(T))]
	pub enum Listing<T: Config> {
		FixedPrice(FixedPriceListing<T>),
		Auction(AuctionListing<T>),
	}

	/// Information about an auction listing v1
	#[derive(Debug, Clone, codec::Encode, codec::Decode, PartialEq, Eq, scale_info::TypeInfo)]
	#[scale_info(skip_type_params(T))]
	pub struct AuctionListing<T: Config> {
		/// The asset to allow bids with
		pub payment_asset: <<T as Config>::MultiCurrency as MultiCurrency>::CurrencyId,
		/// The threshold amount for a succesful bid
		pub reserve_price: <<T as Config>::MultiCurrency as MultiCurrency>::Balance,
		/// When the listing closes
		pub close: T::BlockNumber,
		/// The seller of the tokens
		pub seller: T::AccountId,
		/// The token Ids for sale in this listing
		pub tokens: Vec<TokenId>,
		/// The royalties applicable to this auction
		pub royalties_schedule: RoyaltiesSchedule<T::AccountId>,
	}

	/// Information about a fixed price listing v1
	#[derive(Debug, Clone, codec::Encode, codec::Decode, PartialEq, Eq, scale_info::TypeInfo)]
	#[scale_info(skip_type_params(T))]
	pub struct FixedPriceListing<T: Config> {
		/// The asset to allow bids with
		pub payment_asset: <<T as Config>::MultiCurrency as MultiCurrency>::CurrencyId,
		/// The requested amount for a succesful sale
		pub fixed_price: <<T as Config>::MultiCurrency as MultiCurrency>::Balance,
		/// When the listing closes
		pub close: T::BlockNumber,
		/// The authorised buyer. If unset, any buyer is authorised
		pub buyer: Option<T::AccountId>,
		/// The seller of the tokens
		pub seller: T::AccountId,
		/// The token Ids for sale in this listing
		pub tokens: Vec<TokenId>,
		/// The royalties applicable to this sale
		pub royalties_schedule: RoyaltiesSchedule<T::AccountId>,
	}

	frame_support::decl_storage! {
		trait Store for Module<T: Config> as Nft {
			pub IsSingleIssue get(fn is_single_issue): double_map hasher(twox_64_concat) CollectionId, hasher(twox_64_concat) SeriesId => bool;
			pub CollectionMetadataURI get(fn collection_metadata_uri): map hasher(twox_64_concat) CollectionId => Option<MetadataBaseURI>;
			pub SeriesMetadataURI get(fn series_metadata_uri): double_map hasher(twox_64_concat) CollectionId, hasher(twox_64_concat) SeriesId => Option<Vec<u8>>;
			pub Listings get(fn listings): map hasher(twox_64_concat) ListingId => Option<Listing<T>>;
		}
	}

	frame_support::decl_module! {
		pub struct Module<T: Config> for enum Call where origin: T::Origin { }
	}
}

pub fn migrate_to_v2<T: Config>() -> Weight {
	if StorageVersion::<T>::get() == Releases::V1 as u32 {
		StorageVersion::<T>::put(Releases::V2 as u32);
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
				}) => Listing::<T>::FixedPrice(FixedPriceListing {
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
				}) => Listing::<T>::Auction(AuctionListing {
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
