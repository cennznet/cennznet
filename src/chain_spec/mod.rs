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
//! CENNZNET chain configurations.

use cennznet_primitives::AccountId;
pub use cennznet_runtime::GenesisConfig;
use primitives::{ed25519, ed25519::Public as AuthorityId, sr25519, Pair};
use substrate_service;

pub mod dev;
pub mod mainnet;
pub mod testnet;

pub const TELEMETRY_URL: &str = "ws://cennznet-telemetry.centrality.me:1024";

/// Specialised `ChainSpec`.
pub type ChainSpec = substrate_service::ChainSpec<GenesisConfig>;

pub struct NetworkKeys {
	pub endowed_accounts: Vec<AccountId>,
	pub initial_authorities: Vec<(AccountId, AccountId, AuthorityId)>,
	pub root_key: AccountId,
}

/// Helper function to generate AccountId from seed
pub fn get_account_id_from_seed(seed: &str) -> AccountId {
	sr25519::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

/// Helper function to generate AuthorityId from seed
pub fn get_session_key_from_seed(seed: &str) -> AuthorityId {
	ed25519::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

/// Helper function to generate stash, controller and session key from seed
pub fn get_authority_keys_from_seed(seed: &str) -> (AccountId, AccountId, AuthorityId) {
	(
		get_account_id_from_seed(&format!("{}//stash", seed)),
		get_account_id_from_seed(seed),
		get_session_key_from_seed(seed),
	)
}
