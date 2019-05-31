// Copyright (C) 2019 Centrality Investments Limited
// This file is part of CENNZnet.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.
use super::{
	get_account_id_from_seed, get_authority_keys_from_seed, ChainSpec, GenesisConfig, NetworkKeys, TELEMETRY_URL,
};
use cennznet_runtime::{
	CennzxSpotConfig, ConsensusConfig, ContractConfig, CouncilSeatsConfig, CouncilVotingConfig, DemocracyConfig, Fee,
	FeeRate, FeesConfig, GenericAssetConfig, GrandpaConfig, IndicesConfig, Perbill, Permill, RewardsConfig, Schedule,
	SessionConfig, StakerStatus, StakingConfig, SudoConfig, TimestampConfig,
};
use hex_literal::{hex, hex_impl};
use primitives::crypto::UncheckedInto;
use substrate_telemetry::TelemetryEndpoints;

// should be 1_000_000_000_000_000_000 but remove some zeros to make number smaller to reduce the chance of overflow issue
const DOLLARS: u128 = 1_000_000_000_000;
const MICRO_DOLLARS: u128 = DOLLARS / 1_000_000;

const SECS_PER_BLOCK: u64 = 10;
const MINUTES: u64 = 60 / SECS_PER_BLOCK;
const HOURS: u64 = MINUTES * 60;
const DAYS: u64 = HOURS * 24;

fn genesis(keys: NetworkKeys) -> GenesisConfig {
	let endowed_accounts = keys.endowed_accounts;
	let initial_authorities = keys.initial_authorities;
	let root_key = keys.root_key;

	let transaction_base_fee = 1000 * MICRO_DOLLARS;
	let transaction_byte_fee = 5 * MICRO_DOLLARS;
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
			session_length: 2 * MINUTES,
			keys: initial_authorities
				.iter()
				.map(|x| (x.1.clone(), x.2.clone()))
				.collect::<Vec<_>>(),
		}),
		staking: Some(StakingConfig {
			current_era: 0,
			minimum_validator_count: 4,
			validator_count: 10,
			sessions_per_era: 5,
			bonding_duration: 2,
			offline_slash: Perbill::from_parts(1000000),
			session_reward: Perbill::from_parts(10),
			current_session_reward: 0,
			offline_slash_grace: 3,
			stakers: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.1.clone(), 100 * DOLLARS, StakerStatus::Validator))
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
		contract: Some(ContractConfig {
			signed_claim_handicap: 2,
			rent_byte_price: 4,
			rent_deposit_offset: 1000,
			storage_size_offset: 8,
			surcharge_reward: 150,
			tombstone_deposit: 16,
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
			initial_balance: 10u128.pow(6) * DOLLARS, // 1 million token
			endowed_accounts: endowed_accounts.clone().into_iter().map(Into::into).collect(),
			next_asset_id: 17000,
			create_asset_stake: 1000,
			staking_asset_id: 16000,
			spending_asset_id: 16001,
		}),
		fees: Some(FeesConfig {
			_genesis_phantom_data: Default::default(),
			fee_registry: vec![
				(Fee::fees(fees::Fee::Base), transaction_base_fee),
				(Fee::fees(fees::Fee::Bytes), transaction_byte_fee),
				(Fee::generic_asset(generic_asset::Fee::Transfer), transfer_fee),
			],
		}),
		cennzx_spot: Some(CennzxSpotConfig {
			fee_rate: FeeRate::from_milli(3),
			core_asset_id: 16001,
		}),
		rewards: Some(RewardsConfig {
			block_reward: 10 * MICRO_DOLLARS,
			fee_reward_multiplier: Permill::from_percent(70),
		}),
	}
}

pub fn kauri_config() -> Result<ChainSpec, String> {
	ChainSpec::from_embedded(include_bytes!("../../genesis/kauri/genesis.json"))
		.map_err(|e| format!("Error loading genesis for Kauri CENNZnet testnet {}", e))
}

fn kauri_config_genesis() -> GenesisConfig {
	genesis(kauri_keys())
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

fn kauri_keys() -> NetworkKeys {
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
	let initial_authorities = vec![
		get_authority_keys_from_seed("Andrea"),
		get_authority_keys_from_seed("Brooke"),
		get_authority_keys_from_seed("Courtney"),
		get_authority_keys_from_seed("Drew"),
	];
	let root_key = get_account_id_from_seed("Kauri");
	return NetworkKeys {
		endowed_accounts,
		initial_authorities,
		root_key,
	};
}

pub fn rimu_config() -> Result<ChainSpec, String> {
	ChainSpec::from_embedded(include_bytes!("../../genesis/rimu/genesis.json"))
		.map_err(|e| format!("Error loading genesis for Rimu CENNZnet testnet {}", e))
}

fn rimu_config_genesis() -> GenesisConfig {
	genesis(rimu_keys())
}

pub fn rimu_latest_config() -> Result<ChainSpec, String> {
	Ok(ChainSpec::from_genesis(
		"Rimu CENNZnet 0.9.20",
		"rimu-0.9.20",
		rimu_config_genesis,
		vec![
			String::from(
				"/dns4/cennznet-bootnode-0.rimu.cennznet.com/tcp/30333/p2p/QmNT8nuygWaiaeryw5J5ZLAKrsij3tXCvdwGRxMiEibzqk",
			),
			String::from(
				"/dns4/cennznet-bootnode-1.rimu.cennznet.com/tcp/30333/p2p/QmQzHJTTB4DdSp1VGWzLiQukLeTK3gGQKdCdbBawr7HEt1",
			),
			String::from(
				"/dns4/cennznet-bootnode-2.rimu.cennznet.com/tcp/30333/p2p/QmQwXguMXwvVxxHwyhfdkhRtu3i5hYSgxz7g3La5y1S9ux",
			),
			String::from(
				"/dns4/cennznet-bootnode-3.rimu.cennznet.com/tcp/30333/p2p/QmdaANAbaitFgVc5CmHhAk9DngoTqzLKpxyJeawJfkwoMW",
			),
		],
		Some(TelemetryEndpoints::new(vec![(TELEMETRY_URL.into(), 0)])),
		Some("rimu20"), // protocol id, unique for each chain
		None,
		None,
	))
}

fn rimu_keys() -> NetworkKeys {
	let endowed_accounts = vec![
		hex!["3aebf8155bd297575b4ce00c1e620d5e1701600bda1eb70f72b547ee18c6682e"].unchecked_into(),
		hex!["c4624fe230a2183cb948224ca74aa9108903c305b6fc90a193765ddbf963b328"].unchecked_into(),
		hex!["6ad2b857ee8567bb7deca480ec93f81f854232a143140ba2f2398cf1d0d63d70"].unchecked_into(),
		hex!["1ad144301298528620c6f3c1b0543da4da03b21f06c77d1ce59bdf323f0ad750"].unchecked_into(),
		hex!["8af545920eba09438064452bd5c751217ff330de18b5788020151f96b3acd365"].unchecked_into(),
		hex!["ec4bab950ecb796669b074682600c19b4d1a4ff9ed101e0b5742bcce69f42b7f"].unchecked_into(),
		hex!["3040e1512f5672c364a459bb78dbda76fd9f3a88adc31159e56277055d478202"].unchecked_into(),
		hex!["6e471a4dce84a1c726f5a76dfb12726dde0e9816cc6a7c5ffa60d034b12b777a"].unchecked_into(),
		hex!["4a5a85ec5d121ed5fc4fa0111c060fe5184049d28f34a1856451461d9ddae341"].unchecked_into(),
		hex!["feca098c25921f5294b656dee9e05d44dba944361834ed2f17ca422696302801"].unchecked_into(),
		hex!["2c42bfb9412b21ee3dd3738b165824a7cb021d885152797d04969c3e5e9f0725"].unchecked_into(),
		hex!["442f2e719cd86309778a9eda69cb8caab48229501a19bf7647503fb074015e5f"].unchecked_into(),
		hex!["46d64e97e22ad2b41746ede75b896b73b31cbf22628c36c285571920a3893c02"].unchecked_into(),
		hex!["4438ecb26e6cf143f48449ad220248ccd5f98ab39fc279b9dabcd1c8d9067932"].unchecked_into(),
		hex!["bab8c0b3a663a84cf32aa9c12a5a2b7c8567daaf40a8765ce0abd573c0ad9e21"].unchecked_into(),
	];
	let initial_authorities = vec![
		(
			hex!["3aebf8155bd297575b4ce00c1e620d5e1701600bda1eb70f72b547ee18c6682e"].unchecked_into(),
			hex!["c4624fe230a2183cb948224ca74aa9108903c305b6fc90a193765ddbf963b328"].unchecked_into(),
			hex!["49fda9ab118eeaa60e49625c185c0981c3c8f73fc48ac822415a8faf0357448c"].unchecked_into(),
		),
		(
			hex!["6ad2b857ee8567bb7deca480ec93f81f854232a143140ba2f2398cf1d0d63d70"].unchecked_into(),
			hex!["1ad144301298528620c6f3c1b0543da4da03b21f06c77d1ce59bdf323f0ad750"].unchecked_into(),
			hex!["1ade0fc31f7e3a58cc74f02aa4cec1c0759738f4e5b8bd91d7b402cdfe2c1741"].unchecked_into(),
		),
		(
			hex!["8af545920eba09438064452bd5c751217ff330de18b5788020151f96b3acd365"].unchecked_into(),
			hex!["ec4bab950ecb796669b074682600c19b4d1a4ff9ed101e0b5742bcce69f42b7f"].unchecked_into(),
			hex!["497804545c82571ae18a8bb1899b611f630849ea4118ea237f7064e006404cf9"].unchecked_into(),
		),
		(
			hex!["3040e1512f5672c364a459bb78dbda76fd9f3a88adc31159e56277055d478202"].unchecked_into(),
			hex!["6e471a4dce84a1c726f5a76dfb12726dde0e9816cc6a7c5ffa60d034b12b777a"].unchecked_into(),
			hex!["456437d02aee2b2c848c9efa6af598310d5806580054999fc785c8481c09fa7f"].unchecked_into(),
		),
		(
			hex!["4a5a85ec5d121ed5fc4fa0111c060fe5184049d28f34a1856451461d9ddae341"].unchecked_into(),
			hex!["feca098c25921f5294b656dee9e05d44dba944361834ed2f17ca422696302801"].unchecked_into(),
			hex!["8882490f8cf9b7d1fb8a6c61983112f5ffbd399430fb04039ae626fa991ed9cb"].unchecked_into(),
		),
		(
			hex!["2c42bfb9412b21ee3dd3738b165824a7cb021d885152797d04969c3e5e9f0725"].unchecked_into(),
			hex!["442f2e719cd86309778a9eda69cb8caab48229501a19bf7647503fb074015e5f"].unchecked_into(),
			hex!["6527d5a58dd6b7c4bcc0205be47ff235350b68b455900f96eff410d07bdcd732"].unchecked_into(),
		),
		(
			hex!["46d64e97e22ad2b41746ede75b896b73b31cbf22628c36c285571920a3893c02"].unchecked_into(),
			hex!["4438ecb26e6cf143f48449ad220248ccd5f98ab39fc279b9dabcd1c8d9067932"].unchecked_into(),
			hex!["884f147f6ccadf860e7272d24d5f4b22b4e2e33f25ca3976363199ba97a5124d"].unchecked_into(),
		),
	];
	let root_key = hex!["bab8c0b3a663a84cf32aa9c12a5a2b7c8567daaf40a8765ce0abd573c0ad9e21"].unchecked_into();
	return NetworkKeys {
		endowed_accounts,
		initial_authorities,
		root_key,
	};
}
