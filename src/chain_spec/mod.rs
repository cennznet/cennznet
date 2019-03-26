//! CENNZNET chain configurations.

use cennznet_primitives::AccountId;
pub use cennznet_runtime::GenesisConfig;
use primitives::{ed25519, Ed25519AuthorityId as AuthorityId};
use substrate_service;

use substrate_keystore::pad_seed;

pub mod dev;
pub mod mainnet;
pub mod testnet;

pub const TELEMETRY_URL: &str = "ws://cennznet-telemetry.centrality.me:1024";

/// Specialised `ChainSpec`.
pub type ChainSpec = substrate_service::ChainSpec<GenesisConfig>;

/// Helper function to generate AuthorityID from seed
pub fn get_account_id_from_seed(seed: &str) -> AccountId {
	let padded_seed = pad_seed(seed);
	// NOTE from ed25519 impl:
	// prefer pkcs#8 unless security doesn't matter -- this is used primarily for tests.
	ed25519::Pair::from_seed(&padded_seed).public().0.into()
}

/// Helper function to generate stash, controller and session key from seed
pub fn get_authority_keys_from_seed(seed: &str) -> (AccountId, AccountId, AuthorityId) {
	let padded_seed = pad_seed(seed);
	// NOTE from ed25519 impl:
	// prefer pkcs#8 unless security doesn't matter -- this is used primarily for tests.
	(
		get_account_id_from_seed(&format!("{}-stash", seed)),
		get_account_id_from_seed(seed),
		ed25519::Pair::from_seed(&padded_seed).public().0.into(),
	)
}
