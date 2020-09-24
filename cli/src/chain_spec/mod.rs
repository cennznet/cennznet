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

//! CENNZnet chain configurations.

use cennznet_runtime::{
	constants::{asset::*, currency::*},
	AssetInfo, Block, FeeRate, PerMillion, PerThousand, SessionKeys, StakerStatus, WASM_BINARY,
};
use cennznet_runtime::{
	AuthorityDiscoveryConfig, BabeConfig, CennzxSpotConfig, ContractsConfig, CouncilConfig, GenericAssetConfig,
	GrandpaConfig, ImOnlineConfig, RewardsConfig, SessionConfig, StakingConfig, SudoConfig, SystemConfig,
	TechnicalCommitteeConfig,
};
use core::convert::TryFrom;
use grandpa_primitives::AuthorityId as GrandpaId;
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use sc_chain_spec::ChainSpecExtension;
use sc_service;
use serde::{Deserialize, Serialize};
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_core::{sr25519, Pair, Public};
use sp_runtime::{
	traits::{IdentifyAccount, Verify},
	Perbill,
};

pub use cennznet_primitives::types::{AccountId, Balance, Signature};
pub use cennznet_runtime::GenesisConfig;

pub mod azalea;
pub mod dev;
pub mod nikau;

type AccountPublic = <Signature as Verify>::Signer;

/// A type contains authority keys
pub type AuthorityKeys = (
	// stash account ID
	AccountId,
	// controller account ID
	AccountId,
	// Grandpa ID
	GrandpaId,
	// Babe ID
	BabeId,
	// ImOnline ID
	ImOnlineId,
	// Authority Discovery ID
	AuthorityDiscoveryId,
);

/// A type to hold keys used in CENNZnet node in SS58 format.
pub struct NetworkKeys {
	/// Endowed account address (SS58 format).
	pub endowed_accounts: Vec<AccountId>,
	/// List of authority keys
	pub initial_authorities: Vec<AuthorityKeys>,
	/// Sudo account address (SS58 format).
	pub root_key: AccountId,
}

/// Node `ChainSpec` extensions.
///
/// Additional parameters for some Substrate core modules,
/// customizable from the chain spec.
#[derive(Default, Clone, Serialize, Deserialize, ChainSpecExtension)]
pub struct Extensions {
	/// Block numbers with known hashes.
	pub fork_blocks: sc_client::ForkBlocks<Block>,
}

/// Specialised `ChainSpec`.
pub type ChainSpec = sc_service::GenericChainSpec<GenesisConfig, Extensions>;

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Helper function to generate stash, controller and session key from seed
pub fn get_authority_keys_from_seed(
	seed: &str,
) -> (
	AccountId,
	AccountId,
	GrandpaId,
	BabeId,
	ImOnlineId,
	AuthorityDiscoveryId,
) {
	(
		get_account_id_from_seed::<sr25519::Public>(&format!("{}//stash", seed)),
		get_account_id_from_seed::<sr25519::Public>(seed),
		get_from_seed::<GrandpaId>(seed),
		get_from_seed::<BabeId>(seed),
		get_from_seed::<ImOnlineId>(seed),
		get_from_seed::<AuthorityDiscoveryId>(seed),
	)
}

/// Helper function to generate session keys with authority keys
pub fn session_keys(
	grandpa: GrandpaId,
	babe: BabeId,
	im_online: ImOnlineId,
	authority_discovery: AuthorityDiscoveryId,
) -> SessionKeys {
	SessionKeys {
		grandpa,
		babe,
		im_online,
		authority_discovery,
	}
}

/// Helper function to create GenesisConfig
pub fn config_genesis(network_keys: NetworkKeys, enable_println: bool) -> GenesisConfig {
	const INITIAL_BOND: Balance = 100 * DOLLARS;
	let initial_authorities = network_keys.initial_authorities;
	let root_key = network_keys.root_key;
	let endowed_accounts = network_keys.endowed_accounts;
	let num_endowed_accounts = endowed_accounts.len();

	GenesisConfig {
		frame_system: Some(SystemConfig {
			code: WASM_BINARY.to_vec(),
			changes_trie_config: Default::default(),
		}),
		pallet_session: Some(SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.0.clone(),
						session_keys(x.2.clone(), x.3.clone(), x.4.clone(), x.5.clone()),
					)
				})
				.collect::<Vec<_>>(),
		}),
		crml_rewards: Some(RewardsConfig {
			development_fund_take: Perbill::from_percent(20),
		}),
		crml_staking: Some(StakingConfig {
			current_era: 0,
			validator_count: initial_authorities.len() as u32 * 2,
			minimum_validator_count: initial_authorities.len() as u32,
			stakers: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.1.clone(), INITIAL_BOND, StakerStatus::Validator))
				.collect(),
			invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
			minimum_bond: 1,
			slash_reward_fraction: Perbill::from_percent(10),
			..Default::default()
		}),
		pallet_collective_Instance1: Some(CouncilConfig {
			members: endowed_accounts.iter().cloned().collect::<Vec<_>>()[..(num_endowed_accounts + 1) / 2].to_vec(),
			phantom: Default::default(),
		}),
		pallet_collective_Instance2: Some(TechnicalCommitteeConfig {
			members: endowed_accounts.iter().cloned().collect::<Vec<_>>()[..(num_endowed_accounts + 1) / 2].to_vec(),
			phantom: Default::default(),
		}),
		pallet_contracts: Some(ContractsConfig {
			current_schedule: pallet_contracts::Schedule {
				enable_println, // this should only be enabled on development chains
				..Default::default()
			},
			gas_price: 1 * MICROS,
		}),
		pallet_sudo: Some(SudoConfig { key: root_key.clone() }),
		pallet_babe: Some(BabeConfig { authorities: vec![] }),
		pallet_im_online: Some(ImOnlineConfig { keys: vec![] }),
		pallet_authority_discovery: Some(AuthorityDiscoveryConfig { keys: vec![] }),
		pallet_grandpa: Some(GrandpaConfig { authorities: vec![] }),
		pallet_membership_Instance1: Some(Default::default()),
		pallet_treasury: Some(Default::default()),
		pallet_generic_asset: Some(GenericAssetConfig {
			assets: vec![CENNZ_ASSET_ID, CENTRAPAY_ASSET_ID],
			// Grant root key full permissions (mint,burn,update) on the following assets
			permissions: vec![(CENNZ_ASSET_ID, root_key.clone()), (CENTRAPAY_ASSET_ID, root_key)],
			initial_balance: 10u128.pow(18 + 9), // 1 billion token with 18 decimals
			endowed_accounts: endowed_accounts,
			next_asset_id: NEXT_ASSET_ID,
			staking_asset_id: STAKING_ASSET_ID,
			spending_asset_id: SPENDING_ASSET_ID,
			asset_meta: vec![
				(CENNZ_ASSET_ID, AssetInfo::new(b"CENNZ".to_vec(), 1)),
				(CENTRAPAY_ASSET_ID, AssetInfo::new(b"CPAY".to_vec(), 2)),
			],
		}),
		crml_cennzx_spot: Some(CennzxSpotConfig {
			fee_rate: FeeRate::<PerMillion>::try_from(FeeRate::<PerThousand>::from(3u128)).unwrap(),
			core_asset_id: CENTRAPAY_ASSET_ID,
		}),
	}
}
