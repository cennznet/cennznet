// Copyright 2019 Parity Technologies (UK) Ltd.
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

//! Genesis Configuration.

use crate::keyring::*;
use cennznet_runtime::constants::currency::*;
use cennznet_runtime::{
	ContractsConfig, GenericAssetConfig, GenesisConfig, GrandpaConfig, IndicesConfig, SessionConfig, StakingConfig,
	SystemConfig, WASM_BINARY,
};
use keyring::{Ed25519Keyring, Sr25519Keyring};
use primitives::ChangesTrieConfiguration;
use sr_primitives::Perbill;

/// Create genesis runtime configuration for tests.
pub fn config(support_changes_trie: bool, code: Option<&[u8]>) -> GenesisConfig {
	GenesisConfig {
		system: Some(SystemConfig {
			changes_trie_config: if support_changes_trie {
				Some(ChangesTrieConfiguration {
					digest_interval: 2,
					digest_levels: 2,
				})
			} else {
				None
			},
			code: code.map(|x| x.to_vec()).unwrap_or_else(|| WASM_BINARY.to_vec()),
		}),
		indices: Some(IndicesConfig {
			ids: vec![alice(), bob(), charlie(), dave(), eve(), ferdie()],
		}),
		session: Some(SessionConfig {
			keys: vec![
				(alice(), to_session_keys(&Ed25519Keyring::Alice, &Sr25519Keyring::Alice)),
				(bob(), to_session_keys(&Ed25519Keyring::Bob, &Sr25519Keyring::Bob)),
				(
					charlie(),
					to_session_keys(&Ed25519Keyring::Charlie, &Sr25519Keyring::Charlie),
				),
			],
		}),
		staking: Some(StakingConfig {
			current_era: 0,
			stakers: vec![
				(dave(), alice(), 111 * DOLLARS, staking::StakerStatus::Validator),
				(eve(), bob(), 100 * DOLLARS, staking::StakerStatus::Validator),
				(ferdie(), charlie(), 100 * DOLLARS, staking::StakerStatus::Validator),
			],
			validator_count: 3,
			minimum_validator_count: 0,
			slash_reward_fraction: Perbill::from_percent(10),
			invulnerables: vec![alice(), bob(), charlie()],
			..Default::default()
		}),
		contracts: Some(ContractsConfig {
			current_schedule: Default::default(),
			gas_price: 1 * MILLICENTS,
		}),
		babe: Some(Default::default()),
		grandpa: Some(GrandpaConfig { authorities: vec![] }),
		im_online: Some(Default::default()),
		authority_discovery: Some(Default::default()),
		democracy: Some(Default::default()),
		collective_Instance1: Some(Default::default()),
		collective_Instance2: Some(Default::default()),
		membership_Instance1: Some(Default::default()),
		elections_phragmen: Some(Default::default()),
		sudo: Some(Default::default()),
		generic_asset: Some(GenericAssetConfig {
			assets: vec![16000, 16001, 16002, 16003, 16004, 16005],
			initial_balance: 10u128.pow(18 + 9), // 1 billion token with 18 decimals
			endowed_accounts: vec![alice(), bob(), charlie(), dave(), eve(), ferdie()],
			next_asset_id: 17000,
			staking_asset_id: 16000,
			spending_asset_id: 16001,
		}),
	}
}
