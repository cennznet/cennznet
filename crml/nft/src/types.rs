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

use crate::Trait;
use codec::{Decode, Encode};
use prml_support::MultiCurrencyAccounting;
use sp_runtime::Percent;
use sp_std::prelude::*;
// Counts enum variants at compile time
use variant_count::VariantCount;

/// String Id for an NFT collection
/// limited to 32 utf-8 bytes in practice
pub type CollectionId = Vec<u8>;

/// Name of an NFT attribute
pub type NFTAttributeName = Vec<u8>;

/// Type Id of an NFT attribute
pub type NFTAttributeTypeId = u8;

/// A single data field of an NFT
#[derive(Decode, Encode, Debug, Clone, PartialEq)]
pub struct NFTAttribute {
	/// name of the attribute
	pub name: NFTAttributeName,
	/// value of the attribute
	pub value: NFTAttributeValue,
}

/// Describes the data structure of an NFT class (attribute name, attribute type)
pub type NFTSchema = Vec<(NFTAttributeName, NFTAttributeTypeId)>;

/// The supported attribute data types for an NFT
#[derive(Decode, Encode, Debug, Clone, PartialEq, VariantCount)]
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
	/// Return a new `NFTAttributeValue` with the default value for the given type id.
	/// It will fail if `type_id` is invalid
	pub const fn default_from_type_id(type_id: NFTAttributeTypeId) -> Result<NFTAttributeValue, ()> {
		if !Self::is_valid_type_id(type_id) {
			return Err(());
		}
		match type_id {
			0 => Ok(NFTAttributeValue::I32(0)),
			1 => Ok(NFTAttributeValue::U8(0)),
			2 => Ok(NFTAttributeValue::U16(0)),
			3 => Ok(NFTAttributeValue::U32(0)),
			4 => Ok(NFTAttributeValue::U64(0)),
			5 => Ok(NFTAttributeValue::U128(0)),
			6 => Ok(NFTAttributeValue::Bytes32([0_u8; 32])),
			7 => Ok(NFTAttributeValue::Bytes(vec![])),
			8 => Ok(NFTAttributeValue::String(vec![])),
			9 => Ok(NFTAttributeValue::Hash([0_u8; 32])),
			10 => Ok(NFTAttributeValue::Timestamp(0)),
			11 => Ok(NFTAttributeValue::Url(vec![])),
			_ => Err(()),
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

/// Describes the royalty scheme for secondary sales for an NFT collection/token
#[derive(Default, Debug, Clone, Encode, Decode, PartialEq)]
pub struct RoyaltiesSchedule<AccountId> {
	/// Entitlements on all secondary sales, who and % of the sale price
	pub entitlements: Vec<(AccountId, Percent)>,
}

impl<AccountId> RoyaltiesSchedule<AccountId> {
	/// Validate total royalties are within 100%
	pub fn validate(&self) -> bool {
		self.entitlements.is_empty()
			|| self
				.entitlements
				.iter()
				.map(|(_who, share)| share.deconstruct() as u32)
				.sum::<u32>() <= 100_u32
	}
	/// Calculate the total % entitled for royalties
	/// It will return `0` if the `entitlements` are overcommitted
	pub fn calculate_total_entitlement(&self) -> Percent {
		// if royalties are in a strange state
		if !self.validate() {
			return Percent::zero();
		}
		Percent::from_parts(
			self.entitlements
				.iter()
				.map(|(_who, share)| share.deconstruct())
				.sum::<u8>(),
		)
	}
}

/// A type of NFT sale listing
#[derive(Debug, Clone, Encode, Decode, PartialEq)]
pub enum Listing<T: Trait> {
	DirectSale(DirectSaleListing<T>),
	Auction(AuctionListing<T>),
}

/// Information about an auction listing
#[derive(Debug, Clone, Encode, Decode, PartialEq)]
pub struct AuctionListing<T: Trait> {
	/// The asset to allow bids with
	pub payment_asset: <<T as Trait>::MultiCurrency as MultiCurrencyAccounting>::CurrencyId,
	/// The threshold amount for a succesful bid
	pub reserve_price: <<T as Trait>::MultiCurrency as MultiCurrencyAccounting>::Balance,
	/// When the listing closes
	pub close: T::BlockNumber,
}

/// Information about a fixed price listing
#[derive(Debug, Clone, Encode, Decode, PartialEq)]
pub struct DirectSaleListing<T: Trait> {
	/// The asset to allow bids with
	pub payment_asset: <<T as Trait>::MultiCurrency as MultiCurrencyAccounting>::CurrencyId,
	/// The requested amount for a succesful sale
	pub fixed_price: <<T as Trait>::MultiCurrency as MultiCurrencyAccounting>::Balance,
	/// When the listing closes
	pub close: T::BlockNumber,
	/// authorised buyer
	pub buyer: T::AccountId,
}

#[cfg(test)]
mod test {
	use super::{NFTAttributeValue, RoyaltiesSchedule};
	use sp_runtime::Percent;

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
			entitlements: vec![(1_u32, Percent::from_fraction(0.1))],
		}
		.validate());

		// explicitally specifying zero royalties is odd but fine
		assert!(RoyaltiesSchedule::<u32> {
			entitlements: vec![(1_u32, Percent::from_fraction(0.0))],
		}
		.validate());

		let plan = RoyaltiesSchedule::<u32> {
			entitlements: vec![
				(1_u32, Percent::from_fraction(1.01)), // saturates at 100%
			],
		};
		assert_eq!(plan.entitlements[0].1, Percent::one());
		assert!(plan.validate());
	}

	#[test]
	fn invalid_royalties_plan() {
		// overcommits > 100% to royalties
		assert!(!RoyaltiesSchedule::<u32> {
			entitlements: vec![
				(1_u32, Percent::from_fraction(0.2)),
				(2_u32, Percent::from_fraction(0.81)),
			],
		}
		.validate());
	}
}
