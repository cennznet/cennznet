/* Copyright 2021 Centrality Investments Limited
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

//! Ethereum common types
//! shared between crml/eth-bridge runtime and ethy-gadget client
use codec::{Decode, Encode};
use sp_core::ecdsa::Public;
use sp_runtime::{traits::Convert, KeyTypeId};
use sp_std::{convert::TryInto, prelude::*};

use self::crypto::AuthoritySignature;

/// The `ConsensusEngineId` of ETHY.
pub const ETHY_ENGINE_ID: sp_runtime::ConsensusEngineId = *b"ETH-";

/// Authority set id starts with zero at genesis
pub const GENESIS_AUTHORITY_SET_ID: u64 = 0;

/// The session key type for Ethereum bridge
pub const ETH_BRIDGE_KEY_TYPE: KeyTypeId = KeyTypeId(*b"eth-");

/// Crypto types for Eth bridge protocol
pub mod crypto {
	mod app_crypto {
		use crate::eth::ETH_BRIDGE_KEY_TYPE;
		use sp_application_crypto::{app_crypto, ecdsa};
		app_crypto!(ecdsa, ETH_BRIDGE_KEY_TYPE);
	}
	sp_application_crypto::with_pair! {
		/// An eth bridge keypair using ecdsa as its crypto.
		pub type AuthorityPair = app_crypto::Pair;
	}
	/// An eth bridge signature using ecdsa as its crypto.
	pub type AuthoritySignature = app_crypto::Signature;
	/// An eth bridge identifier using ecdsa as its crypto.
	pub type AuthorityId = app_crypto::Public;
}

/// The index of an authority.
pub type AuthorityIndex = u32;

/// An event message for signing
pub type Message = Vec<u8>;

/// Unique nonce for event proof requests
pub type EventId = u64;

/// A typedef for validator set id.
pub type ValidatorSetId = u64;

/// A set of ETHY authorities, a.k.a. validators.
#[derive(Decode, Encode, Debug, PartialEq, Clone)]
pub struct ValidatorSet<AuthorityId> {
	/// Public keys of the validator set elements
	pub validators: Vec<AuthorityId>,
	/// Identifier of the validator set
	pub id: ValidatorSetId,
}

impl<AuthorityId> ValidatorSet<AuthorityId> {
	/// Return an empty validator set with id of 0.
	pub fn empty() -> Self {
		Self {
			validators: Default::default(),
			id: Default::default(),
		}
	}
}

/// Convert an Ethy secp256k1 public key into an Ethereum addresses
pub struct EthyEcdsaToEthereum;
impl Convert<Public, Option<[u8; 20]>> for EthyEcdsaToEthereum {
	fn convert(a: Public) -> Option<[u8; 20]> {
		use sp_core::crypto::Public;
		let compressed_key = a.as_slice();

		libsecp256k1::PublicKey::parse_slice(compressed_key, Some(libsecp256k1::PublicKeyFormat::Compressed))
			// uncompress the key
			.map(|pub_key| pub_key.serialize().to_vec())
			// now convert to ETH address
			.map(|uncompressed| sp_io::hashing::keccak_256(&uncompressed[1..])[12..].try_into().ok())
			.unwrap_or_default()
	}
}

/// A consensus log item for ETHY.
#[derive(Decode, Encode)]
pub enum ConsensusLog<AuthorityId: Encode + Decode> {
	/// The authorities have changed.
	#[codec(index = 1)]
	AuthoritiesChange(ValidatorSet<AuthorityId>),
	/// Disable the authority with given index.
	#[codec(index = 2)]
	OnDisabled(AuthorityIndex),
	/// A request to sign some data was logged
	/// `Message` is packed bytes e.g. `abi.encodePacked(param0, param1, paramN, validatorSetId, event_id)`
	#[codec(index = 3)]
	OpaqueSigningRequest((Message, EventId)),
	#[codec(index = 4)]
	/// Signal an `AuthoritiesChange` is scheduled for next session
	/// Generate a proof that the current validator set has witnessed the new authority set
	PendingAuthoritiesChange((ValidatorSet<AuthorityId>, EventId)),
}

/// ETHY witness message.
///
/// A witness message is a vote created by an ETHY node for a given 'event' combination
/// and is gossiped to its peers.
#[derive(Clone, Debug, Decode, Encode, PartialEq, Eq)]
pub struct Witness {
	/// The event hash: `keccak(abi.encodePacked(param0, param1, paramN, validator_set_id, event_id))`
	pub digest: [u8; 32],
	/// Event nonce (it is unique across all Ethy event proofs)
	pub event_id: EventId,
	/// The validator set witnessing the message
	pub validator_set_id: ValidatorSetId,
	/// Node public key (i.e. Ethy session key)
	pub authority_id: crypto::AuthorityId,
	/// Node signature
	/// Over `keccak(abi.encodePacked(self.message, self.nonce))`
	/// a 512-bit value, plus 8 bits for recovery ID.
	pub signature: crypto::AuthoritySignature,
}

/// A witness with matching GRANDPA validators' signatures.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct EventProof {
	/// The event witnessed
	/// The hash of: `keccak(abi.encode(param0, param1, ..,paramN, validator_set_id, event_id))`
	pub digest: [u8; 32],
	/// The witness signatures are collected for this event.
	pub event_id: EventId,
	/// The validators set Id that signed the proof
	pub validator_set_id: ValidatorSetId,
	/// GRANDPA validators' signatures for the witness.
	///
	/// The length of this `Vec` must match number of validators in the current set (see
	/// [Witness::validator_set_id]).
	pub signatures: Vec<crypto::AuthoritySignature>,
	/// Block hash of the event
	pub block: [u8; 32],
	/// Metadata tag for the event
	pub tag: Option<Vec<u8>>,
}

impl EventProof {
	/// Return the number of collected signatures.
	pub fn signature_count(&self) -> usize {
		let empty_sig = AuthoritySignature::default();
		self.signatures.iter().filter(|x| x != &&empty_sig).count()
	}
}

/// A [EventProof] with a version number. This variant will be appended
/// to the block justifications for the block for which the signed witness
/// has been generated.
#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub enum VersionedEventProof {
	#[codec(index = 1)]
	/// Current active version
	V1(EventProof),
}

sp_api::decl_runtime_apis! {
	/// API necessary for ETHY voters.
	pub trait EthyApi
	{
		/// Return the current active ETHY validator set
		fn validator_set() -> ValidatorSet<crypto::AuthorityId>;
	}
}
