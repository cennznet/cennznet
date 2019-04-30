use super::{get_account_id_from_seed, get_authority_keys_from_seed, ChainSpec, GenesisConfig, TELEMETRY_URL};
use cennznet_primitives::AccountId;
use cennznet_runtime::{
	CennzxSpotConfig, ConsensusConfig, ContractConfig, CouncilSeatsConfig, CouncilVotingConfig, DemocracyConfig,
	FeeRate, FeesConfig, GenericAssetConfig, GrandpaConfig, IndicesConfig, Perbill, Permill, RewardsConfig, Schedule,
	SessionConfig, StakerStatus, StakingConfig, SudoConfig, TimestampConfig, TreasuryConfig,
};
use primitives::ed25519::Public as AuthorityId;
use substrate_telemetry::TelemetryEndpoints;

const DOLLARS: u128 = 1_000_000_000_000_000_000;
const MICRO_DOLLARS: u128 = DOLLARS / 1_000_000;

const SECS_PER_BLOCK: u64 = 6;
const MINUTES: u64 = 60 / SECS_PER_BLOCK;
const HOURS: u64 = MINUTES * 60;
const DAYS: u64 = HOURS * 24;

fn genesis(initial_authorities: Vec<(AccountId, AccountId, AuthorityId)>, root_key: AccountId) -> GenesisConfig {
	let endowed_accounts = vec![
		get_account_id_from_seed("Andrea"),
		get_account_id_from_seed("Brooke"),
		get_account_id_from_seed("Courtney"),
		get_account_id_from_seed("Drew"),
		get_account_id_from_seed("Emily"),
		get_account_id_from_seed("Frank"),
		get_account_id_from_seed("Centrality"),
		get_account_id_from_seed("Kauri"),
		get_account_id_from_seed("Rimu"),
		get_account_id_from_seed("cennznet-js-test"),
		get_account_id_from_seed("Andrea//stash"),
		get_account_id_from_seed("Brooke//stash"),
		get_account_id_from_seed("Courtney//stash"),
		get_account_id_from_seed("Drew//stash"),
		get_account_id_from_seed("Emily//stash"),
		get_account_id_from_seed("Frank//stash"),
		get_account_id_from_seed("Centrality//stash"),
		get_account_id_from_seed("Kauri//stash"),
		get_account_id_from_seed("Rimu//stash"),
		get_account_id_from_seed("cennznet-js-test//stash"),
	];
	let transaction_base_fee = 1;
	let transaction_byte_fee = 1;
	let transfer_fee = 480 * MICRO_DOLLARS;
	GenesisConfig {
		consensus: Some(ConsensusConfig {
			code: include_bytes!(
				"../../runtime/wasm/target/wasm32-unknown-unknown/release/cennznet_runtime.compact.wasm"
			)
			.to_vec(),
			authorities: initial_authorities.iter().map(|x| x.2.clone()).collect(),
		}),
		system: None,
		indices: Some(IndicesConfig {
			ids: endowed_accounts
				.iter()
				.cloned()
				.chain(initial_authorities.iter().map(|x| x.0.clone()))
				.collect::<Vec<_>>(),
		}),
		session: Some(SessionConfig {
			validators: initial_authorities.iter().map(|x| x.1.clone()).collect(),
			session_length: 5 * MINUTES,
			keys: initial_authorities
				.iter()
				.map(|x| (x.1.clone(), x.2.clone()))
				.collect::<Vec<_>>(),
		}),
		staking: Some(StakingConfig {
			current_era: 0,
			minimum_validator_count: 4,
			validator_count: 10,
			sessions_per_era: 6, // 30 min
			bonding_duration: 2,
			offline_slash: Perbill::from_billionths(1000000),
			session_reward: Perbill::from_billionths(1000),
			current_session_reward: 0,
			offline_slash_grace: 3,
			stakers: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.1.clone(), 1_000_000_000, StakerStatus::Validator))
				.collect(),
			invulnerables: initial_authorities.iter().map(|x| x.1.clone()).collect(),
		}),
		democracy: Some(DemocracyConfig {
			launch_period: 5 * MINUTES,
			voting_period: 10 * MINUTES,
			minimum_deposit: 1000 * DOLLARS,
			public_delay: 10 * MINUTES,
			max_lock_periods: 6,
		}),
		council_seats: Some(CouncilSeatsConfig {
			active_council: endowed_accounts
				.iter()
				.filter(|&endowed| {
					initial_authorities
						.iter()
						.find(|&(_, controller, _)| controller == endowed)
						.is_none()
				})
				.map(|a| (a.clone().into(), 1000000))
				.collect(),
			candidacy_bond: 1000 * DOLLARS,
			voter_bond: 100 * DOLLARS,
			present_slash_per_voter: 1 * DOLLARS,
			carry_count: 6,
			presentation_duration: 1 * HOURS,
			approval_voting_period: 2 * HOURS,
			term_duration: 3 * DAYS,
			desired_seats: 11,
			inactive_grace_period: 1,
		}),
		council_voting: Some(CouncilVotingConfig {
			cooloff_period: 1 * HOURS,
			voting_period: 20 * MINUTES,
			enact_delay_period: 0,
		}),
		timestamp: Some(TimestampConfig {
			minimum_period: SECS_PER_BLOCK / 2, // due to the nature of aura the slots are 2*period
		}),
		treasury: Some(TreasuryConfig {
			proposal_bond: Permill::from_percent(5),
			proposal_bond_minimum: 1 * DOLLARS,
			spend_period: 20 * MINUTES,
			burn: Permill::from_percent(50),
		}),
		contract: Some(ContractConfig {
			contract_fee: 500 * MICRO_DOLLARS,
			call_base_fee: 500,
			create_base_fee: 800,
			creation_fee: 0,
			transaction_base_fee,
			transaction_byte_fee,
			transfer_fee,
			gas_price: 1 * MICRO_DOLLARS,
			max_depth: 1024,
			block_gas_limit: 1_000_000_000_000,
			current_schedule: Schedule {
				version: 0,
				put_code_per_byte_cost: 50,
				grow_mem_cost: 2,
				regular_op_cost: 1,
				return_data_per_byte_cost: 2,
				sandbox_data_read_cost: 1,
				sandbox_data_write_cost: 2,
				event_data_per_byte_cost: 5,
				event_data_base_cost: 20,
				max_stack_height: 64 * 1024,
				max_memory_pages: 16,
				enable_println: false,
			},
		}),
		sudo: Some(SudoConfig { key: root_key }),
		grandpa: Some(GrandpaConfig {
			authorities: initial_authorities.iter().map(|x| (x.2.clone(), 1)).collect(),
		}),
		generic_asset: Some(GenericAssetConfig {
			assets: vec![
				// Staking token
				16000, // CENNZ-T
				// Spending token
				16001, // CENTRAPAY-T
				// Reserve Tokens
				16002, // PLUG-T
				16003, // SYLO-T
				16004, // CERTI-T
				16005, // ARDA-T
			],
			initial_balance: 10u128.pow(18 + 9), // 1 billion token with 18 decimals
			endowed_accounts: endowed_accounts.clone().into_iter().map(Into::into).collect(),
			next_asset_id: 17000,
			create_asset_stake: 1000,
			transfer_fee,
			staking_asset_id: 16000,
			spending_asset_id: 16001,
		}),
		fees: Some(FeesConfig {
			transaction_base_fee: 1000 * MICRO_DOLLARS,
			transaction_byte_fee: 5 * MICRO_DOLLARS,
		}),
		cennzx_spot: Some(CennzxSpotConfig {
			fee_rate: FeeRate::from_milli(3),
			core_asset_id: 16001,
		}),
		rewards: Some(RewardsConfig {
			block_reward: 10 * DOLLARS,
		}),
	}
}

pub fn kauri_config() -> Result<ChainSpec, String> {
	ChainSpec::from_embedded(include_bytes!("../../genesis/kauri/genesis.json"))
		.map_err(|e| format!("Error loading genesis for Kauri CENNZnet testnet {}", e))
}

pub fn rimu_config() -> Result<ChainSpec, String> {
	ChainSpec::from_embedded(include_bytes!("../../genesis/rimu/genesis.json"))
		.map_err(|e| format!("Error loading genesis for Rimu CENNZnet testnet {}", e))
}

fn kauri_config_genesis() -> GenesisConfig {
	genesis(
		vec![
			get_authority_keys_from_seed("Andrea"),
			get_authority_keys_from_seed("Brooke"),
		],
		get_account_id_from_seed("Kauri").into(),
	)
}

fn rimu_config_genesis() -> GenesisConfig {
	genesis(
		vec![
			get_authority_keys_from_seed("Andrea"),
			get_authority_keys_from_seed("Brooke"),
			get_authority_keys_from_seed("Courtney"),
			get_authority_keys_from_seed("Drew"),
		],
		get_account_id_from_seed("Rimu").into(),
	)
}

pub fn kauri_latest_config() -> Result<ChainSpec, String> {
	Ok(ChainSpec::from_genesis(
		"Kauri CENNZnet",
		"kauri",
		kauri_config_genesis,
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
		Some(TelemetryEndpoints::new(vec![(TELEMETRY_URL.into(), 0)])),
		Some("kauri"), // protocol id, unique for each chain
		None,
		None,
	))
}

pub fn rimu_latest_config() -> Result<ChainSpec, String> {
	Ok(ChainSpec::from_genesis(
		"Rimu CENNZnet 0.9.17",
		"rimu-9.17",
		rimu_config_genesis,
		vec![
				String::from("/dns4/cennznet-bootnode-0.centrality.cloud/tcp/30333/p2p/QmQZ8TjTqeDj3ciwr93EJ95hxfDsb9pEYDizUAbWpigtQN"),
				String::from("/dns4/cennznet-bootnode-1.centrality.cloud/tcp/30333/p2p/QmXiB3jqqn2rpiKU7k1h7NJYeBg8WNSx9DiTRKz9ti2KSK"),
				String::from("/dns4/cennznet-bootnode-2.centrality.cloud/tcp/30333/p2p/QmYcHeEWuqtr6Gb5EbK7zEhnaCm5p6vA2kWcVjFKbhApaC")
			],
		Some(TelemetryEndpoints::new(vec![(TELEMETRY_URL.into(), 0)])),
		Some("rimu17"), // protocol id, unique for each chain
		None,
		None,
	))
}

pub fn kauri_dev_config() -> Result<ChainSpec, String> {
	Ok(ChainSpec::from_genesis(
		"Kauri Dev",
		"kauri-dev",
		kauri_config_genesis,
		vec![],
		None,
		Some("kauri"), // protocol id, unique for each chain
		None,
		None,
	))
}
