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

//! NFT module types

use crate::Config;
use cennznet_primitives::types::{AssetId, Balance, BlockNumber};
use codec::{Decode, Encode};
use crml_support::MultiCurrency;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize, Serializer};
use sp_runtime::{PerThing, Permill};
use sp_std::prelude::*;
// Counts enum variants at compile time
use variant_count::VariantCount;

// Time before auction ends that auction is extended if a bid is placed
pub const AUCTION_EXTENSION_PERIOD: BlockNumber = 40;

/// Denotes the metadata URI referencing scheme used by a series
/// Enable token metadata URI construction by clients
#[derive(Decode, Encode, Debug, Clone, PartialEq)]
pub enum MetadataScheme {
	/// Series metadata is hosted by an HTTPS server
	/// Inner value is the URI without trailing '/'
	/// full metadata URI construction: `https://<domain>/<path+>/<serial_number>.json`
	/// Https(b"example.com/metadata")
	///
	Https(Vec<u8>),
	/// Series metadata is hosted by an IPFS directory
	/// Inner value is the directory's IPFS CID
	/// full metadata URI construction: `ipfs://<directory_CID>/<serial_number>.json`
	/// IpfsDir(b"bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi")
	IpfsDir(Vec<u8>),
}

/// Name of an NFT attribute
pub type NFTAttributeName = Vec<u8>;

/// Type Id of an NFT attribute
pub type NFTAttributeTypeId = u8;

/// Describes the data structure of an NFT class (attribute name, attribute type)
pub type NFTSchema = Vec<(NFTAttributeName, NFTAttributeTypeId)>;

/// Contains information of a collection (collection name, collection owner, royalties)
#[derive(Default, Debug, Clone, Encode, Decode, PartialEq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct CollectionInfo<AccountId> {
	#[cfg_attr(feature = "std", serde(serialize_with = "serialize_utf8"))]
	pub name: CollectionNameType,
	pub owner: AccountId,
	#[cfg_attr(feature = "std", serde(serialize_with = "serialize_royalties"))]
	pub royalties: Vec<(AccountId, Permill)>,
}

#[cfg(feature = "std")]
pub fn serialize_utf8<S: Serializer>(v: &Vec<u8>, s: S) -> Result<S::Ok, S::Error> {
	let base64_str = core::str::from_utf8(v).map_err(|_| serde::ser::Error::custom("Byte vec not UTF-8"))?;
	s.serialize_str(&base64_str)
}

#[cfg(feature = "std")]
pub fn serialize_royalties<S: Serializer, AccountId: Serialize>(
	royalties: &Vec<(AccountId, Permill)>,
	s: S,
) -> Result<S::Ok, S::Error> {
	let royalties: Vec<(&AccountId, String)> = royalties
		.iter()
		.map(|(account_id, per_mill)| {
			let per_mill = format!("{:.6}", per_mill.deconstruct() as f32 / 1000000f32);
			(account_id, per_mill)
		})
		.collect();
	royalties.serialize(s)
}

/// Contains information for a particular token. Returns the attributes and owner
#[derive(Eq, PartialEq, Decode, Encode, Default, Debug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct TokenInfo<AccountId> {
	pub attributes: Vec<NFTAttributeValue>,
	pub owner: AccountId,
	#[cfg_attr(feature = "std", serde(serialize_with = "serialize_royalties"))]
	pub royalties: Vec<(AccountId, Permill)>,
}

/// Reason for an NFT being locked (un-transferrable)
#[derive(Decode, Encode, Debug, Clone, Eq, PartialEq)]
pub enum TokenLockReason {
	/// Token is listed for sale
	Listed(ListingId),
}

/// The supported attribute data types for an NFT
#[derive(Decode, Encode, Debug, Clone, Eq, PartialEq, VariantCount)]
#[cfg_attr(feature = "std", derive(Deserialize))]
pub enum NFTAttributeValue {
	I32(i32),
	U8(u8),
	U16(u16),
	U32(u32),
	U64(u64),
	U128(u128),
	Bytes32([u8; 32]),
	// the following are nice aliases for other common attribute types
	// which give some hints to consumers about their intended use
	/// attribute is a byte stream
	Bytes(Vec<u8>),
	// attribute is a string
	String(Vec<u8>),
	/// attribute is a hash value
	Hash([u8; 32]),
	/// attribute is a timestamp (unix)
	Timestamp(u64),
	/// attribute is a stringified URL
	Url(Vec<u8>),
}

#[cfg(feature = "std")]
impl Serialize for NFTAttributeValue {
	fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
		match self {
			Self::I32(val) => s.serialize_i32(*val),
			Self::U8(val) => s.serialize_u8(*val),
			Self::U16(val) => s.serialize_u16(*val),
			Self::U32(val) => s.serialize_u32(*val),
			Self::U64(val) | Self::Timestamp(val) => s.serialize_u64(*val),
			Self::U128(val) => format!("{}", *val).serialize(s),
			Self::Bytes32(val) | Self::Hash(val) => {
				let val_str = format!("0x{}", hex::encode(val));
				s.serialize_str(&val_str)
			}
			Self::String(val) | Self::Url(val) => {
				let val_str = core::str::from_utf8(val).map_err(|_| serde::ser::Error::custom("Byte vec not UTF-8"))?;
				s.serialize_str(&val_str)
			}
			Self::Bytes(val) => {
				let val_str = format!("0x{}", hex::encode(val));
				s.serialize_str(&val_str)
			}
		}
	}
}

impl NFTAttributeValue {
	/// Return the type ID of this attribute value
	pub const fn type_id(&self) -> NFTAttributeTypeId {
		match self {
			NFTAttributeValue::I32(_) => 0,
			NFTAttributeValue::U8(_) => 1,
			NFTAttributeValue::U16(_) => 2,
			NFTAttributeValue::U32(_) => 3,
			NFTAttributeValue::U64(_) => 4,
			NFTAttributeValue::U128(_) => 5,
			NFTAttributeValue::Bytes32(_) => 6,
			NFTAttributeValue::Bytes(_) => 7,
			NFTAttributeValue::String(_) => 8,
			NFTAttributeValue::Hash(_) => 9,
			NFTAttributeValue::Timestamp(_) => 10,
			NFTAttributeValue::Url(_) => 11,
		}
	}
	/// Return whether the given `type_id` is valid to describe an `NFTAttribute`
	pub const fn is_valid_type_id(type_id: NFTAttributeTypeId) -> bool {
		type_id < (Self::VARIANT_COUNT as u8)
	}
	/// Return the byte length of the attribute value, if it exists
	pub fn len(&self) -> usize {
		match self {
			NFTAttributeValue::I32(_) => 4,
			NFTAttributeValue::U8(_) => 1,
			NFTAttributeValue::U16(_) => 2,
			NFTAttributeValue::U32(_) => 4,
			NFTAttributeValue::U64(_) => 8,
			NFTAttributeValue::U128(_) => 16,
			NFTAttributeValue::Bytes32(_) => 32,
			NFTAttributeValue::Bytes(b) => b.len(),
			NFTAttributeValue::String(s) => s.len(),
			NFTAttributeValue::Hash(_) => 32,
			NFTAttributeValue::Timestamp(_) => 8,
			NFTAttributeValue::Url(u) => u.len(),
		}
	}
}

/// The max. number of entitlements any royalties schedule can have
/// just a sensible upper bound
pub(crate) const MAX_ENTITLEMENTS: usize = 8;

/// Reasons for an auction closure
#[derive(Decode, Encode, Debug, Clone, PartialEq, Eq)]
pub enum AuctionClosureReason {
	/// Auction expired with no bids
	ExpiredNoBids,
	/// Auction should have happened but settlement failed due to payment issues
	SettlementFailed,
	/// Auction was cancelled by the vendor
	VendorCancelled,
}

/// Describes the royalty scheme for secondary sales for an NFT collection/token
#[derive(Default, Debug, Clone, Encode, Decode, PartialEq, Eq)]
pub struct RoyaltiesSchedule<AccountId> {
	/// Entitlements on all secondary sales, (beneficiary, % of sale price)
	pub entitlements: Vec<(AccountId, Permill)>,
}

impl<AccountId> RoyaltiesSchedule<AccountId> {
	/// True if entitlements are within valid parameters
	/// - not overcommitted (> 100%)
	/// - < MAX_ENTITLEMENTS
	pub fn validate(&self) -> bool {
		!self.entitlements.is_empty()
			&& self.entitlements.len() <= MAX_ENTITLEMENTS
			&& self
				.entitlements
				.iter()
				.map(|(_who, share)| share.deconstruct() as u32)
				.sum::<u32>() <= Permill::ACCURACY
	}
	/// Calculate the total % entitled for royalties
	/// It will return `0` if the `entitlements` are overcommitted
	pub fn calculate_total_entitlement(&self) -> Permill {
		// if royalties are in a strange state
		if !self.validate() {
			return Permill::zero();
		}
		Permill::from_parts(
			self.entitlements
				.iter()
				.map(|(_who, share)| share.deconstruct())
				.sum::<u32>(),
		)
	}
}

/// The listing response and cursor returned with the RPC getCollectionListing
#[derive(Decode, Encode, Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct ListingResponseWrapper<AccountId> {
	// List of listings to be returned
	pub listings: Vec<ListingResponse<AccountId>>,
	// Cursor pointing to next listing in the series
	#[cfg_attr(feature = "std", serde(serialize_with = "serialize_u128_option"))]
	pub new_cursor: Option<u128>,
}

/// A type to encapsulate both auction listings and fixed price listings for RPC getCollectionListing
#[derive(Decode, Encode, Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct ListingResponse<AccountId> {
	#[cfg_attr(feature = "std", serde(serialize_with = "serialize_u128"))]
	pub id: ListingId,
	#[cfg_attr(feature = "std", serde(serialize_with = "serialize_utf8"))]
	pub listing_type: Vec<u8>,
	pub payment_asset: AssetId,
	#[cfg_attr(feature = "std", serde(serialize_with = "serialize_u128"))]
	pub price: Balance,
	pub end_block: BlockNumber,
	pub buyer: Option<AccountId>,
	pub seller: AccountId,
	pub token_ids: Vec<TokenId>,
	#[cfg_attr(feature = "std", serde(serialize_with = "serialize_royalties"))]
	pub royalties: Vec<(AccountId, Permill)>,
}

#[cfg(feature = "std")]
pub fn serialize_u128<S: Serializer>(val: &u128, s: S) -> Result<S::Ok, S::Error> {
	format!("{}", *val).serialize(s)
}

#[cfg(feature = "std")]
pub fn serialize_u128_option<S: Serializer>(val: &Option<u128>, s: S) -> Result<S::Ok, S::Error> {
	match val {
		Some(v) => format!("{}", *v).serialize(s),
		None => s.serialize_unit(),
	}
}

/// A type of NFT sale listing
#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq)]
pub enum Listing<T: Config> {
	FixedPrice(FixedPriceListing<T>),
	Auction(AuctionListing<T>),
}

/// Information about a marketplace
#[derive(Debug, Clone, Default, Encode, Decode, PartialEq, Eq)]
pub struct Marketplace<AccountId> {
	/// The marketplace account
	pub account: AccountId,
	/// Royalties to go to the marketplace
	pub entitlement: Permill,
}

/// Information about an auction listing
#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq)]
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
	/// The marketplace this is being sold on
	pub marketplace_id: Option<MarketplaceId>,
}

/// Information about a fixed price listing
#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq)]
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
	/// The marketplace this is being sold on
	pub marketplace_id: Option<MarketplaceId>,
}

/// Auto-incrementing Uint
/// Uniquely identifies a collection
pub type CollectionId = u32;

/// NFT colleciton moniker
pub type CollectionNameType = Vec<u8>;

/// Auto-incrementing Uint
/// Uniquely identifies a series of tokens within a collection
pub type SeriesId = u32;

/// Auto-incrementing Uint
/// Uniquely identifies a registered marketplace
pub type MarketplaceId = u32;

/// Auto-incrementing Uint
/// Uniquely identifies a token within a series
pub type SerialNumber = u32;

/// Unique Id for a listing
pub type ListingId = u128;

/// Denotes a quantitiy of tokens
pub type TokenCount = SerialNumber;

/// Global unique token identifier
pub type TokenId = (CollectionId, SeriesId, SerialNumber);

// A value placed in storage that represents the current version of the NFT storage. This value
// is used by the `on_runtime_upgrade` logic to determine whether we run storage migration logic.
// This should match directly with the semantic versions of the Rust crate.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq)]
pub enum Releases {
	/// storage version pre-runtime v41
	V0 = 0,
	/// storage version > runtime v41
	V1 = 1,
	// storage version > runtime v46
	V2 = 2,
}

#[cfg(test)]
mod test {
	use super::{CollectionInfo, ListingResponse, NFTAttributeValue, RoyaltiesSchedule, TokenId, TokenInfo};
	use crate::mock::{AccountId, ExtBuilder};
	use serde_json;
	use sp_runtime::Permill;

	#[test]
	fn valid_type_id_range() {
		// every value < `VARIANT_COUNT` is valid by definition
		assert!((0..NFTAttributeValue::VARIANT_COUNT as u8).all(|id| NFTAttributeValue::is_valid_type_id(id)));
		// every value >= `VARIANT_COUNT` is invalid by definition
		assert!((NFTAttributeValue::VARIANT_COUNT as u8..u8::max_value())
			.all(|id| !NFTAttributeValue::is_valid_type_id(id)));
	}

	#[test]
	fn valid_royalties_plan() {
		assert!(RoyaltiesSchedule::<u32> {
			entitlements: vec![(1_u32, Permill::from_fraction(0.1))],
		}
		.validate());

		// explicitally specifying zero royalties is odd but fine
		assert!(RoyaltiesSchedule::<u32> {
			entitlements: vec![(1_u32, Permill::from_fraction(0.0))],
		}
		.validate());

		let plan = RoyaltiesSchedule::<u32> {
			entitlements: vec![
				(1_u32, Permill::from_fraction(1.01)), // saturates at 100%
			],
		};
		assert_eq!(plan.entitlements[0].1, Permill::one());
		assert!(plan.validate());
	}

	#[test]
	fn invalid_royalties_plan() {
		// overcommits > 100% to royalties
		assert!(!RoyaltiesSchedule::<u32> {
			entitlements: vec![
				(1_u32, Permill::from_fraction(0.2)),
				(2_u32, Permill::from_fraction(0.81)),
			],
		}
		.validate());
	}

	#[test]
	fn collection_info_should_serialize() {
		ExtBuilder::default().build().execute_with(|| {
			let collection_name = b"test-collection".to_vec();
			let collection_owner = 1_u64;
			let royalties = RoyaltiesSchedule::<AccountId> {
				entitlements: vec![
					(3_u64, Permill::from_fraction(0.2)),
					(4_u64, Permill::from_fraction(0.3)),
				],
			};
			let collection_info = CollectionInfo {
				name: collection_name,
				owner: collection_owner,
				royalties: royalties.entitlements,
			};
			let json_str = "{\
				\"name\":\"test-collection\",\
				\"owner\":1,\
				\"royalties\":[\
					[\
						3,\
						\"0.200000\"\
					],\
					[\
						4,\
						\"0.300000\"\
					]\
				]\
			}";

			assert_eq!(serde_json::to_string(&collection_info).unwrap(), json_str);
		});
	}

	#[test]
	fn token_info_should_serialize() {
		ExtBuilder::default().build().execute_with(|| {
			let collection_owner = 1_u64;
			let royalties = RoyaltiesSchedule::<AccountId> {
				entitlements: vec![(3_u64, Permill::from_fraction(0.2))],
			};
			let series_attributes = vec![
				NFTAttributeValue::I32(500),
				NFTAttributeValue::U8(100),
				NFTAttributeValue::U16(500),
				NFTAttributeValue::U32(500),
				NFTAttributeValue::U64(500),
				NFTAttributeValue::U128(500),
				NFTAttributeValue::Bytes32([0x55; 32]),
				NFTAttributeValue::Bytes(hex::decode("5000").unwrap()),
				NFTAttributeValue::String(Vec::from("Test")),
				NFTAttributeValue::Hash([0x55; 32]),
				NFTAttributeValue::Timestamp(500),
				NFTAttributeValue::Url(Vec::from("www.centrality.ai")),
			];

			let token_info = TokenInfo {
				attributes: series_attributes,
				owner: collection_owner,
				royalties: royalties.entitlements,
			};

			let json_str = "{\
				\"attributes\":[\
				500,\
				100,\
				500,\
				500,\
				500,\
				\"500\",\
				\"0x5555555555555555555555555555555555555555555555555555555555555555\",\
				\"0x5000\",\
				\"Test\",\
				\"0x5555555555555555555555555555555555555555555555555555555555555555\",\
				500,\
				\"www.centrality.ai\"],\
				\"owner\":1,\
				\"royalties\":[\
					[\
						3,\
						\"0.200000\"\
					]\
				]\
			}";

			assert_eq!(serde_json::to_string(&token_info).unwrap(), json_str);
		});
	}

	#[test]
	fn collection_listings_should_serialize() {
		ExtBuilder::default().build().execute_with(|| {
			let collection_owner = 1_u64;
			let buyer = 2_u64;
			let royalties = RoyaltiesSchedule::<AccountId> {
				entitlements: vec![(3_u64, Permill::from_fraction(0.2))],
			};
			let token_id: TokenId = (0, 0, 0);

			let listing_response = ListingResponse {
				id: 10,
				listing_type: "fixedPrice".as_bytes().to_vec(),
				payment_asset: 10,
				price: 10,
				end_block: 10,
				buyer: Some(buyer),
				seller: collection_owner,
				royalties: royalties.entitlements,
				token_ids: vec![token_id],
			};

			let json_str = "{\
			\"id\":\"10\",\
			\"listing_type\":\"fixedPrice\",\
			\"payment_asset\":10,\
			\"price\":\"10\",\
			\"end_block\":10,\
			\"buyer\":2,\
			\"seller\":1,\
			\"token_ids\":[[0,0,0]],\
			\"royalties\":[[3,\"0.200000\"]]}\
			";

			assert_eq!(serde_json::to_string(&listing_response).unwrap(), json_str);
		});
	}
}
