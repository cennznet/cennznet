// Copyright 2018-2020 Parity Technologies (UK) Ltd. and Centrality Investments Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

//! Common types used by CENNZnet node.

use codec::{Decode, Encode, HasCompact};
use sp_runtime::{
	generic,
	traits::{BlakeTwo256, IdentifyAccount, Verify},
	DoughnutV0, MultiSignature, OpaqueExtrinsic, RuntimeDebug,
};
use sp_std::vec::Vec;

/// An index to a block.
pub type BlockNumber = u32;

/// Alias to 512-bit hash when used in the context of a transaction signature on the chain.
pub type Signature = MultiSignature;

/// Some way of identifying an account on the chain. We intentionally make it equivalent
/// to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// The type for looking up accounts. We don't expect more than 4 billion of them, but you
/// never know...
pub type AccountIndex = u32;

/// Balance of an account.
pub type Balance = u128;

/// Asset ID for generic asset module.
pub type AssetId = u32;

/// The runtime supported proof of delegation format.
pub type Doughnut = DoughnutV0;

/// Type used for expressing timestamp.
pub type Moment = u64;

/// Index of a transaction in the chain.
pub type Index = u64;

/// A hash of some data used by the chain.
pub type Hash = sp_core::H256;

/// A timestamp: milliseconds since the unix epoch.
/// `u64` is enough to represent a duration of half a billion years, when the
/// time scale is milliseconds.
pub type Timestamp = u64;

/// Digest item type.
pub type DigestItem = generic::DigestItem<Hash>;
/// Header type.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type.
pub type Block = generic::Block<Header, OpaqueExtrinsic>;
/// Block ID.
pub type BlockId = generic::BlockId<Block>;

/// The outer `FeeExchange` type. It is versioned to provide flexibility for future iterations
/// while maintaining backward compatibility.
#[derive(PartialEq, Eq, Clone, Encode, Decode, Debug)]
pub enum FeeExchange<AssetId, Balance> {
	/// A V1 FeeExchange
	#[codec(compact)]
	V1(FeeExchangeV1<AssetId, Balance>),
}

/// A v1 FeeExchange
/// Signals a fee payment requiring the CENNZX-Spot exchange. It is intended to
/// embed within CENNZnet extrinsic payload.
/// It specifies input asset ID and the max. limit of input asset to pay
#[derive(PartialEq, Eq, Clone, Encode, Decode, Debug)]
pub struct FeeExchangeV1<AssetId, Balance> {
	/// The Asset ID to exchange for network fee asset
	#[codec(compact)]
	pub asset_id: AssetId,
	/// The maximum `asset_id` to pay, given the exchange rate
	#[codec(compact)]
	pub max_payment: Balance,
}

impl<AssetId, Balance> FeeExchangeV1<AssetId, Balance> {
	/// Create a new FeeExchangeV1
	pub fn new(asset_id: AssetId, max_payment: Balance) -> Self {
		Self { asset_id, max_payment }
	}
}

impl<AssetId: Copy, Balance: Copy> FeeExchange<AssetId, Balance> {
	/// Create a `FeeExchangeV1`
	pub fn new_v1(id: AssetId, balance: Balance) -> Self {
		FeeExchange::V1(FeeExchangeV1 {
			asset_id: id,
			max_payment: balance,
		})
	}

	/// Return the nominated fee asset id
	pub fn asset_id(&self) -> AssetId {
		match self {
			FeeExchange::V1(x) => x.asset_id,
		}
	}

	/// Return the max. payment limit
	pub fn max_payment(&self) -> Balance {
		match self {
			FeeExchange::V1(x) => x.max_payment,
		}
	}
}

/// The amount of exposure (to slashing) than an individual nominator has.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Encode, Decode, RuntimeDebug)]
pub struct IndividualExposure<AccountId, Balance: HasCompact> {
	/// The stash account of the nominator in question.
	pub who: AccountId,
	/// Amount of funds exposed.
	#[codec(compact)]
	pub value: Balance,
}

/// A snapshot of the stake backing a single validator in the system.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct Exposure<AccountId, Balance: HasCompact> {
	/// The total balance backing this validator.
	#[codec(compact)]
	pub total: Balance,
	/// The validator's own stash that is exposed.
	#[codec(compact)]
	pub own: Balance,
	/// The portions of nominators stashes that are exposed.
	pub others: Vec<IndividualExposure<AccountId, Balance>>,
}
