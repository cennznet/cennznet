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
use core::convert::TryInto;
use uint::construct_uint;
construct_uint! {
	/// 128-bit unsigned integer.
	pub struct U128(2);
}

construct_uint! {
	/// 256-bit unsigned integer.
	pub struct U256(4);
}

use parity_codec::{Compact, CompactAs, Decode, Encode};
use runtime_primitives::traits::As;

/// FeeRate S.F precision
const SCALE_FACTOR: u128 = 1_000_000;

/// FeeRate (based on Permill), uses a scale factor
/// Inner type is `u128` in order to support compatibility with `generic_asset::Balance` type
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
#[derive(Encode, Decode, Default, Copy, Clone, PartialEq, Eq)]
pub struct FeeRate(u128);

impl FeeRate {
	/// Create FeeRate as a decimal where `x / 1000`
	pub fn from_milli(x: u128) -> FeeRate {
		FeeRate(x * SCALE_FACTOR / 1000)
	}

	/// Create a FeeRate from a % i.e. `x / 100`
	pub fn from_percent(x: u128) -> FeeRate {
		FeeRate(x * SCALE_FACTOR / 100)
	}

	/// Divide a `As::as_<u128>` supported numeric by a FeeRate
	pub fn div<N: As<u128>>(lhs: N, rhs: FeeRate) -> N {
		N::sa(lhs.as_() * SCALE_FACTOR / rhs.0)
	}

	/// Divide a `u128` supported numeric by a FeeRate
	pub fn safe_div<N: Into<u128>>(lhs: N, rhs: FeeRate) -> rstd::result::Result<u128, &'static str> {
		let lhs = lhs.into();
		let lhs = U256::from(lhs);
		let scale_factor = U256::from(SCALE_FACTOR);
		let rhs = U256::from(rhs.0);
		let res: u128 = (lhs * scale_factor / rhs).try_into().map_err(|_| "Overflow error")?;

		Ok(res)
	}

	//Self - lhs and N - rhs
	pub fn safe_mul<N: Into<u128>>(lhs: FeeRate, rhs: N) -> rstd::result::Result<u128, &'static str> {
		let rhs = U256::from(rhs.into());
		let scale_factor = U256::from(SCALE_FACTOR);
		let lhs = U256::from(lhs.0);
		let res: u128 = (lhs * rhs / scale_factor).try_into().map_err(|_| "Overflow error")?;

		Ok(res)
	}

	/// Returns the equivalent of 1 or 100%
	pub fn one() -> FeeRate {
		FeeRate(SCALE_FACTOR)
	}
}

impl As<u128> for FeeRate {
	fn as_(self) -> u128 {
		self.0
	}
	/// Convert `u128` into a FeeRate
	fn sa(x: u128) -> Self {
		FeeRate(x)
	}
}

impl rstd::ops::Add<Self> for FeeRate {
	type Output = Self;
	fn add(self, rhs: FeeRate) -> Self::Output {
		FeeRate(self.0 + rhs.0)
	}
}

impl CompactAs for FeeRate {
	type As = u128;
	fn encode_as(&self) -> &u128 {
		&self.0
	}
	fn decode_from(x: u128) -> FeeRate {
		FeeRate(x)
	}
}

impl From<Compact<FeeRate>> for FeeRate {
	fn from(x: Compact<FeeRate>) -> FeeRate {
		x.0
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn div_works() {
		let fee_rate = FeeRate::from_percent(110);
		assert_eq!(FeeRate::div(10, fee_rate), 9); // Float value would be 9.0909

		let fee_rate = FeeRate::from_percent(10);
		assert_eq!(FeeRate::div(10, fee_rate), 100);
	}

	#[test]
	fn safe_div_works() {
		let fee_rate = FeeRate::from_percent(110);
		let lhs: u128 = 10;
		assert_ok!(FeeRate::safe_div(lhs, fee_rate), 9 as u128); // Float value would be 9.0909

		let fee_rate = FeeRate::from_percent(10);
		assert_ok!(FeeRate::safe_div(lhs, fee_rate), 100 as u128);
	}

	#[test]
	fn safe_div_overflow_works() {
		let fee_rate = FeeRate::from_percent(10);
		let lhs: u128 = u128::max_value();
		assert_err!(FeeRate::safe_div(lhs, fee_rate), "Overflow error");
	}

	#[test]
	fn add_works() {
		let fee_rate = FeeRate::from_percent(50) + FeeRate::from_percent(12);
		assert_eq!(fee_rate, FeeRate::from_percent(62));
	}

	#[test]
	fn safe_mul_works() {
		let fee_rate = FeeRate::from_percent(50);
		let rhs: u128 = 2;
		assert_ok!(FeeRate::safe_mul(fee_rate, rhs), 1 as u128);
	}

	#[test]
	fn safe_mul_overflow_works() {
		let fee_rate = FeeRate::from_percent(200);
		let rhs: u128 = u128::max_value();
		assert_err!(FeeRate::safe_mul(fee_rate, rhs), "Overflow error");
	}
}
