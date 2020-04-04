/* Copyright 2019-2020 Centrality Investments Limited
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
pub enum PerThousand {}
impl Scaled for PerThousand {
	const SCALE: LowPrecisionUnsigned = 1_000;
}

#[derive(Debug)]
pub enum FeeRateError {
	Overflow,
}

impl Into<&'static str> for FeeRateError {
	fn into(self) -> &'static str {
		match self {
			FeeRateError::Overflow => "Overflow",
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
	type Error = FeeRateError;
	fn try_from(h: HighPrecisionUnsigned) -> Result<Self, Self::Error> {
		match LowPrecisionUnsigned::try_from(h) {
			Ok(l) => Ok(FeeRate::<S>::from(l)),
			Err(_) => Err(Self::Error::Overflow),
		}
	}
}

impl TryFrom<FeeRate<PerThousand>> for FeeRate<PerMillion> {
	type Error = FeeRateError;
	fn try_from(f: FeeRate<PerThousand>) -> Result<Self, Self::Error> {
		let rate = PerMillion::SCALE / PerThousand::SCALE;
		match f.0.checked_mul(rate) {
			Some(x) => Ok(FeeRate::<PerMillion>::from(x)),
			None => Err(Self::Error::Overflow),
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
		let fee_rate = FeeRate::<PerMillion>::from(1_100_000u128);
		let input = FeeRate::<PerMillion>::from(100_000u128);
		assert_eq!(input.checked_div(fee_rate).unwrap(), FeeRate::<PerMillion>::from(90_909u128));
	}

	#[test]
	fn fee_rate_div_when_divisible() {
		let fee_rate = FeeRate::<PerMillion>::from(100_000u128);
		let input = FeeRate::<PerMillion>::from(100_000u128);
		assert_eq!(input.checked_div(fee_rate).unwrap(), FeeRate::<PerMillion>::one());
	}

	#[test]
	fn fee_rate_div_when_divide_by_zero() {
		let fee_rate = FeeRate::<PerMillion>::from(0);
		let input = FeeRate::<PerMillion>::from(100_000u128);
		assert_eq!(input.checked_div(fee_rate), None);
	}

	#[test]
	fn fee_rate_div_when_overflow() {
		let fee_rate = FeeRate::<PerMillion>::from(100_000u128);
		let input = FeeRate::<PerMillion>::from(LowPrecisionUnsigned::max_value());
		assert_eq!(input.checked_div(fee_rate), None);
	}

	#[test]
	fn fee_rate_mul_no_overflow() {
		assert_eq!(
			FeeRate::<PerMillion>::from(500_000u128)
				.checked_mul(FeeRate::<PerMillion>::from(20_000u128))
				.unwrap(),
			FeeRate::<PerMillion>::from(10_000u128)
		);
	}

	#[test]
	fn fee_rate_mul_when_overflow() {
		let fee_rate = FeeRate::<PerMillion>::from(2_000_000u128);
		let rhs = FeeRate::<PerMillion>::from(LowPrecisionUnsigned::max_value());
		assert_eq!(fee_rate.checked_mul(rhs), None);
	}
}
