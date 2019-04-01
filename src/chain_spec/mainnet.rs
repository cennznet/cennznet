use super::{get_account_id_from_address, get_account_keys_from_address, ChainSpec, GenesisConfig, TELEMETRY_URL};
use cennznet_runtime::{
	ConsensusConfig, ContractConfig, CouncilSeatsConfig, CouncilVotingConfig, DemocracyConfig, FeeRate, FeesConfig,
	GenericAssetConfig, GrandpaConfig, IndicesConfig, Perbill, Permill, RewardsConfig, Schedule, SessionConfig,
	SpotExchangeConfig, StakingConfig, SudoConfig, TimestampConfig, TreasuryConfig,
};
use substrate_telemetry::TelemetryEndpoints;

const DOLLARS: u128 = 1_000_000_000_000_000_000;
const MICRO_DOLLARS: u128 = DOLLARS / 1_000_000;

const SECS_PER_BLOCK: u64 = 6;
const MINUTES: u64 = 60 / SECS_PER_BLOCK;
const HOURS: u64 = MINUTES * 60;
const DAYS: u64 = HOURS * 24;

fn genesis() -> GenesisConfig {
	let initial_authorities = vec![
		// TODO: change to real address
		get_account_keys_from_address(
			"5G39vCzSK17vWyD3xMN2NgeefMngiLBdMMiGGBgEjiz5jGCi",
			"5G39vCzSK17vWyD3xMN2NgeefMngiLBdMMiGGBgEjiz5jGCi",
		),
	];
	// TODO: change to real address
	let root_key = get_account_id_from_address("5G39vCzSK17vWyD3xMN2NgeefMngiLBdMMiGGBgEjiz5jGCi");
	let endowed_accounts = vec![
		// pre seeded accounts
		get_account_id_from_address("5FkXqvea1mmAUGNJ9nJyqp2xJjsU4pmACxP35txnHAVtXKGU"),
		get_account_id_from_address("5D2WWEwn8oUMbSiwuHBUnsyDLytwSrpahta9jvJamjYgAfcf"),
		get_account_id_from_address("5HBmFpcdL3WjUNTUtyAKWDJE96YnA4D3BokkvRRNBBZbPMWE"),
		get_account_id_from_address("5HEdJWWiggQKUSnstM7uRYFkYuAoAesdGh2NMLcFeSnm4zQR"),
		get_account_id_from_address("5EvzCqpvGgayVF8W3iBddUqwmXMQqQmP8ktJhFKEEzA6xfWg"),
	];
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
			validators: initial_authorities.iter().map(|x| x.1.into()).collect(),
			session_length: 1 * HOURS,
			keys: initial_authorities
				.iter()
				.map(|x| (x.1.clone(), x.2.clone()))
				.collect::<Vec<_>>(),
		}),
		staking: Some(StakingConfig {
			current_era: 0,
			minimum_validator_count: 4,
			validator_count: 10,
			sessions_per_era: 6, // 6hr
			bonding_duration: 6 * HOURS,
			offline_slash: Perbill::from_billionths(1000000),
			session_reward: Perbill::from_billionths(1000),
			current_offline_slash: 0,
			current_session_reward: 0,
			offline_slash_grace: 3,
			stakers: initial_authorities
				.iter()
				.map(|x| (x.0.into(), x.1.into(), 1_000_000_000))
				.collect(),
			invulnerables: initial_authorities.iter().map(|x| x.1.into()).collect(),
		}),
		democracy: Some(DemocracyConfig {
			launch_period: 1 * DAYS,
			voting_period: 3 * DAYS,
			minimum_deposit: 1000 * DOLLARS,
			public_delay: 2 * DAYS,
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
			presentation_duration: 1 * DAYS,
			approval_voting_period: 2 * DAYS,
			term_duration: 14 * DAYS,
			desired_seats: 11,
			inactive_grace_period: 1,
		}),
		council_voting: Some(CouncilVotingConfig {
			cooloff_period: 4 * DAYS,
			voting_period: 1 * DAYS,
			enact_delay_period: 0,
		}),
		timestamp: Some(TimestampConfig {
			period: SECS_PER_BLOCK / 2, // due to the nature of aura the slots are 2*period
		}),
		treasury: Some(TreasuryConfig {
			proposal_bond: Permill::from_percent(5),
			proposal_bond_minimum: 1 * DOLLARS,
			spend_period: 1 * DAYS,
			burn: Permill::from_percent(50),
		}),
		contract: Some(ContractConfig {
			contract_fee: 500 * MICRO_DOLLARS,
			call_base_fee: 500,
			create_base_fee: 800,
			gas_price: 1 * MICRO_DOLLARS,
			max_depth: 1024,
			block_gas_limit: 10_000_000,
			current_schedule: Schedule {
				version: 0,
				put_code_per_byte_cost: 50,
				grow_mem_cost: 2,
				regular_op_cost: 1,
				return_data_per_byte_cost: 2,
				sandbox_data_read_cost: 1,
				sandbox_data_write_cost: 2,
				log_event_per_byte_cost: 5,
				max_stack_height: 64 * 1024,
				max_memory_pages: 16,
			},
		}),
		sudo: Some(SudoConfig { key: root_key }),
		grandpa: Some(GrandpaConfig {
			authorities: initial_authorities.iter().map(|x| (x.2.clone(), 1)).collect(),
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
			initial_balance: 250_000 * DOLLARS,
			endowed_accounts: endowed_accounts.clone().into_iter().map(Into::into).collect(),
			// ids smaller than 1_000_000 are reserved
			next_asset_id: 1_000_000,
			create_asset_stake: 1000,
			transfer_fee: 480 * MICRO_DOLLARS,
			next_reserved_asset_id: 100_000,
		}),
		fees: Some(FeesConfig {
			transaction_base_fee: 1000 * MICRO_DOLLARS,
			transaction_byte_fee: 5 * MICRO_DOLLARS,
		}),
		cennzx_spot: Some(SpotExchangeConfig {
			fee_rate: FeeRate::from_milli(3),
			core_asset_id: 10,
		}),
		rewards: Some(RewardsConfig {
			block_reward: 10 * DOLLARS,
		}),
	}
}

pub fn config() -> Result<ChainSpec, String> {
	ChainSpec::from_embedded(include_bytes!("../../genesis/main/genesis.json"))
		.map_err(|e| format!("Error loading genesis for CENNZnet {}", e))
}

pub fn latest_config() -> Result<ChainSpec, String> {
	// TODO: update this
	Ok(ChainSpec::from_genesis(
		"CENNZnet",
		"cennznet",
		genesis,
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
		None,
		None,
		None,
	))
}
