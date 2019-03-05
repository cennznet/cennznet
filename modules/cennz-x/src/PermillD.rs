#[doc(hidden)]
pub use parity_codec as codec;
use runtime_primitives::{traits::{self}};
use parity_codec_derive::{Encode, Decode};
/// Permill is parts-per-million (i.e. after multiplying by this, divide by 1000000).
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
#[derive(Encode, Decode, Default, Copy, Clone, PartialEq, Eq)]
pub struct PermillD(u32);

impl PermillD {
	/// Wraps the argument into `Permill` type.
	pub fn from_millionths(x: u32) -> PermillD { PermillD(x) }

	/// Converts percents into `Permill`.
	pub fn from_percent(x: u32) -> PermillD { PermillD(x * 10_000) }

	/// Converts a fraction into `Permill`.
	#[cfg(feature = "std")]
	pub fn from_fraction(x: f64) -> PermillD { PermillD((x * 1_000_000.0) as u32) }
}

impl<N> ::rstd::ops::Mul<N> for PermillD
	where
		N: traits::As<u64>
{
	type Output = N;
	fn mul(self, b: N) -> Self::Output {
		<N as traits::As<u64>>::sa(b.as_().saturating_mul(self.0 as u64) / 1_000_000)
	}
}

impl<N> ::rstd::ops::Div<N> for PermillD
	where
		N: traits::As<u64>
{
	type Output = N;
	fn div(self, b: N) -> Self::Output {
		<N as traits::As<u64>>::sa(b.as_() / (1_000_000 * self.0) as u64)
	}
}

#[cfg(feature = "std")]
impl From<f64> for PermillD {
	fn from(x: f64) -> PermillD {
		PermillD::from_fraction(x)
	}
}

#[cfg(feature = "std")]
impl From<f32> for PermillD {
	fn from(x: f32) -> PermillD {
		PermillD::from_fraction(x as f64)
	}
}

impl codec::CompactAs for PermillD {
	type As = u32;
	fn encode_as(&self) -> &u32 {
		&self.0
	}
	fn decode_from(x: u32) -> PermillD {
		PermillD(x)
	}
}

impl From<codec::Compact<PermillD>> for PermillD {
	fn from(x: codec::Compact<PermillD>) -> PermillD {
		x.0
	}
}
