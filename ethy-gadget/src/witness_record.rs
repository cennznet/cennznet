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

use cennznet_primitives::eth::{
	crypto::{AuthorityId, AuthoritySignature as Signature},
	EventId, Witness,
};
use log::{debug, error, info, trace, warn};
use std::{collections::HashMap, prelude::*};

/// Tracks live witnesses
///
/// Stores witnesses per message event_id and digest
/// event_id -> digest -> [](authority, signature)
/// this structure allows resiliency incase different digests are witnessed, maliciously or not.
#[derive(Default)]
pub struct WitnessRecord {
	record: HashMap<EventId, HashMap<[u8; 32], Vec<(AuthorityId, Signature)>>>,
	has_voted: HashMap<EventId, Vec<AuthorityId>>,
}

impl WitnessRecord {
	/// Remove a witness record from memory
	pub fn clear(&mut self, event_id: EventId) {
		self.record.remove(&event_id);
	}
	/// Return all known signatures for the witness on (event_id, digest)
	pub fn signatures_for(
		&self,
		event_id: EventId,
		digest: &[u8; 32],
		validators: Vec<AuthorityId>,
	) -> Vec<Option<Signature>> {
		// TODO: can probably do better by storing this in sorted order to begin with...
		let proofs = self.record.get(&event_id).unwrap().get(digest).unwrap();
		validators
			.iter()
			.map(|v| proofs.iter().find(|(id, _sig)| v == id).map(|(_id, sig)| sig.clone()))
			.collect()
	}
	/// Does the event identified by `event_id` `digest` have >= `threshold` support
	pub fn has_consensus(&self, event_id: EventId, digest: &[u8; 32], threshold: usize) -> bool {
		self.record
			.get(&event_id)
			.and_then(|x| x.get(digest))
			.and_then(|v| Some(v.len()))
			.unwrap_or_default()
			>= threshold
	}
	/// Note a witness if we haven't seen it before
	pub fn note(&mut self, witness: &Witness) {
		if self
			.has_voted
			.get(&witness.event_id)
			.map(|votes| votes.binary_search(&witness.authority_id).is_ok())
			.unwrap_or_default()
		{
			// TODO: log/ return something useful
			trace!(target: "ethy", "💎 witness previously seen: {:?}", witness.event_id);
			return;
		}

		if !self.record.contains_key(&witness.event_id) {
			// first witness for this event_id
			let mut digest_signatures = HashMap::<[u8; 32], Vec<(AuthorityId, Signature)>>::default();
			digest_signatures.insert(
				witness.digest,
				vec![(witness.authority_id.clone(), witness.signature.clone())],
			);
			self.record.insert(witness.event_id, digest_signatures);
		} else if !self
			.record
			.get(&witness.event_id)
			.map(|x| x.contains_key(&witness.digest))
			.unwrap_or(false)
		{
			// first witness for this digest
			let digest_signatures = vec![(witness.authority_id.clone(), witness.signature.clone())];
			self.record
				.get_mut(&witness.event_id)
				.unwrap()
				.insert(witness.digest, digest_signatures);
		} else {
			// add witness to known (event_id, digest)
			self.record
				.get_mut(&witness.event_id)
				.unwrap()
				.get_mut(&witness.digest)
				.unwrap()
				.push((witness.authority_id.clone(), witness.signature.clone()));
		}
		trace!(target: "ethy", "💎 witness recorded: {:?}", witness.event_id);

		// Mark authority as voted
		match self.has_voted.get_mut(&witness.event_id) {
			None => {
				// first vote for this event_id we've seen
				self.has_voted
					.insert(witness.event_id, vec![witness.authority_id.clone()]);
			}
			Some(votes) => {
				// subsequent vote for a known event_id
				if let Err(idx) = votes.binary_search(&witness.authority_id) {
					votes.insert(idx, witness.authority_id.clone());
				}
			}
		}
	}
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
		assert_eq!(signed.signature_count(), 2);

		// when
		signed.signatures[2] = None;

		// then
		assert_eq!(signed.signature_count(), 1);
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
