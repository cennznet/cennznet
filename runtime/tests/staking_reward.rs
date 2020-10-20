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
use cennznet_primitives::types::{AccountId, Balance, DigestItem, Header};
use cennznet_runtime::{
	constants::{asset::*, currency::*, time::MILLISECS_PER_BLOCK},
	Babe, Call, CheckedExtrinsic, EpochDuration, Executive, GenericAsset, Runtime, Session, SessionsPerEra,
	SlashDeferDuration, Staking, System, Timestamp, Treasury,
};
use codec::Encode;
use crml_staking::{EraIndex, RewardDestination, StakingLedger};
use frame_support::{
	assert_ok,
	storage::StorageValue,
	traits::{OnInitialize, UnfilteredDispatchable},
};
use frame_system::RawOrigin;
use prml_generic_asset::MultiCurrencyAccounting as MultiCurrency;
use sp_consensus_babe::{digests, AuthorityIndex, BABE_ENGINE_ID};
use sp_runtime::{
	traits::{Header as HeaderT, Zero},
	Perbill,
};
use sp_staking::{
	offence::{Offence, OffenceDetails, OnOffenceHandler},
	SessionIndex,
};
mod common;

use common::helpers::{extrinsic_fee_for, header, header_for_block_number, make_authority_keys, sign};
use common::keyring::{alice, bob, charlie, signed_extra};
use common::mock::ExtBuilder;

/// Get a list of stash accounts only from `authority_keys`
fn stashes_of(authority_keys: &[AuthorityKeys]) -> Vec<AccountId> {
	authority_keys.iter().map(|x| x.0.clone()).collect()
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

/// Issue validator rewards with constant points = `1`
fn reward_validators(validators: Vec<AccountId>) {
	let validators_points = validators.iter().map(|v| (v.clone(), 1_u32));
	Staking::reward_by_ids(validators_points);
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

// Test to show that every extrinsic applied will add transfer fee to
// CurrentEraFeeRewards (until it's paid out at the end of an era)
#[test]
fn current_era_transaction_rewards_storage_update_works() {
	let initial_balance = 10_000 * DOLLARS;
	let mut total_transfer_fee: Balance = 0;

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

			Executive::initialize_block(&header());
			start_era(1);
			advance_session(); // advance a session to trigger the beginning of era 2
			assert_eq!(Staking::current_era(), 2);

			// Start with 0 transaction rewards
			assert_eq!(Staking::current_era_transaction_fee_reward(), 0);

			// Apply first extrinsic and check transaction rewards
			assert!(Executive::apply_extrinsic(xt_1.clone()).is_ok());
			total_transfer_fee += extrinsic_fee_for(&xt_1);
			assert_eq!(Staking::current_era_transaction_fee_reward(), total_transfer_fee);

			// Apply second extrinsic and check transaction rewards
			assert!(Executive::apply_extrinsic(xt_2.clone()).is_ok());
			total_transfer_fee += extrinsic_fee_for(&xt_2);
			assert_eq!(Staking::current_era_transaction_fee_reward(), total_transfer_fee);

			// Advancing sessions shouldn't change transaction rewards storage
			advance_session();
			assert_eq!(Staking::current_era_transaction_fee_reward(), total_transfer_fee);
			advance_session();
			assert_eq!(Staking::current_era_transaction_fee_reward(), total_transfer_fee);

			// At the start of the next era (13th session), transaction rewards should be cleared (and paid out)
			start_era(2);
			advance_session();
			assert_eq!(Staking::current_era(), 3);
			assert_eq!(Staking::current_era_transaction_fee_reward(), 0);
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
				assert_eq!(
					<GenericAsset as MultiCurrency>::free_balance(&stash, Some(CENTRAPAY_ASSET_ID)),
					balance_amount
				);
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
fn staking_inflation_and_reward_should_work() {
	let balance_amount = 100_000_000 * DOLLARS;
	let total_issuance = balance_amount * 12; // 6 pre-configured + 6 stash accounts
	let validators = make_authority_keys(6);
	let staked_amount = balance_amount / validators.len() as Balance;

	ExtBuilder::default()
		.initial_authorities(validators.as_slice())
		.initial_balance(balance_amount)
		.stash(staked_amount)
		.build()
		.execute_with(|| {
			// Total issuance remains unchanged at era 0.
			start_session(0);
			assert_eq!(Staking::current_era(), 0);
			assert_eq!(GenericAsset::total_issuance(CENNZ_ASSET_ID), total_issuance);
			assert_eq!(GenericAsset::total_issuance(CENTRAPAY_ASSET_ID), total_issuance);
			// Add points to each validator which use to allocate staking reward in the next new era
			reward_validators(stashes_of(&validators));

			// Total issuance for CPAY is inflated at the start of era 1, and that for CENNZ is unchanged.
			start_session(1);
			assert_eq!(Staking::current_era(), 1);
			reward_validators(stashes_of(&validators));

			// Compute total payout and inflation for new era
			let (total_payout, inflation_era_1) = Staking::current_total_payout(total_issuance);
			assert_eq!(total_payout, 27_900);
			assert_eq!(inflation_era_1, 74_400);

			// Compute staking reward for each validator
			let validator_len = validators.len() as Balance;
			let per_staking_reward = total_payout / validator_len;

			// validators should receive staking reward after new era
			for stash in stashes_of(&validators) {
				assert_eq!(
					<GenericAsset as MultiCurrency>::free_balance(&stash, Some(CENTRAPAY_ASSET_ID)),
					balance_amount + per_staking_reward
				);
			}

			let sessions_era_1 = vec![2, 3, 4, 5, 6];
			for session in sessions_era_1 {
				start_session(session);
				assert_eq!(Staking::current_era(), 1);
				// Total issuance for CENNZ is unchanged
				assert_eq!(GenericAsset::total_issuance(CENNZ_ASSET_ID), total_issuance);
				// Total issuance for CPAY remain the same within the same era
				assert_eq!(
					GenericAsset::total_issuance(CENTRAPAY_ASSET_ID),
					total_issuance + inflation_era_1
				);

				// The balance of stash accounts remain the same within the same era
				for stash in stashes_of(&validators) {
					assert_eq!(
						<GenericAsset as MultiCurrency>::free_balance(&stash, Some(CENTRAPAY_ASSET_ID)),
						balance_amount + per_staking_reward
					);
				}
			}

			// Total issuance for CPAY is inflated at the start of era 2, and that for CENNZ is unchanged.
			start_session(7);
			assert_eq!(Staking::current_era(), 2);

			let (total_payout, inflation_era_2) = Staking::current_total_payout(total_issuance + inflation_era_1);
			assert_eq!(total_payout, 71_100);
			assert_eq!(inflation_era_2, 189_600);

			// validators should receive staking reward after new era
			let per_staking_reward = total_payout / validator_len + per_staking_reward;
			for (stash, _controller, _, _, _, _) in &validators {
				assert_eq!(
					<GenericAsset as MultiCurrency>::free_balance(&stash, Some(CENTRAPAY_ASSET_ID)),
					balance_amount + per_staking_reward
				);
			}

			let sessions_era_2 = vec![8, 9, 10, 11, 12];
			for session in sessions_era_2 {
				start_session(session);
				assert_eq!(Staking::current_era(), 2);
				// Total issuance for CENNZ is unchanged
				assert_eq!(GenericAsset::total_issuance(CENNZ_ASSET_ID), total_issuance);
				// Total issuance for CPAY remain the same within the same era
				assert_eq!(
					GenericAsset::total_issuance(CENTRAPAY_ASSET_ID),
					total_issuance + inflation_era_1 + inflation_era_2
				);

				// The balance of stash accounts remain the same within the same era
				for (stash, _controller, _, _, _, _) in &validators {
					assert_eq!(
						<GenericAsset as MultiCurrency>::free_balance(&stash, Some(CENTRAPAY_ASSET_ID)),
						balance_amount + per_staking_reward
					);
				}
			}
		});
}

#[test]
fn staking_validators_should_receive_equal_transaction_fee_reward() {
	let validators = make_authority_keys(6);
	let balance_amount = 100_000_000 * DOLLARS;
	let staked_amount = balance_amount / validators.len() as Balance;
	let transfer_amount = 50;
	let runtime_call = Call::GenericAsset(prml_generic_asset::Call::transfer(
		CENTRAPAY_ASSET_ID,
		bob(),
		transfer_amount,
	));

	ExtBuilder::default()
		.initial_authorities(validators.as_slice())
		.initial_balance(balance_amount)
		.stash(staked_amount)
		.build()
		.execute_with(|| {
			let xt = sign(CheckedExtrinsic {
				signed: Some((alice(), signed_extra(0, 0, None))),
				function: runtime_call,
			});

			let fee = extrinsic_fee_for(&xt);
			let per_fee_reward = fee / validators.len() as Balance;

			start_era(1);
			let validator_len = validators.len() as Balance;
			reward_validators(stashes_of(&validators));

			let r = Executive::apply_extrinsic(xt);
			assert!(r.is_ok());

			// Check if the transfer is successful
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&alice(), Some(CENTRAPAY_ASSET_ID)),
				balance_amount - transfer_amount - fee
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&bob(), Some(CENTRAPAY_ASSET_ID)),
				balance_amount + transfer_amount
			);

			// Check if stash account balances are not yet changed
			for (stash, _controller, _, _, _, _) in &validators {
				assert_eq!(
					<GenericAsset as MultiCurrency>::free_balance(&stash, Some(CENTRAPAY_ASSET_ID)),
					balance_amount
				);
			}

			let total_issuance = GenericAsset::total_issuance(CENTRAPAY_ASSET_ID);
			start_era(2);
			let issued_fee_reward = per_fee_reward * validator_len; // Don't use "fee" itself directly
			let (staking_payout, max_payout) = Staking::current_total_payout(total_issuance + issued_fee_reward);
			let per_staking_reward = staking_payout / validator_len;

			// Check total issuance of Spending Asset updated after new era
			assert_eq!(
				GenericAsset::total_issuance(CENTRAPAY_ASSET_ID),
				total_issuance + max_payout + issued_fee_reward,
			);

			// Check if validator balance changed correctly
			for (stash, _controller, _, _, _, _) in validators {
				// Check tx fee reward went to the stash account of validator
				assert_eq!(
					<GenericAsset as MultiCurrency>::free_balance(&stash, Some(CENTRAPAY_ASSET_ID)),
					balance_amount + per_fee_reward + per_staking_reward
				);
			}
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

			let author_stash_id = Session::validators()[(author_index as usize)].clone();

			// The previous session should come to its end
			pallet_babe::CurrentSlot::put(Babe::current_slot() + EpochDuration::get());

			send_heartbeats();

			let author_stash_balance_before_adding_block =
				GenericAsset::free_balance(SPENDING_ASSET_ID, &author_stash_id);

			// Let's go through the first stage of executing the block
			Executive::initialize_block(&header);

			// initializing the last block should have advanced the session and thus changed the era
			assert_eq!(Staking::current_era(), 1);

			// No offences should happened. Thus the number of validators shouldn't have changed
			assert_eq!(Staking::current_elected().len(), validators.len());

			// There should be a reward calculated for the author
			assert!(
				GenericAsset::free_balance(SPENDING_ASSET_ID, &author_stash_id)
					> author_stash_balance_before_adding_block
			);
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

			let author_stash_balance_before_adding_block =
				GenericAsset::free_balance(SPENDING_ASSET_ID, &author_stash_id);

			// Let's go through the first stage of executing the block
			Executive::initialize_block(&header);

			// initializing the last block should have advanced the session and thus changed the era
			assert_eq!(Staking::current_era(), 1);

			// If the offended validator is chilled, in the new era, there should be one less elected validators than before
			assert_eq!(Staking::current_elected().len(), validators.len() - 1);

			// There should be a reward calculated for the author even though the author is chilled
			assert!(
				GenericAsset::free_balance(SPENDING_ASSET_ID, &author_stash_id)
					> author_stash_balance_before_adding_block
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
		.build()
		.execute_with(|| {
			// Initially treasury has no CENNZ
			assert!(GenericAsset::free_balance(CENNZ_ASSET_ID, &Treasury::account_id()).is_zero());

			// validators[0].0 is the stash account of the first validator
			let offender = &validators[0].0;
			// Simulate a slash-able offence on validator[0]
			let offence = OffenceDetails {
				offender: (offender.clone(), Staking::stakers(&offender)),
				reporters: vec![],
			};
			let slash_fraction = Perbill::from_percent(100);
			assert_ok!(Staking::on_offence(
				&[offence],
				&[slash_fraction],
				Staking::current_era_start_session_index(),
			));

			// Fast-forward eras so that the slash is applied
			start_era(SlashDeferDuration::get() + 1);

			// Treasury should receive all of validator[0]'s stake
			assert_eq!(
				GenericAsset::free_balance(CENNZ_ASSET_ID, &Treasury::account_id()),
				initial_balance
			);
			assert!(GenericAsset::free_balance(CENNZ_ASSET_ID, &offender).is_zero());
		});
}

#[test]
fn slashed_cennz_goes_to_reporter() {
	let validators: Vec<AuthorityKeys> = make_authority_keys(6);
	let initial_balance = 1_000 * DOLLARS;
	ExtBuilder::default()
		.initial_authorities(validators.as_slice())
		.initial_balance(initial_balance)
		.stash(initial_balance)
		.build()
		.execute_with(|| {
			// Initially treasury has no CENNZ
			assert!(GenericAsset::free_balance(CENNZ_ASSET_ID, &Treasury::account_id()).is_zero());
			let offender = &validators[0].0;
			let reporter = &validators[1].0;

			// Simulate a slash-able offence on validator[0]
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
			let total_slash = slash_fraction * initial_balance;

			// Fast-forward eras so that the slash is applied
			start_era(SlashDeferDuration::get() + 1);
			// offender CENNZ funds are fully slashed
			assert_eq!(
				GenericAsset::free_balance(CENNZ_ASSET_ID, &offender),
				initial_balance - total_slash
			);
			// reporter is paid a CENNZ reporter's fee
			let reporter_fee = (Staking::slash_reward_fraction() * total_slash) / 2;
			assert_eq!(
				GenericAsset::free_balance(CENNZ_ASSET_ID, &reporter),
				initial_balance + reporter_fee
			);
			// Treasury should receive remainder of slash after the CENNZ reporter's fee
			assert_eq!(
				GenericAsset::free_balance(CENNZ_ASSET_ID, &Treasury::account_id()),
				total_slash - reporter_fee
			);
		});
}
