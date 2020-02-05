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

//! Dev genesis config

use super::{config_genesis, get_account_id_from_seed, get_authority_keys_from_seed, ChainSpec, NetworkKeys};

fn network_keys() -> NetworkKeys {
	let endowed_accounts = vec![
		get_account_id_from_seed("Alice"),
		get_account_id_from_seed("Bob"),
		get_account_id_from_seed("Charlie"),
		get_account_id_from_seed("Dave"),
		get_account_id_from_seed("Eve"),
		get_account_id_from_seed("Ferdie"),
		get_account_id_from_seed("Kauri"),
		get_account_id_from_seed("Rimu"),
		get_account_id_from_seed("Alice//stash"),
		get_account_id_from_seed("Bob//stash"),
		get_account_id_from_seed("Charlie//stash"),
		get_account_id_from_seed("Dave//stash"),
		get_account_id_from_seed("Eve//stash"),
		get_account_id_from_seed("Ferdie//stash"),
	];
	let initial_authorities = vec![get_authority_keys_from_seed("Alice")];
	let root_key = get_account_id_from_seed("Alice");

	NetworkKeys {
		endowed_accounts,
		initial_authorities,
		root_key,
	}
}

pub fn config() -> ChainSpec {
	ChainSpec::from_genesis(
		"Development",
		"dev",
		|| config_genesis(network_keys(), true),
		vec![],
		None,
		None,
		None,
		Default::default(),
	)
}
