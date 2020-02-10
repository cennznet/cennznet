// Copyright 2018-2020 Parity Technologies (UK) Ltd. and Centrality Investments Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

//! Kauri genesis config

use super::{config_genesis, get_account_id_from_seed, get_authority_keys_from_seed, ChainSpec, NetworkKeys};
use sp_core::sr25519;

fn network_keys() -> NetworkKeys {
	let endowed_accounts = vec![
		get_account_id_from_seed::<sr25519::Public>("Andrea"),
		get_account_id_from_seed::<sr25519::Public>("Brooke"),
		get_account_id_from_seed::<sr25519::Public>("Courtney"),
		get_account_id_from_seed::<sr25519::Public>("Drew"),
		get_account_id_from_seed::<sr25519::Public>("Emily"),
		get_account_id_from_seed::<sr25519::Public>("Frank"),
		get_account_id_from_seed::<sr25519::Public>("Centrality"),
		get_account_id_from_seed::<sr25519::Public>("Kauri"),
		get_account_id_from_seed::<sr25519::Public>("Rimu"),
		get_account_id_from_seed::<sr25519::Public>("cennznet-js-test"),
		get_account_id_from_seed::<sr25519::Public>("Andrea//stash"),
		get_account_id_from_seed::<sr25519::Public>("Brooke//stash"),
		get_account_id_from_seed::<sr25519::Public>("Courtney//stash"),
		get_account_id_from_seed::<sr25519::Public>("Drew//stash"),
		get_account_id_from_seed::<sr25519::Public>("Emily//stash"),
		get_account_id_from_seed::<sr25519::Public>("Frank//stash"),
		get_account_id_from_seed::<sr25519::Public>("Centrality//stash"),
		get_account_id_from_seed::<sr25519::Public>("Kauri//stash"),
		get_account_id_from_seed::<sr25519::Public>("Rimu//stash"),
		get_account_id_from_seed::<sr25519::Public>("cennznet-js-test//stash"),
	];
	let initial_authorities = vec![
		get_authority_keys_from_seed("Andrea"),
		get_authority_keys_from_seed("Brooke"),
		get_authority_keys_from_seed("Courtney"),
		get_authority_keys_from_seed("Drew"),
	];
	let root_key = get_account_id_from_seed::<sr25519::Public>("Kauri");

	NetworkKeys {
		endowed_accounts,
		initial_authorities,
		root_key,
	}
}

/// Returns ChainSpec for kauri
pub fn config() -> ChainSpec {
	ChainSpec::from_genesis(
		"Kauri CENNZnet",
		"Kauri",
		|| config_genesis(network_keys(), false),
		vec![],
		None,
		Some("kauri"),
		None,
		Default::default(),
	)
}
