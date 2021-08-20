// Copyright (C) 2020-2021 Parity Technologies (UK) Ltd. and Centrality Investment Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use cennznet_primitives::eth::{crypto::Signature, witness::Witness, ValidatorSetId};
use sp_std::{cmp, collections::HashMap, prelude::*};

/// Tracks live witnesses
///
/// Stores witnesses per message nonce and digest
/// nonce -> digest -> (authority, signature)
/// this structure allows resiliency incase different digests are witnessed, maliciously or not.
pub struct WitnessRecord {
	record: HashMap<Nonce, HashMap<H256, Vec<(AuthorityId, Signature)>>>,
	has_voted: HashMap<Nonce, bitvec::BitVec>,
}

impl WitnessRecord {
	fn note(&mut self, witness: Witness) {
		// TODO: if we have something from this authority already, ignore
		// TODO: only consider this vote if the signature checks out!
		let has_voted = self
			.has_voted
			.get(witness.nonce)
			.get(witness.authority_id)
			.unwrap_or(false);

		if has_voted {
			// TODO: log/ return something useful
			return;
		}

		if !self.record.contains_key(&witness.nonce) {
			// first witness for this nonce
			let mut digest_signatures = HashMap::<H256, Vec<(AuthorityId, Signature)>>::default();
			digest_signatures.insert(witness.digest, vec![(witness.authority_id, witness.signature)]);
			self.record.insert(&witness.nonce, digest_signatures);
		} else if !self.record.get(&witness.nonce).contains_key(&witness.digest) {
			// first witness for this digest
			let digest_signatures = vec![(witness.authority_id, witness.signature)];
			self.record
				.get_mut(&witness.nonce)
				.insert(&witness.digest, digest_signatures);
		} else {
			// add witness to known (nonce, digest)
			let mut signatures = self.record.get(&witness.nonce).get_mut(&witness.digest);
			signatures.push((witness.authority_id, witness.signature));
			self.record.get_mut(&witness.nonce).insert(&witness.digest, signatures);
		}

		self.has_voted.get(witness.nonce).set(witness.authority_id, true);
	}
}

/// A witness signed by GRANDPA validators as part of ETHY protocol.
///
/// The witness contains a [payload] extracted from the finalized block at height [block_number].
/// GRANDPA validators collect signatures on witnesss and a stream of such signed witnesss
/// (see [SignedWitness]) forms the ETHY protocol.
#[derive(Clone, Debug, PartialEq, Eq, codec::Encode, codec::Decode)]
pub struct Witness<TBlockNumber, TPayload> {
	/// The payload being signed.
	pub payload: TPayload,
	/// Validator set is changing once per epoch. The Light Client must be provided by details about
	/// the validator set whenever it's importing first witness with a new `validator_set_id`.
	/// Validator set data MUST be verifiable, for instance using [payload] information.
	pub validator_set_id: ValidatorSetId,
}

impl<TBlockNumber, TPayload> cmp::PartialOrd for Witness<TBlockNumber, TPayload>
where
	TBlockNumber: cmp::Ord,
	TPayload: cmp::Eq,
{
	fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
		Some(self.cmp(other))
	}
}

impl<TBlockNumber, TPayload> cmp::Ord for Witness<TBlockNumber, TPayload>
where
	TBlockNumber: cmp::Ord,
	TPayload: cmp::Eq,
{
	fn cmp(&self, other: &Self) -> cmp::Ordering {
		self.validator_set_id
			.cmp(&other.validator_set_id)
			.then_with(|| self.block_number.cmp(&other.block_number))
	}
}

/// A witness with matching GRANDPA validators' signatures.
#[derive(Clone, Debug, PartialEq, Eq, codec::Encode, codec::Decode)]
pub struct SignedWitness<TBlockNumber, TPayload> {
	/// The witness signatures are collected for.
	pub witness: Witness<TBlockNumber, TPayload>,
	/// GRANDPA validators' signatures for the witness.
	///
	/// The length of this `Vec` must match number of validators in the current set (see
	/// [Witness::validator_set_id]).
	pub signatures: Vec<Option<Signature>>,
}

impl<TBlockNumber, TPayload> SignedWitness<TBlockNumber, TPayload> {
	/// Return the number of collected signatures.
	pub fn no_of_signatures(&self) -> usize {
		self.signatures.iter().filter(|x| x.is_some()).count()
	}
}

/// A [SignedWitness] with a version number. This variant will be appended
/// to the block justifications for the block for which the signed witness
/// has been generated.
#[derive(Clone, Debug, PartialEq, codec::Encode, codec::Decode)]
pub enum VersionedWitness<N, P> {
	#[codec(index = 1)]
	/// Current active version
	V1(SignedWitness<N, P>),
}

#[cfg(test)]
mod tests {

	use sp_core::{keccak_256, Pair};
	use sp_keystore::{testing::KeyStore, SyncCryptoStore, SyncCryptoStorePtr};

	use super::*;
	use codec::Decode;

	use crate::{crypto, KEY_TYPE};

	type TestWitness = Witness<u128, String>;
	type TestSignedWitness = SignedWitness<u128, String>;
	type TestVersionedWitness = VersionedWitness<u128, String>;

	// The mock signatures are equivalent to the ones produced by the ETHY keystore
	fn mock_signatures() -> (crypto::Signature, crypto::Signature) {
		let store: SyncCryptoStorePtr = KeyStore::new().into();

		let alice = sp_core::ecdsa::Pair::from_string("//Alice", None).unwrap();
		let _ = SyncCryptoStore::insert_unknown(&*store, KEY_TYPE, "//Alice", alice.public().as_ref()).unwrap();

		let msg = keccak_256(b"This is the first message");
		let sig1 = SyncCryptoStore::ecdsa_sign_prehashed(&*store, KEY_TYPE, &alice.public(), &msg)
			.unwrap()
			.unwrap();

		let msg = keccak_256(b"This is the second message");
		let sig2 = SyncCryptoStore::ecdsa_sign_prehashed(&*store, KEY_TYPE, &alice.public(), &msg)
			.unwrap()
			.unwrap();

		(sig1.into(), sig2.into())
	}

	#[test]
	fn witness_encode_decode() {
		// given
		let witness: TestWitness = Witness {
			payload: "Hello World!".into(),
			block_number: 5,
			validator_set_id: 0,
		};

		// when
		let encoded = codec::Encode::encode(&witness);
		let decoded = TestWitness::decode(&mut &*encoded);

		// then
		assert_eq!(decoded, Ok(witness));
		assert_eq!(
			encoded,
			hex_literal::hex!("3048656c6c6f20576f726c6421050000000000000000000000000000000000000000000000")
		);
	}

	#[test]
	fn signed_witness_encode_decode() {
		// given
		let witness: TestWitness = Witness {
			payload: "Hello World!".into(),
			block_number: 5,
			validator_set_id: 0,
		};

		let sigs = mock_signatures();

		let signed = SignedWitness {
			witness,
			signatures: vec![None, None, Some(sigs.0), Some(sigs.1)],
		};

		// when
		let encoded = codec::Encode::encode(&signed);
		let decoded = TestSignedWitness::decode(&mut &*encoded);

		// then
		assert_eq!(decoded, Ok(signed));
		assert_eq!(
			encoded,
			hex_literal::hex!("3048656c6c6f20576f726c642105000000000000000000000000000000000000000000000010000001558455ad81279df0795cc985580e4fb75d72d948d1107b2ac80a09abed4da8480c746cc321f2319a5e99a830e314d10dd3cd68ce3dc0c33c86e99bcb7816f9ba01012d6e1f8105c337a86cdd9aaacdc496577f3db8c55ef9e6fd48f2c5c05a2274707491635d8ba3df64f324575b7b2a34487bca2324b6a0046395a71681be3d0c2a00")
		);
	}

	#[test]
	fn signed_witness_count_signatures() {
		// given
		let witness: TestWitness = Witness {
			payload: "Hello World!".into(),
			block_number: 5,
			validator_set_id: 0,
		};

		let sigs = mock_signatures();

		let mut signed = SignedWitness {
			witness,
			signatures: vec![None, None, Some(sigs.0), Some(sigs.1)],
		};
		assert_eq!(signed.no_of_signatures(), 2);

		// when
		signed.signatures[2] = None;

		// then
		assert_eq!(signed.no_of_signatures(), 1);
	}

	#[test]
	fn witness_ordering() {
		fn witness(block_number: u128, validator_set_id: crate::ValidatorSetId) -> TestWitness {
			Witness {
				payload: "Hello World!".into(),
				block_number,
				validator_set_id,
			}
		}

		// given
		let a = witness(1, 0);
		let b = witness(2, 1);
		let c = witness(10, 0);
		let d = witness(10, 1);

		// then
		assert!(a < b);
		assert!(a < c);
		assert!(c < b);
		assert!(c < d);
		assert!(b < d);
	}

	#[test]
	fn versioned_witness_encode_decode() {
		let witness: TestWitness = Witness {
			payload: "Hello World!".into(),
			block_number: 5,
			validator_set_id: 0,
		};

		let sigs = mock_signatures();

		let signed = SignedWitness {
			witness,
			signatures: vec![None, None, Some(sigs.0), Some(sigs.1)],
		};

		let versioned = TestVersionedWitness::V1(signed.clone());

		let encoded = codec::Encode::encode(&versioned);

		assert_eq!(1, encoded[0]);
		assert_eq!(encoded[1..], codec::Encode::encode(&signed));

		let decoded = TestVersionedWitness::decode(&mut &*encoded);

		assert_eq!(decoded, Ok(versioned));
	}
}
