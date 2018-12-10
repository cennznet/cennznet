//! CENNZNET chain configurations.

use primitives::{AuthorityId, ed25519};
use cennznet_primitives::AccountId;
use cennznet_runtime::{ConsensusConfig, CouncilSeatsConfig, CouncilVotingConfig, DemocracyConfig,
	SessionConfig, StakingConfig, TimestampConfig, BalancesConfig, TreasuryConfig,
	UpgradeKeyConfig, ContractConfig, GrandpaConfig, Permill, Perbill};
pub use cennznet_runtime::GenesisConfig;
use substrate_service;

use substrate_keystore::pad_seed;

const DEV_TELEMETRY_URL: Option<&str> = Some("wss://cennznet-telemetry.centrality.me");

/// Specialised `ChainSpec`.
pub type ChainSpec = substrate_service::ChainSpec<GenesisConfig>;

/// Helper function to generate AuthorityID from seed
pub fn get_authority_id_from_seed(seed: &str) -> AuthorityId {
	let padded_seed = pad_seed(seed);
	// NOTE from ed25519 impl:
	// prefer pkcs#8 unless security doesn't matter -- this is used primarily for tests.
	ed25519::Pair::from_seed(&padded_seed).public().0.into()
}

/// genesis config for DEV env
fn cennznet_dev_genesis(
	initial_authorities: Vec<AuthorityId>,
	upgrade_key: AccountId,
	endowed_accounts: Option<Vec<AuthorityId>>,
) -> GenesisConfig {
	let endowed_accounts = endowed_accounts.unwrap_or_else(|| {
		vec![
			get_authority_id_from_seed("Andrea"),
			get_authority_id_from_seed("Brooke"),
			get_authority_id_from_seed("Courtney"),
			get_authority_id_from_seed("Drew"),
			get_authority_id_from_seed("Emily"),
			get_authority_id_from_seed("Frank"),
		]
	});
	GenesisConfig {
		consensus: Some(ConsensusConfig {
			code: include_bytes!("../runtime/wasm/target/wasm32-unknown-unknown/release/cennznet_runtime.compact.wasm").to_vec(),
			authorities: initial_authorities.clone(),
			_genesis_phantom_data: Default::default(),
		}),
		system: None,
		balances: Some(BalancesConfig {
			transaction_base_fee: 1,
			transaction_byte_fee: 0,
			existential_deposit: 500,
			transfer_fee: 0,
			creation_fee: 0,
			reclaim_rebate: 0,
			balances: endowed_accounts.iter().map(|&k| (k.into(), (1 << 60))).collect(),
			_genesis_phantom_data: Default::default(),
		}),
		session: Some(SessionConfig {
			validators: initial_authorities.iter().cloned().map(Into::into).collect(),
			session_length: 10,
			_genesis_phantom_data: Default::default(),
		}),
		staking: Some(StakingConfig {
			current_era: 0,
			intentions: initial_authorities.iter().cloned().map(Into::into).collect(),
			minimum_validator_count: 2,
			validator_count: 3,
			sessions_per_era: 5,
			bonding_duration: 2 * 60 * 12,
			offline_slash: Perbill::zero(),
			session_reward: Perbill::zero(),
			current_offline_slash: 0,
			current_session_reward: 0,
			offline_slash_grace: 0,
			_genesis_phantom_data: Default::default(),
		}),
		democracy: Some(DemocracyConfig {
			launch_period: 9,
			voting_period: 18,
			minimum_deposit: 10,
			_genesis_phantom_data: Default::default(),
		}),
		council_seats: Some(CouncilSeatsConfig {
			active_council: endowed_accounts.iter()
			.filter(|a| initial_authorities.iter().find(|&b| a.0 == b.0).is_none())
				.map(|a| (a.clone().into(), 1000000)).collect(),
			candidacy_bond: 10,
			voter_bond: 2,
			present_slash_per_voter: 1,
			carry_count: 4,
			presentation_duration: 10,
			approval_voting_period: 20,
			term_duration: 1000000,
			desired_seats: (endowed_accounts.len() - initial_authorities.len()) as u32,
			inactive_grace_period: 1,
			_genesis_phantom_data: Default::default(),
		}),
		council_voting: Some(CouncilVotingConfig {
			cooloff_period: 75,
			voting_period: 20,
			_genesis_phantom_data: Default::default(),
		}),
		timestamp: Some(TimestampConfig {
			period: 5,                    // 5 second block time.
			_genesis_phantom_data: Default::default(),
		}),
		treasury: Some(TreasuryConfig {
			proposal_bond: Permill::from_percent(5),
			proposal_bond_minimum: 1_000_000,
			spend_period: 12 * 60 * 24,
			burn: Permill::from_percent(50),
			_genesis_phantom_data: Default::default(),
		}),
		contract: Some(ContractConfig {
			contract_fee: 21,
			call_base_fee: 135,
			create_base_fee: 175,
			gas_price: 1,
			max_depth: 1024,
			block_gas_limit: 10_000_000,
			current_schedule: Default::default(),
			_genesis_phantom_data: Default::default(),
		}),
		upgrade_key: Some(UpgradeKeyConfig {
			key: upgrade_key,
			_genesis_phantom_data: Default::default(),
		}),
		grandpa: Some(GrandpaConfig {
			authorities: initial_authorities.clone().into_iter().map(|k| (k, 1)).collect(),
			_genesis_phantom_data: Default::default(),
		}),
		sylo: None,
	}
}

pub fn local_dev_genesis(
	initial_authorities: Vec<AuthorityId>,
	upgrade_key: AccountId,
	endowed_accounts: Option<Vec<AuthorityId>>,
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
			code: include_bytes!("../runtime/wasm/target/wasm32-unknown-unknown/release/cennznet_runtime.compact.wasm").to_vec(),
			authorities: initial_authorities.clone(),
			_genesis_phantom_data: Default::default(),
		}),
		system: None,
		balances: Some(BalancesConfig {
			transaction_base_fee: 1,
			transaction_byte_fee: 0,
			existential_deposit: 500,
			transfer_fee: 0,
			creation_fee: 0,
			reclaim_rebate: 0,
			balances: endowed_accounts.iter().map(|&k| (k.into(), (1 << 60))).collect(),
			_genesis_phantom_data: Default::default(),
		}),
		session: Some(SessionConfig {
			validators: initial_authorities.iter().cloned().map(Into::into).collect(),
			session_length: 10,
			_genesis_phantom_data: Default::default(),
		}),
		staking: Some(StakingConfig {
			current_era: 0,
			intentions: initial_authorities.iter().cloned().map(Into::into).collect(),
			minimum_validator_count: 1,
			validator_count: 2,
			sessions_per_era: 5,
			bonding_duration: 2 * 60 * 12,
			offline_slash: Perbill::zero(),
			session_reward: Perbill::zero(),
			current_offline_slash: 0,
			current_session_reward: 0,
			offline_slash_grace: 0,
			_genesis_phantom_data: Default::default(),
		}),
		democracy: Some(DemocracyConfig {
			launch_period: 9,
			voting_period: 18,
			minimum_deposit: 10,
			_genesis_phantom_data: Default::default(),
		}),
		council_seats: Some(CouncilSeatsConfig {
			active_council: endowed_accounts.iter()
			.filter(|a| initial_authorities.iter().find(|&b| a.0 == b.0).is_none())
				.map(|a| (a.clone().into(), 1000000)).collect(),
			candidacy_bond: 10,
			voter_bond: 2,
			present_slash_per_voter: 1,
			carry_count: 4,
			presentation_duration: 10,
			approval_voting_period: 20,
			term_duration: 1000000,
			desired_seats: (endowed_accounts.len() - initial_authorities.len()) as u32,
			inactive_grace_period: 1,
			_genesis_phantom_data: Default::default(),
		}),
		council_voting: Some(CouncilVotingConfig {
			cooloff_period: 75,
			voting_period: 20,
			_genesis_phantom_data: Default::default(),
		}),
		timestamp: Some(TimestampConfig {
			period: 5,                    // 5 second block time.
			_genesis_phantom_data: Default::default(),
		}),
		treasury: Some(TreasuryConfig {
			proposal_bond: Permill::from_percent(5),
			proposal_bond_minimum: 1_000_000,
			spend_period: 12 * 60 * 24,
			burn: Permill::from_percent(50),
			_genesis_phantom_data: Default::default(),
		}),
		contract: Some(ContractConfig {
			contract_fee: 21,
			call_base_fee: 135,
			create_base_fee: 175,
			gas_price: 1,
			max_depth: 1024,
			block_gas_limit: 10_000_000,
			current_schedule: Default::default(),
			_genesis_phantom_data: Default::default(),
		}),
		upgrade_key: Some(UpgradeKeyConfig {
			key: upgrade_key,
			_genesis_phantom_data: Default::default(),
		}),
		grandpa: Some(GrandpaConfig {
			authorities: initial_authorities.clone().into_iter().map(|k| (k, 1)).collect(),
			_genesis_phantom_data: Default::default(),
		}),
		sylo: None,
	}
}

/// The CENNZnet DEV testnet config (load from "genesis/dev.json")
pub fn cennznet_dev_config() -> Result<ChainSpec, String> {
	ChainSpec::from_embedded(include_bytes!("../genesis/dev.json")).map_err(|e| format!("{} at genesis/dev.json", e))
}

/// The CENNZnet DEV testnet genesis (created from code)
pub fn cennznet_dev_config_genesis() -> GenesisConfig {
	cennznet_dev_genesis(
		vec![
			get_authority_id_from_seed("Andrea"),
			get_authority_id_from_seed("Brooke"),
			get_authority_id_from_seed("Courtney"),
		],
		get_authority_id_from_seed("Centrality").into(),
		None,
	)
}

/// Local cennznet dev config (multivalidator Alice + Bob)
pub fn local_cennznet_dev_config() -> Result<ChainSpec, String> {
	Ok(
		ChainSpec::from_genesis("Local CENNZnet DEV", "local_cennznet_dev", cennznet_dev_config_genesis, vec![], DEV_TELEMETRY_URL, None, None, None)
	)
}

fn local_dev_config_genesis() -> GenesisConfig {
	local_dev_genesis(
		vec![
			get_authority_id_from_seed("Alice"),
		],
		get_authority_id_from_seed("Alice").into(),
		None,
	)
}

/// Local testnet config
pub fn local_dev_config() -> Result<ChainSpec, String> {
	Ok(
		ChainSpec::from_genesis("Development", "development", local_dev_config_genesis, vec![], None, None, None, None)
	)
}
