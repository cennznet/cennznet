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
	Babe, Call, CheckedExtrinsic, EpochDuration, Executive, ImOnline, Rewards, Runtime, Session, SessionsPerEra,
	SlashDeferDuration, Staking, System, Timestamp, Treasury,
};
use codec::Encode;
use crml_staking::{EraIndex, HandlePayee, OnEndEra, RewardCalculation, StakingLedger, ValidatorPrefs};
use frame_support::{
	assert_ok,
	storage::{StorageDoubleMap, StorageValue},
	traits::{Currency, Get, OnFinalize, OnInitialize, UnfilteredDispatchable},
	IterableStorageDoubleMap, IterableStorageMap,
};
use frame_system::RawOrigin;
use pallet_im_online::UnresponsivenessOffence;
use sp_consensus_babe::{digests, AuthorityIndex, BABE_ENGINE_ID};
use sp_runtime::{
	traits::{Header as HeaderT, Saturating, Zero},
	Perbill,
};
use sp_staking::{
	offence::{Offence, OffenceDetails, OnOffenceHandler},
	SessionIndex,
};
mod common;

use common::helpers::{extrinsic_fee_for, header_for_block_number, make_authority_keys, sign};
use common::keyring::{alice, bob, charlie, signed_extra};
use common::mock::ExtBuilder;

/// Alias for the runtime configured staking reward currency
type RewardCurrency = <Runtime as crml_staking::rewards::Trait>::CurrencyToReward;
/// Alias for the runtime configured staking currency
type StakeCurrency = <Runtime as crml_staking::Trait>::Currency;

/// Progress to the given block, triggering session and era changes as we progress.
///
/// This will finalize the previous block, initialize up to the given block, essentially simulating
/// a block import/propose process where we first initialize the block, then execute some stuff (not
/// in the function), and then finalize the block.
pub(crate) fn run_to_block(n: BlockNumber) {
	Staking::on_finalize(System::block_number());
	for b in (System::block_number() + 1)..=n {
		System::set_block_number(b);
		Session::on_initialize(b);
		Staking::on_initialize(b);
		Rewards::on_initialize(b);
		// Session modules asks babe module if it can rotate
		Timestamp::set_timestamp(30_000 + b as u64 * MILLISECS_PER_BLOCK);
		// force babe slot increment (normally this is set using system digest)
		pallet_babe::CurrentSlot::put(n as u64);
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

/// Progresses from the current block number (whatever that may be) to the `P * session_index + 1`.
pub(crate) fn start_session(session_index: SessionIndex) {
	run_to_block(session_index * EpochDuration::get() as u32);
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
	assert!(current_era() >= active_era());
}

/// Get a block header and set the author of that block in a way that is recognisable by BABE.
/// The author will be specified by its index in the Session::validators() list. So the author
/// should be a current validator. Return the modified header.
fn set_author(mut header: Header, author_index: AuthorityIndex) -> Header {
	use digests::{PreDigest, SecondaryPlainPreDigest};

	let digest_data = PreDigest::SecondaryPlain(SecondaryPlainPreDigest {
		authority_index: author_index,
		slot_number: Babe::current_slot(),
	});

	let digest = header.digest_mut();
	digest
		.logs
		.push(DigestItem::PreRuntime(BABE_ENGINE_ID, digest_data.encode()));

	header
}

/// Send heartbeats for the current authorities
fn send_heartbeats() {
	for i in 0..Session::validators().len() {
		let heartbeat_data = pallet_im_online::Heartbeat {
			block_number: System::block_number(),
			network_state: Default::default(),
			session_index: Session::current_index(),
			authority_index: i as u32,
			validators_len: Session::validators().len() as u32,
		};
		let call = pallet_im_online::Call::heartbeat(heartbeat_data, Default::default());
		let _ = <pallet_im_online::Call<Runtime> as UnfilteredDispatchable>::dispatch_bypass_filter(
			call,
			RawOrigin::None.into(),
		);
	}
}

#[test]
fn start_active_era_works() {
	ExtBuilder::default().build().execute_with(|| {
		let blocks_per_era = SessionsPerEra::get() * EpochDuration::get() as u32;
		start_active_era(1);
		assert_eq!(System::block_number(), blocks_per_era);
		assert_eq!(Session::current_index(), SessionsPerEra::get());
		assert_eq!(Staking::active_era().unwrap().index, 1);

		// one session extra, should schedule the next era (poorly named 'current era')
		advance_session();
		assert_eq!(Staking::current_era().unwrap(), 2);

		start_active_era(2);
		assert_eq!(System::block_number(), blocks_per_era * 2);
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
			for (stash, controller, _, _, _, _) in validators {
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
fn current_era_transaction_rewards_storage_update_works() {
	let initial_balance = 10_000 * DOLLARS;
	let validators = make_authority_keys(6);
	let staked_amount = initial_balance / validators.len() as Balance;

	let runtime_call_1 = Call::GenericAsset(prml_generic_asset::Call::transfer(CENTRAPAY_ASSET_ID, bob(), 123));
	let runtime_call_2 = Call::GenericAsset(prml_generic_asset::Call::transfer(CENTRAPAY_ASSET_ID, charlie(), 456));

	ExtBuilder::default()
		.initial_authorities(validators.as_slice())
		.initial_balance(initial_balance)
		.stash(staked_amount)
		.build()
		.execute_with(|| {
			let xt_1 = sign(CheckedExtrinsic {
				signed: Some((alice(), signed_extra(0, 0, None))),
				function: runtime_call_1.clone(),
			});
			let xt_2 = sign(CheckedExtrinsic {
				signed: Some((bob(), signed_extra(0, 0, None))),
				function: runtime_call_2.clone(),
			});

			// Start the first era
			advance_session();
			assert_eq!(active_era(), 1);

			// Start with 0 transaction rewards
			assert!(Rewards::transaction_fee_pot().is_zero());

			// Apply first extrinsic and check transaction rewards
			let r = Executive::apply_extrinsic(xt_1.clone());
			assert!(r.is_ok());
			let mut era1_tx_fee = extrinsic_fee_for(&xt_1);
			assert_eq!(Rewards::transaction_fee_pot(), era1_tx_fee);

			// Apply second extrinsic and check transaction rewards
			let r2 = Executive::apply_extrinsic(xt_2.clone());
			assert!(r2.is_ok());
			era1_tx_fee += extrinsic_fee_for(&xt_2);
			assert_eq!(Rewards::transaction_fee_pot(), era1_tx_fee);

			// Advancing sessions shouldn't change transaction rewards storage
			advance_session();
			advance_session();
			assert_eq!(active_era(), 1);
			assert_eq!(Rewards::transaction_fee_pot(), era1_tx_fee);

			// At the start of the next era, transaction rewards should be cleared (and paid out)
			start_active_era(2);
			assert_eq!(active_era(), 2);
			advance_session();
			assert!(Rewards::transaction_fee_pot().is_zero());
		});
}

#[test]
fn elected_validators_receive_transaction_fee_reward() {
	// Make some txs
	// Start a new era to payout last eras validators
	// Check payouts happen as expected and total issuance is maintained
	let validators = make_authority_keys(6);
	let initial_balance = 100_000_000 * DOLLARS;
	let staked_amount = initial_balance / validators.len() as Balance;
	let transfer_amount = 50;
	let runtime_call = Call::GenericAsset(prml_generic_asset::Call::transfer(
		CENTRAPAY_ASSET_ID,
		bob(),
		transfer_amount,
	));

	ExtBuilder::default()
		.initial_authorities(validators.as_slice())
		.initial_balance(initial_balance)
		.stash(staked_amount)
		.build()
		.execute_with(|| {
			let make_validator_0_author_block = || {
				let header_of_last_block = header_for_block_number((System::block_number() + 1).into());
				let header = set_author(header_of_last_block, 0);
				Executive::initialize_block(&header);
			};
			make_validator_0_author_block();

			// reward currency is also fee currency
			let initial_issuance = RewardCurrency::total_issuance();

			let xt = sign(CheckedExtrinsic {
				signed: Some((alice(), signed_extra(0, 0, None))),
				function: runtime_call,
			});

			let tx_fee = extrinsic_fee_for(&xt);
			let r = Executive::apply_extrinsic(xt);
			assert!(r.is_ok());

			let issuance_after_fees_burned = RewardCurrency::total_issuance();
			assert_eq!(issuance_after_fees_burned, initial_issuance - tx_fee);

			// tx fees are tracked by the Rewards module
			let reward_parts = Rewards::calculate_total_reward();
			assert_eq!(Rewards::target_inflation_per_staking_era(), reward_parts.inflation);
			assert_eq!(tx_fee, reward_parts.transaction_fees);
			assert_eq!(Rewards::target_inflation_per_staking_era() + tx_fee, reward_parts.total);

			// treasury has nothing at this point
			assert!(RewardCurrency::free_balance(&Treasury::account_id()).is_zero());

			let e0 = crml_staking::ErasValidatorPrefs::<Runtime>::iter_prefix(0_u32)
				.map(|(stash, _prefs)| stash)
				.collect::<Vec<AccountId>>();
			let e1 = crml_staking::ErasValidatorPrefs::<Runtime>::iter_prefix(1_u32)
				.map(|(stash, _prefs)| stash)
				.collect::<Vec<AccountId>>();

			// TODO: validators are coming through empty here...
			send_heartbeats();
			start_active_era(1);

			let e2 = crml_staking::ErasValidatorPrefs::<Runtime>::iter_prefix(0_u32)
				.map(|(stash, _prefs)| stash)
				.collect::<Vec<AccountId>>();
			let e3 = crml_staking::ErasValidatorPrefs::<Runtime>::iter_prefix(1_u32)
				.map(|(stash, _prefs)| stash)
				.collect::<Vec<AccountId>>();

			// end era 1, reward payouts are scheduled
			println!("{:?}", crml_staking::rewards::ScheduledPayoutEra::get());
			println!("{:?}", reward_parts.stakers_cut);
			let payouts: Vec<(BlockNumber, (AccountId, Balance))> =
				crml_staking::rewards::ScheduledPayouts::<Runtime>::iter().collect();
			for (block, (who, amount)) in payouts {
				println!("{:?} {:?} {:?}", block, who, amount);
			}

			// treasury is paid its cut of network tx fees
			assert_eq!(
				RewardCurrency::free_balance(&Treasury::account_id()),
				reward_parts.treasury_cut
			);

			// // Run additional blocks to execute payouts
			// // let payouts_complete_session_index = BlockPayoutInterval::get() as u32 * validators.len() as u32;
			// // start_session(Session::current_index() + payouts_complete_session_index + 1);
			// start_active_era(2);
			// advance_session();

			// Validators get the rest of the reward
			let per_validator_reward_era_0 = reward_parts.stakers_cut / validators.len() as Balance;
			for (stash, _, _, _, _, _) in &validators {
				assert_eq!(
					RewardCurrency::free_balance(stash),
					initial_balance + per_validator_reward_era_0
				)
			}

			// treasury will accrue some remainder from per validator payouts
			assert_eq!(
				RewardCurrency::free_balance(&Treasury::account_id()),
				reward_parts.treasury_cut
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

	ExtBuilder::default()
		.initial_authorities(validators.as_slice())
		.initial_balance(initial_balance)
		.stash(staked_amount)
		.build()
		.execute_with(|| {
			start_active_era(1);

			let author_index = 0;
			let author_stash_id = Session::validators()[(author_index as usize)].clone();
			let initial_reward_issuance = RewardCurrency::total_issuance();

			// No fee is collected yet, the reward is only mined.
			let total_payout = Rewards::calculate_total_reward().total;
			assert_eq!(total_payout, Rewards::target_inflation_per_staking_era());

			let header_of_last_block = header_for_block_number((System::block_number() + 1).into());
			let header = set_author(header_of_last_block, author_index.clone() as u32);
			Executive::initialize_block(&header);

			// treasury would be almost empty as there hasn't been any transaction fees.
			let treasury_balance_era_1 = RewardCurrency::free_balance(&Treasury::account_id());
			assert_eq!(treasury_balance_era_1, 3);

			// In era 1, the rewards are just earned through the inflation as there was no transactions
			let per_validator_reward_era_1 = Rewards::target_inflation_per_staking_era() / validators.len() as Balance;
			for (stash, _, _, _, _, _) in &validators {
				assert_eq!(
					RewardCurrency::free_balance(stash),
					initial_balance + per_validator_reward_era_1
				)
			}

			start_active_era(2);
			Executive::initialize_block(&header_for_block_number((System::block_number() + 1).into()));

			// The whole reward of the era goes to the author
			let author_reward = Rewards::target_inflation_per_staking_era();

			for (stash, _controller, _, _, _, _) in &validators {
				if stash == &author_stash_id {
					assert_eq!(
						RewardCurrency::free_balance(&stash),
						initial_balance + per_validator_reward_era_1 + author_reward
					);
				} else {
					assert_eq!(
						RewardCurrency::free_balance(&stash),
						initial_balance + per_validator_reward_era_1
					);
				}
			}

			// Check total issuance of spending asset updated after new era
			assert_eq!(
				RewardCurrency::total_issuance(),
				initial_reward_issuance + Rewards::target_inflation_per_staking_era() * 2
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
			let final_session_of_era_index = SessionsPerEra::get() - 1;
			start_session(final_session_of_era_index);
			// The final session falls in the era 0
			assert_eq!(active_era(), 0);

			// Make sure we have the correct number of validators elected
			assert_eq!(Session::validators().len(), validators.len());

			// Make a block header whose author is specified as below
			let author_index = 0; // index 0 of validators
			let author_stash_id = Session::validators()[(author_index as usize)].clone();
			let first_block_of_era_1 = System::block_number() + 1;
			let header_of_last_block = header_for_block_number(first_block_of_era_1.into());
			let header = set_author(header_of_last_block, author_index.clone() as u32);

			// The previous session should come to its end
			pallet_babe::CurrentSlot::put(Babe::current_slot() + EpochDuration::get());

			send_heartbeats();

			let author_reward_balance_before_adding_block = RewardCurrency::free_balance(&author_stash_id);

			Executive::initialize_block(&header);
			advance_session();

			// initializing the last block should have advanced the session and thus changed the era
			assert_eq!(active_era(), 1);

			// No offences should happened. Thus the number of validators shouldn't have changed
			assert_eq!(Session::validators().len(), validators.len());

			// There should be a reward given to the author at the very next block
			Executive::initialize_block(&header_for_block_number((System::block_number() + 1).into()));
			assert!(RewardCurrency::free_balance(&author_stash_id) > author_reward_balance_before_adding_block);
		});
}

#[test]
/// This tests if authorship reward of the last block in an era is factored in, even when the author
/// is chilled and thus not going to be an authority in the next era.
fn authorship_reward_of_a_chilled_validator() {
	let validators = make_authority_keys(6);
	let initial_balance = 1_000 * DOLLARS;

	ExtBuilder::default()
		.initial_authorities(validators.as_slice())
		.initial_balance(initial_balance)
		.stash(initial_balance)
		.build()
		.execute_with(|| {
			let final_session_of_era_index = SessionsPerEra::get() - 1;
			start_session(final_session_of_era_index);

			// The last session falls in the era 0
			assert_eq!(active_era(), 0);

			// make sure we have the correct number of validators elected
			assert_eq!(Session::validators().len(), validators.len());

			// Make a block header whose author is specified as below
			let author_index = 0; // index 0 of validators
			let first_block_of_era_1 = System::block_number() + 1;
			let header_of_last_block = header_for_block_number(first_block_of_era_1.into());
			let header = set_author(header_of_last_block, author_index.clone() as u32);

			let author_stash_id = Session::validators()[(author_index as usize)].clone();

			// Report an offence for the author of the block that is going to be initialised
			assert_ok!(Staking::on_offence(
				&[sp_staking::offence::OffenceDetails {
					offender: (
						author_stash_id.clone(),
						Staking::eras_stakers(active_era(), &author_stash_id)
					),
					reporters: vec![],
				}],
				&[Perbill::from_percent(0)],
				Session::current_index(),
			));

			// The previous session should come to its end
			pallet_babe::CurrentSlot::put(Babe::current_slot() + EpochDuration::get());

			send_heartbeats();

			let author_reward_balance_before_adding_block = RewardCurrency::free_balance(&author_stash_id);

			Executive::initialize_block(&header);
			advance_session();

			// initializing the last block should have advanced the session and thus changed the era
			assert_eq!(active_era(), 1);

			// If the offended validator is chilled, in the new era, there should be one less elected validators than before
			assert_eq!(Session::validators().len(), validators.len() - 1);

			// There should be a reward calculated for the author even though the author is chilled
			Executive::initialize_block(&header_for_block_number((System::block_number() + 1).into()));
			assert!(RewardCurrency::free_balance(&author_stash_id) > author_reward_balance_before_adding_block);
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
			// Once `SlashDeferDuration` + 1 eras have passed the offence from era(0) will be applied.
			start_active_era(SlashDeferDuration::get() + 1);

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

			let slash_fraction = Perbill::from_percent(90);
			assert_ok!(Staking::on_offence(
				&[offence],
				&[slash_fraction],
				Staking::eras_start_session_index(active_era()).expect("session index exists"),
			));

			// Fast-forward eras so that the slash is applied
			start_active_era(SlashDeferDuration::get() + 1);

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
