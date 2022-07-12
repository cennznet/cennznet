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

use super::{config_genesis, get_account_id_from_seed, get_authority_keys_from_seed, CENNZnetChainSpec, NetworkKeys};
use sc_service::ChainType;
use sp_core::sr25519;
use std::str::FromStr;

fn network_keys() -> NetworkKeys {
	let endowed_accounts = vec![
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		get_account_id_from_seed::<sr25519::Public>("Bob"),
		get_account_id_from_seed::<sr25519::Public>("Charlie"),
		get_account_id_from_seed::<sr25519::Public>("Dave"),
		get_account_id_from_seed::<sr25519::Public>("Eve"),
		get_account_id_from_seed::<sr25519::Public>("Ferdie"),
		get_account_id_from_seed::<sr25519::Public>("Kauri"),
		get_account_id_from_seed::<sr25519::Public>("Rimu"),
		get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
		get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
		get_account_id_from_seed::<sr25519::Public>("Charlie//stash"),
		get_account_id_from_seed::<sr25519::Public>("Dave//stash"),
		get_account_id_from_seed::<sr25519::Public>("Eve//stash"),
		get_account_id_from_seed::<sr25519::Public>("Ferdie//stash"),
		// this is the dervied ss58 address for the development evm address ('0x12B29179a7F858478Fde74f842126CdA5eA7AC35')
		sp_runtime::AccountId32::from_str("5EK7n4pa3FcCGoxvnJ4Qghe4xHJLJyNT6vsPWEJaSWUiTCVp")
			.expect("address is valid ss58"),
	];
	let initial_authorities = vec![get_authority_keys_from_seed("Alice")];
	let root_key = get_account_id_from_seed::<sr25519::Public>("Alice");

	NetworkKeys {
		endowed_accounts,
		initial_authorities,
		root_key,
	}
}

/// Returns ChainSpec for dev
pub fn config() -> CENNZnetChainSpec {
	CENNZnetChainSpec::from_genesis(
		"Development",
		"dev",
		ChainType::Development,
		|| config_genesis(network_keys()),
		vec![],
		None,
		None,
		None,
		Default::default(),
		Default::default(),
	)
}
