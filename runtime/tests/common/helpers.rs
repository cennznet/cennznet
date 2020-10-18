/* Copyright 2019-2020 Centrality Investments Limited
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

//! Test helper functions

use cennznet_cli::chain_spec::{get_authority_keys_from_seed, AuthorityKeys};
use cennznet_primitives::types::{Balance, BlockNumber, Header};
use cennznet_runtime::{CheckedExtrinsic, Runtime, UncheckedExtrinsic};
use codec::Encode;
use frame_support::weights::GetDispatchInfo;
use sp_runtime::{testing::Digest, traits::Header as HeaderT};

/// A genesis hash to use for extrinsic signing
const GENESIS_HASH: [u8; 32] = [69u8; 32];
/// The runtime spec version number for signing
const SPEC_VERSION: u32 = cennznet_runtime::VERSION.spec_version;
/// The runtime tx version number for signing
const TX_VERSION: u32 = cennznet_runtime::VERSION.transaction_version;

/// Sign the given `CheckedExtrinsic` `xt` using pre-configured genesis hash, spec version, and tx version, values.
pub fn sign(xt: CheckedExtrinsic) -> UncheckedExtrinsic {
	super::keyring::sign(xt, SPEC_VERSION, TX_VERSION, GENESIS_HASH)
}

/// Calculate the transaction fees of `xt` according to the current runtime implementation.
/// Ignores tip.
pub fn extrinsic_fee_for(xt: &UncheckedExtrinsic) -> Balance {
	crml_transaction_payment::Module::<Runtime>::compute_fee(xt.encode().len() as u32, &xt.get_dispatch_info(), 0)
}

pub fn header_for_block_number(n: BlockNumber) -> Header {
	HeaderT::new(
		n,                        // block number
		sp_core::H256::default(), // extrinsics_root
		sp_core::H256::default(), // state_root
		GENESIS_HASH.into(),      // parent_hash
		Digest::default(),        // digest
	)
}

pub fn header() -> Header {
	header_for_block_number(1)
}

/// Get `n` (stash, controller, session[]) keys for test network authorities
pub fn make_authority_keys(n: usize) -> Vec<AuthorityKeys> {
	assert!(n < 7, "This function provides at most 6 authorities");
	// note: this could be extended arbitrarily with additional seeds
	// provided the matching stash and controller accounts are also funded
	let accounts = vec!["Alice", "Bob", "Charlie", "Dave", "Eve", "Ferdie"];
	accounts
		.iter()
		.take(n)
		.map(|s| get_authority_keys_from_seed(s))
		.collect()
}
