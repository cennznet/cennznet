//! Cennznet implementation of an unchecked (pre-verification) extrinsic.

#[cfg(feature = "std")]
use std::fmt;

use rstd::prelude::*;
use runtime_primitives::codec::{Compact, Decode, Encode, Input};
use runtime_primitives::generic::{CheckedExtrinsic, Era};
use runtime_primitives::traits::{
	self, BlockNumberToHash, Checkable, CurrentHeight, Extrinsic, Lookup, MaybeDisplay, Member, SimpleArithmetic,
};

use runtime_io::blake2_256;

const TRANSACTION_VERSION: u8 = 0b0000_00001;
const MASK_VERSION: u8 = 0b0000_1111;
const BIT_SIGNED: u8 = 0b1000_0000;
const BIT_DOUGHNUT: u8 = 0b0100_0000;
const BIT_CENNZ_X: u8 = 0b0010_0000;

fn encode_with_vec_prefix<T: Encode, F: Fn(&mut Vec<u8>)>(encoder: F) -> Vec<u8> {
	let size = ::rstd::mem::size_of::<T>();
	let reserve = match size {
		0...0b00111111 => 1,
		0...0b00111111_11111111 => 2,
		_ => 4,
	};
	let mut v = Vec::with_capacity(reserve + size);
	v.resize(reserve, 0);
	encoder(&mut v);

	// need to prefix with the total length to ensure it's binary comptible with
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
pub struct CennznetExtrinsic<Address, Index, Call, Signature> {
	/// The signature, address, number of extrinsics have come before from
	/// the same signer and an era describing the longevity of this transaction,
	/// if this is a signed extrinsic.
	pub signature: Option<(Address, Signature, Compact<Index>, Era)>,
	/// The function that should be called.
	pub function: Call,
}

impl<Address, Index, Call, Signature> CennznetExtrinsic<Address, Index, Call, Signature> {
	/// New instance of a signed extrinsic aka "transaction".
	pub fn new_signed(index: Index, function: Call, signed: Address, signature: Signature, era: Era) -> Self {
		CennznetExtrinsic {
			signature: Some((signed, signature, index.into(), era)),
			function,
		}
	}

	/// New instance of an unsigned extrinsic aka "inherent".
	pub fn new_unsigned(function: Call) -> Self {
		CennznetExtrinsic {
			signature: None,
			function,
		}
	}
}

impl<Address: Encode, Index: Encode, Call: Encode, Signature: Encode> Extrinsic
	for CennznetExtrinsic<Address, Index, Call, Signature>
{
	fn is_signed(&self) -> Option<bool> {
		Some(self.signature.is_some())
	}
}

impl<Address, AccountId, Index, Call, Signature, Context, Hash, BlockNumber> Checkable<Context>
	for CennznetExtrinsic<Address, Index, Call, Signature>
where
	Address: Member + MaybeDisplay,
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

impl<Address, Index, Call, Signature> Decode for CennznetExtrinsic<Address, Index, Call, Signature>
where
	Address: Decode,
	Signature: Decode,
	Compact<Index>: Decode,
	Call: Decode,
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
		let has_cennzx = version & BIT_CENNZ_X != 0;
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

		let _cennzx = if has_cennzx {
			// TODO: decode cennzx
			Some(())
		} else {
			None
		};

		Some(CennznetExtrinsic { signature, function })
	}
}

impl<Address, Index, Call, Signature> Encode for CennznetExtrinsic<Address, Index, Call, Signature>
where
	Address: Encode,
	Signature: Encode,
	Compact<Index>: Encode,
	Call: Encode,
{
	fn encode(&self) -> Vec<u8> {
		encode_with_vec_prefix::<Self, _>(|v| {
			// 1 byte version id.
			let mut version = TRANSACTION_VERSION;
			if self.signature.is_some() {
				version |= BIT_SIGNED;
			}
			// TODO: update version if has doughnut
			// TODO: update version if has cennzx

			v.push(version);

			if let Some(s) = self.signature.as_ref() {
				s.encode_to(v);
			}
			self.function.encode_to(v);

			// TODO: encode doughnut
			// TODO: encode cennzx
		})
	}
}

#[cfg(feature = "std")]
impl<Address: Encode, Index, Signature: Encode, Call: Encode> serde::Serialize
	for CennznetExtrinsic<Address, Index, Call, Signature>
where
	Compact<Index>: Encode,
{
	fn serialize<S>(&self, seq: S) -> Result<S::Ok, S::Error>
	where
		S: ::serde::Serializer,
	{
		self.using_encoded(|bytes| seq.serialize_bytes(bytes))
	}
}

#[cfg(feature = "std")]
impl<Address, Index, Call, Signature> fmt::Debug for CennznetExtrinsic<Address, Index, Call, Signature>
where
	Address: fmt::Debug,
	Index: fmt::Debug,
	Call: fmt::Debug,
{
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		// TODO: write doughnut
		// TODO: write cennzx
		write!(
			f,
			"CennznetExtrinsic({:?}, {:?})",
			self.signature.as_ref().map(|x| (&x.0, &x.2)),
			self.function
		)
	}
}
