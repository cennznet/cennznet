//! CENNZNET chain configurations.

use primitives::{Ed25519AuthorityId, ed25519};
use cennznet_primitives::AccountId;
use cennznet_runtime::{ConsensusConfig, CouncilSeatsConfig, CouncilVotingConfig, DemocracyConfig,
	SessionConfig, StakingConfig, TimestampConfig, BalancesConfig, TreasuryConfig,
	SudoConfig, ContractConfig, GrandpaConfig, IndicesConfig, GenericAssetConfig, Permill, Perbill};
pub use cennznet_runtime::GenesisConfig;
use substrate_service;

use substrate_keystore::pad_seed;

const DEV_TELEMETRY_URL: Option<&str> = Some("ws://cennznet-telemetry.centrality.me:1024");

/// Specialised `ChainSpec`.
pub type ChainSpec = substrate_service::ChainSpec<GenesisConfig>;

/// Helper function to generate AuthorityID from seed
pub fn get_authority_id_from_seed(seed: &str) -> Ed25519AuthorityId {
	let padded_seed = pad_seed(seed);
	// NOTE from ed25519 impl:
	// prefer pkcs#8 unless security doesn't matter -- this is used primarily for tests.
	ed25519::Pair::from_seed(&padded_seed).public().0.into()
}

/// Helper function to populate genesis generic asset balances for endowed accounts.
fn build_balances_for_accounts(
	asset_ids: Vec<u32>,
	accounts: Vec<AccountId>,
	amount: u128,
) -> Vec<((u32, AccountId), u128)> {
	asset_ids.iter().flat_map(
		|asset_id| accounts.iter().cloned().map(move |account_id| ((asset_id.clone(), account_id), amount))
	).collect()
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
			get_authority_id_from_seed("Centrality")
		]
	});
	GenesisConfig {
		consensus: Some(ConsensusConfig {
			code: include_bytes!("../runtime/wasm/target/wasm32-unknown-unknown/release/cennznet_runtime.compact.wasm").to_vec(),
			authorities: initial_authorities.clone(),
		}),
		system: None,
		balances: Some(BalancesConfig {
			transaction_base_fee: 10,
			transaction_byte_fee: 1,
			existential_deposit: 50,
			transfer_fee: 1,
			creation_fee: 1,
			balances: endowed_accounts.iter().map(|&k| (k.into(), (1 << 60))).collect(),
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
		sudo: Some(SudoConfig {
			key: root_key,
		}),
		grandpa: Some(GrandpaConfig {
			authorities: initial_authorities.clone().into_iter().map(|k| (k, 1)).collect(),
		}),
		generic_asset: Some(GenericAssetConfig {
			total_supply: vec![
				// staking token
				(0, 10u128.pow(30)),
				// spending token
				(10, 10u128.pow(30))
			],
			free_balance: build_balances_for_accounts(vec![0, 10], endowed_accounts.iter().cloned().map(Into::into).collect(), 10u128.pow(28)),
			// ids smaller than 1_000_000 are reserved
			next_asset_id: 1_000_000,
			// dummy
			dummy: 0,
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
			code: include_bytes!("../runtime/wasm/target/wasm32-unknown-unknown/release/cennznet_runtime.compact.wasm").to_vec(),
			authorities: initial_authorities.clone(),
		}),
		system: None,
		balances: Some(BalancesConfig {
			transaction_base_fee: 1,
			transaction_byte_fee: 1,
			existential_deposit: 50,
			transfer_fee: 1,
			creation_fee: 1,
			balances: endowed_accounts.iter().map(|&k| (k.into(), (1 << 60))).collect(),
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
		sudo: Some(SudoConfig {
			key: root_key,
		}),
		grandpa: Some(GrandpaConfig {
			authorities: initial_authorities.clone().into_iter().map(|k| (k, 1)).collect(),
		}),
		generic_asset: Some(GenericAssetConfig {
			total_supply: vec![
				// staking token
				(0, 10u128.pow(30)),
				// spending token
				(10, 10u128.pow(30))
			],
			free_balance: build_balances_for_accounts(vec![0, 10], endowed_accounts.iter().cloned().map(Into::into).collect(), 10u128.pow(28)),
			// ids smaller than 1_000_000 are reserved
			next_asset_id: 1_000_000,
			// dummy
			dummy: 0,
		}),
	}
}

/// The CENNZnet DEV testnet config
pub fn cennznet_dev_config() -> Result<ChainSpec, String> {
	ChainSpec::from_embedded(include_bytes!("../genesis/dev/genesis.json")).map_err(|e| format!("Error loading genesis for CENNZnet DEV testnet {}", e))
}

/// The CENNZnet UAT testnet config
pub fn cennznet_uat_config() -> Result<ChainSpec, String> {
	ChainSpec::from_embedded(include_bytes!("../genesis/uat/genesis.json")).map_err(|e| format!("Error loading genesis for CENNZnet UAT testnet {}", e))
}

/// The CENNZnet DEV/UAT testnet genesis (created from code)
pub fn cennznet_dev_uat_config_genesis() -> GenesisConfig {
	cennznet_dev_uat_genesis(
		vec![
			get_authority_id_from_seed("Andrea"),
			get_authority_id_from_seed("Brooke"),
			get_authority_id_from_seed("Courtney"),
		],
		get_authority_id_from_seed("Centrality").into(),
		None,
	)
}

/// The CENNZnet DEV testnet config with latest runtime
pub fn cennznet_dev_config_latest() -> Result<ChainSpec, String> {
	Ok(
		ChainSpec::from_genesis("CENNZnet DEV", "cennznet_dev", cennznet_dev_uat_config_genesis, vec![
			String::from("/dns4/cennznet-node-0.centrality.me/tcp/30333/p2p/Qmdpvn9xttHZ5SQePVhhsk8dFMHCUaS3EDQcGDZ8MuKbx2"),
			String::from("/dns4/cennznet-node-1.centrality.me/tcp/30333/p2p/QmRaZu8UNGejxuGB9pMhjw5GZEVVBkaRiYYhhLYYUkT8qa"),
			String::from("/dns4/cennznet-node-2.centrality.me/tcp/30333/p2p/QmTEUaAyqq3spjKSFLWw5gG8tzZ6xwbt5ptTKvs65VkBPJ")
		], DEV_TELEMETRY_URL, None, None, None)
	)
}

/// The CENNZnet UAT testnet config with latest runtime
pub fn cennznet_uat_config_latest() -> Result<ChainSpec, String> {
	Ok(
		ChainSpec::from_genesis("CENNZnet UAT", "cennznet_uat", cennznet_dev_uat_config_genesis, vec![
			String::from("/dns4/cennznet-node-0.centrality.cloud/tcp/30333/p2p/QmQZ8TjTqeDj3ciwr93EJ95hxfDsb9pEYDizUAbWpigtQN"),
			String::from("/dns4/cennznet-node-1.centrality.cloud/tcp/30333/p2p/QmXiB3jqqn2rpiKU7k1h7NJYeBg8WNSx9DiTRKz9ti2KSK"),
			String::from("/dns4/cennznet-node-2.centrality.cloud/tcp/30333/p2p/QmYcHeEWuqtr6Gb5EbK7zEhnaCm5p6vA2kWcVjFKbhApaC")
		], DEV_TELEMETRY_URL, None, None, None)
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
