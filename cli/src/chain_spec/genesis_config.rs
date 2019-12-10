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
use cennznet_runtime::{Block, FeeRate};
use chain_spec::ChainSpecExtension;
use grandpa_primitives::AuthorityId as GrandpaId;
use hex_literal::hex;
use im_online::sr25519::AuthorityId as ImOnlineId;
use primitives::{crypto::UncheckedInto, Pair, Public};
use serde::{Deserialize, Serialize};
use sr_primitives::Perbill;
use substrate_service;
use substrate_telemetry::TelemetryEndpoints;

pub use cennznet_primitives::{AccountId, Balance};
pub use cennznet_runtime::GenesisConfig;

const STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";

pub struct NetworkKeys {
	pub endowed_accounts: Vec<AccountId>,
	pub initial_authorities: Vec<(AccountId, AccountId, GrandpaId, BabeId, ImOnlineId)>,
	pub root_key: AccountId,
}

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

/// Helper function to create GenesisConfig for testing
pub fn config_genesis(network_keys: NetworkKeys, enable_println: bool) -> GenesisConfig {
	const STASH: Balance = 100 * DOLLARS;
	let initial_authorities = network_keys.initial_authorities;
	let endowed_accounts = network_keys.root_key;
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
			// create_asset_stake: 1000, // removed upstream
			staking_asset_id: STAKING_ASSET_ID,
			spending_asset_id: SPENDING_ASSET_ID,
		}),
		cennzx_spot: Some(CennzxSpotConfig {
			fee_rate: FeeRate::from_milli(3),
			core_asset_id: CENTRAPAY_ASSET_ID,
		}),
	}
}

// fn staging_testnet_config_genesis() -> GenesisConfig {
// 	// stash, controller, session-key
// 	// generated with secret:
// 	// for i in 1 2 3 4 ; do for j in stash controller; do subkey inspect "$secret"/fir/$j/$i; done; done
// 	// and
// 	// for i in 1 2 3 4 ; do for j in session; do subkey --ed25519 inspect "$secret"//fir//$j//$i; done; done

// 	let initial_authorities: Vec<(AccountId, AccountId, GrandpaId, BabeId, ImOnlineId)> = vec![
// 		(
// 			// 5Fbsd6WXDGiLTxunqeK5BATNiocfCqu9bS1yArVjCgeBLkVy
// 			hex!["9c7a2ee14e565db0c69f78c7b4cd839fbf52b607d867e9e9c5a79042898a0d12"].unchecked_into(),
// 			// 5EnCiV7wSHeNhjW3FSUwiJNkcc2SBkPLn5Nj93FmbLtBjQUq
// 			hex!["781ead1e2fa9ccb74b44c19d29cb2a7a4b5be3972927ae98cd3877523976a276"].unchecked_into(),
// 			// 5Fb9ayurnxnaXj56CjmyQLBiadfRCqUbL2VWNbbe1nZU6wiC
// 			hex!["9becad03e6dcac03cee07edebca5475314861492cdfc96a2144a67bbe9699332"].unchecked_into(),
// 			// 5EZaeQ8djPcq9pheJUhgerXQZt9YaHnMJpiHMRhwQeinqUW8
// 			hex!["6e7e4eb42cbd2e0ab4cae8708ce5509580b8c04d11f6758dbf686d50fe9f9106"].unchecked_into(),
// 			// 5EZaeQ8djPcq9pheJUhgerXQZt9YaHnMJpiHMRhwQeinqUW8
// 			hex!["6e7e4eb42cbd2e0ab4cae8708ce5509580b8c04d11f6758dbf686d50fe9f9106"].unchecked_into(),
// 		),
// 		(
// 			// 5ERawXCzCWkjVq3xz1W5KGNtVx2VdefvZ62Bw1FEuZW4Vny2
// 			hex!["68655684472b743e456907b398d3a44c113f189e56d1bbfd55e889e295dfde78"].unchecked_into(),
// 			// 5Gc4vr42hH1uDZc93Nayk5G7i687bAQdHHc9unLuyeawHipF
// 			hex!["c8dc79e36b29395413399edaec3e20fcca7205fb19776ed8ddb25d6f427ec40e"].unchecked_into(),
// 			// 5EockCXN6YkiNCDjpqqnbcqd4ad35nU4RmA1ikM4YeRN4WcE
// 			hex!["7932cff431e748892fa48e10c63c17d30f80ca42e4de3921e641249cd7fa3c2f"].unchecked_into(),
// 			// 5DhLtiaQd1L1LU9jaNeeu9HJkP6eyg3BwXA7iNMzKm7qqruQ
// 			hex!["482dbd7297a39fa145c570552249c2ca9dd47e281f0c500c971b59c9dcdcd82e"].unchecked_into(),
// 			// 5DhLtiaQd1L1LU9jaNeeu9HJkP6eyg3BwXA7iNMzKm7qqruQ
// 			hex!["482dbd7297a39fa145c570552249c2ca9dd47e281f0c500c971b59c9dcdcd82e"].unchecked_into(),
// 		),
// 		(
// 			// 5DyVtKWPidondEu8iHZgi6Ffv9yrJJ1NDNLom3X9cTDi98qp
// 			hex!["547ff0ab649283a7ae01dbc2eb73932eba2fb09075e9485ff369082a2ff38d65"].unchecked_into(),
// 			// 5FeD54vGVNpFX3PndHPXJ2MDakc462vBCD5mgtWRnWYCpZU9
// 			hex!["9e42241d7cd91d001773b0b616d523dd80e13c6c2cab860b1234ef1b9ffc1526"].unchecked_into(),
// 			// 5E1jLYfLdUQKrFrtqoKgFrRvxM3oQPMbf6DfcsrugZZ5Bn8d
// 			hex!["5633b70b80a6c8bb16270f82cca6d56b27ed7b76c8fd5af2986a25a4788ce440"].unchecked_into(),
// 			// 5DhKqkHRkndJu8vq7pi2Q5S3DfftWJHGxbEUNH43b46qNspH
// 			hex!["482a3389a6cf42d8ed83888cfd920fec738ea30f97e44699ada7323f08c3380a"].unchecked_into(),
// 			// 5DhKqkHRkndJu8vq7pi2Q5S3DfftWJHGxbEUNH43b46qNspH
// 			hex!["482a3389a6cf42d8ed83888cfd920fec738ea30f97e44699ada7323f08c3380a"].unchecked_into(),
// 		),
// 		(
// 			// 5HYZnKWe5FVZQ33ZRJK1rG3WaLMztxWrrNDb1JRwaHHVWyP9
// 			hex!["f26cdb14b5aec7b2789fd5ca80f979cef3761897ae1f37ffb3e154cbcc1c2663"].unchecked_into(),
// 			// 5EPQdAQ39WQNLCRjWsCk5jErsCitHiY5ZmjfWzzbXDoAoYbn
// 			hex!["66bc1e5d275da50b72b15de072a2468a5ad414919ca9054d2695767cf650012f"].unchecked_into(),
// 			// 5DMa31Hd5u1dwoRKgC4uvqyrdK45RHv3CpwvpUC1EzuwDit4
// 			hex!["3919132b851ef0fd2dae42a7e734fe547af5a6b809006100f48944d7fae8e8ef"].unchecked_into(),
// 			// 5C4vDQxA8LTck2xJEy4Yg1hM9qjDt4LvTQaMo4Y8ne43aU6x
// 			hex!["00299981a2b92f878baaf5dbeba5c18d4e70f2a1fcd9c61b32ea18daf38f4378"].unchecked_into(),
// 			// 5C4vDQxA8LTck2xJEy4Yg1hM9qjDt4LvTQaMo4Y8ne43aU6x
// 			hex!["00299981a2b92f878baaf5dbeba5c18d4e70f2a1fcd9c61b32ea18daf38f4378"].unchecked_into(),
// 		),
// 	];

// 	// generated with secret: subkey inspect "$secret"/fir
// 	let root_key: AccountId = hex![
// 		// 5Ff3iXP75ruzroPWRP2FYBHWnmGGBSb63857BgnzCoXNxfPo
// 		"9ee5e5bdc0ec239eb164f865ecc345ce4c88e76ee002e0f7e318097347471809"
// 	]
// 	.unchecked_into();

// 	let endowed_accounts: Vec<AccountId> = vec![root_key.clone()];

// 	testnet_genesis(initial_authorities, root_key, Some(endowed_accounts), false)
// }

// Staging testnet config
// pub fn staging_testnet_config() -> ChainSpec {
// 	let boot_nodes = vec![];
// 	ChainSpec::from_genesis(
// 		"Staging Testnet",
// 		"staging_testnet",
// 		staging_testnet_config_genesis,
// 		boot_nodes,
// 		Some(TelemetryEndpoints::new(vec![(STAGING_TELEMETRY_URL.to_string(), 0)])),
// 		None,
// 		None,
// 		Default::default(),
// 	)
// }
