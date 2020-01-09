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

//! CENNZnet chain configurations.

use babe_primitives::AuthorityId as BabeId;
use cennznet_runtime::constants::{asset::*, currency::*, time::*};
use cennznet_runtime::{
	AuthorityDiscoveryConfig, BabeConfig, CennzxSpotConfig, ContractsConfig, CouncilConfig, DemocracyConfig,
	ElectionsConfig, GenericAssetConfig, GrandpaConfig, ImOnlineConfig, IndicesConfig, SessionConfig, SessionKeys,
	StakerStatus, StakingConfig, SudoConfig, SystemConfig, TechnicalCommitteeConfig, WASM_BINARY,
};
use cennznet_runtime::{Block, FeeRate, PerMilli, PerMillion};
use chain_spec::ChainSpecExtension;
use core::convert::TryFrom;
use grandpa_primitives::AuthorityId as GrandpaId;
use im_online::sr25519::AuthorityId as ImOnlineId;
use primitives::{Pair, Public};
use serde::{Deserialize, Serialize};
use sr_primitives::Perbill;
use substrate_service;

pub use cennznet_primitives::types::{AccountId, Balance};
pub use cennznet_runtime::GenesisConfig;

pub mod dev;
pub mod kauri;
pub mod rimu;

pub struct NetworkKeys {
	pub endowed_accounts: Vec<AccountId>,
	pub initial_authorities: Vec<(AccountId, AccountId, GrandpaId, BabeId, ImOnlineId)>,
	pub root_key: AccountId,
}

/// Node `ChainSpec` extensions.
///
/// Additional parameters for some Substrate core modules,
/// customizable from the chain spec.
#[derive(Default, Clone, Serialize, Deserialize, ChainSpecExtension)]
pub struct Extensions {
	/// Block numbers with known hashes.
	pub fork_blocks: client::ForkBlocks<Block>,
}

/// Specialised `ChainSpec`.
pub type ChainSpec = substrate_service::ChainSpec<GenesisConfig, Extensions>;

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

pub fn get_account_id_from_seed(seed: &str) -> AccountId {
	get_from_seed::<AccountId>(seed)
}

/// Helper function to generate stash, controller and session key from seed
pub fn get_authority_keys_from_seed(seed: &str) -> (AccountId, AccountId, GrandpaId, BabeId, ImOnlineId) {
	(
		get_account_id_from_seed(&format!("{}//stash", seed)),
		get_account_id_from_seed(seed),
		get_from_seed::<GrandpaId>(seed),
		get_from_seed::<BabeId>(seed),
		get_from_seed::<ImOnlineId>(seed),
	)
}

/// Helper function to generate session keys
fn session_keys(grandpa: GrandpaId, babe: BabeId, im_online: ImOnlineId) -> SessionKeys {
	SessionKeys {
		grandpa,
		babe,
		im_online,
	}
}

/// Helper function to create GenesisConfig
pub fn config_genesis(network_keys: NetworkKeys, enable_println: bool) -> GenesisConfig {
	const STASH: Balance = 100 * DOLLARS;
	let initial_authorities = network_keys.initial_authorities;
	let endowed_accounts = network_keys.endowed_accounts;
	let root_key = network_keys.root_key;

	GenesisConfig {
		system: Some(SystemConfig {
			code: WASM_BINARY.to_vec(),
			changes_trie_config: Default::default(),
		}),
		indices: Some(IndicesConfig {
			ids: endowed_accounts
				.iter()
				.cloned()
				.chain(initial_authorities.iter().map(|x| x.0.clone()))
				.collect::<Vec<_>>(),
		}),
		session: Some(SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), session_keys(x.2.clone(), x.3.clone(), x.4.clone())))
				.collect::<Vec<_>>(),
		}),
		staking: Some(StakingConfig {
			current_era: 0,
			validator_count: initial_authorities.len() as u32 * 2,
			minimum_validator_count: initial_authorities.len() as u32,
			stakers: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.1.clone(), STASH, StakerStatus::Validator))
				.collect(),
			invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
			slash_reward_fraction: Perbill::from_percent(10),
			..Default::default()
		}),
		democracy: Some(DemocracyConfig::default()),
		collective_Instance1: Some(CouncilConfig {
			members: vec![],
			phantom: Default::default(),
		}),
		collective_Instance2: Some(TechnicalCommitteeConfig {
			members: vec![],
			phantom: Default::default(),
		}),
		elections_phragmen: Some(ElectionsConfig {
			members: endowed_accounts.iter().take(2).cloned().collect(),
			term_duration: 28 * DAYS,
			desired_members: 4,
			desired_runners_up: 1,
		}),
		contracts: Some(ContractsConfig {
			current_schedule: contracts::Schedule {
				enable_println, // this should only be enabled on development chains
				..Default::default()
			},
			gas_price: 1 * MILLICENTS,
		}),
		sudo: Some(SudoConfig { key: root_key }),
		babe: Some(BabeConfig { authorities: vec![] }),
		im_online: Some(ImOnlineConfig { keys: vec![] }),
		authority_discovery: Some(AuthorityDiscoveryConfig { keys: vec![] }),
		grandpa: Some(GrandpaConfig { authorities: vec![] }),
		membership_Instance1: Some(Default::default()),
		generic_asset: Some(GenericAssetConfig {
			assets: vec![
				CENNZ_ASSET_ID,
				CENTRAPAY_ASSET_ID,
				PLUG_ASSET_ID,
				SYLO_ASSET_ID,
				CERTI_ASSET_ID,
				ARDA_ASSET_ID,
				NEXT_ASSET_ID,
			],
			initial_balance: 10u128.pow(18 + 9), // 1 billion token with 18 decimals
			endowed_accounts: endowed_accounts.clone(),
			next_asset_id: NEXT_ASSET_ID,
			staking_asset_id: STAKING_ASSET_ID,
			spending_asset_id: SPENDING_ASSET_ID,
		}),
		cennzx_spot: Some(CennzxSpotConfig {
			fee_rate: FeeRate::<PerMillion>::try_from(FeeRate::<PerMilli>::from(3u128)).unwrap(),
			core_asset_id: CENTRAPAY_ASSET_ID,
		}),
	}
}
