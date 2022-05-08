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

//! Staking reward tests

use cennznet_cli::chain_spec::AuthorityKeys;
use cennznet_primitives::types::{AccountId, Balance, BlockNumber, DigestItem, Header};
use cennznet_runtime::{
	constants::{asset::*, currency::*, time::MILLISECS_PER_BLOCK},
	Babe, CENNZnetGasWeightMapping, Call, CheckedExtrinsic, DefaultBaseFeePerGas, EpochDuration, Executive,
	MaxNominatorRewardedPerValidator, Rewards, Runtime, Session, SessionsPerEra, SignedExtra, SlashDeferDuration,
	Staking, System, Timestamp, Treasury,
};
use codec::{Decode, Encode};
use crml_staking::{ActiveEra, ActiveEraInfo, EraIndex, HandlePayee, RewardCalculation, StakingLedger};
use crml_support::{PrefixedAddressMapping, H160, U256};
use frame_support::{
	storage::StorageValue,
	traits::{Currency, Get, OffchainWorker, OnFinalize, OnInitialize},
	IterableStorageMap,
};
use hex_literal::hex;
use pallet_ethereum::{Transaction, TransactionAction};
use pallet_evm::{AddressMapping, GasWeightMapping};
use pallet_im_online::UnresponsivenessOffence;
use rustc_hex::FromHex;
use sp_consensus_babe::{digests, AuthorityIndex, Slot, BABE_ENGINE_ID};
use sp_core::{crypto::UncheckedFrom, H256};
use sp_runtime::{
	traits::{Applyable, Header as HeaderT, Saturating, Zero},
	Perbill,
};
use sp_staking::{
	offence::{DisableStrategy, Offence, OffenceDetails, OnOffenceHandler},
	SessionIndex,
};

mod common;

use common::helpers::{extrinsic_fee_for, header_for_block_number, make_authority_keys, sign};
use common::keyring::{alice, bob, charlie, signed_extra};
use common::mock::ExtBuilder;

/// Alias for the runtime configured staking reward currency
type RewardCurrency = <Runtime as crml_staking::rewards::Config>::CurrencyToReward;
/// Alias for the runtime configured staking currency
type StakeCurrency = <Runtime as crml_staking::Config>::Currency;

pub const INIT_TIMESTAMP: u64 = 30_000;

pub const ERC20_CONTRACT_BYTECODE: &str = include_str!("./res/erc20_contract_bytecode.txt");

// An Ethereum Transaction
pub struct EIP1559UnsignedTransaction {
	pub nonce: U256,
	pub max_priority_fee_per_gas: U256,
	pub max_fee_per_gas: U256,
	pub gas_limit: U256,
	pub action: TransactionAction,
	pub value: U256,
	pub input: Vec<u8>,
}

impl EIP1559UnsignedTransaction {
	pub fn sign(&self, secret: &H256, chain_id: Option<u64>) -> Transaction {
		let secret = {
			let mut sk: [u8; 32] = [0u8; 32];
			sk.copy_from_slice(&secret[0..]);
			libsecp256k1::SecretKey::parse(&sk).unwrap()
		};
		let chain_id = chain_id.unwrap_or(<Runtime as pallet_evm::Config>::ChainId::get());
		let msg = ethereum::EIP1559TransactionMessage {
			chain_id,
			nonce: self.nonce,
			max_priority_fee_per_gas: self.max_priority_fee_per_gas,
			max_fee_per_gas: self.max_fee_per_gas,
			gas_limit: self.gas_limit,
			action: self.action,
			value: self.value,
			input: self.input.clone(),
			access_list: vec![],
		};
		let signing_message = libsecp256k1::Message::parse_slice(&msg.hash()[..]).unwrap();

		let (signature, recid) = libsecp256k1::sign(&signing_message, &secret);
		let rs = signature.serialize();
		let r = H256::from_slice(&rs[0..32]);
		let s = H256::from_slice(&rs[32..64]);
		Transaction::EIP1559(ethereum::EIP1559Transaction {
			chain_id: msg.chain_id,
			nonce: msg.nonce,
			max_priority_fee_per_gas: msg.max_priority_fee_per_gas,
			max_fee_per_gas: msg.max_fee_per_gas,
			gas_limit: msg.gas_limit,
			action: msg.action,
			value: msg.value,
			input: msg.input.clone(),
			access_list: msg.access_list,
			odd_y_parity: recid.serialize() != 0,
			r,
			s,
		})
	}
}

/// Progress to the given block, triggering session and era changes as we progress.
///
/// This will finalize the previous block, initialize up to the given block, essentially simulating
/// a block import/propose process where we first initialize the block, then execute some stuff (not
/// in the function), and then finalize the block.
pub(crate) fn run_to_block(n: BlockNumber) {
	println!("call run to block: {:?}", n);
	Staking::on_finalize(System::block_number());
	for b in (System::block_number() + 1)..=n {
		println!(
			"start block: {:?}, era: {:?}, session: {:?}",
			b,
			active_era(),
			Session::current_index()
		);
		System::set_block_number(b);
		// The real config creates 120 blocks a session
		// The integration test config creates 12 blocks a session for speed
		// advancing the timestamp in-step with the real config (`* 10`) ensures the inflation reward calculations are accurate
		Timestamp::set_timestamp(b as u64 * MILLISECS_PER_BLOCK * 10 + INIT_TIMESTAMP);
		Session::on_initialize(b);
		Staking::on_initialize(b);
		Staking::offchain_worker(b);
		Rewards::on_initialize(b);
		<pallet_babe::CurrentSlot<Runtime>>::put(Slot::from(b as u64));
		if b != n {
			Staking::on_finalize(System::block_number());
		}
	}
}

/// Convenient getter for current era aka (scheduled active after session delay)
pub(crate) fn current_era() -> EraIndex {
	Staking::current_era().expect("current era is set")
}

/// Convenient getter for active era
pub(crate) fn active_era() -> EraIndex {
	Staking::active_era().expect("active era is set").index
}

/// Progresses from the current block number (whatever that may be) to the `epoch duration * session_index + 1`.
pub(crate) fn start_session(session_index: SessionIndex) {
	run_to_block(session_index * EpochDuration::get() as u32 + 1);
	// session must have progressed properly.
	assert_eq!(
		Session::current_index(),
		session_index,
		"current session index = {}, expected = {}",
		Session::current_index(),
		session_index,
	);
}

/// start the next session
pub(crate) fn advance_session() {
	start_session(Session::current_index() + 1)
}

/// Progress until the given era.
pub(crate) fn start_active_era(era_index: EraIndex) {
	start_session((era_index * <SessionsPerEra as Get<u32>>::get()).into());
	assert_eq!(active_era(), era_index);
	// One way or another, current_era must have changed before the active era
	assert_eq!(current_era(), active_era());
}

/// Get a block header and set the author of that block in a way that is recognisable by BABE.
/// The author will be specified by its index in the Session::validators() list. So the author
/// should be a current validator. Return the modified header.
fn set_author(mut header: Header, author_index: AuthorityIndex) -> Header {
	use digests::{PreDigest, SecondaryPlainPreDigest};

	let digest_data = PreDigest::SecondaryPlain(SecondaryPlainPreDigest {
		authority_index: author_index,
		slot: Babe::current_slot(),
	});

	let digest = header.digest_mut();
	digest
		.logs
		.push(DigestItem::PreRuntime(BABE_ENGINE_ID, digest_data.encode()));

	header
}

#[test]
fn start_active_era_works() {
	ExtBuilder::default()
		.initial_balance(1 * DOLLARS)
		.build()
		.execute_with(|| {
			let blocks_per_era = SessionsPerEra::get() * EpochDuration::get() as u32;
			start_active_era(1);
			assert_eq!(System::block_number(), blocks_per_era + 1);
			assert_eq!(Session::current_index(), SessionsPerEra::get());
			assert_eq!(Staking::active_era().unwrap().index, 1);

			// one session extra, should schedule the next era (poorly named 'current era')
			advance_session();
			assert_eq!(Staking::current_era().unwrap(), 2);

			start_active_era(2);
			assert_eq!(System::block_number(), blocks_per_era * 2 + 1);
			assert_eq!(Session::current_index(), SessionsPerEra::get() * 2);
			assert_eq!(Staking::active_era().unwrap().index, 2);
			assert_eq!(Staking::current_era().unwrap(), 2);

			advance_session();
			assert_eq!(Staking::current_era().unwrap(), 3);
		})
}

#[test]
fn staking_genesis_config_works() {
	let validators = make_authority_keys(6);
	let balance_amount = 1 * DOLLARS;
	let staked_amount = balance_amount / validators.len() as Balance;
	ExtBuilder::default()
		.initial_authorities(validators.as_slice())
		.initial_balance(balance_amount)
		.stash(staked_amount)
		.build()
		.execute_with(|| {
			for (stash, controller, _, _, _, _, _) in validators {
				// Check validator is included in current elected accounts
				assert!(Session::validators().contains(&stash));
				// Check that RewardDestination is Stash (default)
				assert_eq!(Rewards::payee(&stash), stash);
				// Check validator free balance
				assert_eq!(RewardCurrency::free_balance(&stash), balance_amount);
				// Check how much is at stake
				assert_eq!(
					Staking::ledger(controller),
					Some(StakingLedger {
						stash,
						total: staked_amount,
						active: staked_amount,
						unlocking: vec![],
					})
				);
			}
		});
}

#[test]
fn era_transaction_fees_collected() {
	// Check era transaction fees are tracked
	let initial_balance = 10_000 * DOLLARS;
	let validators = make_authority_keys(6);
	let staked_amount = initial_balance / validators.len() as Balance;

	let runtime_call_1 = Call::GenericAsset(crml_generic_asset::Call::transfer {
		asset_id: CPAY_ASSET_ID,
		to: bob(),
		amount: 123,
	});
	let runtime_call_2 = Call::GenericAsset(crml_generic_asset::Call::transfer {
		asset_id: CPAY_ASSET_ID,
		to: charlie(),
		amount: 456,
	});

	ExtBuilder::default()
		.initial_authorities(validators.as_slice())
		.initial_balance(initial_balance)
		.stash(staked_amount)
		.build()
		.execute_with(|| {
			let era_duration_ms = Rewards::FULL_ERA_DURATION;
			let xt_1 = sign(CheckedExtrinsic {
				signed: fp_self_contained::CheckedSignature::Signed(alice(), signed_extra(0, 0, None)),
				function: runtime_call_1.clone(),
			});
			let xt_2 = sign(CheckedExtrinsic {
				signed: fp_self_contained::CheckedSignature::Signed(bob(), signed_extra(0, 0, None)),
				function: runtime_call_2.clone(),
			});

			// Start with 0 transaction rewards
			assert!(Rewards::calculate_total_reward(era_duration_ms)
				.transaction_fees
				.is_zero());

			// Apply first extrinsic and check transaction rewards
			let r = Executive::apply_extrinsic(xt_1.clone());
			assert!(r.is_ok());
			let mut era1_tx_fees = extrinsic_fee_for(&xt_1);
			assert_eq!(Rewards::calculate_total_reward(era_duration_ms).transaction_fees, 0);

			// Apply second extrinsic and check transaction rewards
			let r2 = Executive::apply_extrinsic(xt_2.clone());
			assert!(r2.is_ok());
			era1_tx_fees += extrinsic_fee_for(&xt_2);
			assert_eq!(Rewards::calculate_total_reward(era_duration_ms).transaction_fees, 0);

			// Advancing sessions shouldn't change transaction rewards storage
			advance_session();
			advance_session();
			start_active_era(1);

			// At the start of the next era, transaction rewards should be cleared
			assert!(Rewards::calculate_total_reward(era_duration_ms)
				.transaction_fees
				.is_zero());
		});
}

#[test]
fn elected_validators_do_not_receive_transaction_fees() {
	// Check block transaction fees are distributed to validators along with inflation
	let validators = make_authority_keys(6);
	let initial_balance = 100_000_000 * DOLLARS;
	let staked_amount = initial_balance / validators.len() as Balance;
	let transfer_amount = 50;
	let runtime_call = Call::GenericAsset(crml_generic_asset::Call::transfer {
		asset_id: CPAY_ASSET_ID,
		to: bob(),
		amount: transfer_amount,
	});

	ExtBuilder::default()
		.initial_authorities(validators.as_slice())
		.initial_balance(initial_balance)
		.stash(staked_amount)
		.build()
		.execute_with(|| {
			// start from era 1
			start_active_era(1);
			let era1_start_ms = Timestamp::now();
			// create a transaction
			let xt = sign(CheckedExtrinsic {
				signed: fp_self_contained::CheckedSignature::Signed(alice(), signed_extra(0, 0, None)),
				function: runtime_call,
			});
			let tx_fee = extrinsic_fee_for(&xt);

			let make_block_with_author = |author_index: u32| {
				let header_of_last_block = header_for_block_number((System::block_number() + 1).into());
				let header = set_author(header_of_last_block, author_index);
				Executive::initialize_block(&header);
				// add tx to block
				let r = Executive::apply_extrinsic(xt);
				assert!(r.is_ok());
			};

			// reward currency = tx fee currency, will be burned for tx fees
			let initial_issuance = RewardCurrency::total_issuance();
			make_block_with_author(0);
			// NOTE: ignore block authoring points in this test so the payout will be equal
			// block author distribution is checked in other tests
			crml_staking::rewards::CurrentEraRewardPoints::<Runtime>::kill();

			let issuance_after_fees_burned = RewardCurrency::total_issuance();
			assert_eq!(issuance_after_fees_burned, initial_issuance - tx_fee);

			// treasury has nothing at this point
			assert!(RewardCurrency::free_balance(&Treasury::account_id()).is_zero());

			// end era 1, reward payouts are scheduled
			start_active_era(2);
			// tx fees are tracked by the Rewards module
			let era_duration_ms = Timestamp::now() - era1_start_ms;
			let reward_parts = Rewards::calculate_total_reward(era_duration_ms);
			assert_eq!(
				Perbill::from_rational(era_duration_ms, Rewards::FULL_ERA_DURATION)
					* Rewards::target_inflation_per_staking_era(),
				reward_parts.inflation
			);
			assert_eq!(0, reward_parts.transaction_fees);
			assert_eq!(reward_parts.inflation, reward_parts.total);
			// treasury is paid its cut of network tx fees
			assert_eq!(0, reward_parts.treasury_cut);

			// trigger payouts
			run_to_block(System::block_number() + 20);
			let per_validator_reward_era_1 = reward_parts.stakers_cut / validators.len() as Balance;
			for (stash, _, _, _, _, _, _) in &validators {
				assert_eq!(
					RewardCurrency::free_balance(stash),
					initial_balance + per_validator_reward_era_1,
				)
			}

			assert_eq!(
				RewardCurrency::free_balance(&Treasury::account_id()),
				reward_parts.total - (per_validator_reward_era_1 * validators.len() as Balance)
			);
		});
}

#[test]
fn elected_validators_receive_rewards_according_to_authorship_points() {
	// Start a new era to payout last eras validators
	// Check payouts happen as expected and total issuance is maintained
	let validators = make_authority_keys(6);
	let initial_balance = 100_000_000 * DOLLARS;
	let staked_amount = initial_balance / validators.len() as Balance;
	let era_duration_ms = Rewards::FULL_ERA_DURATION;

	ExtBuilder::default()
		.initial_authorities(validators.as_slice())
		.initial_balance(initial_balance)
		.stash(staked_amount)
		.build()
		.execute_with(|| {
			let initial_reward_issuance = RewardCurrency::total_issuance();
			// start era 1, era 0 has no reward
			assert!(Rewards::calculate_total_reward(era_duration_ms).total.is_zero());
			start_active_era(1);
			let era1_start_ms = Timestamp::now();
			assert_eq!(RewardCurrency::total_issuance(), initial_reward_issuance);

			// inflation kicks in here
			let total_reward_era_1 = Rewards::calculate_total_reward(era_duration_ms).total;
			assert!(total_reward_era_1 > 0);

			// make a block this era
			let author_index = 0;
			let author_stash_id = Session::validators()[(author_index as usize)].clone();

			// make a block for validator 0
			let header_of_last_block = header_for_block_number((System::block_number() + 1).into());
			let header = set_author(header_of_last_block, author_index.clone() as u32);
			Executive::initialize_block(&header);

			// start era 2
			start_active_era(2);
			let total_reward_era_1 = Rewards::calculate_total_reward(Timestamp::now() - era1_start_ms).total;
			// skip a few blocks to ensure payouts are made for era 1
			advance_session();

			for (stash, _controller, _, _, _, _, _) in &validators {
				if stash == &author_stash_id {
					assert_eq!(
						RewardCurrency::free_balance(&stash),
						// author made all the blocks so gets all the reward
						initial_balance + total_reward_era_1
					);
				} else {
					assert_eq!(RewardCurrency::free_balance(&stash), initial_balance);
				}
			}

			// Check total issuance of spending asset updated after new era
			assert_eq!(
				RewardCurrency::total_issuance(),
				initial_reward_issuance + total_reward_era_1
			);
		});
}

#[test]
/// This tests if authorship reward of the last block in an era is factored in.
fn authorship_reward_of_last_block_in_an_era() {
	let validators = make_authority_keys(6);
	let initial_balance = 1_000 * DOLLARS;

	ExtBuilder::default()
		.initial_authorities(validators.as_slice())
		.initial_balance(initial_balance)
		.stash(initial_balance)
		.build()
		.execute_with(|| {
			// start era 1, era 0 has no reward
			assert!(Rewards::calculate_total_reward(Rewards::FULL_ERA_DURATION)
				.total
				.is_zero());
			start_active_era(1);
			let era1_start_ms = Timestamp::now();

			let final_session_of_era_index = Session::current_index() + SessionsPerEra::get() - 1;
			start_session(final_session_of_era_index);

			// The final session falls in the era 1
			assert_eq!(active_era(), 1);

			// Make a block header whose author is specified as below
			let author_index = 0; // index 0 of validators
			let author_stash_id = Session::validators()[(author_index as usize)].clone();
			let first_block_of_era_2 = System::block_number() + 1;
			let header_of_last_block = header_for_block_number(first_block_of_era_2.into());
			let header = set_author(header_of_last_block, author_index.clone() as u32);

			let author_reward_balance_before_adding_block = RewardCurrency::free_balance(&author_stash_id);
			Executive::initialize_block(&header);

			// next era
			advance_session();
			let staker_reward_era_1 = Rewards::calculate_total_reward(Timestamp::now() - era1_start_ms).stakers_cut;
			assert_eq!(active_era(), 2);
			// trigger payout
			advance_session();
			assert_eq!(
				RewardCurrency::free_balance(&author_stash_id),
				author_reward_balance_before_adding_block + staker_reward_era_1
			);
		});
}

#[test]
fn slashed_cennz_goes_to_treasury() {
	let validators: Vec<AuthorityKeys> = make_authority_keys(6);
	let initial_balance = 1_000 * DOLLARS;
	ExtBuilder::default()
		.initial_authorities(validators.as_slice())
		.initial_balance(initial_balance)
		.stash(initial_balance)
		.invulnerables_off()
		.build()
		.execute_with(|| {
			// Initially treasury has no CENNZ
			assert!(StakeCurrency::free_balance(&Treasury::account_id()).is_zero());

			let validator_set_count = validators.len() as u32;
			let offenders_count = validator_set_count; // All validators are offenders

			// calculate the total slashed amount for an Unresponsiveness offence
			let slashed_amount = UnresponsivenessOffence::<()>::slash_fraction(offenders_count, validator_set_count);
			let per_offender_slash = slashed_amount * initial_balance;
			let total_slashed_cennz = per_offender_slash.saturating_mul(offenders_count.into());

			// Fast-forward eras, 'i'm online' heartbeats are not submitted during this process which will
			// result in automatic slash reports being generated by the protocol against all staked validators.
			// Once `SlashDeferDuration` eras have passed the offence from era(0) will be applied.
			// Fast-forward eras so that the slash is applied
			start_session(SlashDeferDuration::get() * SessionsPerEra::get() + 1);

			// Treasury should receive all offenders stake
			assert_eq!(
				StakeCurrency::free_balance(&Treasury::account_id()),
				total_slashed_cennz,
			);
			// All validators stashes are slashed entirely
			validators.iter().for_each(|validator_keys| {
				assert_eq!(
					StakeCurrency::free_balance(&validator_keys.0),
					initial_balance - per_offender_slash
				)
			});
		});
}

#[test]
fn slashed_cennz_goes_to_reporter() {
	let validators: Vec<AuthorityKeys> = make_authority_keys(1);
	let initial_balance = 1_000 * DOLLARS;
	ExtBuilder::default()
		.initial_authorities(validators.as_slice())
		.initial_balance(initial_balance)
		.stash(initial_balance)
		.invulnerables_off()
		.build()
		.execute_with(|| {
			// Initially treasury has no CENNZ
			assert!(StakeCurrency::free_balance(&Treasury::account_id()).is_zero());
			let offender = &validators[0].0;
			let reporter = bob();

			// Make a slash-able offence report on validator[0]
			let offence = OffenceDetails {
				// validators[0].0 is the stash account of the first validator
				offender: (offender.clone(), Staking::eras_stakers(active_era(), &offender)),
				reporters: vec![reporter.clone()],
			};

			// report offence
			let slash_fraction = Perbill::from_percent(90);
			assert_eq!(
				Staking::on_offence(
					&[offence],
					&[slash_fraction],
					Staking::eras_start_session_index(active_era()).expect("session index exists"),
					DisableStrategy::WhenSlashed,
				),
				700_000_000
			);

			// Fast-forward eras so that the slash is applied
			start_active_era(active_era() + SlashDeferDuration::get() + 1);

			// offender CENNZ funds are slashed
			let total_slash = slash_fraction * initial_balance;
			assert_eq!(StakeCurrency::free_balance(&offender), initial_balance - total_slash);
			// reporter fee calculation doesn't have a nice API
			// so we reproduce it here from variables, this is the calculation for the first slash of a slashing span.
			let reporter_fee = (Staking::slash_reward_fraction().saturating_mul(crml_staking::REWARD_F1)) * total_slash;
			// reporter is paid a CENNZ reporter's fee
			assert_eq!(StakeCurrency::free_balance(&reporter), initial_balance + reporter_fee);
			// Treasury should receive remainder of slash after the CENNZ reporter's fee
			assert_eq!(
				StakeCurrency::free_balance(&Treasury::account_id()),
				total_slash - reporter_fee
			);
		});
}

#[test]
fn reward_scheduling() {
	let validators: Vec<AuthorityKeys> = make_authority_keys(6);
	let initial_balance = 1_000 * DOLLARS;
	ExtBuilder::default()
		.initial_authorities(validators.as_slice())
		.initial_balance(initial_balance)
		.stash(initial_balance)
		.build()
		.execute_with(|| {
			start_active_era(1);
			let era1_start_ms = Timestamp::now();
			// era 0 has no reward
			start_active_era(2);
			// era 1 reward payouts should be scheduled
			let per_validator_reward = Rewards::calculate_total_reward(Timestamp::now() - era1_start_ms).stakers_cut
				/ validators.len() as Balance;

			assert_eq!(crml_staking::rewards::ScheduledPayoutEra::get(), 1,);
			let scheduled_payouts = crml_staking::rewards::ScheduledPayouts::<Runtime>::iter()
				.collect::<Vec<(BlockNumber, (AccountId, Balance))>>();
			for (_block, (who, amount)) in scheduled_payouts.into_iter() {
				assert!(Session::validators().iter().find(|v| *v == &who).is_some());
				assert_eq!(amount, per_validator_reward);
			}
		})
}

#[test]
fn reward_scheduling_short_era() {
	let validators: Vec<AuthorityKeys> = make_authority_keys(6);
	let initial_balance = 1_000 * DOLLARS;
	ExtBuilder::default()
		.initial_authorities(validators.as_slice())
		.initial_balance(initial_balance)
		.stash(initial_balance)
		.build()
		.execute_with(|| {
			start_active_era(1);
			let era1_start_ms = Timestamp::now();
			// era 0 has no reward
			start_active_era(2);
			// era 1 reward payouts should be scheduled
			let per_validator_reward = Rewards::calculate_total_reward(Timestamp::now() - era1_start_ms).stakers_cut
				/ validators.len() as Balance;

			ForceEra::put(Forcing::ForceNew);
			// start active era 3 early
			advance_session();
			assert_eq!(active_era(), 3);

			let scheduled_payouts = crml_staking::rewards::ScheduledPayouts::<Runtime>::iter()
				.collect::<Vec<(BlockNumber, (AccountId, Balance, EraIndex))>>();
			for (_block, (who, amount, era)) in scheduled_payouts.into_iter() {
				assert!(Session::validators().iter().find(|v| *v == &who).is_some());
				assert_eq!(amount, per_validator_reward);
			}
		})
}

#[test]
fn max_nominators_rewarded() {
	let validators: Vec<AuthorityKeys> = make_authority_keys(6);
	let initial_balance = 1_000 * DOLLARS;
	ExtBuilder::default()
		.initial_authorities(validators.as_slice())
		.initial_balance(initial_balance)
		.stash(initial_balance)
		.build()
		.execute_with(|| {
			// era 0 has no reward
			start_active_era(1);

			// integer IDs for nominators addresses
			let stash = |n: u32| -> AccountId { AccountId::unchecked_from(H256::from_low_u64_be(n as u64 + 10_000)) };

			// nominate max nominators for validator 0
			for n in 1..=MaxNominatorRewardedPerValidator::get() {
				Staking::set_bond(stash(n), 1_000_000);
				Staking::set_nominations(stash(n), vec![Session::validators()[0].clone()]);
			}

			// add one more, with less stake
			let left_out = MaxNominatorRewardedPerValidator::get() + 1;
			Staking::set_bond(stash(left_out), 999_999);
			Staking::set_nominations(stash(left_out), vec![Session::validators()[0].clone()]);

			start_active_era(2);
			// nominations are active now
			start_active_era(3);
			// nominations should be paid out (era 2)
			// skip blocks to ensure payouts are made
			advance_session();

			// paid rewards
			for n in 1..=MaxNominatorRewardedPerValidator::get() {
				assert!(RewardCurrency::free_balance(&stash(n)) > Zero::zero());
			}

			// unpaid, missed the cut
			assert!(RewardCurrency::free_balance(&stash(left_out)).is_zero());
		});
}

#[test]
fn accrued_payout_simple() {
	let validators: Vec<AuthorityKeys> = make_authority_keys(6);
	let initial_balance = 1_000 * DOLLARS;
	ExtBuilder::default()
		.initial_authorities(validators.as_slice())
		.initial_balance(initial_balance)
		.stash(initial_balance)
		.build()
		.execute_with(|| {
			start_active_era(1);
			// finalize the last era block to set ActiveEra info
			advance_session();
			// set the timestamp to simulate full era duration
			Timestamp::set_timestamp(Timestamp::now() + Rewards::FULL_ERA_DURATION);
			let reward_parts = Rewards::calculate_total_reward(Rewards::FULL_ERA_DURATION);
			let per_validator_reward_era_1 = reward_parts.stakers_cut / validators.len() as Balance;
			assert_eq!(
				per_validator_reward_era_1,
				Staking::accrued_payout(&Session::validators()[0])
			);

			// empty start time or empty active era info return 0
			let active_era_no_start_time = ActiveEraInfo::decode(&mut &[0_u8, 0, 0, 0, 0][..]).expect("it decodes");
			ActiveEra::put(active_era_no_start_time);
			assert!(Staking::accrued_payout(&Session::validators()[0]).is_zero());
			ActiveEra::kill();
			assert!(Staking::accrued_payout(&Session::validators()[0]).is_zero());
		});
}

#[test]
fn accrued_payout_nominators() {
	let validators: Vec<AuthorityKeys> = make_authority_keys(6);
	let initial_balance = 1_000 * DOLLARS;
	ExtBuilder::default()
		.initial_authorities(validators.as_slice())
		.initial_balance(initial_balance)
		.stash(initial_balance)
		.build()
		.execute_with(|| {
			// era 0 has no reward
			start_active_era(1);

			// setup nominator stakes for next era
			// integer IDs for nominators addresses
			let stash = |n: u32| -> AccountId { AccountId::unchecked_from(H256::from_low_u64_be(n as u64 + 10_000)) };

			// Make some nominations
			Staking::set_bond(stash(1), 1_000_000);
			Staking::set_nominations(stash(1), vec![Session::validators()[0].clone()]);

			Staking::set_bond(stash(2), 1_000_000);
			Staking::set_nominations(stash(2), vec![Session::validators()[0].clone()]);
			// set payee differently
			Rewards::set_payee(&stash(2), &stash(9));

			// start era 2
			// nominations are active now we should see reward accruing
			start_active_era(2);

			// after first block there is some reward accruing
			run_to_block(System::block_number() + 1);
			let accrued_1_block_1 = Staking::accrued_payout(&stash(1));
			let accrued_2_block_1 = Staking::accrued_payout(&stash(2));
			assert!(accrued_1_block_1 > Zero::zero());
			assert!(accrued_2_block_1 > Zero::zero());

			// reward continues to accrue as time passes / blocks are made
			run_to_block(System::block_number() + 1);
			let accrued_1_block_2 = Staking::accrued_payout(&stash(1));
			let accrued_2_block_2 = Staking::accrued_payout(&stash(2));
			assert!(accrued_1_block_2 > accrued_1_block_1);
			assert!(accrued_2_block_2 > accrued_2_block_1);

			// Run to last block of the era (blocks_per_sesssion * sessions_per_era - the 2 blocks executed already) - 1
			run_to_block(
				System::block_number() + EpochDuration::get() as BlockNumber * SessionsPerEra::get() as BlockNumber
					- 2 as BlockNumber - 1 as BlockNumber,
			);
			// Calculate final era payouts
			// manually add the per block reward for the current block (+2)
			let accrued_1_era2 = Staking::accrued_payout(&stash(1)) + 2;
			let accrued_2_era2 = Staking::accrued_payout(&stash(2)) + 2;

			// End era 2 and trigger payouts
			run_to_block(System::block_number() + 10);

			assert_eq!(RewardCurrency::free_balance(&stash(1)), accrued_1_era2);
			assert!(RewardCurrency::free_balance(&stash(2)).is_zero()); // stash(2) not the payee
			assert_eq!(RewardCurrency::free_balance(&stash(9)), accrued_2_era2); // stash(2) payee
		});
}

#[test]
fn block_author_receives_evm_priority_fee_reward() {
	// Check priority fee is paid out to
	let validators = make_authority_keys(6);
	let initial_balance = 100_000_000 * DOLLARS;
	let staked_amount = initial_balance / validators.len() as Balance;
	let caller = H160::from_slice(&hex!("919b7aeee2ee14e5ceeee688736a38497d79c501"));
	let caller_cennz: AccountId = PrefixedAddressMapping::into_account_id(caller.clone());

	ExtBuilder::default()
		.initialise_eth_accounts(vec![caller_cennz.clone()])
		.initial_authorities(validators.as_slice())
		.initial_balance(initial_balance)
		.stash(staked_amount)
		.build()
		.execute_with(|| {
			start_active_era(1);
			let make_block_with_author = |author_index: u32| {
				let header_of_last_block = header_for_block_number((System::block_number() + 1).into());
				let header = set_author(header_of_last_block, author_index);
				Executive::initialize_block(&header);
			};
			make_block_with_author(0);

			// Create Ethereum transaction
			let priority_fee: u128 = 12_000_000_000;
			let t = EIP1559UnsignedTransaction {
				nonce: U256::zero(),
				max_priority_fee_per_gas: U256::from(priority_fee),
				max_fee_per_gas: U256::from(DefaultBaseFeePerGas::get()),
				gas_limit: U256::from(4_000_000),
				action: pallet_ethereum::TransactionAction::Create,
				value: U256::zero(),
				input: FromHex::from_hex(ERC20_CONTRACT_BYTECODE).unwrap(),
			};
			let secret_key = H256::from_slice(&hex!(
				"3d53e3c2162ba346648689696bde867f5089ba4e35eee5640f49d335b9a87f30"
			));
			let transaction = t.sign(&secret_key, None);

			let call = pallet_ethereum::Call::<Runtime>::transact { transaction };
			let source = call.check_self_contained().unwrap().unwrap();
			let extrinsic = fp_self_contained::CheckedExtrinsic::<_, _, SignedExtra, _> {
				signed: fp_self_contained::CheckedSignature::SelfContained(source),
				function: Call::Ethereum(call),
			};
			use frame_support::weights::GetDispatchInfo as _;
			let dispatch_info = extrinsic.get_dispatch_info();
			let res = extrinsic.apply::<Runtime>(&dispatch_info, 0);
			// Calculate actual priority fee based on used gas
			let actual_weight = res.unwrap().unwrap().actual_weight.unwrap();
			let used_gas = CENNZnetGasWeightMapping::weight_to_gas(actual_weight.into());
			let actual_priority_fee = cennznet_runtime::impls::scale_to_4dp(priority_fee * used_gas as u128);

			// Get current validators (Can't use previous value as they have been sorted)
			let validators = <pallet_session::Pallet<Runtime>>::validators();
			assert_eq!(
				RewardCurrency::free_balance(&validators[0].clone()), // Get stash account
				initial_balance + actual_priority_fee,
			);
		});
}
