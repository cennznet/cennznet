// Copyright 2019 Centrality Investments Limited
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
use super::{get_account_id_from_seed, get_authority_keys_from_seed, ChainSpec, GenesisConfig};
use cennznet_primitives::AccountId;
use cennznet_runtime::{
	CennzxSpotConfig, ConsensusConfig, ContractConfig, CouncilSeatsConfig, CouncilVotingConfig, DemocracyConfig, Fee,
	FeeRate, FeesConfig, GenericAssetConfig, GrandpaConfig, IndicesConfig, Perbill, Permill, RewardsConfig, Schedule,
	SessionConfig, StakerStatus, StakingConfig, SudoConfig, TimestampConfig,
};
use primitives::ed25519::Public as AuthorityId;

pub fn genesis(initial_authorities: Vec<(AccountId, AccountId, AuthorityId)>, root_key: AccountId) -> GenesisConfig {
	let endowed_accounts = vec![
		get_account_id_from_seed("Alice"),
		get_account_id_from_seed("Bob"),
		get_account_id_from_seed("Charlie"),
		get_account_id_from_seed("Dave"),
		get_account_id_from_seed("Eve"),
		get_account_id_from_seed("Ferdie"),
		get_account_id_from_seed("Alice//stash"),
		get_account_id_from_seed("Bob//stash"),
		get_account_id_from_seed("Charlie//stash"),
		get_account_id_from_seed("Dave//stash"),
		get_account_id_from_seed("Eve//stash"),
		get_account_id_from_seed("Ferdie//stash"),
	];
	let transaction_base_fee = 1;
	let transaction_byte_fee = 1;
	let transfer_fee = 20;
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
			session_length: 10,
			keys: initial_authorities
				.iter()
				.map(|x| (x.1.clone(), x.2.clone()))
				.collect::<Vec<_>>(),
		}),
		staking: Some(StakingConfig {
			current_era: 0,
			minimum_validator_count: 1,
			validator_count: 4,
			sessions_per_era: 5,
			bonding_duration: 12,
			offline_slash: Perbill::from_parts(1000),
			session_reward: Perbill::from_parts(10000),
			current_session_reward: 0,
			offline_slash_grace: 0,
			stakers: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.1.clone(), 1_000_000_000, StakerStatus::Validator))
				.collect(),
			invulnerables: initial_authorities.iter().map(|x| x.1.clone()).collect(),
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
			minimum_period: 2, // block_time = period * 2
		}),
		contract: Some(ContractConfig {
			signed_claim_handicap: 2,
			rent_byte_price: 1,
			rent_deposit_offset: 1000,
			storage_size_offset: 8,
			surcharge_reward: 150,
			tombstone_deposit: 16,
			contract_fee: 21,
			call_base_fee: 135,
			create_base_fee: 175,
			creation_fee: 0,
			transaction_base_fee,
			transaction_byte_fee,
			transfer_fee,
			gas_price: 1,
			max_depth: 1024,
			block_gas_limit: 10_000_000_000,
			current_schedule: Schedule {
				enable_println: true,
				..Default::default()
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
			block_reward: 1000,
			fee_reward_multiplier: Permill::from_percent(100),
		}),
	}
}

fn config_genesis() -> GenesisConfig {
	genesis(
		vec![get_authority_keys_from_seed("Alice")],
		get_account_id_from_seed("Alice").into(),
	)
}

/// Local testnet config
pub fn config() -> Result<ChainSpec, String> {
	Ok(ChainSpec::from_genesis(
		"Development",
		"dev",
		config_genesis,
		vec![],
		None,
		Some("dev"), // protocol id, unique for each chain
		None,
		None,
	))
}
