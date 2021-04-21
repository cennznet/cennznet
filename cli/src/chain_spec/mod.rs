// Copyright 2018-2021 Parity Technologies (UK) Ltd. and Centrality Investments Ltd.
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

use cennznet_primitives::types::Block;
use cennznet_runtime::constants::{asset::*, currency::*};
use cennznet_runtime::{
	AssetInfo, AuthorityDiscoveryConfig, BabeConfig, CennzxConfig, ContractsConfig, FeeRate, GenericAssetConfig,
	GrandpaConfig, ImOnlineConfig, PerMillion, PerThousand, RewardsConfig, SessionConfig, SessionKeys, StakerStatus,
	StakingConfig, SudoConfig, SystemConfig, WASM_BINARY,
};
use core::convert::TryFrom;
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use sc_chain_spec::ChainSpecExtension;
use serde::{Deserialize, Serialize};
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_core::{sr25519, Pair, Public};
use sp_finality_grandpa::AuthorityId as GrandpaId;
use sp_runtime::{
	traits::{IdentifyAccount, Verify},
	FixedPointNumber, FixedU128, Perbill,
};

pub use cennznet_primitives::types::{AccountId, Balance, Signature};
pub use cennznet_runtime::GenesisConfig;

pub mod dev;
pub mod nikau;
pub mod rata;

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
#[serde(rename_all = "camelCase")]
pub struct Extensions {
	/// Block numbers with known hashes.
	pub fork_blocks: sc_client_api::ForkBlocks<Block>,
	/// Known bad block hashes.
	pub bad_blocks: sc_client_api::BadBlocks<Block>,
}

/// Specialised `ChainSpec`.
pub type CENNZnetChainSpec = sc_service::GenericChainSpec<GenesisConfig, Extensions>;

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
pub fn config_genesis(network_keys: NetworkKeys) -> GenesisConfig {
	const INITIAL_BOND: Balance = 100 * DOLLARS;
	let initial_authorities = network_keys.initial_authorities;
	let root_key = network_keys.root_key;
	let mut endowed_accounts = network_keys.endowed_accounts;
	initial_authorities.iter().for_each(|x| {
		if !endowed_accounts.contains(&x.0) {
			endowed_accounts.push(x.0.clone())
		}
	});

	GenesisConfig {
		frame_system: Some(SystemConfig {
			code: WASM_BINARY.expect("wasm binary not available").to_vec(),
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
		crml_staking: Some(StakingConfig {
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
		pallet_contracts: Some(ContractsConfig {
			current_schedule: Default::default(),
		}),
		pallet_sudo: Some(SudoConfig { key: root_key.clone() }),
		pallet_babe: Some(BabeConfig { authorities: vec![] }),
		pallet_im_online: Some(ImOnlineConfig { keys: vec![] }),
		pallet_authority_discovery: Some(AuthorityDiscoveryConfig { keys: vec![] }),
		pallet_grandpa: Some(GrandpaConfig { authorities: vec![] }),
		prml_generic_asset: Some(GenericAssetConfig {
			assets: vec![CENNZ_ASSET_ID, CENTRAPAY_ASSET_ID],
			// Grant root key full permissions (mint,burn,update) on the following assets
			permissions: vec![
				(CENNZ_ASSET_ID, root_key.clone()),
				(CENTRAPAY_ASSET_ID, root_key.clone()),
			],
			initial_balance: 1_000_000 * DOLLARS, // 1,000,000.0000 (4dp asset)
			endowed_accounts,
			next_asset_id: NEXT_ASSET_ID,
			staking_asset_id: STAKING_ASSET_ID,
			spending_asset_id: SPENDING_ASSET_ID,
			asset_meta: vec![
				(CENNZ_ASSET_ID, AssetInfo::new(b"CENNZ".to_vec(), 4, 1)),
				(CENTRAPAY_ASSET_ID, AssetInfo::new(b"CPAY".to_vec(), 4, 1)),
			],
		}),
		crml_cennzx: Some(CennzxConfig {
			// 0.003%
			fee_rate: FeeRate::<PerMillion>::try_from(FeeRate::<PerThousand>::from(3u128)).unwrap(),
			core_asset_id: CENTRAPAY_ASSET_ID,
		}),
		crml_staking_rewards: Some(RewardsConfig {
			// 20% of all fees
			development_fund_take: Perbill::from_percent(20),
			// 80% APY
			inflation_rate: FixedU128::saturating_from_rational(8, 10),
		}),
	}
}

#[cfg(test)]
pub(crate) mod tests {
	use super::*;
	use crate::service::{new_full_base, new_light_base, NewFullBase};
	use sc_service::ChainType;
	use sc_service_test;
	use sp_runtime::BuildStorage;

	fn local_testnet_genesis_instant_single() -> GenesisConfig {
		let endowed_accounts = vec![
			get_account_id_from_seed::<sr25519::Public>("Alice"),
			get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
		];
		let initial_authorities = vec![get_authority_keys_from_seed("Alice")];
		let root_key = get_account_id_from_seed::<sr25519::Public>("Alice");

		config_genesis(NetworkKeys {
			endowed_accounts,
			initial_authorities,
			root_key,
		})
	}

	fn local_testnet_genesis_instant_multi() -> GenesisConfig {
		let endowed_accounts = vec![
			get_account_id_from_seed::<sr25519::Public>("Alice"),
			get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
			get_account_id_from_seed::<sr25519::Public>("Bob"),
			get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
		];
		let initial_authorities = vec![
			get_authority_keys_from_seed("Alice"),
			get_authority_keys_from_seed("Bob"),
		];
		let root_key = get_account_id_from_seed::<sr25519::Public>("Alice");

		config_genesis(NetworkKeys {
			endowed_accounts,
			initial_authorities,
			root_key,
		})
	}

	/// Local testnet config (single validator - Alice)
	pub fn integration_test_config_with_single_authority() -> CENNZnetChainSpec {
		CENNZnetChainSpec::from_genesis(
			"Integration Test",
			"test",
			ChainType::Development,
			local_testnet_genesis_instant_single,
			vec![],
			None,
			None,
			None,
			Default::default(),
		)
	}

	/// Local testnet config (multivalidator Alice + Bob)
	pub fn integration_test_config_with_two_authorities() -> CENNZnetChainSpec {
		CENNZnetChainSpec::from_genesis(
			"Integration Test",
			"test",
			ChainType::Development,
			local_testnet_genesis_instant_multi,
			vec![],
			None,
			None,
			None,
			Default::default(),
		)
	}

	#[test]
	#[ignore]
	fn test_connectivity() {
		sc_service_test::connectivity(
			integration_test_config_with_two_authorities(),
			|config| {
				let NewFullBase {
					task_manager,
					client,
					network,
					transaction_pool,
					..
				} = new_full_base(config, |_, _| ())?;
				Ok(sc_service_test::TestNetComponents::new(
					task_manager,
					client,
					network,
					transaction_pool,
				))
			},
			|config| {
				let (keep_alive, _, _, client, network, transaction_pool) = new_light_base(config)?;
				Ok(sc_service_test::TestNetComponents::new(
					keep_alive,
					client,
					network,
					transaction_pool,
				))
			},
		);
	}

	#[test]
	fn test_create_development_chain_spec() {
		dev::config().build_storage().unwrap();
	}
}
