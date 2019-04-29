use super::{ChainSpec, GenesisConfig, TELEMETRY_URL};
use cennznet_primitives::AccountId;
use cennznet_runtime::{
	CennzxSpotConfig, ConsensusConfig, ContractConfig, CouncilSeatsConfig, CouncilVotingConfig, DemocracyConfig,
	FeeRate, FeesConfig, GenericAssetConfig, GrandpaConfig, IndicesConfig, Perbill, Permill, RewardsConfig, Schedule,
	SessionConfig, StakerStatus, StakingConfig, SudoConfig, TimestampConfig, TreasuryConfig,
};
use hex_literal::{hex, hex_impl};
use primitives::{crypto::UncheckedInto, ed25519::Public as AuthorityId};
use substrate_telemetry::TelemetryEndpoints;

const DOLLARS: u128 = 1_000_000_000_000_000_000;
const MICRO_DOLLARS: u128 = DOLLARS / 1_000_000;

const SECS_PER_BLOCK: u64 = 6;
const MINUTES: u64 = 60 / SECS_PER_BLOCK;
const HOURS: u64 = MINUTES * 60;
const DAYS: u64 = HOURS * 24;

fn genesis() -> GenesisConfig {
	// TODO: change to real addresses
	let initial_authorities: Vec<(AccountId, AccountId, AuthorityId)> = vec![
		(
			hex!["72b52eb36f57b4bae756e4f064cf2e97df80d5f9c2f06ff31206a9be8c7b371c"].unchecked_into(),
			hex!["f0fae46aeb1a7ce8ca65f2bf885d09cd7f525bc00e9f6e73b5ea74402a2c4c19"].unchecked_into(),
			hex!["e29624233b2cba342750217aa1883f6ec624134dd306efd230a988e5cb37d9ed"].unchecked_into(),
		),
		(
			hex!["2254035a15597c1c19968be71593d2d0131e18ae90049e49178970f583ac3e17"].unchecked_into(),
			hex!["eacb8edf6b05cb909a3d2bd8c6bffb13be3069ec6a69f1fa25e46103c5190267"].unchecked_into(),
			hex!["e19b6b89729a41638e57dead9c993425287d386fa4963306b63f018732843495"].unchecked_into(),
		),
		(
			hex!["fe6211db8bd436e0d1cf37398eac655833fb47497e0f72ec00ab160c88966b7e"].unchecked_into(),
			hex!["f06dd616c75cc4b2b01f325accf79b4f66a525ede0a59f48dcce2322b8798f5c"].unchecked_into(),
			hex!["1be80f2d4513a1fbe0e5163874f729baa5498486ac3914ac3fe2e1817d7b3f44"].unchecked_into(),
		),
		(
			hex!["60779817899466dbd476a0bc3a38cc64b7774d5fb646c3d291684171e67a0743"].unchecked_into(),
			hex!["2a32622a5da54a80dc704a05f2d761c96d4748beedd83f61ca20a90f4a257678"].unchecked_into(),
			hex!["f54d9f5ed217ce07c0c5faa5277a0356f8bfd884d201f9d2c9e171568e1bf077"].unchecked_into(),
		),
	];
	let root_key = hex!["f54d9f5ed217ce07c0c5faa5277a0356f8bfd884d201f9d2c9e171568e1bf077"].unchecked_into();
	let endowed_accounts: Vec<AccountId> =
		vec![hex!["c224ccba63292331623bbf06a55f46607824c2580071a80a17c53cab2f999e2f"].unchecked_into()];
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
			current_session_reward: 0,
			offline_slash_grace: 3,
			stakers: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.1.clone(), 1_000_000_000, StakerStatus::Validator))
				.collect(),
			invulnerables: initial_authorities.iter().map(|x| x.1.clone()).collect(),
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
			minimum_period: SECS_PER_BLOCK / 2, // due to the nature of aura the slots are 2*period
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
				event_data_per_byte_cost: 5,
				event_data_base_cost: 20,
				sandbox_data_read_cost: 1,
				sandbox_data_write_cost: 2,
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
			initial_balance: 250_000 * DOLLARS,
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
			fee_reward_multiplier: Perbill::one(),
			average_cost_per_transaction: 3u128.pow(15),
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
