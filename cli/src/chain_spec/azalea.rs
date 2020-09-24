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

//! CENNZNet MainNet V1 (Azalea) genesis config
use super::{session_keys, ChainSpec, NetworkKeys};
use cennznet_primitives::types::{AccountId, AssetId, Balance};
use cennznet_runtime::{
	constants::currency::*, AssetInfo, FeeRate, PerMillion, PerThousand, StakerStatus, WASM_BINARY,
};
use cennznet_runtime::{
	AuthorityDiscoveryConfig, BabeConfig, CennzxSpotConfig, ContractsConfig, CouncilConfig, GenericAssetConfig,
	GenesisConfig, GrandpaConfig, ImOnlineConfig, RewardsConfig, SessionConfig, StakingConfig, SudoConfig,
	SystemConfig, TechnicalCommitteeConfig,
};
use core::convert::TryFrom;
use sp_core::crypto::UncheckedInto;
use sp_runtime::Perbill;

use grandpa_primitives::AuthorityId as GrandpaId;
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_babe::AuthorityId as BabeId;

// Reserve Asset IDs
// We leave ID '0' as it logically indicates 'no asset ID'
/// CENNZ asset id on Azalea
pub const CENNZ_ASSET_ID: AssetId = 1;
/// CPAY asset id on Azalea
pub const CENTRAPAY_ASSET_ID: AssetId = 2;
/// Starting id for newly created assets on Azalea
pub const NEXT_ASSET_ID: AssetId = 1_000;
/// CENNZ is the staking asset id on Azalea
pub const STAKING_ASSET_ID: AssetId = CENNZ_ASSET_ID;
/// CPAY is the spending asset id on Azalea
pub const SPENDING_ASSET_ID: AssetId = CENTRAPAY_ASSET_ID;

fn network_keys() -> NetworkKeys {
	let root_key: AccountId = hex!["f4d373896af0d70b40c8b80af075f2761043c1a63798a2c5cb95d68f1d66bd2f"].into();
	let initial_authorities: Vec<(
		AccountId,
		AccountId,
		GrandpaId,
		BabeId,
		ImOnlineId,
		AuthorityDiscoveryId,
	)> = vec![
		// The initial 12 validators keys
		// - stash
		// - controller
		// 4 Session keys:
		// - grandpa
		// - babe
		// - im online
		// - authority discovery
		(
			hex!["1e1c67b09dd6cb1ac71d822ff9cd1f53d8eb80494d6c5772ff318cb4d8c4cd36"].into(),
			hex!["7ccfdcaa7e02a25237aaf74dc8db330a4c74e00c6f3dcfbcc024972d94a75604"].into(),
			hex!["8394712d6551b665db4898ea7ed6c44a5ac0fd1a5426de63c92b4c9ac5e82439"].unchecked_into(),
			hex!["26fc5b66b0822e650e1da0901d8bcd94f305b9338d489364a18c04a640c2bc6c"].unchecked_into(),
			hex!["26fc5b66b0822e650e1da0901d8bcd94f305b9338d489364a18c04a640c2bc6c"].unchecked_into(),
			hex!["26fc5b66b0822e650e1da0901d8bcd94f305b9338d489364a18c04a640c2bc6c"].unchecked_into(),
		),
		(
			hex!["f2854288a283577a0f39a2e34b519bc976fa66582288a98fa73214fdc5e36c17"].into(),
			hex!["3459c63219e0150584ff6aa35ee7bfa2d5fd4282ba490f75c7eb85ec0c724b20"].into(),
			hex!["5dfb9f9f5b14f4caf8f4e628e04480fa0f9d3320b9ccdfcbfc9b3afc7d81b1d8"].unchecked_into(),
			hex!["52a9edf0253a60cb88f251c3b3bbd304f096fc02eefea80aaccba22ab469217e"].unchecked_into(),
			hex!["52a9edf0253a60cb88f251c3b3bbd304f096fc02eefea80aaccba22ab469217e"].unchecked_into(),
			hex!["52a9edf0253a60cb88f251c3b3bbd304f096fc02eefea80aaccba22ab469217e"].unchecked_into(),
		),
		(
			hex!["4cea20b60cb177ecfeb5c321cd5ddc9daa1eeb0091b1e63d18863c6cc9656b55"].into(),
			hex!["484d7f927880162bda5c4744254e4865bcb759652b9a712b405d9eba30cf7922"].into(),
			hex!["2fb112a115054898c1bae0870906052858e11f3cf0d29f40349acccae64e4c27"].unchecked_into(),
			hex!["aa8c3e6506b432459ff57075eed503a4f6062d286fe1bbca4a2c49a4ec1b7069"].unchecked_into(),
			hex!["aa8c3e6506b432459ff57075eed503a4f6062d286fe1bbca4a2c49a4ec1b7069"].unchecked_into(),
			hex!["aa8c3e6506b432459ff57075eed503a4f6062d286fe1bbca4a2c49a4ec1b7069"].unchecked_into(),
		),
		(
			hex!["8abf2505bc274ec7b4df6e40a6c8740eeda4f4781a9bd62d701c1a18a10b436a"].into(),
			hex!["22205a6abbcabe2d3ca42c0a098fc85a38e0a0a5c2f66e127bf661fca1275712"].into(),
			hex!["a83c514053c8a9c264de93eb8c5eced82bcf7d1923487c5eb82883efd8e11c3a"].unchecked_into(),
			hex!["c43d061b0b7d0f8ec1b3cf47272c8e797cd6c86c67e9c3566543e9ab024b2e21"].unchecked_into(),
			hex!["c43d061b0b7d0f8ec1b3cf47272c8e797cd6c86c67e9c3566543e9ab024b2e21"].unchecked_into(),
			hex!["c43d061b0b7d0f8ec1b3cf47272c8e797cd6c86c67e9c3566543e9ab024b2e21"].unchecked_into(),
		),
		(
			hex!["c6d767c05a5d80e351a38ee5fb9a8d9e6f51b009a30b13ebfced9a63523fe130"].into(),
			hex!["4a5c946a7bf3c1f86c1b9d81f6ec186b632fb4d6f38a14932e557155dea08c3e"].into(),
			hex!["b0b1daac688e7083761e900e8fc481e1c96273f738da63ae8d5e80745e68b88d"].unchecked_into(),
			hex!["b01d2b3308c189c7f9d2ea156ab05b7352e9a4b43800474bffa7db32ff3def5a"].unchecked_into(),
			hex!["b01d2b3308c189c7f9d2ea156ab05b7352e9a4b43800474bffa7db32ff3def5a"].unchecked_into(),
			hex!["b01d2b3308c189c7f9d2ea156ab05b7352e9a4b43800474bffa7db32ff3def5a"].unchecked_into(),
		),
		(
			hex!["a601feba2b0c438abe632b7a1e42f871a4d96c50ed3df55f1679a5d69fe2cf2a"].into(),
			hex!["2e7cc07944ba66ab6225648f56fcac2d36151cda5f13a14f14874e06ae9a8d62"].into(),
			hex!["72347c51b186cf44052c7ca9d86d4294fca386d6aca3275b981f1fb09f77da3f"].unchecked_into(),
			hex!["d669af759ce293a57e200c33195725a7e65d489082da42bc5c1876af13f0d417"].unchecked_into(),
			hex!["d669af759ce293a57e200c33195725a7e65d489082da42bc5c1876af13f0d417"].unchecked_into(),
			hex!["d669af759ce293a57e200c33195725a7e65d489082da42bc5c1876af13f0d417"].unchecked_into(),
		),
		(
			hex!["1ed4183a1e05fb8765392d2fc5dc52388d26062bc6b953ddb51d6b7addd7ef69"].into(),
			hex!["f2f5929f16024052cacd31401b1c305074456ed038e2009ee2629cb353ca0f4e"].into(),
			hex!["766f0a3005dd0f66781c47f64fe0c671f70f2c13281a1aa0913398dd76ff67ea"].unchecked_into(),
			hex!["b8c207e74acf1fc82102587adba37de727c613b36688e6338d2980c0d120ee10"].unchecked_into(),
			hex!["b8c207e74acf1fc82102587adba37de727c613b36688e6338d2980c0d120ee10"].unchecked_into(),
			hex!["b8c207e74acf1fc82102587adba37de727c613b36688e6338d2980c0d120ee10"].unchecked_into(),
		),
		(
			hex!["20f19870983485e540112505cf09a5aa2a13fefbd7d0bb0cc555f6a0ea32b840"].into(),
			hex!["0616a564295884eed6037a5cde55fccaf9a109e4cfc14c46acd2dedcafb9ec53"].into(),
			hex!["0229880b50f2d9fd83eb0ca418f5298f2b9d65a074f79b3187eac4574a6c49d8"].unchecked_into(),
			hex!["7c878f20b644b59bb42a3c245060e9f611dd2226c33de3a95ce99095adac1963"].unchecked_into(),
			hex!["7c878f20b644b59bb42a3c245060e9f611dd2226c33de3a95ce99095adac1963"].unchecked_into(),
			hex!["7c878f20b644b59bb42a3c245060e9f611dd2226c33de3a95ce99095adac1963"].unchecked_into(),
		),
		(
			hex!["0822b09bc448275b46a9edf0ad56f93550fdc7908858c14831ee13de697e0a29"].into(),
			hex!["68b012bd8dadbbcd0134956d90ea3bf25eda88aec6bd5247e510e59a96b40675"].into(),
			hex!["8cfebca49d22151511ad4fae9ee394c4a5f12faacd110586eed56810135db16b"].unchecked_into(),
			hex!["fa3535733bc3faa490f1614ac568114febb1dcfa486386bde97900421648c52f"].unchecked_into(),
			hex!["fa3535733bc3faa490f1614ac568114febb1dcfa486386bde97900421648c52f"].unchecked_into(),
			hex!["fa3535733bc3faa490f1614ac568114febb1dcfa486386bde97900421648c52f"].unchecked_into(),
		),
		(
			hex!["8e2e72c48e2271194a04b016a1f9aaa770370904f9046bb6afa7e235da264d18"].into(),
			hex!["f25057854417d06ee51fc3210e0ff996894aea40bc61fa97a884ef1c90904975"].into(),
			hex!["9f56827ee7bcb5d8fc1e2435d1507de658e1c9527b74214a4d3a5b85782c5826"].unchecked_into(),
			hex!["6abdbd6cf81c5614ee8d88cfcea1b63815d568246d4e0dd96142383e27856668"].unchecked_into(),
			hex!["6abdbd6cf81c5614ee8d88cfcea1b63815d568246d4e0dd96142383e27856668"].unchecked_into(),
			hex!["6abdbd6cf81c5614ee8d88cfcea1b63815d568246d4e0dd96142383e27856668"].unchecked_into(),
		),
		(
			hex!["2692657edfb0ec23565a1c0c01aa0815eb459ce9f267076986530089dab7a96a"].into(),
			hex!["e618c1d6e00a1f25402af29f060b7af2f1f11f377c8c6a9a43586cd39c89a432"].into(),
			hex!["f5d2693f177ea44d6628f13a2db10a2848e83885391681a5710736b3bcab9078"].unchecked_into(),
			hex!["0c874fc16ec3cdfbec89f81051b3ff14762be701aca492f5e8c828bbab88a97b"].unchecked_into(),
			hex!["0c874fc16ec3cdfbec89f81051b3ff14762be701aca492f5e8c828bbab88a97b"].unchecked_into(),
			hex!["0c874fc16ec3cdfbec89f81051b3ff14762be701aca492f5e8c828bbab88a97b"].unchecked_into(),
		),
		(
			hex!["065f97d33ac0f47e079b28d5ecc3645ac69ae58a4e024608310d69c203ea2c07"].into(),
			hex!["68749f084bab53534f8ef065160d0e18d0e375603dcba9250bf642b2773a865f"].into(),
			hex!["693fc1d55856cf72085e72823c61d6d8f75fbcf89347f0a263ef1d03d01b22b4"].unchecked_into(),
			hex!["4c281d2ebaecd55439931f3c7f135f3b6ca8ea1e6bf1fc814a611186b678f632"].unchecked_into(),
			hex!["4c281d2ebaecd55439931f3c7f135f3b6ca8ea1e6bf1fc814a611186b678f632"].unchecked_into(),
			hex!["4c281d2ebaecd55439931f3c7f135f3b6ca8ea1e6bf1fc814a611186b678f632"].unchecked_into(),
		),
	];
	let mut endowed_accounts = vec![root_key.clone()];
	for (stash, controller, _, _, _, _) in &initial_authorities {
		// Validator stash and controller accounts should be pre-funded
		// to allow an immediate network start
		endowed_accounts.push(stash.clone());
		endowed_accounts.push(controller.clone());
	}

	NetworkKeys {
		endowed_accounts,
		initial_authorities,
		root_key,
	}
}

/// Returns ChainSpec for MainNet Azalea
pub fn config() -> ChainSpec {
	ChainSpec::from_genesis(
		"CENNZnet Azalea",                 // name
		"CENNZnet Azalea V1",              // ID
		|| config_genesis(network_keys()), // constructor
		// boot nodes
		vec![
			"/dns4/bootnode-0.cennznet.cloud/tcp/30333/p2p/QmfZPLAQGLc8UsmQpAwj3CTo3xTtfKxFiqD4SNvESv4Qn6".to_owned(),
			"/dns4/bootnode-1.cennznet.cloud/tcp/30333/p2p/QmW5Uc3YuX7Ch9H6zikk4pfzqVZu1aPkN3htFd8r7BdhfC".to_owned(),
			"/dns4/bootnode-2.cennznet.cloud/tcp/30333/p2p/QmTP4ywbknv5DEPXUKoZKD3wkxnBVpm48x5Qqcevay3D9m".to_owned(),
			"/dns4/bootnode-3.cennznet.cloud/tcp/30333/p2p/QmZKzALNjgXHtybv5oFFdpRVYXW5Cmq14yb6CxG9Y2iKqB".to_owned(),
			"/dns4/bootnode-4.cennznet.cloud/tcp/30333/p2p/Qmdj7T3jvW4NonXP7yTQtNgEXta689nBZeN2agfpCSiSC4".to_owned(),
			"/dns4/bootnode-5.cennznet.cloud/tcp/30333/p2p/Qmdh9erm8wY7taRtwFLqKddavyZoJtVqmc2hb2T9w2KKfP".to_owned(),
			"/dns4/bootnode-6.cennznet.cloud/tcp/30333/p2p/Qma4MkSe8zgYTa8RACt3CMvWYyayesfanq65ASV7ePruhN".to_owned(),
			"/dns4/bootnode-7.cennznet.cloud/tcp/30333/p2p/QmWwHTPc77UtQQ3zibccjwZioeu9SJTojNGZevionNyKkn".to_owned(),
			"/dns4/bootnode-8.cennznet.cloud/tcp/30333/p2p/QmeSgGkM3TwGwb5oLgaYNietGzQRcrDBvTAk2o9azz6dJL".to_owned(),
		],
		None,                       // telemetry
		Some("cennznet-azalea-v1"), // lib-p2p protocol ID
		None,                       // properties
		Default::default(),         // generic extension types
	)
}

/// Helper function to create GenesisConfig
pub fn config_genesis(network_keys: NetworkKeys) -> GenesisConfig {
	// The initial amount to bond validators with for staking
	const INITIAL_BOND: Balance = 10_000 * DOLLARS;
	// The minimum bond for network staking
	const MINIMUM_BOND: Balance = INITIAL_BOND - (1 * DOLLARS);

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
				enable_println: false, // this should only be enabled on development chains
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
			initial_balance: INITIAL_BOND * 2,
			endowed_accounts: endowed_accounts,
			next_asset_id: NEXT_ASSET_ID,
			staking_asset_id: STAKING_ASSET_ID,
			spending_asset_id: SPENDING_ASSET_ID,
			asset_meta: vec![
				(CENNZ_ASSET_ID, AssetInfo::new(b"CENNZ".to_vec(), 1)),
				(CENTRAPAY_ASSET_ID, AssetInfo::new(b"CPAY".to_vec(), 2)),
			],
		}),
		crml_rewards: Some(RewardsConfig {
			development_fund_take: Perbill::from_percent(20),
		}),
		crml_staking: Some(StakingConfig {
			current_era: 0,
			validator_count: 12,
			minimum_validator_count: 6,
			stakers: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.1.clone(), INITIAL_BOND, StakerStatus::Validator))
				.collect(),
			invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
			minimum_bond: MINIMUM_BOND,
			slash_reward_fraction: Perbill::from_percent(10),
			..Default::default()
		}),
		crml_cennzx_spot: Some(CennzxSpotConfig {
			fee_rate: FeeRate::<PerMillion>::try_from(FeeRate::<PerThousand>::from(3u128)).unwrap(),
			core_asset_id: CENTRAPAY_ASSET_ID,
		}),
	}
}
