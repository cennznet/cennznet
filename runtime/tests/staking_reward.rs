/* Copyright 2019-2020 Centrality Investments Limited
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
use cennznet_primitives::types::{Balance, DigestItem, Header};
use cennznet_runtime::{
	constants::{asset::*, currency::*, time::MILLISECS_PER_BLOCK},
	Babe, Call, CheckedExtrinsic, EpochDuration, Executive, Rewards, Runtime, Session, SessionsPerEra,
	SlashDeferDuration, Staking, System, Timestamp, Treasury,
};
use codec::Encode;
use crml_staking::{EraIndex, RewardDestination, StakerRewardPayment, StakingLedger};
use frame_support::{
	assert_ok,
	storage::StorageValue,
	traits::{Currency, OnInitialize, UnfilteredDispatchable},
};
use frame_system::RawOrigin;
use pallet_im_online::UnresponsivenessOffence;
use sp_consensus_babe::{digests, AuthorityIndex, BABE_ENGINE_ID};
use sp_runtime::{
	traits::{Header as HeaderT, Saturating, Zero},
	FixedPointNumber, Perbill,
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
		<pallet_im_online::Call<Runtime> as UnfilteredDispatchable>::dispatch_bypass_filter(
			call,
			RawOrigin::None.into(),
		)
		.unwrap();
	}
}

/// Prior to rotating to a new session, we should make sure the authority heartbeats are sent to the
/// ImOnline module, time is set accordingly and the babe's current slot is adjusted
fn pre_rotate_session() {
	send_heartbeats();
	Timestamp::set_timestamp(Timestamp::now() + 1000);
	pallet_babe::CurrentSlot::put(Babe::current_slot() + EpochDuration::get());
}

fn rotate_to_session(index: SessionIndex) {
	assert!(Session::current_index() <= index);
	Session::on_initialize(System::block_number());

	let rotations = index - Session::current_index();
	for _i in 0..rotations {
		pre_rotate_session();
		Session::rotate_session();
	}
}

fn start_session(session_index: SessionIndex) {
	// If we run the function for the first time, block_number is 1, which won't
	// trigger Babe::should_end_session() so we have to run one extra loop. But
	// successive calls don't need to run one extra loop. See Babe::should_epoch_change()
	let up_to_session_index = if Session::current_index().is_zero() {
		session_index + 1
	} else {
		session_index
	};
	for i in Session::current_index()..up_to_session_index {
		// TODO Untie the block number from the session index as they are independent concepts.
		System::set_block_number((i + 1).into());
		pallet_babe::CurrentSlot::put(Babe::current_slot() + EpochDuration::get());
		Timestamp::set_timestamp((System::block_number() * MILLISECS_PER_BLOCK as u32).into());
		Session::on_initialize(System::block_number()); // this ends session
	}
	assert_eq!(Session::current_index(), session_index);
}

fn advance_session() {
	let current_index = Session::current_index();
	start_session(current_index + 1);
}

// Starts all sessions up to `era_index` (eg, start_era(2) will start 14 sessions)
fn start_era(era_index: EraIndex) {
	start_session((era_index * SessionsPerEra::get()).into());
	assert_eq!(Staking::current_era(), era_index);
}

#[test]
fn start_session_works() {
	ExtBuilder::default().build().execute_with(|| {
		start_session(1);
		start_session(3);
		start_session(5);
	});
}

#[test]
fn advance_session_works() {
	ExtBuilder::default().build().execute_with(|| {
		let session_index = 12;
		start_session(session_index);
		advance_session();
		advance_session();
		advance_session();
		assert_eq!(Session::current_index(), 15);
	});
}

#[test]
fn start_era_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Staking::current_era(), 0);
		start_era(1);
		assert_eq!(Staking::current_era(), 1);
		start_era(10);
		assert_eq!(Staking::current_era(), 10);
	});
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
				assert!(Staking::current_elected().contains(&stash));
				// Check that RewardDestination is Stash (default)
				assert_eq!(Staking::payee(&stash), RewardDestination::Stash);
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

	let runtime_call_1 = Call::GenericAsset(prml_generic_asset::Call::transfer(CENTRAPAY_ASSET_ID, bob(), 123));
	let runtime_call_2 = Call::GenericAsset(prml_generic_asset::Call::transfer(CENTRAPAY_ASSET_ID, charlie(), 456));

	ExtBuilder::default()
		.initial_balance(initial_balance)
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
			assert_eq!(Staking::current_era(), 1);

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
			assert_eq!(Staking::current_era(), 1);
			assert_eq!(Rewards::transaction_fee_pot(), era1_tx_fee);

			// At the start of the next era, transaction rewards should be cleared (and paid out)
			start_era(2);
			assert_eq!(Staking::current_era(), 2);
			advance_session();
			assert!(Rewards::transaction_fee_pot().is_zero());
		});
}

#[test]
fn elected_validators_receive_equal_transaction_fee_reward() {
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

			// reward is fees * inflation
			let total_payout = Rewards::calculate_next_reward_payout();
			assert_eq!(total_payout, Rewards::inflation_rate().saturating_mul_acc_int(tx_fee));

			// treasury should be empty until rewarded
			assert!(RewardCurrency::free_balance(&Treasury::account_id()).is_zero());

			// Submit a tx, rotate era, check rewards are paid
			start_era(1);

			// Check if stash account balances are not yet changed
			let per_fee_reward: Balance = (Perbill::one().saturating_sub(Rewards::development_fund_take()))
				* total_payout / validators.len() as Balance;
			for (stash, _controller, _, _, _, _) in &validators {
				assert_eq!(RewardCurrency::free_balance(&stash), initial_balance + per_fee_reward);
			}

			// treasury gets it's cut
			let treasury_cut = Rewards::development_fund_take() * total_payout;
			let validator_cut = per_fee_reward * validators.len() as Balance;
			let remainder = total_payout - treasury_cut - validator_cut;
			assert_eq!(
				RewardCurrency::free_balance(&Treasury::account_id()),
				treasury_cut + remainder
			);

			// Check total issuance of spending asset updated after new era
			assert_eq!(
				RewardCurrency::total_issuance(),
				issuance_after_fees_burned + total_payout
			);
		});
}

#[test]
/// This tests if authorship reward of the last block in an era is factored in.
fn authorship_points_of_last_block_in_an_era() {
	let validators = make_authority_keys(6);
	let initial_balance = 1_000 * DOLLARS;

	ExtBuilder::default()
		.initial_authorities(validators.as_slice())
		.initial_balance(initial_balance)
		.stash(initial_balance)
		.build()
		.execute_with(|| {
			let final_session_of_era_index = SessionsPerEra::get() - 1;
			rotate_to_session(final_session_of_era_index);

			// The final session falls in the era 0
			assert_eq!(Staking::current_era(), 0);

			// Make sure we have the correct number of validators elected
			assert_eq!(Staking::current_elected().len(), validators.len());

			// Make a block header whose author is specified as below
			let author_index = 0; // index 0 of validators
			let first_block_of_era_1 = System::block_number() + 1;
			let header_of_last_block = header_for_block_number(first_block_of_era_1.into());
			let header = set_author(header_of_last_block, author_index.clone());

			// The previous session should come to its end
			pallet_babe::CurrentSlot::put(Babe::current_slot() + EpochDuration::get());

			send_heartbeats();

			// Let's go through the first stage of executing the block
			assert!(Staking::current_era_points()
				.individual_points()
				.get(author_index as usize)
				.is_none());
			Executive::initialize_block(&header);
			advance_session();

			// initializing the last block should have advanced the session and thus changed the era
			assert_eq!(Staking::current_era(), 1);

			// No offences should happened. Thus the number of validators shouldn't have changed
			assert_eq!(Staking::current_elected().len(), validators.len());

			// There should be a reward calculated for the author
			assert!(!Staking::current_era_points().individual_points()[author_index as usize] > 0);
		});
}

#[test]
/// This tests if authorship reward of the last block in an era is factored in, even when the author
/// is chilled and thus not going to be an authority in the next era.
fn authorship_points_of_a_chilled_validator() {
	let validators = make_authority_keys(6);
	let initial_balance = 1_000 * DOLLARS;

	ExtBuilder::default()
		.initial_authorities(validators.as_slice())
		.initial_balance(initial_balance)
		.stash(initial_balance)
		.build()
		.execute_with(|| {
			let final_session_of_era_index = SessionsPerEra::get() - 1;
			rotate_to_session(final_session_of_era_index);

			// The last session falls in the era 0
			assert_eq!(Staking::current_era(), 0);

			// make sure we have the correct number of validators elected
			assert_eq!(Staking::current_elected().len(), validators.len());

			// Make a block header whose author is specified as below
			let author_index = 0; // index 0 of validators
			let first_block_of_era_1 = System::block_number() + 1;
			let header_of_last_block = header_for_block_number(first_block_of_era_1.into());
			let header = set_author(header_of_last_block, author_index.clone());

			let author_stash_id = Session::validators()[(author_index as usize)].clone();

			// Report an offence for the author of the block that is going to be initialised
			assert_ok!(Staking::on_offence(
				&[sp_staking::offence::OffenceDetails {
					offender: (author_stash_id.clone(), Staking::stakers(&author_stash_id)),
					reporters: vec![],
				}],
				&[Perbill::from_percent(0)],
				Session::current_index(),
			));

			// The previous session should come to its end
			pallet_babe::CurrentSlot::put(Babe::current_slot() + EpochDuration::get());

			send_heartbeats();

			// Let's go through the first stage of executing the block
			assert!(Staking::current_era_points()
				.individual_points()
				.get(author_index as usize)
				.is_none());
			Executive::initialize_block(&header);
			advance_session();

			// initializing the last block should have advanced the session and thus changed the era
			assert_eq!(Staking::current_era(), 1);

			// If the offended validator is chilled, in the new era, there should be one less elected validators than before
			assert_eq!(Staking::current_elected().len(), validators.len() - 1);

			// There should be a reward calculated for the author even though the author is chilled
			assert!(Staking::current_era_points().individual_points()[author_index as usize] > 0);
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
			start_era(SlashDeferDuration::get() + 1);

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
				offender: (offender.clone(), Staking::stakers(&offender)),
				reporters: vec![reporter.clone()],
			};
			let slash_fraction = Perbill::from_percent(90);
			assert_ok!(Staking::on_offence(
				&[offence],
				&[slash_fraction],
				Staking::current_era_start_session_index(),
			));

			// Fast-forward eras so that the slash is applied
			start_era(SlashDeferDuration::get() + 1);

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
