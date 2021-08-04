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
	AssetInfo, AuthorityDiscoveryConfig, BabeConfig, CennzxConfig, EthBridgeConfig, FeeRate, GenericAssetConfig,
	GrandpaConfig, ImOnlineConfig, PerMillion, PerThousand, RewardsConfig, SessionConfig, SessionKeys, StakerStatus,
	StakingConfig, SudoConfig, SystemConfig, WASM_BINARY,
};
use core::convert::TryFrom;
use crml_eth_bridge::crypto::AuthorityId as EthBridgeId;
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use sc_chain_spec::ChainSpecExtension;
use serde::{Deserialize, Serialize};
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_core::{sr25519, Pair, Public, H160};
use sp_finality_grandpa::AuthorityId as GrandpaId;
use sp_runtime::{
	traits::{IdentifyAccount, Verify},
	FixedPointNumber, FixedU128, Perbill,
};
use std::str::FromStr;

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
	// Ethereum bridge ID
	EthBridgeId,
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
	EthBridgeId,
) {
	(
		get_account_id_from_seed::<sr25519::Public>(&format!("{}//stash", seed)),
		get_account_id_from_seed::<sr25519::Public>(seed),
		get_from_seed::<GrandpaId>(seed),
		get_from_seed::<BabeId>(seed),
		get_from_seed::<ImOnlineId>(seed),
		get_from_seed::<AuthorityDiscoveryId>(seed),
		get_from_seed::<EthBridgeId>(seed),
	)
}

/// Helper function to generate session keys with authority keys
pub fn session_keys(
	grandpa: GrandpaId,
	babe: BabeId,
	im_online: ImOnlineId,
	authority_discovery: AuthorityDiscoveryId,
	eth_bridge: EthBridgeId,
) -> SessionKeys {
	SessionKeys {
		grandpa,
		babe,
		im_online,
		authority_discovery,
		eth_bridge,
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

	// well-known ERC20 token addresses
	// metadata used by Eth bridge to map token claims when creating generic assets
	let erc20s = vec![
		// test only
		(
			H160::from_str("0x1215b4ec8161b7959a115805bf980e57a085c3e5").unwrap(),
			b"YOLO".to_vec(),
			18,
		),
		// end test
		(
			H160::from_str("0xd4fffa07929b1901fdb30c1c67f80e1185d4210f").unwrap(),
			b"CERTI".to_vec(),
			18,
		),
		(
			H160::from_str("0xf293d23bf2cdc05411ca0eddd588eb1977e8dcd4").unwrap(),
			b"SYLO".to_vec(),
			18,
		),
		(
			H160::from_str("0x1122b6a0e00dce0563082b6e2953f3a943855c1f").unwrap(),
			b"CENNZ".to_vec(),
			18,
		),
		(
			H160::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap(),
			b"WETH".to_vec(),
			18,
		),
		(
			H160::from_str("0x6B175474E89094C44Da98b954EedeAC495271d0F").unwrap(),
			b"DAI".to_vec(),
			18,
		),
		(
			H160::from_str("0xE41d2489571d322189246DaFA5ebDe1F4699F498").unwrap(),
			b"ZRX".to_vec(),
			18,
		),
		(
			H160::from_str("0xD533a949740bb3306d119CC777fa900bA034cd52").unwrap(),
			b"CRV".to_vec(),
			18,
		),
		(
			H160::from_str("0x1f9840a85d5aF5bf1D1762F925BDADdC4201F984").unwrap(),
			b"UNI".to_vec(),
			18,
		),
		(
			H160::from_str("0xdAC17F958D2ee523a2206206994597C13D831ec7").unwrap(),
			b"USDT".to_vec(),
			6,
		),
		(
			H160::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap(),
			b"USDC".to_vec(),
			6,
		),
		(
			H160::from_str("0x4575f41308EC1483f3d399aa9a2826d74Da13Deb").unwrap(),
			b"OXT".to_vec(),
			18,
		),
		(
			H160::from_str("0x9f8F72aA9304c8B593d555F12eF6589cC3A579A2").unwrap(),
			b"MKR".to_vec(),
			18,
		),
		(
			H160::from_str("0x514910771AF9Ca656af840dff83E8264EcF986CA").unwrap(),
			b"LINK".to_vec(),
			18,
		),
		(
			H160::from_str("0x1985365e9f78359a9B6AD760e32412f4a445E862").unwrap(),
			b"REP".to_vec(),
			18,
		),
		(
			H160::from_str("0x221657776846890989a759BA2973e427DfF5C9bB").unwrap(),
			b"REPv2".to_vec(),
			18,
		),
		(
			H160::from_str("0xdd974D5C2e2928deA5F71b9825b8b646686BD200").unwrap(),
			b"KNC".to_vec(),
			18,
		),
		(
			H160::from_str("0xc00e94Cb662C3520282E6f5717214004A7f26888").unwrap(),
			b"COMP".to_vec(),
			18,
		),
		(
			H160::from_str("0xBA11D00c5f74255f56a5E366F4F77f5A186d7f55").unwrap(),
			b"BAND".to_vec(),
			18,
		),
		(
			H160::from_str("0x1776e1F26f98b1A5dF9cD347953a26dd3Cb46671").unwrap(),
			b"NMR".to_vec(),
			18,
		),
		(
			H160::from_str("0x04Fa0d235C4abf4BcF4787aF4CF447DE572eF828").unwrap(),
			b"UMA".to_vec(),
			18,
		),
		(
			H160::from_str("0xBBbbCA6A901c926F240b89EacB641d8Aec7AEafD").unwrap(),
			b"LRC".to_vec(),
			18,
		),
		(
			H160::from_str("0x0bc529c00C6401aEF6D220BE8C6Ea1667F6Ad93e").unwrap(),
			b"YFI".to_vec(),
			18,
		),
		(
			H160::from_str("0x408e41876cCCDC0F92210600ef50372656052a38").unwrap(),
			b"REN".to_vec(),
			18,
		),
		(
			H160::from_str("0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599").unwrap(),
			b"WBTC".to_vec(),
			8,
		),
		(
			H160::from_str("0xba100000625a3754423978a60c9317c58a424e3D").unwrap(),
			b"BAL".to_vec(),
			18,
		),
		(
			H160::from_str("0x4fE83213D56308330EC302a8BD641f1d0113A4Cc").unwrap(),
			b"NU".to_vec(),
			18,
		),
		(
			H160::from_str("0x7Fc66500c84A76Ad7e9c93437bFc5Ac33E2DDaE9").unwrap(),
			b"AAVE".to_vec(),
			18,
		),
		(
			H160::from_str("0xc944E90C64B2c07662A292be6244BDf05Cda44a7").unwrap(),
			b"GRT".to_vec(),
			18,
		),
		(
			H160::from_str("0x1F573D6Fb3F13d689FF844B4cE37794d79a7FF1C").unwrap(),
			b"BNT".to_vec(),
			18,
		),
		(
			H160::from_str("0xC011a73ee8576Fb46F5E1c5751cA3B9Fe0af2a6F").unwrap(),
			b"SNX".to_vec(),
			18,
		),
		(
			H160::from_str("0x0F5D2fB29fb7d3CFeE444a200298f468908cC942").unwrap(),
			b"MANA".to_vec(),
			18,
		),
		(
			H160::from_str("0xA4e8C3Ec456107eA67d3075bF9e3DF3A75823DB0").unwrap(),
			b"LOOM".to_vec(),
			18,
		),
		(
			H160::from_str("0x41e5560054824eA6B0732E656E3Ad64E20e94E45").unwrap(),
			b"CVC".to_vec(),
			8,
		),
		(
			H160::from_str("0x0AbdAce70D3790235af448C88547603b945604ea").unwrap(),
			b"DNT".to_vec(),
			18,
		),
		(
			H160::from_str("0xB64ef51C888972c908CFacf59B47C1AfBC0Ab8aC").unwrap(),
			b"STORJ".to_vec(),
			8,
		),
		(
			H160::from_str("0xfF20817765cB7f73d4bde2e66e067E58D11095C2").unwrap(),
			b"AMP".to_vec(),
			18,
		),
		(
			H160::from_str("0x6810e776880C02933D47DB1b9fc05908e5386b96").unwrap(),
			b"GNO".to_vec(),
			18,
		),
		(
			H160::from_str("0x960b236A07cf122663c4303350609A66A7B288C0").unwrap(),
			b"ANT".to_vec(),
			18,
		),
		(
			H160::from_str("0x85Eee30c52B0b379b046Fb0F85F4f3Dc3009aFEC").unwrap(),
			b"KEEP".to_vec(),
			18,
		),
		(
			H160::from_str("0x8dAEBADE922dF735c38C80C7eBD708Af50815fAa").unwrap(),
			b"TBTC".to_vec(),
			18,
		),
		(
			H160::from_str("0xec67005c4E498Ec7f55E092bd1d35cbC47C91892").unwrap(),
			b"MLN".to_vec(),
			18,
		),
	];

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
						session_keys(x.2.clone(), x.3.clone(), x.4.clone(), x.5.clone(), x.6.clone()),
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
		pallet_sudo: Some(SudoConfig { key: root_key.clone() }),
		pallet_babe: Some(BabeConfig { authorities: vec![] }),
		pallet_im_online: Some(ImOnlineConfig { keys: vec![] }),
		pallet_authority_discovery: Some(AuthorityDiscoveryConfig { keys: vec![] }),
		pallet_grandpa: Some(GrandpaConfig { authorities: vec![] }),
		crml_generic_asset: Some(GenericAssetConfig {
			assets: vec![CENNZ_ASSET_ID, CPAY_ASSET_ID],
			// Grant root key full permissions (mint,burn,update) on the following assets
			permissions: vec![(CENNZ_ASSET_ID, root_key.clone()), (CPAY_ASSET_ID, root_key.clone())],
			initial_balance: 1_000_000 * DOLLARS, // 1,000,000.0000 (4dp asset)
			endowed_accounts,
			next_asset_id: NEXT_ASSET_ID,
			staking_asset_id: STAKING_ASSET_ID,
			spending_asset_id: SPENDING_ASSET_ID,
			asset_meta: vec![
				(CENNZ_ASSET_ID, AssetInfo::new(b"CENNZ".to_vec(), 4, 1)),
				(CPAY_ASSET_ID, AssetInfo::new(b"CPAY".to_vec(), 4, 1)),
			],
		}),
		crml_cennzx: Some(CennzxConfig {
			// 0.003%
			fee_rate: FeeRate::<PerMillion>::try_from(FeeRate::<PerThousand>::from(3u128)).unwrap(),
			core_asset_id: CPAY_ASSET_ID,
		}),
		crml_staking_rewards: Some(RewardsConfig {
			// 20% of all fees
			development_fund_take: Perbill::from_percent(20),
			// 80% APY
			inflation_rate: FixedU128::saturating_from_rational(8, 10),
		}),
		crml_eth_bridge: Some(EthBridgeConfig { erc20s }),
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
