
use cennznet_primitives::AccountId;
use cennznet_runtime::{
	ConsensusConfig, ContractConfig, CouncilSeatsConfig, CouncilVotingConfig, DemocracyConfig, FeeRate, FeesConfig,
	GenericAssetConfig, GrandpaConfig, IndicesConfig, Perbill, Permill, RewardsConfig, SessionConfig,
	SpotExchangeConfig, StakingConfig, SudoConfig, TimestampConfig, TreasuryConfig,
};
use primitives::Ed25519AuthorityId as AuthorityId;
use substrate_telemetry::TelemetryEndpoints;
use super::{TELEMETRY_URL, get_account_id_from_seed, get_authority_keys_from_seed, ChainSpec, GenesisConfig};

fn genesis(
	initial_authorities: Vec<(AccountId, AccountId, AuthorityId)>,
	root_key: AccountId,
	endowed_accounts: Option<Vec<AccountId>>,
) -> GenesisConfig {
	let endowed_accounts = endowed_accounts.unwrap_or_else(|| {
		vec![
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
		]
	});
	GenesisConfig {
		consensus: Some(ConsensusConfig {
			code: include_bytes!("../../runtime/wasm/target/wasm32-unknown-unknown/release/cennznet_runtime.compact.wasm")
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
			session_length: 20,
			keys: initial_authorities
				.iter()
				.map(|x| (x.1.clone(), x.2.clone()))
				.collect::<Vec<_>>(),
		}),
		staking: Some(StakingConfig {
			current_era: 0,
			minimum_validator_count: 2,
			validator_count: 10,
			sessions_per_era: 3,
			bonding_duration: 2 * 60 * 12,
			offline_slash: Perbill::from_billionths(1000),
			session_reward: Perbill::from_billionths(1000000),
			current_offline_slash: 0,
			current_session_reward: 0,
			offline_slash_grace: 1,
			stakers: initial_authorities
				.iter()
				.map(|x| (x.0.into(), x.1.into(), 1_000_000_000))
				.collect(),
			invulnerables: initial_authorities.iter().map(|x| x.1.into()).collect(),
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
				.filter(|&endowed| {
					initial_authorities
						.iter()
						.find(|&(_, controller, _)| controller == endowed)
						.is_none()
				})
				.map(|a| (a.clone().into(), 1000000))
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
			initial_balance: 10u128.pow(18 + 9), // 1 billion token with 18 decimals
			endowed_accounts: endowed_accounts.clone().into_iter().map(Into::into).collect(),
			// ids smaller than 1_000_000 are reserved
			next_asset_id: 1_000_000,
			create_asset_stake: 1000,
			transfer_fee: 20,
			next_reserved_asset_id: 100_000,
		}),
		fees: Some(FeesConfig {
			transaction_base_fee: 10,
			transaction_byte_fee: 1,
		}),
		cennz_x: Some(SpotExchangeConfig {
			fee_rate: FeeRate::from_milli(3),
			core_asset_id: 10,
		}),
		rewards: Some(RewardsConfig {
			block_reward: 10u128.pow(18),
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
			get_authority_keys_from_seed("Courtney"),
			get_authority_keys_from_seed("Drew"),
		],
		get_account_id_from_seed("Kauri").into(),
		None,
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
		None,
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
		None,
		None,
		None,
	))
}

pub fn rimu_latest_config() -> Result<ChainSpec, String> {
	Ok(ChainSpec::from_genesis(
		"Rimu CENNZnet 0.9.13",
		"rimu-9.13",
		rimu_config_genesis,
		vec![
				String::from("/dns4/cennznet-bootnode-0.centrality.cloud/tcp/30333/p2p/QmQZ8TjTqeDj3ciwr93EJ95hxfDsb9pEYDizUAbWpigtQN"),
				String::from("/dns4/cennznet-bootnode-1.centrality.cloud/tcp/30333/p2p/QmXiB3jqqn2rpiKU7k1h7NJYeBg8WNSx9DiTRKz9ti2KSK"),
				String::from("/dns4/cennznet-bootnode-2.centrality.cloud/tcp/30333/p2p/QmYcHeEWuqtr6Gb5EbK7zEhnaCm5p6vA2kWcVjFKbhApaC")
			],
		Some(TelemetryEndpoints::new(vec![(TELEMETRY_URL.into(), 0)])),
		None,
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
		None,
		None,
		None,
	))
}
