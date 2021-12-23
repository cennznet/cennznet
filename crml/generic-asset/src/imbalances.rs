// Copyright 2019-2020 by  Centrality Investments Ltd.
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

//! Imbalances are an elaborate method of automatically managing total issuance of a currency
//! when they are dropped a hook is triggered to update the currency total issuance accordingly.
//! The may be added and subsracted from each other for efficiencies sake.
//!
//! These should only be created through an instance of `Currency` which will provide the correct asset ID
//!

// wrapping these imbalances in a private module is necessary to ensure absolute
// privacy of the inner member.

use crate::{Config, TotalIssuance};
use frame_support::{
	storage::StorageMap,
	traits::{Imbalance, SameOrOther, TryDrop},
};
use sp_runtime::traits::{Saturating, Zero};
use sp_std::{mem, result};

/// Opaque, move-only struct with private fields that serves as a token
/// denoting that funds have been created without any equal and opposite
/// accounting.
#[must_use]
#[derive(Debug, PartialEq)]
pub struct PositiveImbalance<T: Config> {
	amount: T::Balance,
	asset_id: T::AssetId,
}

impl<T: Config> PositiveImbalance<T> {
	/// Create a new positive imbalance from a `balance` and with the given `asset_id`.
	pub fn new(amount: T::Balance, asset_id: T::AssetId) -> Self {
		PositiveImbalance { amount, asset_id }
	}
	pub fn asset_id(&self) -> T::AssetId {
		self.asset_id
	}
}

impl<T: Config> Default for PositiveImbalance<T> {
	fn default() -> Self {
		PositiveImbalance {
			amount: Zero::zero(),
			asset_id: Zero::zero(),
		}
	}
}

/// Opaque, move-only struct with private fields that serves as a token
/// denoting that funds have been destroyed without any equal and opposite
/// accounting.
#[must_use]
#[derive(Debug, PartialEq)]
pub struct NegativeImbalance<T: Config> {
	amount: T::Balance,
	asset_id: T::AssetId,
}

impl<T: Config> Default for NegativeImbalance<T> {
	fn default() -> Self {
		NegativeImbalance {
			amount: Zero::zero(),
			asset_id: Zero::zero(),
		}
	}
}

impl<T: Config> NegativeImbalance<T> {
	/// Create a new negative imbalance from a `balance` and with the given `asset_id`.
	pub fn new(amount: T::Balance, asset_id: T::AssetId) -> Self {
		NegativeImbalance { amount, asset_id }
	}
	pub fn asset_id(&self) -> T::AssetId {
		self.asset_id
	}
}

impl<T: Config> TryDrop for PositiveImbalance<T> {
	fn try_drop(self) -> result::Result<(), Self> {
		self.drop_zero()
	}
}

impl<T: Config> Imbalance<T::Balance> for PositiveImbalance<T> {
	type Opposite = NegativeImbalance<T>;

	fn zero() -> Self {
		Self::new(Zero::zero(), Zero::zero())
	}
	fn drop_zero(self) -> result::Result<(), Self> {
		if self.amount.is_zero() || self.asset_id.is_zero() {
			Ok(())
		} else {
			Err(self)
		}
	}
	fn split(self, amount: T::Balance) -> (Self, Self) {
		let first = self.amount.min(amount);
		let second = self.amount - first;
		let asset_id = self.asset_id;

		mem::forget(self);
		(Self::new(first, asset_id), Self::new(second, asset_id))
	}
	fn merge(mut self, other: Self) -> Self {
		self.amount = self.amount.saturating_add(other.amount);
		mem::forget(other);

		self
	}
	fn subsume(&mut self, other: Self) {
		self.amount = self.amount.saturating_add(other.amount);
		mem::forget(other);
	}
	fn offset(self, other: Self::Opposite) -> SameOrOther<Self, Self::Opposite> {
		let (a, b) = (self.amount, other.amount);
		let asset_id = self.asset_id;
		mem::forget((self, other));

		if a >= b {
			SameOrOther::Same(Self::new(a - b, asset_id))
		} else {
			SameOrOther::Other(NegativeImbalance::new(b - a, asset_id))
		}
	}
	fn peek(&self) -> T::Balance {
		self.amount
	}
}

impl<T: Config> TryDrop for NegativeImbalance<T> {
	fn try_drop(self) -> result::Result<(), Self> {
		self.drop_zero()
	}
}

impl<T: Config> Imbalance<T::Balance> for NegativeImbalance<T> {
	type Opposite = PositiveImbalance<T>;

	fn zero() -> Self {
		Self::new(Zero::zero(), Zero::zero())
	}
	fn drop_zero(self) -> result::Result<(), Self> {
		if self.amount.is_zero() || self.asset_id.is_zero() {
			Ok(())
		} else {
			Err(self)
		}
	}
	fn split(self, amount: T::Balance) -> (Self, Self) {
		let first = self.amount.min(amount);
		let second = self.amount - first;
		let asset_id = self.asset_id;

		mem::forget(self);
		(Self::new(first, asset_id), Self::new(second, asset_id))
	}
	fn merge(mut self, other: Self) -> Self {
		self.amount = self.amount.saturating_add(other.amount);
		mem::forget(other);

		self
	}
	fn subsume(&mut self, other: Self) {
		self.amount = self.amount.saturating_add(other.amount);
		mem::forget(other);
	}
	fn offset(self, other: Self::Opposite) -> SameOrOther<Self, Self::Opposite> {
		let (a, b) = (self.amount, other.amount);
		let asset_id = self.asset_id;
		mem::forget((self, other));

		if a >= b {
			SameOrOther::Same(Self::new(a - b, asset_id))
		} else {
			SameOrOther::Other(PositiveImbalance::new(b - a, asset_id))
		}
	}
	fn peek(&self) -> T::Balance {
		self.amount
	}
}

impl<T: Config> Drop for PositiveImbalance<T> {
	/// Basic drop handler will just square up the total issuance.
	fn drop(&mut self) {
		<TotalIssuance<T>>::mutate(self.asset_id, |v| *v = v.saturating_add(self.amount));
	}
}

impl<T: Config> Drop for NegativeImbalance<T> {
	/// Basic drop handler will just square up the total issuance.
	fn drop(&mut self) {
		<TotalIssuance<T>>::mutate(self.asset_id, |v| *v = v.saturating_sub(self.amount));
	}
}

/// The result of an offset operation
#[derive(Debug)]
pub enum OffsetResult<T: Config, I: Imbalance<T::Balance>> {
	Imbalance(I),
	Opposite(I::Opposite),
}
