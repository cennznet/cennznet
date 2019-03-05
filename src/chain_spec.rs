//! CENNZNET chain configurations.

use cennznet_primitives::AccountId;
pub use cennznet_runtime::GenesisConfig;
use cennznet_runtime::{
	BalancesConfig, ConsensusConfig, ContractConfig, CouncilSeatsConfig, CouncilVotingConfig, DemocracyConfig,
	FeesConfig, GenericAssetConfig, GrandpaConfig, IndicesConfig, Perbill, Permill, SessionConfig, SpotExchangeConfig,
	StakingConfig, SudoConfig, TimestampConfig, TreasuryConfig,
};
use primitives::{ed25519, Ed25519AuthorityId};
use substrate_service;

use substrate_keystore::pad_seed;
use substrate_telemetry::TelemetryEndpoints;

const DEV_TELEMETRY_URL: &str = "ws://cennznet-telemetry.centrality.me:1024";

/// Specialised `ChainSpec`.
pub type ChainSpec = substrate_service::ChainSpec<GenesisConfig>;

/// Helper function to generate AuthorityID from seed
pub fn get_authority_id_from_seed(seed: &str) -> Ed25519AuthorityId {
	let padded_seed = pad_seed(seed);
	// NOTE from ed25519 impl:
	// prefer pkcs#8 unless security doesn't matter -- this is used primarily for tests.
	ed25519::Pair::from_seed(&padded_seed).public().0.into()
}

/// genesis config for DEV/UAT env
fn cennznet_dev_uat_genesis(
	initial_authorities: Vec<Ed25519AuthorityId>,
	root_key: AccountId,
	endowed_accounts: Option<Vec<Ed25519AuthorityId>>,
) -> GenesisConfig {
	let endowed_accounts = endowed_accounts.unwrap_or_else(|| {
		vec![
			get_authority_id_from_seed("Andrea"),
			get_authority_id_from_seed("Brooke"),
			get_authority_id_from_seed("Courtney"),
			get_authority_id_from_seed("Drew"),
			get_authority_id_from_seed("Emily"),
			get_authority_id_from_seed("Frank"),
			get_authority_id_from_seed("Centrality"),
			get_authority_id_from_seed("Kauri"),
			get_authority_id_from_seed("Rimu"),
		]
	});
	GenesisConfig {
		consensus: Some(ConsensusConfig {
			code: include_bytes!("../runtime/wasm/target/wasm32-unknown-unknown/release/cennznet_runtime.compact.wasm")
				.to_vec(),
			authorities: initial_authorities.clone(),
		}),
		system: None,
		balances: Some(BalancesConfig {
			existential_deposit: 50,
			transfer_fee: 1,
			creation_fee: 1,
			vesting: vec![],
		}),
		indices: Some(IndicesConfig {
			ids: endowed_accounts.iter().map(|x| x.0.into()).collect(),
		}),
		session: Some(SessionConfig {
			validators: initial_authorities.iter().cloned().map(Into::into).collect(),
			session_length: 10,
		}),
		staking: Some(StakingConfig {
			current_era: 0,
			intentions: initial_authorities.iter().cloned().map(Into::into).collect(),
			minimum_validator_count: 2,
			validator_count: 5,
			sessions_per_era: 5,
			bonding_duration: 2 * 60 * 12,
			offline_slash: Perbill::from_billionths(1_001),
			session_reward: Perbill::from_billionths(2_065),
			current_offline_slash: 0,
			current_session_reward: 0,
			offline_slash_grace: 1,
			invulnerables: initial_authorities.iter().cloned().map(Into::into).collect(),
		}),
		democracy: Some(DemocracyConfig {
			launch_period: 9,
			voting_period: 18,
			minimum_deposit: 100,
			public_delay: 5,
			max_lock_periods: 6,
		}),
		council_seats: Some(CouncilSeatsConfig {
			active_council: endowed_accounts
				.iter()
				.filter(|a| initial_authorities.iter().find(|&b| a.0 == b.0).is_none())
				.map(|a| (a.clone().into(), 1_000_000))
				.collect(),
			candidacy_bond: 10,
			voter_bond: 2,
			present_slash_per_voter: 1,
			carry_count: 4,
			presentation_duration: 10,
			approval_voting_period: 20,
			term_duration: 1_000_000,
			desired_seats: (endowed_accounts.len() - initial_authorities.len()) as u32,
			inactive_grace_period: 1,
		}),
		council_voting: Some(CouncilVotingConfig {
			cooloff_period: 75,
			voting_period: 20,
			enact_delay_period: 0,
		}),
		timestamp: Some(TimestampConfig {
			period: 3, // block time = period * 2
		}),
		treasury: Some(TreasuryConfig {
			proposal_bond: Permill::from_percent(5),
			proposal_bond_minimum: 1_000_000,
			spend_period: 12 * 60 * 24,
			burn: Permill::from_percent(50),
		}),
		contract: Some(ContractConfig {
			contract_fee: 21,
			call_base_fee: 135,
			create_base_fee: 175,
			gas_price: 1,
			max_depth: 1024,
			block_gas_limit: 10_000_000,
			current_schedule: Default::default(),
		}),
		sudo: Some(SudoConfig { key: root_key }),
		grandpa: Some(GrandpaConfig {
			authorities: initial_authorities.clone().into_iter().map(|k| (k, 1)).collect(),
		}),
		generic_asset: Some(GenericAssetConfig {
			assets: vec![
				// Staking token
				0, // CENNZ
				// Spending token
				10, // CENTRAPAY
				// Reserve Tokens
				100, // PLUG
				101, // SYLO
				102, // CERTI
				103, // ARDA
			],
			initial_balance: 10u128.pow(18 + 9), // 1 billion token with 18 decimals
			endowed_accounts: endowed_accounts.clone().into_iter().map(Into::into).collect(),
			// ids smaller than 1_000_000 are reserved
			next_asset_id: 1_000_000,
			create_asset_stake: 1000,
			transfer_fee: 20,
		}),
		fees: Some(FeesConfig {
			transaction_base_fee: 10,
			transaction_byte_fee: 1,
		}),
		cennz_x: Some(SpotExchangeConfig {
			fee_rate: Permill::from_millionths(3000),
			core_asset_id: 10,
		}),
	}
}

pub fn local_dev_genesis(
	initial_authorities: Vec<Ed25519AuthorityId>,
	root_key: AccountId,
	endowed_accounts: Option<Vec<Ed25519AuthorityId>>,
) -> GenesisConfig {
	let endowed_accounts = endowed_accounts.unwrap_or_else(|| {
		vec![
			get_authority_id_from_seed("Alice"),
			get_authority_id_from_seed("Bob"),
			get_authority_id_from_seed("Charlie"),
			get_authority_id_from_seed("Dave"),
			get_authority_id_from_seed("Eve"),
			get_authority_id_from_seed("Ferdie"),
		]
	});
	GenesisConfig {
		consensus: Some(ConsensusConfig {
			code: include_bytes!("../runtime/wasm/target/wasm32-unknown-unknown/release/cennznet_runtime.compact.wasm")
				.to_vec(),
			authorities: initial_authorities.clone(),
		}),
		system: None,
		balances: Some(BalancesConfig {
			existential_deposit: 50,
			transfer_fee: 1,
			creation_fee: 1,
			vesting: vec![],
		}),
		indices: Some(IndicesConfig {
			ids: endowed_accounts.iter().map(|x| x.0.into()).collect(),
		}),
		session: Some(SessionConfig {
			validators: initial_authorities.iter().cloned().map(Into::into).collect(),
			session_length: 10,
		}),
		staking: Some(StakingConfig {
			current_era: 0,
			intentions: initial_authorities.iter().cloned().map(Into::into).collect(),
			minimum_validator_count: 1,
			validator_count: 2,
			sessions_per_era: 5,
			bonding_duration: 2 * 60 * 12,
			offline_slash: Perbill::from_billionths(10),
			session_reward: Perbill::from_billionths(10),
			current_offline_slash: 0,
			current_session_reward: 0,
			offline_slash_grace: 0,
			invulnerables: initial_authorities.iter().cloned().map(Into::into).collect(),
		}),
		democracy: Some(DemocracyConfig {
			launch_period: 9,
			voting_period: 18,
			minimum_deposit: 10,
			public_delay: 0,
			max_lock_periods: 6,
		}),
		council_seats: Some(CouncilSeatsConfig {
			active_council: endowed_accounts
				.iter()
				.filter(|a| initial_authorities.iter().find(|&b| a.0 == b.0).is_none())
				.map(|a| (a.clone().into(), 1_000_000))
				.collect(),
			candidacy_bond: 10,
			voter_bond: 2,
			present_slash_per_voter: 1,
			carry_count: 4,
			presentation_duration: 10,
			approval_voting_period: 20,
			term_duration: 1_000_000,
			desired_seats: (endowed_accounts.len() - initial_authorities.len()) as u32,
			inactive_grace_period: 1,
		}),
		council_voting: Some(CouncilVotingConfig {
			cooloff_period: 75,
			voting_period: 20,
			enact_delay_period: 0,
		}),
		timestamp: Some(TimestampConfig {
			period: 2, // block time = period * 2
		}),
		treasury: Some(TreasuryConfig {
			proposal_bond: Permill::from_percent(5),
			proposal_bond_minimum: 1_000_000,
			spend_period: 12 * 60 * 24,
			burn: Permill::from_percent(50),
		}),
		contract: Some(ContractConfig {
			contract_fee: 21,
			call_base_fee: 135,
			create_base_fee: 175,
			gas_price: 1,
			max_depth: 1024,
			block_gas_limit: 10_000_000,
			current_schedule: Default::default(),
		}),
		sudo: Some(SudoConfig { key: root_key }),
		grandpa: Some(GrandpaConfig {
			authorities: initial_authorities.clone().into_iter().map(|k| (k, 1)).collect(),
		}),
		generic_asset: Some(GenericAssetConfig {
			assets: vec![
				// Staking token
				0, // CENNZ
				// Spending token
				10, // CENTRAPAY
				// Reserve Tokens
				100, // PLUG
				101, // SYLO
				102, // CERTI
				103, // ARDA
			],
			initial_balance: 10u128.pow(18 + 9), // 1 billion token with 18 decimals
			endowed_accounts: endowed_accounts.clone().into_iter().map(Into::into).collect(),
			// ids smaller than 1_000_000 are reserved
			next_asset_id: 1_000_000,
			create_asset_stake: 1000,
			transfer_fee: 20,
		}),
		fees: Some(FeesConfig {
			transaction_base_fee: 1,
			transaction_byte_fee: 1,
		}),
		cennz_x: Some(SpotExchangeConfig {
			fee_rate: Permill::from_millionths(3000),
			core_asset_id: 10,
		}),
	}
}

/// The CENNZnet DEV testnet config
pub fn cennznet_dev_config() -> Result<ChainSpec, String> {
	ChainSpec::from_embedded(include_bytes!("../genesis/dev/genesis.json"))
		.map_err(|e| format!("Error loading genesis for Kauri CENNZnet testnet {}", e))
}

/// The CENNZnet UAT testnet config
pub fn cennznet_uat_config() -> Result<ChainSpec, String> {
	ChainSpec::from_embedded(include_bytes!("../genesis/uat/genesis.json"))
		.map_err(|e| format!("Error loading genesis for Rimu CENNZnet testnet {}", e))
}

/// The CENNZnet Kauri testnet genesis)
pub fn cennznet_kauri_config_genesis() -> GenesisConfig {
	cennznet_dev_uat_genesis(
		vec![
			get_authority_id_from_seed("Andrea"),
			get_authority_id_from_seed("Brooke"),
			get_authority_id_from_seed("Courtney"),
		],
		get_authority_id_from_seed("Kauri").into(),
		None,
	)
}

/// The CENNZnet Rimu testnet genesis
pub fn cennznet_rimu_config_genesis() -> GenesisConfig {
	cennznet_dev_uat_genesis(
		vec![
			get_authority_id_from_seed("Andrea"),
			get_authority_id_from_seed("Brooke"),
			get_authority_id_from_seed("Courtney"),
		],
		get_authority_id_from_seed("Rimu").into(),
		None,
	)
}

/// The CENNZnet DEV testnet config with latest runtime
pub fn cennznet_dev_config_latest() -> Result<ChainSpec, String> {
	Ok(ChainSpec::from_genesis(
		"Kauri CENNZnet",
		"kauri",
		cennznet_kauri_config_genesis,
		vec![
			String::from(
				"/dns4/cennznet-bootnode-0.centrality.me/tcp/30333/p2p/Qmdpvn9xttHZ5SQePVhhsk8dFMHCUaS3EDQcGDZ8MuKbx2",
			),
			String::from(
				"/dns4/cennznet-bootnode-1.centrality.me/tcp/30333/p2p/QmRaZu8UNGejxuGB9pMhjw5GZEVVBkaRiYYhhLYYUkT8qa",
			),
			String::from(
				"/dns4/cennznet-bootnode-2.centrality.me/tcp/30333/p2p/QmTEUaAyqq3spjKSFLWw5gG8tzZ6xwbt5ptTKvs65VkBPJ",
			),
		],
		Some(TelemetryEndpoints::new(vec![(DEV_TELEMETRY_URL.into(), 0)])),
		None,
		None,
		None,
	))
}

/// The CENNZnet UAT testnet config with latest runtime
pub fn cennznet_uat_config_latest() -> Result<ChainSpec, String> {
	Ok(ChainSpec::from_genesis(
		"Rimu CENNZnet",
		"rimu",
		cennznet_rimu_config_genesis,
		vec![
				String::from("/dns4/cennznet-bootnode-0.centrality.cloud/tcp/30333/p2p/QmQZ8TjTqeDj3ciwr93EJ95hxfDsb9pEYDizUAbWpigtQN"),
				String::from("/dns4/cennznet-bootnode-1.centrality.cloud/tcp/30333/p2p/QmXiB3jqqn2rpiKU7k1h7NJYeBg8WNSx9DiTRKz9ti2KSK"),
				String::from("/dns4/cennznet-bootnode-2.centrality.cloud/tcp/30333/p2p/QmYcHeEWuqtr6Gb5EbK7zEhnaCm5p6vA2kWcVjFKbhApaC")
			],
		Some(TelemetryEndpoints::new(vec![(DEV_TELEMETRY_URL.into(), 0)])),
		None,
		None,
		None,
	))
}

fn local_dev_config_genesis() -> GenesisConfig {
	local_dev_genesis(
		vec![get_authority_id_from_seed("Alice")],
		get_authority_id_from_seed("Alice").into(),
		None,
	)
}

/// The CENNZnet Kauri testnet config for local test purpose
pub fn cennznet_dev_local_config() -> Result<ChainSpec, String> {
	Ok(ChainSpec::from_genesis(
		"Kauri Dev",
		"kauri-dev",
		cennznet_kauri_config_genesis,
		vec![],
		None,
		None,
		None,
		None,
	))
}

/// Local testnet config
pub fn local_dev_config() -> Result<ChainSpec, String> {
	Ok(ChainSpec::from_genesis(
		"Development",
		"development",
		local_dev_config_genesis,
		vec![],
		None,
		None,
		None,
		None,
	))
}
