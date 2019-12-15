// Copyright 2018-2019 Parity Technologies (UK) Ltd. and Centrality Investments Ltd.
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

fn network_keys() -> NetworkKeys {
	let endowed_accounts = vec![
		get_account_id_from_seed("Andrea"),
		get_account_id_from_seed("Brooke"),
		get_account_id_from_seed("Courtney"),
		get_account_id_from_seed("Drew"),
		get_account_id_from_seed("Emily"),
		get_account_id_from_seed("Frank"),
		get_account_id_from_seed("Centrality"),
		get_account_id_from_seed("Kauri"),
		get_account_id_from_seed("Rimu"),
		get_account_id_from_seed("cennznet-js-test"),
		get_account_id_from_seed("Andrea//stash"),
		get_account_id_from_seed("Brooke//stash"),
		get_account_id_from_seed("Courtney//stash"),
		get_account_id_from_seed("Drew//stash"),
		get_account_id_from_seed("Emily//stash"),
		get_account_id_from_seed("Frank//stash"),
		get_account_id_from_seed("Centrality//stash"),
		get_account_id_from_seed("Kauri//stash"),
		get_account_id_from_seed("Rimu//stash"),
		get_account_id_from_seed("cennznet-js-test//stash"),
	];
	let initial_authorities = vec![
		get_authority_keys_from_seed("Andrea"),
		get_authority_keys_from_seed("Brooke"),
		get_authority_keys_from_seed("Courtney"),
		get_authority_keys_from_seed("Drew"),
	];
	let root_key = get_account_id_from_seed("Kauri");

	NetworkKeys {
		endowed_accounts,
		initial_authorities,
		root_key,
	}
}

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
