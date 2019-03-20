//!
//! CENNZ-X Types
//!
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

impl<N> rstd::ops::Mul<N> for FeeRate
where
	N: As<u128>,
{
	type Output = N;
	fn mul(self, rhs: N) -> Self::Output {
		N::sa(N::as_(rhs).saturating_mul(self.0) / SCALE_FACTOR)
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
	fn mul_works() {
		let fee_rate = FeeRate::from_percent(50);
		assert_eq!(fee_rate * 2, 1);
	}

	#[test]
	fn add_works() {
		let fee_rate = FeeRate::from_percent(50) + FeeRate::from_percent(12);
		assert_eq!(fee_rate, FeeRate::from_percent(62));
	}
}
