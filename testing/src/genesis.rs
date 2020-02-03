// Copyright 2019-2020 Parity Technologies (UK) Ltd.
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
use cennznet_runtime::{
	constants::{asset::*, currency::*},
	FeeRate, PerMilli, PerMillion,
};
use cennznet_runtime::{
	CennzxSpotConfig, ContractsConfig, GenericAssetConfig, GenesisConfig, GrandpaConfig, IndicesConfig, SessionConfig,
	StakingConfig, SystemConfig, WASM_BINARY,
};
use core::convert::TryFrom;
use sp_core::ChangesTrieConfiguration;
use sp_keyring::{Ed25519Keyring, Sr25519Keyring};
use sp_runtime::Perbill;

/// Create genesis runtime configuration for tests.
pub fn config(support_changes_trie: bool, code: Option<&[u8]>) -> GenesisConfig {
	GenesisConfig {
		frame_system: Some(SystemConfig {
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
		pallet_indices: Some(IndicesConfig {
			ids: vec![alice(), bob(), charlie(), dave(), eve(), ferdie()],
		}),
		pallet_generic_asset: Some(GenericAssetConfig {
			assets: vec![
				CENNZ_ASSET_ID,
				CENTRAPAY_ASSET_ID,
				PLUG_ASSET_ID,
				SYLO_ASSET_ID,
				CERTI_ASSET_ID,
				ARDA_ASSET_ID,
			],
			initial_balance: 111 * DOLLARS,
			endowed_accounts: vec![alice(), bob(), charlie(), dave(), eve(), ferdie()],
			next_asset_id: NEXT_ASSET_ID,
			staking_asset_id: STAKING_ASSET_ID,
			spending_asset_id: SPENDING_ASSET_ID,
		}),
		pallet_session: Some(SessionConfig {
			keys: vec![
				(alice(), to_session_keys(&Ed25519Keyring::Alice, &Sr25519Keyring::Alice)),
				(bob(), to_session_keys(&Ed25519Keyring::Bob, &Sr25519Keyring::Bob)),
				(
					charlie(),
					to_session_keys(&Ed25519Keyring::Charlie, &Sr25519Keyring::Charlie),
				),
			],
		}),
		pallet_staking: Some(StakingConfig {
			current_era: 0,
			stakers: vec![
				(dave(), alice(), 111 * DOLLARS, pallet_staking::StakerStatus::Validator),
				(eve(), bob(), 100 * DOLLARS, pallet_staking::StakerStatus::Validator),
				(
					ferdie(),
					charlie(),
					100 * DOLLARS,
					pallet_staking::StakerStatus::Validator,
				),
			],
			validator_count: 3,
			minimum_validator_count: 0,
			slash_reward_fraction: Perbill::from_percent(10),
			invulnerables: vec![alice(), bob(), charlie()],
			..Default::default()
		}),
		pallet_contracts: Some(ContractsConfig {
			current_schedule: Default::default(),
			gas_price: 1 * MILLICENTS,
		}),
		pallet_babe: Some(Default::default()),
		pallet_grandpa: Some(GrandpaConfig { authorities: vec![] }),
		pallet_im_online: Some(Default::default()),
		pallet_authority_discovery: Some(Default::default()),
		pallet_democracy: Some(Default::default()),
		pallet_collective_Instance1: Some(Default::default()),
		pallet_collective_Instance2: Some(Default::default()),
		pallet_membership_Instance1: Some(Default::default()),
		pallet_sudo: Some(Default::default()),
		pallet_treasury: Some(Default::default()),
		pallet_society: Some(SocietyConfig {
			members: vec![alice(), bob()],
			pot: 0,
			max_members: 999,
		}),
		cennzx_spot: Some(CennzxSpotConfig {
			fee_rate: FeeRate::<PerMillion>::try_from(FeeRate::<PerMilli>::from(3u128)).unwrap(),
			core_asset_id: CENTRAPAY_ASSET_ID,
		}),
	}
}
