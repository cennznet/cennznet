// Copyright 2020-2021 Plug New Zealand Limited & Centrality Investments Limited
// This file is part of Plug.

// Plug is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Plug is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Plug.  If not, see <http://www.gnu.org/licenses/>.

#![cfg_attr(not(feature = "std"), no_std)]

//! # Common crml types and traits

// Note: in the following traits the terms:
// - 'token' / 'asset' / 'currency' and
// - 'balance' / 'value' / 'amount'
// are used interchangeably as they make more sense in certain contexts.
use frame_support::traits::{ExistenceRequirement, Imbalance, SignedImbalance, WithdrawReasons};
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, MaybeSerializeDeserialize, Saturating},
	DispatchError, DispatchResult,
};
use sp_std::{fmt::Debug, prelude::*, result};

/// Something which provides an ID with authority from chain storage
pub trait AssetIdAuthority {
	/// The asset ID type e.g a `u32`
	type AssetId;
	/// Return the authoritative asset ID (no `&self`)
	fn asset_id() -> Self::AssetId;
}

/// An abstraction over the accounting behaviour of a fungible, multi-currency system
/// Currencies in the system are identifiable by a unique `CurrencyId`
pub trait MultiCurrency {
	/// The ID type for an account in the system
	type AccountId: Debug + Default;
	/// The balance of an account for a particular currency
	type Balance: AtLeast32BitUnsigned + Copy + MaybeSerializeDeserialize + Debug + Default + Saturating;
	/// The ID type of a currency in the system
	type CurrencyId: Debug + Default;
	/// The opaque token type for an imbalance of a particular currency. This is returned by unbalanced operations
	/// and must be dealt with. It may be dropped but cannot be cloned.
	type NegativeImbalance: Imbalance<Self::Balance, Opposite = Self::PositiveImbalance>;
	/// The opaque token type for an imbalance of a particular currency. This is returned by unbalanced operations
	/// and must be dealt with. It may be dropped but cannot be cloned.
	type PositiveImbalance: Imbalance<Self::Balance, Opposite = Self::NegativeImbalance>;

	// PUBLIC IMMUTABLES

	/// The minimum balance any single account may have. This is equivalent to the `Balances` module's
	/// `ExistentialDeposit`.
	fn minimum_balance(currency: Self::CurrencyId) -> Self::Balance;

	/// Return the currency Id of the system fee currency
	fn fee_currency() -> Self::CurrencyId;

	/// The combined balance (free + reserved) of `who` for the given `currency`.
	fn total_balance(who: &Self::AccountId, currency: Self::CurrencyId) -> Self::Balance;

	/// The 'free' balance of a given account.
	///
	/// This is the only balance that matters in terms of most operations on tokens. It alone
	/// is used to determine the balance when in the contract execution environment. When this
	/// balance falls below the value of `ExistentialDeposit`, then the 'current account' is
	/// deleted: specifically `FreeBalance`. Further, the `OnFreeBalanceZero` callback
	/// is invoked, giving a chance to external modules to clean up data associated with
	/// the deleted account.
	///
	/// `system::AccountNonce` is also deleted if `ReservedBalance` is also zero (it also gets
	/// collapsed to zero if it ever becomes less than `ExistentialDeposit`.
	fn free_balance(who: &Self::AccountId, currency: Self::CurrencyId) -> Self::Balance;

	/// Returns `Ok` iff the account is able to make a withdrawal of the given amount
	/// for the given reason. Basically, it's just a dry-run of `withdraw`.
	///
	/// `Err(...)` with the reason why not otherwise.
	fn ensure_can_withdraw(
		who: &Self::AccountId,
		currency: Self::CurrencyId,
		_amount: Self::Balance,
		reasons: WithdrawReasons,
		new_balance: Self::Balance,
	) -> DispatchResult;

	// PUBLIC MUTABLES (DANGEROUS)

	/// Adds up to `value` to the free balance of `who`. If `who` doesn't exist, it is created.
	///
	/// Infallible.
	fn deposit_creating(
		who: &Self::AccountId,
		currency: Self::CurrencyId,
		value: Self::Balance,
	) -> Self::PositiveImbalance;

	/// Mints `value` to the free balance of `who`.
	///
	/// If `who` doesn't exist, nothing is done and an Err returned.
	fn deposit_into_existing(
		who: &Self::AccountId,
		currency: Self::CurrencyId,
		value: Self::Balance,
	) -> result::Result<Self::PositiveImbalance, DispatchError>;

	/// Ensure an account's free balance equals some value; this will create the account
	/// if needed.
	///
	/// Returns a signed imbalance and status to indicate if the account was successfully updated or update
	/// has led to killing of the account.
	fn make_free_balance_be(
		who: &Self::AccountId,
		currency: Self::CurrencyId,
		balance: Self::Balance,
	) -> SignedImbalance<Self::Balance, Self::PositiveImbalance>;

	/// Transfer some liquid free balance to another staker.
	///
	/// This is a very high-level function. It will ensure all appropriate fees are paid
	/// and no imbalance in the system remains.
	fn transfer(
		source: &Self::AccountId,
		dest: &Self::AccountId,
		currency: Self::CurrencyId,
		value: Self::Balance,
		existence_requirement: ExistenceRequirement,
	) -> DispatchResult;

	/// Removes some free balance from `who` account for `reason` if possible. If `liveness` is
	/// `KeepAlive`, then no less than `ExistentialDeposit` must be left remaining.
	///
	/// This checks any locks, vesting, and liquidity requirements. If the removal is not possible,
	/// then it returns `Err`.
	///
	/// If the operation is successful, this will return `Ok` with a `NegativeImbalance` whose value
	/// is `value`.
	fn withdraw(
		who: &Self::AccountId,
		currency: Self::CurrencyId,
		value: Self::Balance,
		reasons: WithdrawReasons,
		liveness: ExistenceRequirement,
	) -> result::Result<Self::NegativeImbalance, DispatchError>;

	/// Move `amount` from free balance to reserved balance.
	///
	/// If the free balance is lower than `amount`, then no funds will be moved and an `Err` will
	/// be returned. This is different behavior than `unreserve`.
	fn reserve(who: &Self::AccountId, currency: Self::CurrencyId, amount: Self::Balance) -> DispatchResult;

	/// Move upto `amount` of reserved balance from `who` to the free balance of `beneficiary`.
	fn repatriate_reserved(
		who: &Self::AccountId,
		currency: Self::CurrencyId,
		beneficiary: &Self::AccountId,
		amount: Self::Balance,
	) -> result::Result<Self::Balance, DispatchError>;

	/// Moves up to `amount` from reserved balance to free balance. This function cannot fail.
	///
	/// As many assets up to `amount` will be moved as possible. If the reserve balance of `who`
	/// is less than `amount`, then the remaining amount will be returned.
	fn unreserve(who: &Self::AccountId, currency: Self::CurrencyId, amount: Self::Balance) -> Self::Balance;

	/// Bring a new currency into existence
	/// Returns the new currency Id
	/// `owner` - the asset owner address
	/// `total_supply` - number of whole tokens to mint to `owner`
	/// `decimal_places` - metadata denoting the decimal places for balances of the asset
	/// `minimum_balance` - a minimum balance for an account to exist
	/// `symbol` - ticker for the asset
	fn create(
		owner: &Self::AccountId,
		initial_supply: Self::Balance,
		decimal_places: u8,
		minimum_balance: u64,
		symbol: Vec<u8>,
	) -> Result<Self::CurrencyId, ()>;
}
