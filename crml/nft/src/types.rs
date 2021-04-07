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

/// Type Id of an NFTField
pub type NFTFieldTypeId = u8;

/// Describes the data structure of an NFT class
pub type NFTSchema = Vec<NFTFieldTypeId>;

/// String Id for an NFT collection
/// limited to 32 utf-8 bytes in practice
pub type CollectionId = Vec<u8>;

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
	pub fn calculate_total_entitlement(&self) -> Percent {
		Percent::from_fraction(
			self.entitlements
				.iter()
				.map(|(_who, share)| share.deconstruct() as u32)
				.sum::<u32>() as f64,
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

/// The supported data types for an NFT
#[derive(Decode, Encode, Debug, Copy, Clone, PartialEq, VariantCount)]
pub enum NFTField {
	I32(i32),
	U8(u8),
	U16(u16),
	U32(u32),
	U64(u64),
	U128(u128),
	Bytes32([u8; 32]),
}

impl NFTField {
	/// Return the type ID of this field
	pub const fn type_id(&self) -> NFTFieldTypeId {
		match self {
			NFTField::I32(_) => 0,
			NFTField::U8(_) => 1,
			NFTField::U16(_) => 2,
			NFTField::U32(_) => 3,
			NFTField::U64(_) => 4,
			NFTField::U128(_) => 5,
			NFTField::Bytes32(_) => 6,
		}
	}
	/// Return a new `NFTField` with the default value for the given type id.
	/// It will fail if `type_id` is invalid
	pub const fn default_from_type_id(type_id: NFTFieldTypeId) -> Result<NFTField, ()> {
		if !Self::is_valid_type_id(type_id) {
			return Err(());
		}
		match type_id {
			0 => Ok(NFTField::I32(0)),
			1 => Ok(NFTField::U8(0)),
			2 => Ok(NFTField::U16(0)),
			3 => Ok(NFTField::U32(0)),
			4 => Ok(NFTField::U64(0)),
			5 => Ok(NFTField::U128(0)),
			6 => Ok(NFTField::Bytes32([0_u8; 32])),
			_ => Err(()),
		}
	}
	/// Return whether the given `type_id` is valid to describe an `NFTField`
	pub const fn is_valid_type_id(type_id: NFTFieldTypeId) -> bool {
		type_id < (Self::VARIANT_COUNT as u8)
	}
}

#[cfg(test)]
mod test {
	use super::{NFTField, RoyaltiesSchedule};
	use sp_runtime::Percent;

	#[test]
	fn valid_type_id_range() {
		// every value < `VARIANT_COUNT` is valid by definition
		assert!((0..NFTField::VARIANT_COUNT as u8).all(|id| NFTField::is_valid_type_id(id)));
		// every value >= `VARIANT_COUNT` is invalid by definition
		assert!((NFTField::VARIANT_COUNT as u8..u8::max_value()).all(|id| !NFTField::is_valid_type_id(id)));
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
