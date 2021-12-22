//! Generic Asset Types

use codec::{Decode, Encode, Error as CodecError, HasCompact, Input, MaxEncodedLen, Output};
use frame_support::traits::{LockIdentifier, WithdrawReasons};
use scale_info::{Type, TypeDefPrimitive, TypeInfo};
use sp_runtime::RuntimeDebug;
use sp_std::{ops::BitOr, prelude::*, vec};

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct BalanceLockOld<Balance> {
	pub id: LockIdentifier,
	pub amount: Balance,
	pub reasons: WithdrawReasons,
}

/// `TypeInfo` can't be derived for the `WithdrawReasons` type automatically.
/// This is a nonsense implementation to allow the `decl_storage!` macro to compile
/// for the lock storage migration
/// TODO: remove after runtime update to > v46 versions
impl<Balance: 'static> TypeInfo for BalanceLockOld<Balance> {
	type Identity = Self;
	fn type_info() -> Type {
		TypeDefPrimitive::U8.into()
	}
}

impl<Balance> BalanceLockOld<Balance> {
	/// Upgrade to new balance lock
	/// `WithdrawReasons` updated to `Reasons`
	pub fn upgrade(self) -> BalanceLock<Balance> {
		BalanceLock {
			id: self.id,
			amount: self.amount,
			reasons: self.reasons.into(),
		}
	}
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub struct BalanceLock<Balance> {
	pub id: LockIdentifier,
	pub amount: Balance,
	pub reasons: Reasons,
}

/// Simplified reasons for withdrawing balance.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub enum Reasons {
	/// Paying system transaction fees.
	Fee = 0,
	/// Any reason other than paying system transaction fees.
	Misc = 1,
	/// Any reason at all.
	All = 2,
}

impl From<WithdrawReasons> for Reasons {
	fn from(r: WithdrawReasons) -> Reasons {
		if r == WithdrawReasons::from(WithdrawReasons::TRANSACTION_PAYMENT) {
			Reasons::Fee
		} else if r.contains(WithdrawReasons::TRANSACTION_PAYMENT) {
			Reasons::All
		} else {
			Reasons::Misc
		}
	}
}

impl BitOr for Reasons {
	type Output = Reasons;
	fn bitor(self, other: Reasons) -> Reasons {
		if self == other {
			return self;
		}
		Reasons::All
	}
}

/// Asset Metadata
#[derive(Encode, Decode, PartialEq, Eq, Clone, RuntimeDebug, TypeInfo)]
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
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub struct AssetOptions<Balance: HasCompact, AccountId> {
	/// Initial number of whole tokens to be issued. All deposited to the creator of the asset.
	#[codec(compact)]
	pub initial_issuance: Balance,
	/// Which accounts are allowed to possess this asset.
	pub permissions: PermissionLatest<AccountId>,
}

/// Owner of an asset.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo)]
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

//All balances belonging to account
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub struct AllBalances<Balance> {
	/// Reserved balance
	pub reserved: Balance,
	///Staked balance (Locked)
	pub staked: Balance,
	/// Available balance (Free - staked)
	pub available: Balance,
}

/// Asset permissions
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo)]
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

#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo)]
#[repr(u8)]
enum PermissionVersionNumber {
	V1 = 0,
}

/// Versioned asset permission
#[derive(Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
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
