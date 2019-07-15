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
//! Cennznet implementation of an unchecked (pre-verification) extrinsic.

#[cfg(feature = "std")]
use std::fmt;

use crate::util::encode_with_vec_prefix;
use rstd::prelude::*;
use runtime_io::blake2_256;
use runtime_primitives::codec::{Compact, Decode, Encode, HasCompact, Input};
use runtime_primitives::generic::Era;
use runtime_primitives::traits::{
	self, BlockNumberToHash, Checkable, CurrentHeight, DoughnutApi, Doughnuted, Extrinsic, Lookup, MaybeDisplay,
	Member, SimpleArithmetic,
};

const TRANSACTION_VERSION: u8 = 0b0000_00001;
const MASK_VERSION: u8 = 0b0000_1111;
const BIT_SIGNED: u8 = 0b1000_0000;
const BIT_DOUGHNUT: u8 = 0b0100_0000;
const BIT_CENNZ_X: u8 = 0b0010_0000;

/// A extrinsic right from the external world. This is unchecked and so
/// can contain a signature.
#[derive(PartialEq, Eq, Clone)]
pub struct CennznetExtrinsic<AccountId, Address, Index, Call, Signature, Balance: HasCompact, Doughnut> {
	/// The signature, address, number of extrinsics have come before from
	/// the same signer and an era describing the longevity of this transaction,
	/// if this is a signed extrinsic.
	pub signature: Option<(Address, Signature, Compact<Index>, Era)>,
	/// The function that should be called.
	pub function: Call,
	/// Doughnut attached
	pub doughnut: Option<Doughnut>,
	/// Signals fee payment should use the CENNZX-Spot exchange
	pub fee_exchange: Option<FeeExchange<Balance>>,
	_phantom: rstd::marker::PhantomData<AccountId>,
}

/// Definition of something that the external world might want to say; its
/// existence implies that it has been checked and is good, particularly with
/// regards to the signature.
#[derive(PartialEq, Eq, Clone)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct CheckedCennznetExtrinsic<AccountId, Index, Call, Balance: HasCompact, Doughnut> {
	/// Who this purports to be from and the number of extrinsics that have come before
	/// from the same signer, if anyone (note this is not a signature).
	pub signed: Option<(AccountId, Index)>,
	/// The function that should be called.
	pub function: Call,
	/// Signals fee payment should use the CENNZX-Spot exchange
	pub fee_exchange: Option<FeeExchange<Balance>>,
	/// An attached doughnut, if any
	pub doughnut: Option<Doughnut>,
}

impl<AccountId, Index, Call, Balance: HasCompact, Doughnut> Doughnuted
	for CheckedCennznetExtrinsic<AccountId, Index, Call, Balance, Doughnut>
where
	Doughnut: Encode + Clone + DoughnutApi,
{
	type Doughnut = Doughnut;
	fn doughnut(&self) -> Option<&Self::Doughnut> {
		self.doughnut.as_ref()
	}
}

impl<AccountId, Index, Call, Balance, Doughnut> traits::Applyable
	for CheckedCennznetExtrinsic<AccountId, Index, Call, Balance, Doughnut>
where
	AccountId: Member + MaybeDisplay,
	Index: Member + MaybeDisplay + SimpleArithmetic,
	Call: Member,
	Balance: Member + HasCompact,
	Doughnut: Member,
{
	type Index = Index;
	type AccountId = AccountId;
	type Call = Call;

	fn index(&self) -> Option<&Self::Index> {
		self.signed.as_ref().map(|x| &x.1)
	}

	fn sender(&self) -> Option<&Self::AccountId> {
		self.signed.as_ref().map(|x| &x.0)
	}

	fn call(&self) -> &Self::Call {
		&self.function
	}

	fn deconstruct(self) -> (Self::Call, Option<Self::AccountId>) {
		(self.function, self.signed.map(|x| x.0))
	}
}

impl<AccountId, Address, Index, Call, Signature, Balance: HasCompact, Doughnut>
	CennznetExtrinsic<AccountId, Address, Index, Call, Signature, Balance, Doughnut>
{
	/// New instance of a signed extrinsic aka "transaction".
	pub fn new_signed(
		index: Index,
		function: Call,
		signed: Address,
		signature: Signature,
		era: Era,
		doughnut: Option<Doughnut>,
	) -> Self {
		Self {
			signature: Some((signed, signature, index.into(), era)),
			function,
			doughnut,
			fee_exchange: None,
			_phantom: rstd::marker::PhantomData,
		}
	}

	/// New instance of an unsigned extrinsic aka "inherent".
	pub fn new_unsigned(function: Call) -> Self {
		Self {
			signature: None,
			function,
			doughnut: None,
			fee_exchange: None,
			_phantom: rstd::marker::PhantomData,
		}
	}
}

impl<AccountId, Address, Index, Call, Signature, Balance: HasCompact, Doughnut> Extrinsic
	for CennznetExtrinsic<AccountId, Address, Index, Call, Signature, Balance, Doughnut>
{
	fn is_signed(&self) -> Option<bool> {
		Some(self.signature.is_some())
	}
}

impl<AccountId, Address, Index, Call, Signature, Context, Hash, BlockNumber, Balance, Doughnut> Checkable<Context>
	for CennznetExtrinsic<AccountId, Address, Index, Call, Signature, Balance, Doughnut>
where
	Address: Member + MaybeDisplay,
	Balance: HasCompact,
	Index: Member + MaybeDisplay + SimpleArithmetic,
	Compact<Index>: Encode,
	Call: Encode + Member,
	Signature: Member + traits::Verify<Signer = AccountId> + Encode + Decode,
	AccountId: Member + MaybeDisplay + Decode + Encode,
	BlockNumber: SimpleArithmetic,
	Hash: Encode,
	Context: Lookup<Source = Address, Target = AccountId>
		+ CurrentHeight<BlockNumber = BlockNumber>
		+ BlockNumberToHash<BlockNumber = BlockNumber, Hash = Hash>,
	Doughnut: Encode + DoughnutApi,
	<Doughnut as DoughnutApi>::AccountId: AsRef<[u8]>,
	<Doughnut as DoughnutApi>::Signature: AsRef<[u8]>,
{
	type Checked = CheckedCennznetExtrinsic<AccountId, Index, Call, Balance, Doughnut>;

	fn check(self, context: &Context) -> Result<Self::Checked, &'static str> {
		// There's no signature so we're done
		if self.signature.is_none() {
			return Ok(Self::Checked {
				signed: None,
				function: self.function,
				fee_exchange: self.fee_exchange,
				doughnut: None,
			});
		};

		// If doughnut signer switch is needed. This index will become stale...
		let (signed, signature, index, era) = self.signature.unwrap();
		let h = context
			.block_number_to_hash(BlockNumber::sa(era.birth(context.current_height().as_())))
			.ok_or("transaction birth block ancient")?;
		let signed = context.lookup(signed)?;

		let verify_signature = |payload: &[u8]| {
			if payload.len() > 256 {
				signature.verify(&blake2_256(payload)[..], &signed)
			} else {
				signature.verify(payload, &signed)
			}
		};

		// Signature may be standard, contain a doughnut and/or a fee exchange operation
		let verified = match (&self.doughnut, &self.fee_exchange) {
			(Some(doughnut), Some(fee_exchange)) => {
				(&index, &self.function, era, h, doughnut, fee_exchange).using_encoded(verify_signature)
			}
			(Some(doughnut), None) => (&index, &self.function, era, h, doughnut).using_encoded(verify_signature),
			(None, Some(fee_exchange)) => {
				(&index, &self.function, era, h, fee_exchange).using_encoded(verify_signature)
			}
			(None, None) => (&index, &self.function, era, h).using_encoded(verify_signature),
		};

		if !verified {
			return Err("bad signature in extrinsic");
		}

		// Verify doughnut signature. It should be signed by the issuer.
		if let Some(ref d) = self.doughnut {
			// TODO: Move this check into the doughnut crate
			let holder = AccountId::decode(&mut d.holder().as_ref())
				.ok_or("doughnut holder incompatible with runtime AccountId")?;
			if holder != signed {
				return Err("bad signature in extrinsic");
			}
			let issuer = AccountId::decode(&mut d.issuer().as_ref())
				.ok_or("doughnut issuer incompatible with runtime AccountId")?;
			let signature = Signature::decode(&mut d.signature().as_ref())
				.ok_or("doughnut signature incompatible with runtime Signature")?;
			if !signature.verify(d.payload().as_ref(), &issuer) {
				return Err("bad signature in doughnut");
			}
		}

		Ok(Self::Checked {
			signed: Some((signed, index.0)),
			function: self.function,
			fee_exchange: self.fee_exchange,
			doughnut: self.doughnut,
		})
	}
}

impl<AccountId, Address, Index, Call, Signature, Balance, Doughnut> Decode
	for CennznetExtrinsic<AccountId, Address, Index, Call, Signature, Balance, Doughnut>
where
	AccountId: Decode,
	Address: Decode,
	Signature: Decode,
	Compact<Index>: Decode,
	Call: Decode,
	Balance: HasCompact,
	Doughnut: Decode,
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

		let doughnut = if has_doughnut {
			Some(Decode::decode(input)?)
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
			doughnut,
			fee_exchange,
			_phantom: rstd::marker::PhantomData,
		})
	}
}

impl<AccountId, Address, Index, Call, Signature, Balance, Doughnut> Encode
	for CennznetExtrinsic<AccountId, Address, Index, Call, Signature, Balance, Doughnut>
where
	AccountId: Encode,
	Address: Encode,
	Signature: Encode,
	Compact<Index>: Encode,
	Call: Encode,
	Balance: HasCompact,
	Doughnut: Encode,
{
	fn encode(&self) -> Vec<u8> {
		encode_with_vec_prefix::<Self, _>(|v| {
			// 1 byte version id.
			let mut version = TRANSACTION_VERSION;
			if self.signature.is_some() {
				version |= BIT_SIGNED;
			}
			if self.doughnut.is_some() {
				version |= BIT_DOUGHNUT;
			}
			if self.fee_exchange.is_some() {
				version |= BIT_CENNZ_X;
			}
			v.push(version);

			if let Some(s) = self.signature.as_ref() {
				s.encode_to(v);
			}
			self.function.encode_to(v);
			if let Some(d) = self.doughnut.as_ref() {
				d.encode_to(v);
			}
			if let Some(f) = self.fee_exchange.as_ref() {
				f.encode_to(v);
			}
		})
	}
}

#[cfg(feature = "std")]
impl<AccountId: Encode, Address: Encode, Index, Signature: Encode, Call: Encode, Balance, Doughnut: Encode>
	serde::Serialize for CennznetExtrinsic<AccountId, Address, Index, Call, Signature, Balance, Doughnut>
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
impl<AccountId, Address, Index, Call, Signature, Balance, Doughnut> fmt::Debug
	for CennznetExtrinsic<AccountId, Address, Index, Call, Signature, Balance, Doughnut>
where
	AccountId: fmt::Debug,
	Address: fmt::Debug,
	Index: fmt::Debug,
	Call: fmt::Debug,
	Balance: fmt::Debug + HasCompact,
	Signature: fmt::Debug,
	Doughnut: fmt::Debug,
{
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(
			f,
			"CennznetExtrinsic({:?}, {:?}, {:?}, {:?})",
			self.signature.as_ref().map(|x| (&x.0, &x.2)),
			self.function,
			self.doughnut,
			self.fee_exchange
		)
	}
}

/// Signals a fee payment requiring the CENNZX-Spot exchange. It is intended to
/// embed within CENNZnet extrinsics.
/// It specifies input asset ID and the max. input asset to pay. The actual
/// fee amount to pay is calculated via the fees module and current exchange prices.
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

impl<AccountId, Address, Index, Call, Signature, Balance: HasCompact, Doughnut> Doughnuted
	for CennznetExtrinsic<AccountId, Address, Index, Call, Signature, Balance, Doughnut>
where
	Doughnut: Encode + Clone + DoughnutApi,
{
	type Doughnut = Doughnut;
	fn doughnut(&self) -> Option<&Doughnut> {
		self.doughnut.as_ref()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use primitives::H256;

	#[test]
	fn it_works_with_fee_exchange() {
		let mut extrinsic = CennznetExtrinsic::<H256, H256, u32, (), (), u128, ()>::new_unsigned(());
		extrinsic.fee_exchange = Some(FeeExchange::new(0, 1_000_000));
		let buf = Encode::encode(&extrinsic);
		let decoded = Decode::decode(&mut &buf[..]).unwrap();

		assert_eq!(extrinsic, decoded);
	}
}
