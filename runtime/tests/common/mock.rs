/* Copyright 2019-2021 Centrality Investments Limited
*
* Licensed under the LGPL, Version 3.0 (the "License");
* you may not use this file except in compliance with the License.
* Unless required by applicable law or agreed to in writing, software
* distributed under the License is distributed on an "AS IS" BASIS,
* WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
* See the License for the specific language governing permissions and
* limitations under the License.
* You may obtain a copy of the License at the root of this project source code,
* or at:
*     https://centrality.ai/licenses/gplv3.txt
*     https://centrality.ai/licenses/lgplv3.txt
*/

//! Mock runtime storage setup

use cennznet_cli::chain_spec::{session_keys, AuthorityKeys};
use cennznet_primitives::types::Balance;
use cennznet_runtime::{constants::asset::*, GenericAsset, Runtime, StakerStatus};
use core::convert::TryFrom;
use crml_cennzx::{FeeRate, PerMillion, PerThousand};
use crml_support::MultiCurrency;
use frame_support::traits::GenesisBuild;
use hex_literal::hex;
use sp_runtime::{AccountId32, FixedPointNumber, FixedU128, Perbill};

use crate::common::helpers::{make_authority_keys, GENESIS_HASH};
use crate::common::keyring::*;

/// The default number of validators for mock storage setup
const DEFAULT_VALIDATOR_COUNT: usize = 3;

pub struct ExtBuilder {
	initial_balance: Balance,
	// Configurable fields for staking module tests
	stash: Balance,
	// The initial authority set
	initial_authorities: Vec<AuthorityKeys>,
	/// Whether to make authorities invulnerable
	invulnerable: bool,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			initial_balance: 0,
			stash: 0,
			initial_authorities: Default::default(),
			invulnerable: true,
		}
	}
}

impl ExtBuilder {
	pub fn initial_balance(mut self, initial_balance: Balance) -> Self {
		self.initial_balance = initial_balance;
		self
	}
	// set invulnerables off (it's on by default)
	pub fn invulnerables_off(mut self) -> Self {
		self.invulnerable = false;
		self
	}
	pub fn initial_authorities(mut self, initial_authorities: &[AuthorityKeys]) -> Self {
		self.initial_authorities = initial_authorities.to_vec();
		self
	}
	pub fn stash(mut self, stash: Balance) -> Self {
		self.stash = stash;
		self
	}
	pub fn build(self) -> sp_io::TestExternalities {
		let eth_account = AccountId32::from(hex!("63766d3a00000000000000a86e122edbdcba4bf24a2abf89f5c230b37df49d4a"));
		let mut endowed_accounts = vec![alice(), bob(), charlie(), dave(), eve(), ferdie(), eth_account];
		let initial_authorities = if self.initial_authorities.is_empty() {
			make_authority_keys(DEFAULT_VALIDATOR_COUNT)
		} else {
			self.initial_authorities.clone()
		};
		let stash_accounts: Vec<_> = initial_authorities.iter().map(|x| x.0.clone()).collect();
		endowed_accounts.extend(stash_accounts.clone());

		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();
		crml_cennzx::GenesisConfig::<Runtime> {
			fee_rate: FeeRate::<PerMillion>::try_from(FeeRate::<PerThousand>::from(3u128)).unwrap(),
			core_asset_id: CPAY_ASSET_ID,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		crml_generic_asset::GenesisConfig::<Runtime> {
			assets: vec![CENNZ_ASSET_ID, CPAY_ASSET_ID],
			initial_balance: self.initial_balance,
			endowed_accounts: endowed_accounts,
			next_asset_id: NEXT_ASSET_ID,
			staking_asset_id: STAKING_ASSET_ID,
			spending_asset_id: SPENDING_ASSET_ID,
			permissions: vec![],
			asset_meta: vec![],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let min_validator_count = initial_authorities.len().min(3) as u32;

		let invulnerables = if self.invulnerable { stash_accounts } else { vec![] };

		crml_staking::GenesisConfig::<Runtime> {
			minimum_bond: 1,
			validator_count: initial_authorities.len() as u32,
			minimum_validator_count: min_validator_count,
			stakers: initial_authorities
				.clone()
				.iter()
				.map(|x| (x.0.clone(), x.1.clone(), self.stash, StakerStatus::Validator))
				.collect(),
			slash_reward_fraction: Perbill::from_percent(10),
			invulnerables,
			..Default::default()
		}
		.assimilate_storage(&mut t)
		.unwrap();

		pallet_session::GenesisConfig::<Runtime> {
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
		}
		.assimilate_storage(&mut t)
		.unwrap();

		crml_staking::rewards::GenesisConfig {
			development_fund_take: Perbill::from_percent(10),
			inflation_rate: FixedU128::saturating_from_rational(8u32, 10u32),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| {
			// Ensure our test genesis hash exists in storage.
			// This allows signed extrinsics to validate.
			frame_system::Pallet::<Runtime>::set_parent_hash(GENESIS_HASH.into());
		});

		ext
	}
}

#[test]
fn runtime_mock_setup_works() {
	let amount = 100;
	ExtBuilder::default()
		.initial_balance(amount)
		.stash(amount)
		.build()
		.execute_with(|| {
			let tests = vec![
				(alice(), amount),
				(bob(), amount),
				(charlie(), amount),
				(dave(), amount),
				(eve(), amount),
				(ferdie(), amount),
			];
			let assets = vec![CENNZ_ASSET_ID, CPAY_ASSET_ID];
			for asset in &assets {
				for (account, balance) in &tests {
					assert_eq!(
						<GenericAsset as MultiCurrency>::free_balance(&account, *asset),
						*balance,
					);
					assert_eq!(<GenericAsset as MultiCurrency>::free_balance(&account, 123), 0)
				}
				// NOTE: 9 = 6 pre-configured accounts + 3 ExtBuilder.validator_count (to generate stash accounts)
				assert_eq!(GenericAsset::total_issuance(asset), amount * 9);
			}
		});
}

pub mod contracts {
	//! Test contracts

	/// Contract WABT for reading 32 bytes from memory
	pub const CONTRACT_READ_32_BYTES: &str = r#"
	(module
		(import "env" "ext_scratch_read" (func $ext_scratch_read (param i32 i32 i32)))
		(import "env" "memory" (memory 1 1))
		(func (export "deploy"))
		(func (export "call")
			(call $ext_scratch_read
				(i32.const 0)
				(i32.const 0)
				(i32.const 4)
			)
		)

		;; 32 bytes for reading
		(data (i32.const 4)
			"\09\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00"
		)
	)"#;

	/// Contract WABT for a contract which will fail during execution
	pub const CONTRACT_WITH_TRAP: &str = r#"
	(module
		(import "env" "ext_scratch_read" (func $ext_scratch_read (param i32 i32 i32)))
		(import "env" "memory" (memory 1 1))
		(func (export "deploy"))
		(func (export "call")
			unreachable
		)
	)"#;

	/// Contract WABT for a contract which dispatches a generic asset transfer of CENNZ to charlie
	pub const CONTRACT_WITH_GA_TRANSFER: &str = r#"
	(module
		(import "env" "ext_dispatch_call" (func $ext_dispatch_call (param i32 i32)))
		(import "env" "memory" (memory 1 1))
		(func (export "call")
			(call $ext_dispatch_call
				(i32.const 8) ;; Pointer to the start of encoded call buffer
				(i32.const 40) ;; Length of the buffer
			)
		)
		(func (export "deploy"))
		(data (i32.const 8) "\05\01\01\FA\90\B5\AB\20\5C\69\74\C9\EA\84\1B\E6\88\86\46\33\DC\9C\A8\A3\57\84\3E\EA\CF\23\14\64\99\65\FE\22\82\3F\5C\02")
	)"#;
}
