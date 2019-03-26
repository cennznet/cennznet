//! Cennznet implementation of an unchecked (pre-verification) extrinsic.

#[cfg(feature = "std")]
use std::fmt;

use rstd::prelude::*;
use runtime_io::blake2_256;
use runtime_primitives::codec::{Compact, Decode, Encode, HasCompact, Input};
use runtime_primitives::generic::{CheckedExtrinsic, Era};
use runtime_primitives::traits::{
	self, BlockNumberToHash, Checkable, CurrentHeight, Extrinsic, Lookup, MaybeDisplay, Member, SimpleArithmetic,
};

const TRANSACTION_VERSION: u8 = 0b0000_00001;
const MASK_VERSION: u8 = 0b0000_1111;
const BIT_SIGNED: u8 = 0b1000_0000;
const BIT_DOUGHNUT: u8 = 0b0100_0000;
const BIT_CENNZ_X: u8 = 0b0010_0000;

fn encode_with_vec_prefix<T: Encode, F: Fn(&mut Vec<u8>)>(encoder: F) -> Vec<u8> {
	let size = ::rstd::mem::size_of::<T>();
	let reserve = match size {
		x if x <= 0b0011_1111 => 1,
		x if x <= 0b0011_1111_1111_1111 => 2,
		_ => 4,
	};
	let mut v = Vec::with_capacity(reserve + size);
	v.resize(reserve, 0);
	encoder(&mut v);

	// need to prefix with the total length to ensure it's binary compatible with
	// Vec<u8>.
	let mut length: Vec<()> = Vec::new();
	length.resize(v.len() - reserve, ());
	length.using_encoded(|s| {
		v.splice(0..reserve, s.iter().cloned());
	});

	v
}

/// A extrinsic right from the external world. This is unchecked and so
/// can contain a signature.
#[derive(PartialEq, Eq, Clone)]
pub struct CennznetExtrinsic<Address, Index, Call, Signature, Balance: HasCompact> {
	/// The signature, address, number of extrinsics have come before from
	/// the same signer and an era describing the longevity of this transaction,
	/// if this is a signed extrinsic.
	pub signature: Option<(Address, Signature, Compact<Index>, Era)>,
	/// The function that should be called.
	pub function: Call,
	/// Signal fee payment to use the spot exchange (CENNZ-X)
	pub fee_exchange: Option<FeeExchange<Balance>>,
}

impl<Address, Index, Call, Signature, Balance: HasCompact> CennznetExtrinsic<Address, Index, Call, Signature, Balance> {
	/// New instance of a signed extrinsic aka "transaction".
	pub fn new_signed(index: Index, function: Call, signed: Address, signature: Signature, era: Era) -> Self {
		CennznetExtrinsic {
			signature: Some((signed, signature, index.into(), era)),
			function,
			fee_exchange: None,
		}
	}

	/// New instance of an unsigned extrinsic aka "inherent".
	pub fn new_unsigned(function: Call) -> Self {
		CennznetExtrinsic {
			signature: None,
			function,
			fee_exchange: None,
		}
	}
}

impl<Address: Encode, Index: Encode, Call: Encode, Signature: Encode, Balance: HasCompact> Extrinsic
	for CennznetExtrinsic<Address, Index, Call, Signature, Balance>
{
	fn is_signed(&self) -> Option<bool> {
		Some(self.signature.is_some())
	}
}

impl<Address, AccountId, Index, Call, Signature, Context, Hash, BlockNumber, Balance> Checkable<Context>
	for CennznetExtrinsic<Address, Index, Call, Signature, Balance>
where
	Address: Member + MaybeDisplay,
	Balance: HasCompact,
	Index: Member + MaybeDisplay + SimpleArithmetic,
	Compact<Index>: Encode,
	Call: Encode + Member,
	Signature: Member + traits::Verify<Signer = AccountId>,
	AccountId: Member + MaybeDisplay,
	BlockNumber: SimpleArithmetic,
	Hash: Encode,
	Context: Lookup<Source = Address, Target = AccountId>
		+ CurrentHeight<BlockNumber = BlockNumber>
		+ BlockNumberToHash<BlockNumber = BlockNumber, Hash = Hash>,
{
	type Checked = CheckedExtrinsic<AccountId, Index, Call>;

	fn check(self, context: &Context) -> Result<Self::Checked, &'static str> {
		Ok(match self.signature {
			Some((signed, signature, index, era)) => {
				let h = context
					.block_number_to_hash(BlockNumber::sa(era.birth(context.current_height().as_())))
					.ok_or("transaction birth block ancient")?;
				let signed = context.lookup(signed)?;
				let raw_payload = (index, self.function, era, h);
				if !raw_payload.using_encoded(|payload| {
					if payload.len() > 256 {
						signature.verify(&blake2_256(payload)[..], &signed)
					} else {
						signature.verify(payload, &signed)
					}
				}) {
					return Err(runtime_primitives::BAD_SIGNATURE);
				}
				CheckedExtrinsic {
					signed: Some((signed, (raw_payload.0).0)),
					function: raw_payload.1,
				}
			}
			None => CheckedExtrinsic {
				signed: None,
				function: self.function,
			},
		})
	}
}

impl<Address, Index, Call, Signature, Balance> Decode for CennznetExtrinsic<Address, Index, Call, Signature, Balance>
where
	Address: Decode,
	Signature: Decode,
	Compact<Index>: Decode,
	Call: Decode,
	Balance: HasCompact,
{
	fn decode<I: Input>(input: &mut I) -> Option<Self> {
		// This is a little more complicated than usual since the binary format must be compatible
		// with substrate's generic `Vec<u8>` type. Basically this just means accepting that there
		// will be a prefix of vector length (we don't need
		// to use this).
		let _length_do_not_remove_me_see_above: Vec<()> = Decode::decode(input)?;

		let version = input.read_byte()?;

		let is_signed = version & BIT_SIGNED != 0;
		let has_doughnut = version & BIT_DOUGHNUT != 0;
		let has_fee_exchange = version & BIT_CENNZ_X != 0;
		let version = version & MASK_VERSION;

		if version != TRANSACTION_VERSION {
			return None;
		}

		let signature = if is_signed { Some(Decode::decode(input)?) } else { None };
		let function = Decode::decode(input)?;

		let _doughnut = if has_doughnut {
			// TODO: decode doughnut
			Some(())
		} else {
			None
		};

		let fee_exchange = if has_fee_exchange {
			Some(Decode::decode(input)?)
		} else {
			None
		};

		Some(CennznetExtrinsic {
			signature,
			function,
			fee_exchange,
		})
	}
}

impl<Address, Index, Call, Signature, Balance> Encode for CennznetExtrinsic<Address, Index, Call, Signature, Balance>
where
	Address: Encode,
	Signature: Encode,
	Compact<Index>: Encode,
	Call: Encode,
	Balance: HasCompact,
{
	fn encode(&self) -> Vec<u8> {
		encode_with_vec_prefix::<Self, _>(|v| {
			// 1 byte version id.
			let mut version = TRANSACTION_VERSION;
			if self.signature.is_some() {
				version |= BIT_SIGNED;
			}
			// TODO: update version if has doughnut
			if self.fee_exchange.is_some() {
				version |= BIT_CENNZ_X;
			}
			v.push(version);

			if let Some(s) = self.signature.as_ref() {
				s.encode_to(v);
			}
			self.function.encode_to(v);
			// TODO: encode doughnut
			if let Some(f) = self.fee_exchange.as_ref() {
				f.encode_to(v);
			}
		})
	}
}

#[cfg(feature = "std")]
impl<Address: Encode, Index, Signature: Encode, Call: Encode, Balance> serde::Serialize
	for CennznetExtrinsic<Address, Index, Call, Signature, Balance>
where
	Compact<Index>: Encode,
	Balance: HasCompact,
{
	fn serialize<S>(&self, seq: S) -> Result<S::Ok, S::Error>
	where
		S: ::serde::Serializer,
	{
		self.using_encoded(|bytes| seq.serialize_bytes(bytes))
	}
}

#[cfg(feature = "std")]
impl<Address, Index, Call, Signature, Balance> fmt::Debug
	for CennznetExtrinsic<Address, Index, Call, Signature, Balance>
where
	Address: fmt::Debug,
	Index: fmt::Debug,
	Call: fmt::Debug,
	Balance: fmt::Debug + HasCompact,
{
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		// TODO: write doughnut
		write!(
			f,
			"CennznetExtrinsic({:?}, {:?}, {:?})",
			self.signature.as_ref().map(|x| (&x.0, &x.2)),
			self.function,
			self.fee_exchange
		)
	}
}

/// Signals a fee payment requiring the spot exchange, intended to embed within CENNZnet extrinsics.
/// Specifies the input asset ID and the max. input amount only. Actual fee amount and to/from is given-
/// as part of the `TransferAsset::transfer` call.
#[derive(PartialEq, Eq, Clone, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct FeeExchange<Balance: HasCompact> {
	// TODO: use runtime `AssetId` type instead of `u32` directly
	/// The asset ID to pay in exchange for fee asset
	#[codec(compact)]
	pub asset_id: u32,
	/// The max. amount of `asset_id` to pay for the needed fee amount.
	/// The operation should fail otherwise.
	#[codec(compact)]
	pub max_payment: Balance,
}

impl<Balance: HasCompact> FeeExchange<Balance> {
	/// Create a new FeeExchange
	pub fn new(asset_id: u32, max_payment: Balance) -> Self {
		Self { asset_id, max_payment }
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use primitives::H256;

	#[test]
	fn it_works_with_fee_exchange() {
		let mut extrinsic = CennznetExtrinsic::<H256, u32, (), (), u128>::new_unsigned(());
		extrinsic.fee_exchange = Some(FeeExchange::new(0, 1_000_000));
		let buf = Encode::encode(&extrinsic);
		let decoded = Decode::decode(&mut &buf[..]).unwrap();

		assert_eq!(extrinsic, decoded);
	}
}
