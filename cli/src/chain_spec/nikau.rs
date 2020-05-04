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

//! CENNZNet Nikau V1 test net genesis config
use super::{config_genesis, get_account_id_from_seed, get_authority_keys_from_seed, ChainSpec, NetworkKeys};
use sp_core::sr25519;

fn network_keys() -> NetworkKeys {
	let endowed_accounts = vec![
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		get_account_id_from_seed::<sr25519::Public>("Bob"),
		get_account_id_from_seed::<sr25519::Public>("Charlie"),
		get_account_id_from_seed::<sr25519::Public>("Dave"),
		get_account_id_from_seed::<sr25519::Public>("Eve"),
		get_account_id_from_seed::<sr25519::Public>("Nikau"),
		get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
		get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
		get_account_id_from_seed::<sr25519::Public>("Charlie//stash"),
		get_account_id_from_seed::<sr25519::Public>("Dave//stash"),
		get_account_id_from_seed::<sr25519::Public>("Eve//stash"),
		get_account_id_from_seed::<sr25519::Public>("Nikau//stash"),
	];
	let initial_authorities = vec![
		get_authority_keys_from_seed("Alice"),
		get_authority_keys_from_seed("Bob"),
		get_authority_keys_from_seed("Charlie"),
		get_authority_keys_from_seed("Dave"),
		get_authority_keys_from_seed("Eve"),
	];
	let root_key = get_account_id_from_seed::<sr25519::Public>("Nikau");

	NetworkKeys {
		endowed_accounts,
		initial_authorities,
		root_key,
	}
}

/// Returns ChainSpec for Nikau test net
pub fn config() -> ChainSpec {
	ChainSpec::from_genesis(
		"CENNZnet Nikau",                        // name
		"CENNZnet Nikau V1",                     // ID
		|| config_genesis(network_keys(), true), // constructor
		vec![],                                  // boot nodes
		None,                                    // telemetry
		Some("cennznet-nikau-v1"),               // lib-p2p protocol ID
		None,                                    // properties
		Default::default(),                      // generic extension types
	)
}
