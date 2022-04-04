#[allow(dead_code)]
pub mod v1_storage {
	use crate::{CollectionId, Config, ListingId, MultiCurrency, RoyaltiesSchedule, SeriesId, TokenId};
	use codec::{Decode, Encode};
	use scale_info::TypeInfo;
	use sp_std::prelude::*;

	#[derive(Decode, Encode, Debug, Clone, PartialEq, TypeInfo)]
	pub enum MetadataBaseURI {
		Ipfs,
		Https(Vec<u8>),
	}

	/// A type of NFT sale listing
	#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	pub enum Listing<T: Config> {
		FixedPrice(FixedPriceListing<T>),
		Auction(AuctionListing<T>),
	}

	/// Information about an auction listing v1
	#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq, TypeInfo)]
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
	#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq, TypeInfo)]
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

	pub struct Module<T>(sp_std::marker::PhantomData<T>);
	frame_support::decl_storage! {
		trait Store for Module<T: Config> as Nft {
			pub IsSingleIssue get(fn is_single_issue): double_map hasher(twox_64_concat) CollectionId, hasher(twox_64_concat) SeriesId => bool;
			pub CollectionMetadataURI get(fn collection_metadata_uri): map hasher(twox_64_concat) CollectionId => Option<MetadataBaseURI>;
			pub SeriesMetadataURI get(fn series_metadata_uri): double_map hasher(twox_64_concat) CollectionId, hasher(twox_64_concat) SeriesId => Option<Vec<u8>>;
			pub Listings get(fn listings): map hasher(twox_64_concat) ListingId => Option<Listing<T>>;
		}
	}
}
