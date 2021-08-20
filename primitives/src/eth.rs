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

//! Ethereum bridge common types
//! shared between crml/eth-bridge runtime & ethy-gadget client

use codec::{Decode, Encode};
use sp_core::H256;
use sp_runtime::KeyTypeId;
use sp_std::prelude::*;

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

/// A message for signing
pub type Message = Vec<u8>;

/// Unique nonce for signed message requests
pub type Nonce = u64;

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
	#[codec(index = 3)]
	OpaqueSigningRequest((Message, Nonce)),
}

/// ETHY witness message.
///
/// A vote message is a direct vote created by a ETHY node on every voting round
/// and is gossiped to its peers.
#[derive(Debug, Decode, Encode)]
pub struct Witness {
	/// The message hash. `keccak(abi.encodePacked(message, nonce))`
	pub digest: H256,
	/// Message nonce
	pub nonce: Nonce,
	/// Node authority id
	pub authority_id: AuthorityIndex,
	/// Node signature
	/// Over `keccak(abi.encodePacked(self.message, self.nonce))`
	pub signature: sp_application_crypto::ecdsa::Signature,
}

sp_api::decl_runtime_apis! {
	/// API necessary for ETHY voters.
	pub trait EthyApi
	{
		/// Return the current active ETHY validator set
		fn validator_set() -> ValidatorSet<crypto::AuthorityId>;
	}
}
