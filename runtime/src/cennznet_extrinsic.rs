//! Cennznet implementation of an unchecked (pre-verification) extrinsic.

#[cfg(feature = "std")]
use std::fmt;

use rstd::prelude::*;
use runtime_primitives::codec::{Compact, Decode, Encode, Input};
use runtime_primitives::generic::{CheckedExtrinsic, Era};
use runtime_primitives::traits::{self, BlockNumberToHash, Checkable, CurrentHeight, Extrinsic, Lookup, MaybeDisplay,
								 Member, SimpleArithmetic};
use runtime_io::{blake2_256};

use doughnut;

const TRANSACTION_VERSION: u8 = 1;

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
pub struct CennznetExtrinsic<AccountId, Address, Index, Call, Signature> {
	/// The signature, address, number of extrinsics have come before from
	/// the same signer and an era describing the longevity of this transaction,
	/// if this is a signed extrinsic.
	pub signature: Option<(Address, Signature, Compact<Index>, Era)>,
	/// The function that should be called.
	pub function: Call,
	pub doughnut: Option<doughnut::Doughnut<AccountId, Signature>>,
}

impl<AccountId, Address, Index, Call, Signature> CennznetExtrinsic<AccountId, Address, Index, Call, Signature> {
	/// New instance of a signed extrinsic aka "transaction".
	pub fn new_signed(index: Index, function: Call, signed: Address, signature: Signature, era: Era) -> Self {
		CennznetExtrinsic {
			signature: Some((signed, signature, index.into(), era)),
			function,
			doughnut: None
		}
	}

	/// New instance of an unsigned extrinsic aka "inherent".
	pub fn new_unsigned(function: Call) -> Self {
		CennznetExtrinsic {
			signature: None,
			function,
			doughnut: None
		}
	}
}

impl<AccountId: Encode, Address: Encode, Index: Encode, Call: Encode, Signature: Encode> Extrinsic for CennznetExtrinsic<AccountId, Address, Index, Call, Signature> {
	fn is_signed(&self) -> Option<bool> {
		Some(self.signature.is_some())
	}
}

impl<Address, AccountId, Index, Call, Signature, Context, Hash, BlockNumber> Checkable<Context>
for CennznetExtrinsic<AccountId, Address, Index, Call, Signature>
	where
		Address: Member + MaybeDisplay,
		Index: Member + MaybeDisplay + SimpleArithmetic,
		Compact<Index>: Encode,
		Call: Encode + Member,
		Signature: Member + traits::Verify<Signer=AccountId> + Encode,
		AccountId: Member + MaybeDisplay + Encode,
		BlockNumber: SimpleArithmetic,
		Hash: Encode,
		Address: Encode,
		Context: Lookup<Source=Address, Target=AccountId>
		+ CurrentHeight<BlockNumber=BlockNumber>
		+ BlockNumberToHash<BlockNumber=BlockNumber, Hash=Hash>,
{
	type Checked = CheckedExtrinsic<AccountId, Index, Call>;

	fn check(self, context: &Context) -> Result<Self::Checked, &'static str> {
		Ok(match self.signature {
			Some((signed, signature, index, era)) => {
				let h = context.block_number_to_hash(BlockNumber::sa(era.birth(context.current_height().as_())))
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
					return Err("bad signature in extrinsic")
				}
				// let now = match SystemTime::now().duration_since(UNIX_EPOCH) {
				// 	Ok(n)=> n.as_secs(), 
				// 	Err(_) => return Err("SystemTime before UNIX EPOCH!")
				// };
				match self.doughnut {
					Some(d) => CheckedExtrinsic {
								signed: Some((d.certificate.issuer, (raw_payload.0).0)),
								function: raw_payload.1,
					},
					None => CheckedExtrinsic {
						signed: Some((signed, (raw_payload.0).0)),
						function: raw_payload.1,
					}
				}
			}
			None => CheckedExtrinsic {
				signed: None,
				function: self.function,
			},
		})
	}
}

impl<AccountId, Address, Index, Call, Signature> Decode
for CennznetExtrinsic<AccountId, Address, Index, Call, Signature>
	where
		AccountId: Decode,
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

		let is_signed = version & 0b1000_0000 != 0;
		let has_doughtnut = version &0b0100_0000 !=0;
		let version = version & 0b0111_1111;
		if version != TRANSACTION_VERSION {
			return None
		}

		Some(CennznetExtrinsic {
			signature: if is_signed { Some(Decode::decode(input)?) } else { None },
			function: Decode::decode(input)?,
			doughnut: if has_doughtnut {Some(Decode::decode(input)?)} else { None }
		})
	}
}

impl<AccountId, Address, Index, Call, Signature> Encode
for CennznetExtrinsic<AccountId, Address, Index, Call, Signature>
	where
		Address: Encode,
		Signature: Encode,
		Compact<Index>: Encode,
		Call: Encode,
{
	fn encode(&self) -> Vec<u8> {
		encode_with_vec_prefix::<Self, _>(|v| {
			// 1 byte version id.
			match self.signature.as_ref() {
				Some(s) => {
					v.push(TRANSACTION_VERSION | 0b1000_0000);
					s.encode_to(v);
				}
				None => {
					v.push(TRANSACTION_VERSION & 0b0111_1111);
				}
			}
			self.function.encode_to(v);
		})
	}
}

#[cfg(feature = "std")]
impl<AccountId: Encode, Address: Encode, Index, Signature: Encode, Call: Encode> serde::Serialize
for CennznetExtrinsic<AccountId, Address, Index, Call, Signature>
	where Compact<Index>: Encode
{
	fn serialize<S>(&self, seq: S) -> Result<S::Ok, S::Error> where S: ::serde::Serializer {
		self.using_encoded(|bytes| seq.serialize_bytes(bytes))
	}
}

#[cfg(feature = "std")]
impl<AccountId, Address, Index, Call, Signature> fmt::Debug for CennznetExtrinsic<AccountId, Address, Index, Call, Signature> where
	AccountId: fmt::Debug,
	Address: fmt::Debug,
	Index: fmt::Debug,
	Call: fmt::Debug,
{
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "CennznetExtrinsic({:?}, {:?})", self.signature.as_ref().map(|x| (&x.0, &x.2)), self.function)
	}
}

#[cfg(test)]
mod tests {
	use runtime_primitives::codec::{Compact, Decode, Encode};
	use runtime_primitives::generic::CheckedExtrinsic;
	use runtime_primitives::generic::Era;
	use runtime_primitives::serde_derive::{Deserialize, Serialize};
	use runtime_primitives::traits::BlockNumberToHash;
	use runtime_primitives::traits::Checkable;
	use runtime_primitives::traits::CurrentHeight;
	use runtime_primitives::traits::Extrinsic;
	use runtime_primitives::traits::Lazy;
	use runtime_primitives::traits::Lookup;
	use runtime_primitives::traits::Verify;
	use substrate_primitives::blake2_256;

	use crate::cennznet_extrinsic::CennznetExtrinsic;

	struct TestContext;
	impl Lookup for TestContext {
		type Source = u64;
		type Target = u64;
		fn lookup(&self, s: u64) -> Result<u64, &'static str> { Ok(s) }
	}
	impl CurrentHeight for TestContext {
		type BlockNumber = u64;
		fn current_height(&self) -> u64 { 42 }
	}
	impl BlockNumberToHash for TestContext {
		type BlockNumber = u64;
		type Hash = u64;
		fn block_number_to_hash(&self, n: u64) -> Option<u64> { Some(n) }
	}

	#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize, Encode, Decode)]
	struct TestSig(u64, Vec<u8>);
	impl Verify for TestSig {
		type Signer = u64;
		fn verify<L: Lazy<[u8]>>(&self, mut msg: L, signer: &Self::Signer) -> bool {
			*signer == self.0 && msg.get() == &self.1[..]
		}
	}

	const DUMMY_ACCOUNTID: u64 = 0;

	type Ex = CennznetExtrinsic<u64, u64, u64, Vec<u8>, TestSig>;
	type CEx = CheckedExtrinsic<u64, u64, Vec<u8>>;

	#[test]
	fn unsigned_codec_should_work() {
		let ux = Ex::new_unsigned(vec![0u8;0]);
		let encoded = ux.encode();
		assert_eq!(Ex::decode(&mut &encoded[..]), Some(ux));
	}

	#[test]
	fn signed_codec_should_work() {
		let ux = Ex::new_signed(0, vec![0u8;0], DUMMY_ACCOUNTID, TestSig(DUMMY_ACCOUNTID, (DUMMY_ACCOUNTID, vec![0u8;0], Era::immortal(), 0u64).encode()), Era::immortal());
		let encoded = ux.encode();
		assert_eq!(Ex::decode(&mut &encoded[..]), Some(ux));
	}

	#[test]
	fn large_signed_codec_should_work() {
		let ux = Ex::new_signed(0, vec![0u8;0], DUMMY_ACCOUNTID, TestSig(DUMMY_ACCOUNTID, (DUMMY_ACCOUNTID, vec![0u8; 257], Era::immortal(), 0u64).using_encoded(blake2_256)[..].to_owned()), Era::immortal());
		let encoded = ux.encode();
		assert_eq!(Ex::decode(&mut &encoded[..]), Some(ux));
	}

	#[test]
	fn unsigned_check_should_work() {
		let ux = Ex::new_unsigned(vec![0u8;0]);
		assert!(!ux.is_signed().unwrap_or(false));
		assert!(<Ex as Checkable<TestContext>>::check(ux, &TestContext).is_ok());
	}

	#[test]
	fn badly_signed_check_should_fail() {
		let ux = Ex::new_signed(0, vec![0u8;0], DUMMY_ACCOUNTID, TestSig(DUMMY_ACCOUNTID, vec![0u8]), Era::immortal());
		assert!(ux.is_signed().unwrap_or(false));
		assert_eq!(<Ex as Checkable<TestContext>>::check(ux, &TestContext), Err("bad signature in extrinsic"));
	}

	#[test]
	fn immortal_signed_check_should_work() {
		let ux = Ex::new_signed(0, vec![0u8;0], DUMMY_ACCOUNTID, TestSig(DUMMY_ACCOUNTID, (Compact::from(DUMMY_ACCOUNTID), vec![0u8;0], Era::immortal(), 0u64).encode()), Era::immortal());
		assert!(ux.is_signed().unwrap_or(false));
		assert_eq!(<Ex as Checkable<TestContext>>::check(ux, &TestContext), Ok(CEx { signed: Some((DUMMY_ACCOUNTID, 0)), function: vec![0u8;0] }));
	}

	#[test]
	fn mortal_signed_check_should_work() {
		let ux = Ex::new_signed(0, vec![0u8;0], DUMMY_ACCOUNTID, TestSig(DUMMY_ACCOUNTID, (Compact::from(DUMMY_ACCOUNTID), vec![0u8;0], Era::mortal(32, 42), 42u64).encode()), Era::mortal(32, 42));
		assert!(ux.is_signed().unwrap_or(false));
		assert_eq!(<Ex as Checkable<TestContext>>::check(ux, &TestContext), Ok(CEx { signed: Some((DUMMY_ACCOUNTID, 0)), function: vec![0u8;0] }));
	}

	#[test]
	fn later_mortal_signed_check_should_work() {
		let ux = Ex::new_signed(0, vec![0u8;0], DUMMY_ACCOUNTID, TestSig(DUMMY_ACCOUNTID, (Compact::from(DUMMY_ACCOUNTID), vec![0u8;0], Era::mortal(32, 11), 11u64).encode()), Era::mortal(32, 11));
		assert!(ux.is_signed().unwrap_or(false));
		assert_eq!(<Ex as Checkable<TestContext>>::check(ux, &TestContext), Ok(CEx { signed: Some((DUMMY_ACCOUNTID, 0)), function: vec![0u8;0] }));
	}

	#[test]
	fn too_late_mortal_signed_check_should_fail() {
		let ux = Ex::new_signed(0, vec![0u8;0], DUMMY_ACCOUNTID, TestSig(DUMMY_ACCOUNTID, (DUMMY_ACCOUNTID, vec![0u8;0], Era::mortal(32, 10), 10u64).encode()), Era::mortal(32, 10));
		assert!(ux.is_signed().unwrap_or(false));
		assert_eq!(<Ex as Checkable<TestContext>>::check(ux, &TestContext), Err("bad signature in extrinsic"));
	}

	#[test]
	fn too_early_mortal_signed_check_should_fail() {
		let ux = Ex::new_signed(0, vec![0u8;0], DUMMY_ACCOUNTID, TestSig(DUMMY_ACCOUNTID, (DUMMY_ACCOUNTID, vec![0u8;0], Era::mortal(32, 43), 43u64).encode()), Era::mortal(32, 43));
		assert!(ux.is_signed().unwrap_or(false));
		assert_eq!(<Ex as Checkable<TestContext>>::check(ux, &TestContext), Err("bad signature in extrinsic"));
	}

	#[test]
	fn encoding_matches_vec() {
		let ex = Ex::new_unsigned(vec![0u8;0]);
		let encoded = ex.encode();
		let decoded = Ex::decode(&mut encoded.as_slice()).unwrap();
		assert_eq!(decoded, ex);
		let as_vec: Vec<u8> = Decode::decode(&mut encoded.as_slice()).unwrap();
		assert_eq!(as_vec.encode(), encoded);
	}
}
