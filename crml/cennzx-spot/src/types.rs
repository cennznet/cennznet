// Copyright (C) 2019 Centrality Investments Limited
// This file is part of CENNZnet.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.
//!
//! CENNZX-SPOT Types
//!
use codec::{Decode, Encode};
use core::{
	convert::{From, Into, TryFrom},
	marker::PhantomData,
};

pub use primitive_types::U256 as HighPrecisionUnsigned;
pub use u128 as LowPrecisionUnsigned;

/// A trait for values which hold an implicit scale factor that needs to be taken into account in calculations
pub trait Scaled {
	const SCALE: LowPrecisionUnsigned;
}

/// Per millionth of unit price
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PerMillion {}
impl Scaled for PerMillion {
	const SCALE: LowPrecisionUnsigned = 1_000_000;
}

/// Per thousandth of unit price
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PerMilli {}
impl Scaled for PerMilli {
	const SCALE: LowPrecisionUnsigned = 1_000;
}

/// Per hundredth of unit price
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PerCent {}
impl Scaled for PerCent {
	const SCALE: LowPrecisionUnsigned = 100;
}

#[derive(Debug)]
pub enum Error {
	Overflow,
	DivideByZero,
	EmptyPool,
}

impl Into<&'static str> for Error {
	fn into(self) -> &'static str {
		match self {
			Error::Overflow => "Overflow",
			Error::DivideByZero => "DivideByZero",
			Error::EmptyPool => "EmptyPool",
		}
	}
}

/// Inner type is `LowPrecisionUnsigned` in order to support compatibility with `pallet_generic_asset::Balance` type
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Encode, Decode, Copy, Clone, Debug, PartialEq)]
pub struct FeeRate<S: Scaled>(LowPrecisionUnsigned, PhantomData<S>);

impl<S: Scaled> Default for FeeRate<S> {
	fn default() -> Self {
		FeeRate::<S>::from(LowPrecisionUnsigned::default())
	}
}

impl<S: Scaled> TryFrom<HighPrecisionUnsigned> for FeeRate<S> {
	type Error = Error;
	fn try_from(h: HighPrecisionUnsigned) -> Result<Self, Self::Error> {
		match LowPrecisionUnsigned::try_from(h) {
			Ok(l) => Ok(FeeRate::<S>::from(l)),
			Err(_) => Err(Error::Overflow),
		}
	}
}

impl TryFrom<FeeRate<PerMilli>> for FeeRate<PerMillion> {
	type Error = Error;
	fn try_from(f: FeeRate<PerMilli>) -> Result<Self, Self::Error> {
		let rate = PerMillion::SCALE / PerMilli::SCALE;
		match f.0.checked_mul(rate) {
			Some(x) => Ok(FeeRate::<PerMillion>::from(x)),
			None => Err(Error::Overflow),
		}
	}
}

impl TryFrom<FeeRate<PerCent>> for FeeRate<PerMillion> {
	type Error = Error;
	fn try_from(f: FeeRate<PerCent>) -> Result<Self, Self::Error> {
		let rate = PerMillion::SCALE / PerCent::SCALE;
		match f.0.checked_mul(rate) {
			Some(x) => Ok(FeeRate::<PerMillion>::from(x)),
			None => Err(Error::Overflow),
		}
	}
}

impl<S: Scaled> FeeRate<S> {
	pub fn checked_div(&self, d: Self) -> Option<Self> {
		match HighPrecisionUnsigned::from(self.0)
			.saturating_mul(S::SCALE.into())
			.checked_div(d.0.into())
		{
			Some(h) => Self::try_from(h).ok(),
			None => None,
		}
	}

	pub fn checked_mul(&self, m: Self) -> Option<Self> {
		match HighPrecisionUnsigned::from(self.0)
			.saturating_mul(m.0.into())
			.checked_div(S::SCALE.into())
		{
			Some(h) => Self::try_from(h).ok(),
			None => None,
		}
	}

	pub fn checked_add(&self, a: Self) -> Option<Self> {
		match self.0.checked_add(a.0) {
			Some(x) => Some(FeeRate::<S>::from(x)),
			None => None,
		}
	}

	pub fn one() -> Self {
		FeeRate::<S>::from(S::SCALE)
	}
}

impl<S: Scaled> From<LowPrecisionUnsigned> for FeeRate<S> {
	fn from(f: LowPrecisionUnsigned) -> Self {
		FeeRate::<S>(f, PhantomData)
	}
}

impl<S: Scaled> From<FeeRate<S>> for LowPrecisionUnsigned {
	fn from(f: FeeRate<S>) -> Self {
		f.0
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn fee_rate_div_when_indivisible() {
		let fee_rate = FeeRate::<PerCent>::from(110u128);
		let input = FeeRate::<PerCent>::from(10u128);
		assert_eq!(input.checked_div(fee_rate).unwrap(), FeeRate::<PerCent>::from(9u128));
	}

	#[test]
	fn fee_rate_div_when_divisible() {
		let fee_rate = FeeRate::<PerCent>::from(10u128);
		let input = FeeRate::<PerCent>::from(10u128);
		assert_eq!(input.checked_div(fee_rate).unwrap(), FeeRate::<PerCent>::one());
	}

	#[test]
	fn fee_rate_div_when_divide_by_zero() {
		let fee_rate = FeeRate::<PerCent>::from(0);
		let input = FeeRate::<PerCent>::from(10u128);
		assert_eq!(input.checked_div(fee_rate), None);
	}

	#[test]
	fn fee_rate_div_when_overflow() {
		let fee_rate = FeeRate::<PerCent>::from(10);
		let input = FeeRate::<PerCent>::from(LowPrecisionUnsigned::max_value());
		assert_eq!(input.checked_div(fee_rate), None);
	}

	#[test]
	fn fee_rate_mul_no_overflow() {
		assert_eq!(
			FeeRate::<PerCent>::from(50u128)
				.checked_mul(FeeRate::<PerCent>::from(2u128))
				.unwrap(),
			FeeRate::<PerCent>::from(1u128)
		);
	}

	#[test]
	fn fee_rate_mul_when_overflow() {
		let fee_rate = FeeRate::<PerCent>::from(200u128);
		let rhs = FeeRate::<PerCent>::from(LowPrecisionUnsigned::max_value());
		assert_eq!(fee_rate.checked_mul(rhs), None);
	}
}
