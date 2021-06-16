//! Generic Asset Types

use codec::{Decode, Encode, Error as CodecError, HasCompact, Input, Output};
use frame_support::traits::{LockIdentifier, WithdrawReasons};
use sp_runtime::RuntimeDebug;
use sp_std::prelude::*;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct BalanceLock<Balance> {
	pub id: LockIdentifier,
	pub amount: Balance,
	pub reasons: WithdrawReasons,
}

/// Asset Metadata
#[derive(Encode, Decode, PartialEq, Eq, Clone, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct AssetInfo {
	symbol: Vec<u8>,
	decimal_places: u8,
	existential_deposit: u64,
}

impl AssetInfo {
	/// Create a new asset info by specifying its name/symbol and the number of decimal places
	/// in the asset's balance. i.e. balance x 10 ^ -decimals will be the value for display
	pub fn new(symbol: Vec<u8>, decimal_places: u8, existential_deposit: u64) -> Self {
		Self {
			symbol,
			decimal_places,
			existential_deposit,
		}
	}

	pub fn existential_deposit(&self) -> u64 {
		self.existential_deposit
	}

	pub fn decimal_places(&self) -> u8 {
		self.decimal_places
	}
}

impl Default for AssetInfo {
	fn default() -> Self {
		Self {
			symbol: vec![],
			decimal_places: 4,
			existential_deposit: 1,
		}
	}
}

/// Asset creation options.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub struct AssetOptions<Balance: HasCompact, AccountId> {
	/// Initial number of whole tokens to be issued. All deposited to the creator of the asset.
	#[codec(compact)]
	pub initial_issuance: Balance,
	/// Which accounts are allowed to possess this asset.
	pub permissions: PermissionLatest<AccountId>,
}

/// Owner of an asset.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub enum Owner<AccountId> {
	/// No owner.
	None,
	/// Owned by an AccountId
	Address(AccountId),
}

impl<AccountId> Default for Owner<AccountId> {
	fn default() -> Self {
		Owner::None
	}
}

/// Asset permissions
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub struct PermissionsV1<AccountId> {
	/// Who have permission to update asset permission
	pub update: Owner<AccountId>,
	/// Who have permission to mint new asset
	pub mint: Owner<AccountId>,
	/// Who have permission to burn asset
	pub burn: Owner<AccountId>,
}

impl<AccountId: Clone> PermissionsV1<AccountId> {
	/// Create a new `PermissionV1` with all permission to the given `owner`
	pub fn new(owner: AccountId) -> Self {
		Self {
			update: Owner::Address(owner.clone()),
			mint: Owner::Address(owner.clone()),
			burn: Owner::Address(owner),
		}
	}
}

#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
#[repr(u8)]
enum PermissionVersionNumber {
	V1 = 0,
}

/// Versioned asset permission
#[derive(Clone, PartialEq, Eq, RuntimeDebug)]
pub enum PermissionVersions<AccountId> {
	V1(PermissionsV1<AccountId>),
}

/// Asset permission types
pub enum PermissionType {
	/// Permission to burn asset permission
	Burn,
	/// Permission to mint new asset
	Mint,
	/// Permission to update asset
	Update,
}

/// Alias to latest asset permissions
pub type PermissionLatest<AccountId> = PermissionsV1<AccountId>;

impl<AccountId> Default for PermissionVersions<AccountId> {
	fn default() -> Self {
		PermissionVersions::V1(Default::default())
	}
}

impl<AccountId: Encode> Encode for PermissionVersions<AccountId> {
	fn encode_to<T: Output + ?Sized>(&self, dest: &mut T) {
		match self {
			PermissionVersions::V1(payload) => {
				dest.write(&PermissionVersionNumber::V1.encode());
				dest.write(&payload.encode());
			}
		}
	}
}

impl<AccountId: Encode> codec::EncodeLike for PermissionVersions<AccountId> {}

impl<AccountId: Decode> Decode for PermissionVersions<AccountId> {
	fn decode<I: Input>(input: &mut I) -> core::result::Result<Self, CodecError> {
		let version = PermissionVersionNumber::decode(input)?;
		Ok(match version {
			PermissionVersionNumber::V1 => PermissionVersions::V1(Decode::decode(input)?),
		})
	}
}

impl<AccountId> Default for PermissionsV1<AccountId> {
	fn default() -> Self {
		PermissionsV1 {
			update: Owner::None,
			mint: Owner::None,
			burn: Owner::None,
		}
	}
}

impl<AccountId> Into<PermissionLatest<AccountId>> for PermissionVersions<AccountId> {
	fn into(self) -> PermissionLatest<AccountId> {
		match self {
			PermissionVersions::V1(v1) => v1,
		}
	}
}

/// Converts the latest permission to other version.
impl<AccountId> Into<PermissionVersions<AccountId>> for PermissionLatest<AccountId> {
	fn into(self) -> PermissionVersions<AccountId> {
		PermissionVersions::V1(self)
	}
}
