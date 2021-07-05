// This file is part of Substrate.

// Copyright (C) 2017-2020 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Tests for the module.

use super::*;
use frame_election_provider_support::Support;
use frame_support::{
	assert_noop, assert_ok,
	traits::{Currency, OnInitialize, ReservableCurrency},
	StorageMap,
};
use mock::*;
use pallet_balances::Error as BalancesError;
use sp_runtime::traits::BadOrigin;
use sp_staking::offence::OffenceDetails;
use substrate_test_utils::assert_eq_uvec;

#[test]
fn active_era_advances() {
	ExtBuilder::default().build_and_execute(|| {
		start_active_era(1);
		start_active_era(2);
		start_active_era(3);
	})
}

#[test]
fn force_unstake_works() {
	ExtBuilder::default().build_and_execute(|| {
		// Account 11 is stashed and locked, and account 10 is the controller
		assert_eq!(Staking::bonded(&11), Some(10));
		// Adds 2 slashing spans
		add_slash(&11);
		// Cant transfer
		assert_noop!(
			Balances::transfer(Origin::signed(11), 1, 10),
			BalancesError::<Test, _>::LiquidityRestrictions
		);
		// Force unstake requires root.
		assert_noop!(Staking::force_unstake(Origin::signed(11), 11), BadOrigin);
		// We now force them to unstake
		assert_ok!(Staking::force_unstake(Origin::root(), 11));
		// No longer bonded.
		assert_eq!(Staking::bonded(&11), None);
		// Transfer works.
		assert_ok!(Balances::transfer(Origin::signed(11), 1, 10));
	});
}

#[test]
fn kill_stash_works() {
	ExtBuilder::default().build_and_execute(|| {
		// Account 11 is stashed and locked, and account 10 is the controller
		assert_eq!(Staking::bonded(&11), Some(10));
		// Adds 2 slashing spans
		add_slash(&11);
		// Only can kill a stash account
		assert_noop!(Staking::kill_stash(&12), Error::<Test>::NotStash);
		// Correct inputs, everything works
		assert_ok!(Staking::kill_stash(&11));
		// No longer bonded.
		assert_eq!(Staking::bonded(&11), None);
	});
}

#[test]
fn basic_setup_works() {
	// Verifies initial conditions of mock
	ExtBuilder::default().build_and_execute(|| {
		// Account 11 is stashed and locked, and account 10 is the controller
		assert_eq!(Staking::bonded(&11), Some(10));
		// Account 21 is stashed and locked, and account 20 is the controller
		assert_eq!(Staking::bonded(&21), Some(20));
		// Account 1 is not a stashed
		assert_eq!(Staking::bonded(&1), None);

		// Account 10 controls the stash from account 11, which is 100 * balance_factor units
		assert_eq!(
			Staking::ledger(&10),
			Some(StakingLedger {
				stash: 11,
				total: 1000,
				active: 1000,
				unlocking: vec![]
			})
		);
		// Account 20 controls the stash from account 21, which is 200 * balance_factor units
		assert_eq!(
			Staking::ledger(&20),
			Some(StakingLedger {
				stash: 21,
				total: 1000,
				active: 1000,
				unlocking: vec![]
			})
		);
		// Account 1 does not control any stash
		assert_eq!(Staking::ledger(&1), None);

		// ValidatorPrefs are default
		assert_eq_uvec!(
			<Validators<Test>>::iter().collect::<Vec<_>>(),
			vec![
				(31, ValidatorPrefs::default()),
				(21, ValidatorPrefs::default()),
				(11, ValidatorPrefs::default())
			]
		);

		assert_eq!(
			Staking::ledger(100),
			Some(StakingLedger {
				stash: 101,
				total: 500,
				active: 500,
				unlocking: vec![]
			})
		);
		assert_eq!(Staking::nominators(101).unwrap().targets, vec![11, 21]);

		assert_eq!(
			Staking::eras_stakers(Staking::active_era().unwrap().index, 11),
			Exposure {
				total: 1125,
				own: 1000,
				others: vec![IndividualExposure { who: 101, value: 125 }]
			},
		);
		assert_eq!(
			Staking::eras_stakers(Staking::active_era().unwrap().index, 21),
			Exposure {
				total: 1375,
				own: 1000,
				others: vec![IndividualExposure { who: 101, value: 375 }]
			},
		);

		// initial total stake = 1125 + 1375
		assert_eq!(Staking::eras_total_stake(Staking::active_era().unwrap().index), 2500);

		// The number of validators required.
		assert_eq!(Staking::validator_count(), 2);

		// Initial Era and session
		assert_eq!(Staking::active_era().unwrap().index, 0);

		// Account 10 has `balance_factor` free balance
		assert_eq!(Balances::free_balance(10), 1);
		assert_eq!(Balances::free_balance(10), 1);

		// New era is not being forced
		assert_eq!(Staking::force_era(), Forcing::NotForcing);
	});
}

#[test]
fn change_controller_works() {
	ExtBuilder::default().build_and_execute(|| {
		// 10 and 11 are bonded as stash controller.
		assert_eq!(Staking::bonded(&11), Some(10));

		// 10 can control 11 who is initially a validator.
		assert_ok!(Staking::chill(Origin::signed(10)));

		// change controller
		assert_ok!(Staking::set_controller(Origin::signed(11), 5));
		assert_eq!(Staking::bonded(&11), Some(5));
		mock::start_active_era(1);

		// 10 is no longer in control.
		assert_noop!(
			Staking::validate(Origin::signed(10), ValidatorPrefs::default()),
			Error::<Test>::NotController,
		);
		assert_ok!(Staking::validate(Origin::signed(5), ValidatorPrefs::default()));
	})
}

#[test]
fn staking_should_work() {
	ExtBuilder::default()
		.nominate(false)
		.fair(false) // to give 20 more staked value
		.build()
		.execute_with(|| {
			// remember + compare this along with the test.
			assert_eq_uvec!(validator_controllers(), vec![20, 10]);

			// put some money in account that we'll use.
			for i in 1..5 {
				let _ = Balances::make_free_balance_be(&i, 2000);
			}

			// --- Block 2:
			start_session(2);
			// add a new candidate for being a validator. account 3 controlled by 4.
			assert_ok!(Staking::bond(Origin::signed(3), 4, 1500, RewardDestination::Controller));
			assert_ok!(Staking::validate(Origin::signed(4), ValidatorPrefs::default()));

			// No effects will be seen so far.
			assert_eq_uvec!(validator_controllers(), vec![20, 10]);

			// --- Block 3:
			start_session(3);

			// No effects will be seen so far. Era has not been yet triggered.
			assert_eq_uvec!(validator_controllers(), vec![20, 10]);

			// --- Block 4: the validators will now be queued.
			start_session(4);
			assert_eq!(Staking::active_era().unwrap().index, 1);

			// --- Block 5: the validators are still in queue.
			start_session(5);

			// --- Block 6: the validators will now be changed.
			start_session(6);

			assert_eq_uvec!(validator_controllers(), vec![20, 4]);
			// --- Block 6: Unstake 4 as a validator, freeing up the balance stashed in 3
			// 4 will chill
			Staking::chill(Origin::signed(4)).unwrap();

			// --- Block 7: nothing. 4 is still there.
			start_session(7);
			assert_eq_uvec!(validator_controllers(), vec![20, 4]);

			// --- Block 8:
			start_session(8);

			// --- Block 9: 4 will not be a validator.
			start_session(9);
			assert_eq_uvec!(validator_controllers(), vec![20, 10]);

			// Note: the stashed value of 4 is still lock
			assert_eq!(
				Staking::ledger(&4),
				Some(StakingLedger {
					stash: 3,
					total: 1500,
					active: 1500,
					unlocking: vec![],
				})
			);
			// e.g. it cannot reserve more than 500 that it has free from the total 2000
			assert_noop!(
				Balances::reserve(&3, 501),
				BalancesError::<Test, _>::LiquidityRestrictions
			);
			assert_ok!(Balances::reserve(&3, 409));
		});
}

#[test]
fn less_than_needed_candidates_works() {
	ExtBuilder::default()
		.minimum_validator_count(1)
		.validator_count(4)
		.nominate(false)
		.num_validators(3)
		.build()
		.execute_with(|| {
			assert_eq!(Staking::validator_count(), 4);
			assert_eq!(Staking::minimum_validator_count(), 1);
			assert_eq_uvec!(validator_controllers(), vec![30, 20, 10]);

			mock::start_active_era(1);

			// Previous set is selected. NO election algorithm is even executed.
			assert_eq_uvec!(validator_controllers(), vec![30, 20, 10]);

			// But the exposure is updated in a simple way. No external votes exists.
			// This is purely self-vote.
			assert!(
				ErasStakers::<Test>::iter_prefix_values(Staking::active_era().unwrap().index)
					.all(|exposure| exposure.others.is_empty())
			);
		});
}

#[test]
fn no_candidate_emergency_condition() {
	ExtBuilder::default()
		.minimum_validator_count(1)
		.validator_count(15)
		.num_validators(4)
		.validator_pool(true)
		.nominate(false)
		.build()
		.execute_with(|| {
			// initial validators
			assert_eq_uvec!(validator_controllers(), vec![10, 20, 30, 40]);
			let prefs = ValidatorPrefs {
				commission: Perbill::one(),
			};
			<Staking as crate::Store>::Validators::insert(11, prefs.clone());

			// set the minimum validator count.
			<Staking as crate::Store>::MinimumValidatorCount::put(10);

			// try to chill
			let _ = Staking::chill(Origin::signed(10));

			// trigger era
			mock::start_active_era(1);

			// Previous ones are elected. chill is invalidates. TODO: #2494
			assert_eq_uvec!(validator_controllers(), vec![10, 20, 30, 40]);
			// Though the validator preferences has been removed.
			assert!(Staking::validators(11) != prefs);
		});
}

#[test]
fn nominators_also_get_slashed_pro_rata() {
	ExtBuilder::default().build_and_execute(|| {
		mock::start_active_era(1);
		let slash_percent = Perbill::from_percent(5);
		let initial_exposure = Staking::eras_stakers(active_era(), 11);
		// 101 is a nominator for 11
		assert_eq!(initial_exposure.others.first().unwrap().who, 101,);

		// staked values;
		let nominator_stake = Staking::ledger(100).unwrap().active;
		let nominator_balance = balances(&101).0;
		let validator_stake = Staking::ledger(10).unwrap().active;
		let validator_balance = balances(&11).0;
		let exposed_stake = initial_exposure.total;
		let exposed_validator = initial_exposure.own;
		let exposed_nominator = initial_exposure.others.first().unwrap().value;

		// 11 goes offline
		on_offence_now(
			&[OffenceDetails {
				offender: (11, initial_exposure.clone()),
				reporters: vec![],
			}],
			&[slash_percent],
		);

		// both stakes must have been decreased.
		assert!(Staking::ledger(100).unwrap().active < nominator_stake);
		assert!(Staking::ledger(10).unwrap().active < validator_stake);

		let slash_amount = slash_percent * exposed_stake;
		let validator_share = Perbill::from_rational(exposed_validator, exposed_stake) * slash_amount;
		let nominator_share = Perbill::from_rational(exposed_nominator, exposed_stake) * slash_amount;

		// both slash amounts need to be positive for the test to make sense.
		assert!(validator_share > 0);
		assert!(nominator_share > 0);

		// both stakes must have been decreased pro-rata.
		assert_eq!(Staking::ledger(100).unwrap().active, nominator_stake - nominator_share,);
		assert_eq!(Staking::ledger(10).unwrap().active, validator_stake - validator_share,);
		assert_eq!(
			balances(&101).0, // free balance
			nominator_balance - nominator_share,
		);
		assert_eq!(
			balances(&11).0, // free balance
			validator_balance - validator_share,
		);
		// Because slashing happened.
		assert!(is_disabled(10));
	});
}

#[test]
fn double_staking_should_fail() {
	// should test (in the same order):
	// * an account already bonded as stash cannot be be stashed again.
	// * an account already bonded as stash cannot nominate.
	// * an account already bonded as controller can nominate.
	ExtBuilder::default().build_and_execute(|| {
		let arbitrary_value = 5;
		// 2 = controller, 1 stashed => ok
		assert_ok!(Staking::bond(
			Origin::signed(1),
			2,
			arbitrary_value,
			RewardDestination::default()
		));
		// 4 = not used so far, 1 stashed => not allowed.
		assert_noop!(
			Staking::bond(Origin::signed(1), 4, arbitrary_value, RewardDestination::default()),
			Error::<Test>::AlreadyBonded,
		);
		// 1 = stashed => attempting to nominate should fail.
		assert_noop!(
			Staking::nominate(Origin::signed(1), vec![1]),
			Error::<Test>::NotController
		);
		// 2 = controller  => nominating should work.
		assert_ok!(Staking::nominate(Origin::signed(2), vec![1]));
	});
}

#[test]
fn double_controlling_should_fail() {
	// should test (in the same order):
	// * an account already bonded as controller CANNOT be reused as the controller of another account.
	ExtBuilder::default().build_and_execute(|| {
		let arbitrary_value = 5;
		// 2 = controller, 1 stashed => ok
		assert_ok!(Staking::bond(
			Origin::signed(1),
			2,
			arbitrary_value,
			RewardDestination::default(),
		));
		// 2 = controller, 3 stashed (Note that 2 is reused.) => no-op
		assert_noop!(
			Staking::bond(Origin::signed(3), 2, arbitrary_value, RewardDestination::default()),
			Error::<Test>::AlreadyPaired,
		);
	});
}

#[test]
fn session_and_eras_work_simple() {
	ExtBuilder::default().period(1).build_and_execute(|| {
		assert_eq!(active_era(), 0);
		assert_eq!(current_era(), 0);
		assert_eq!(Session::current_index(), 1);
		assert_eq!(System::block_number(), 1);

		// Session 1: this is basically a noop. This has already been started.
		start_session(1);
		assert_eq!(Session::current_index(), 1);
		assert_eq!(active_era(), 0);
		assert_eq!(System::block_number(), 1);

		// Session 2: No change.
		start_session(2);
		assert_eq!(Session::current_index(), 2);
		assert_eq!(active_era(), 0);
		assert_eq!(System::block_number(), 2);

		// Session 3: Era increment.
		start_session(3);
		assert_eq!(Session::current_index(), 3);
		assert_eq!(active_era(), 1);
		assert_eq!(System::block_number(), 3);

		// Session 4: No change.
		start_session(4);
		assert_eq!(Session::current_index(), 4);
		assert_eq!(active_era(), 1);
		assert_eq!(System::block_number(), 4);

		// Session 5: No change.
		start_session(5);
		assert_eq!(Session::current_index(), 5);
		assert_eq!(active_era(), 1);
		assert_eq!(System::block_number(), 5);

		// Session 6: Era increment.
		start_session(6);
		assert_eq!(Session::current_index(), 6);
		assert_eq!(active_era(), 2);
		assert_eq!(System::block_number(), 6);
	});
}

#[test]
fn session_and_eras_work_complex() {
	ExtBuilder::default().period(5).build_and_execute(|| {
		assert_eq!(active_era(), 0);
		assert_eq!(Session::current_index(), 0);
		assert_eq!(System::block_number(), 1);

		start_session(1);
		assert_eq!(Session::current_index(), 1);
		assert_eq!(active_era(), 0);
		assert_eq!(System::block_number(), 5);

		start_session(2);
		assert_eq!(Session::current_index(), 2);
		assert_eq!(active_era(), 0);
		assert_eq!(System::block_number(), 10);

		start_session(3);
		assert_eq!(Session::current_index(), 3);
		assert_eq!(active_era(), 1);
		assert_eq!(System::block_number(), 15);

		start_session(4);
		assert_eq!(Session::current_index(), 4);
		assert_eq!(active_era(), 1);
		assert_eq!(System::block_number(), 20);

		start_session(5);
		assert_eq!(Session::current_index(), 5);
		assert_eq!(active_era(), 1);
		assert_eq!(System::block_number(), 25);

		start_session(6);
		assert_eq!(Session::current_index(), 6);
		assert_eq!(active_era(), 2);
		assert_eq!(System::block_number(), 30);
	});
}

#[test]
fn forcing_new_era_works() {
	ExtBuilder::default().build_and_execute(|| {
		// normal flow of session.
		start_session(1);
		assert_eq!(active_era(), 0);

		start_session(2);
		assert_eq!(active_era(), 0);

		start_session(3);
		assert_eq!(active_era(), 1);

		// no era change.
		ForceEra::put(Forcing::ForceNone);

		start_session(4);
		assert_eq!(active_era(), 1);
		assert!(!Staking::was_end_era_forced());

		start_session(5);
		assert_eq!(active_era(), 1);

		start_session(6);
		assert_eq!(active_era(), 1);

		start_session(7);
		assert_eq!(active_era(), 1);

		// back to normal.
		// this immediately starts a new session.
		ForceEra::put(Forcing::NotForcing);

		start_session(8);
		assert_eq!(active_era(), 1);

		start_session(9);
		assert_eq!(active_era(), 2);
		// forceful change
		ForceEra::put(Forcing::ForceAlways);

		start_session(10);
		assert_eq!(active_era(), 2);
		assert!(Staking::was_end_era_forced());

		start_session(11);
		assert_eq!(active_era(), 3);
		assert!(Staking::was_end_era_forced());

		start_session(12);
		assert_eq!(active_era(), 4);
		assert!(Staking::was_end_era_forced());

		// just one forceful change
		ForceEra::put(Forcing::ForceNew);
		start_session(13);
		assert_eq!(active_era(), 5);
		assert!(Staking::was_end_era_forced());
		assert_eq!(ForceEra::get(), Forcing::NotForcing);

		start_session(14);
		assert_eq!(active_era(), 6);
		assert!(!Staking::was_end_era_forced());

		start_session(15);
		assert_eq!(active_era(), 6);
	});
}

#[test]
fn cannot_transfer_staked_balance() {
	// Tests that a stash account cannot transfer funds
	ExtBuilder::default().nominate(false).build_and_execute(|| {
		// Confirm account 11 is stashed
		assert_eq!(Staking::bonded(&11), Some(10));
		// Confirm account 11 has some free balance
		assert_eq!(Balances::free_balance(11), 1000);
		// Confirm account 11 (via controller 10) is totally staked
		assert_eq!(Staking::eras_stakers(active_era(), 11).total, 1000);
		// Confirm account 11 cannot transfer as a result
		assert_noop!(
			Balances::transfer(Origin::signed(11), 20, 1),
			BalancesError::<Test, _>::LiquidityRestrictions
		);

		// Give account 11 extra free balance
		let _ = Balances::make_free_balance_be(&11, 10000);
		// Confirm that account 11 can now transfer some balance
		assert_ok!(Balances::transfer(Origin::signed(11), 20, 1));
	});
}

#[test]
fn cannot_transfer_staked_balance_2() {
	// Tests that a stash account cannot transfer funds
	// Same test as above but with 20, and more accurate.
	// 21 has 2000 free balance but 1000 at stake
	ExtBuilder::default().nominate(false).fair(true).build_and_execute(|| {
		// Confirm account 21 is stashed
		assert_eq!(Staking::bonded(&21), Some(20));
		// Confirm account 21 has some free balance
		assert_eq!(Balances::free_balance(21), 2000);
		// Confirm account 21 (via controller 20) is totally staked
		assert_eq!(
			Staking::eras_stakers(Staking::active_era().unwrap().index, 21).total,
			1000
		);
		// Confirm account 21 can transfer at most 1000
		assert_noop!(
			Balances::transfer(Origin::signed(21), 20, 1001),
			BalancesError::<Test, _>::LiquidityRestrictions
		);
		assert_ok!(Balances::transfer(Origin::signed(21), 20, 1000));
	});
}

#[test]
fn cannot_reserve_staked_balance() {
	// Checks that a bonded account cannot reserve balance from free balance
	ExtBuilder::default().build_and_execute(|| {
		// Confirm account 11 is stashed
		assert_eq!(Staking::bonded(&11), Some(10));
		// Confirm account 11 has some free balance
		assert_eq!(Balances::free_balance(11), 1000);
		// Confirm account 11 (via controller 10) is totally staked
		assert_eq!(
			Staking::eras_stakers(Staking::active_era().unwrap().index, 11).own,
			1000
		);
		// Confirm account 11 cannot reserve as a result
		assert_noop!(
			Balances::reserve(&11, 1),
			BalancesError::<Test, _>::LiquidityRestrictions,
		);

		// Give account 11 extra free balance
		let _ = Balances::make_free_balance_be(&11, 10000);
		// Confirm account 11 can now reserve balance
		assert_ok!(Balances::reserve(&11, 1));
	});
}

#[test]
fn bond_extra_works() {
	// Tests that extra `free_balance` in the stash can be added to stake
	// NOTE: this tests only verifies `StakingLedger` for correct updates
	// See `bond_extra_and_withdraw_unbonded_works` for more details and updates on `Exposure`.
	ExtBuilder::default().build_and_execute(|| {
		// Check that account 10 is a validator
		assert!(<Validators<Test>>::contains_key(11));
		// Check that account 10 is bonded to account 11
		assert_eq!(Staking::bonded(&11), Some(10));
		// Check how much is at stake
		assert_eq!(
			Staking::ledger(&10),
			Some(StakingLedger {
				stash: 11,
				total: 1000,
				active: 1000,
				unlocking: vec![],
			})
		);

		// Give account 11 some large free balance greater than total
		let _ = Balances::make_free_balance_be(&11, 1000000);

		// Call the bond_extra function from controller, add only 100
		assert_ok!(Staking::bond_extra(Origin::signed(11), 100));
		// There should be 100 more `total` and `active` in the ledger
		assert_eq!(
			Staking::ledger(&10),
			Some(StakingLedger {
				stash: 11,
				total: 1000 + 100,
				active: 1000 + 100,
				unlocking: vec![],
			})
		);

		// Call the bond_extra function with a large number, should handle it
		assert_ok!(Staking::bond_extra(Origin::signed(11), Balance::max_value()));
		// The full amount of the funds should now be in the total and active
		assert_eq!(
			Staking::ledger(&10),
			Some(StakingLedger {
				stash: 11,
				total: 1000000,
				active: 1000000,
				unlocking: vec![],
			})
		);
	});
}

#[test]
fn bond_extra_and_withdraw_unbonded_works() {
	// * Should test
	// * Given an account being bonded [and chosen as a validator](not mandatory)
	// * It can add extra funds to the bonded account.
	// * it can unbond a portion of its funds from the stash account.
	// * Once the unbonding period is done, it can actually take the funds out of the stash.
	ExtBuilder::default().nominate(false).build_and_execute(|| {
		// Set payee to controller. avoids confusion
		assert_ok!(Staking::set_payee(Origin::signed(10), RewardDestination::Controller));

		// Give account 11 some large free balance greater than total
		let _ = Balances::make_free_balance_be(&11, 1000000);

		// Initial config should be correct
		assert_eq!(Staking::active_era().unwrap().index, 0);

		// check the balance of a validator accounts.
		assert_eq!(Balances::total_balance(&10), 1);

		// confirm that 10 is a normal validator and gets paid at the end of the era.
		mock::start_active_era(1);

		// Initial state of 10
		assert_eq!(
			Staking::ledger(&10),
			Some(StakingLedger {
				stash: 11,
				total: 1000,
				active: 1000,
				unlocking: vec![],
			})
		);
		assert_eq!(
			Staking::eras_stakers(Staking::active_era().unwrap().index, 11),
			Exposure {
				total: 1000,
				own: 1000,
				others: vec![]
			}
		);

		// deposit the extra 100 units
		Staking::bond_extra(Origin::signed(11), 100).unwrap();

		assert_eq!(
			Staking::ledger(&10),
			Some(StakingLedger {
				stash: 11,
				total: 1000 + 100,
				active: 1000 + 100,
				unlocking: vec![],
			})
		);
		// Exposure is a snapshot! only updated after the next era update.
		assert_ne!(
			Staking::eras_stakers(Staking::active_era().unwrap().index, 11),
			Exposure {
				total: 1000 + 100,
				own: 1000 + 100,
				others: vec![]
			}
		);

		// trigger next era.
		mock::start_active_era(2);
		assert_eq!(Staking::active_era().unwrap().index, 2);

		// ledger should be the same.
		assert_eq!(
			Staking::ledger(&10),
			Some(StakingLedger {
				stash: 11,
				total: 1000 + 100,
				active: 1000 + 100,
				unlocking: vec![],
			})
		);
		// Exposure is now updated.
		assert_eq!(
			Staking::eras_stakers(Staking::active_era().unwrap().index, 11),
			Exposure {
				total: 1000 + 100,
				own: 1000 + 100,
				others: vec![]
			}
		);

		// Unbond almost all of the funds in stash.
		Staking::unbond(Origin::signed(10), 1000).unwrap();
		assert_eq!(
			Staking::ledger(&10),
			Some(StakingLedger {
				stash: 11,
				total: 1000 + 100,
				active: 100,
				unlocking: vec![UnlockChunk {
					value: 1000,
					era: 2 + 3
				}],
			}),
		);

		// Attempting to free the balances now will fail. 2 eras need to pass.
		assert_ok!(Staking::withdraw_unbonded(Origin::signed(10)));
		assert_eq!(
			Staking::ledger(&10),
			Some(StakingLedger {
				stash: 11,
				total: 1000 + 100,
				active: 100,
				unlocking: vec![UnlockChunk {
					value: 1000,
					era: 2 + 3
				}],
			}),
		);

		// trigger next era.
		mock::start_active_era(3);

		// nothing yet
		assert_ok!(Staking::withdraw_unbonded(Origin::signed(10)));
		assert_eq!(
			Staking::ledger(&10),
			Some(StakingLedger {
				stash: 11,
				total: 1000 + 100,
				active: 100,
				unlocking: vec![UnlockChunk {
					value: 1000,
					era: 2 + 3
				}],
			}),
		);

		// trigger next era.
		mock::start_active_era(5);

		assert_ok!(Staking::withdraw_unbonded(Origin::signed(10)));
		// Now the value is free and the staking ledger is updated.
		assert_eq!(
			Staking::ledger(&10),
			Some(StakingLedger {
				stash: 11,
				total: 100,
				active: 100,
				unlocking: vec![],
			}),
		);
	})
}

#[test]
fn too_many_unbond_calls_should_not_work() {
	ExtBuilder::default().build_and_execute(|| {
		// locked at era 0 until 3
		for _ in 0..MAX_UNLOCKING_CHUNKS - 1 {
			assert_ok!(Staking::unbond(Origin::signed(10), 1));
		}

		mock::start_active_era(1);

		// locked at era 1 until 4
		assert_ok!(Staking::unbond(Origin::signed(10), 1));
		// can't do more.
		assert_noop!(Staking::unbond(Origin::signed(10), 1), Error::<Test>::NoMoreChunks);

		mock::start_active_era(3);

		assert_noop!(Staking::unbond(Origin::signed(10), 1), Error::<Test>::NoMoreChunks);
		// free up.
		assert_ok!(Staking::withdraw_unbonded(Origin::signed(10)));

		// Can add again.
		assert_ok!(Staking::unbond(Origin::signed(10), 1));
		assert_eq!(Staking::ledger(&10).unwrap().unlocking.len(), 2);
	})
}

#[test]
fn rebond_works() {
	// * Should test
	// * Given an account being bonded [and chosen as a validator](not mandatory)
	// * it can unbond a portion of its funds from the stash account.
	// * it can re-bond a portion of the funds scheduled to unlock.
	ExtBuilder::default().nominate(false).build().execute_with(|| {
		// Set payee to controller. avoids confusion
		assert_ok!(Staking::set_payee(Origin::signed(10), RewardDestination::Controller));

		// Give account 11 some large free balance greater than total
		let _ = Balances::make_free_balance_be(&11, 1000000);

		// confirm that 10 is a normal validator and gets paid at the end of the era.
		mock::start_active_era(1);

		// Initial state of 10
		assert_eq!(
			Staking::ledger(&10),
			Some(StakingLedger {
				stash: 11,
				total: 1000,
				active: 1000,
				unlocking: vec![],
			})
		);

		mock::start_active_era(2);
		assert_eq!(Staking::active_era().unwrap().index, 2);

		// Try to rebond some funds. We get an error since no fund is unbonded.
		assert_noop!(Staking::rebond(Origin::signed(10), 500), Error::<Test>::NoUnlockChunk,);

		// Unbond almost all of the funds in stash.
		Staking::unbond(Origin::signed(10), 900).unwrap();
		assert_eq!(
			Staking::ledger(&10),
			Some(StakingLedger {
				stash: 11,
				total: 1000,
				active: 100,
				unlocking: vec![UnlockChunk { value: 900, era: 2 + 3 }],
			})
		);

		// Re-bond all the funds unbonded.
		Staking::rebond(Origin::signed(10), 900).unwrap();
		assert_eq!(
			Staking::ledger(&10),
			Some(StakingLedger {
				stash: 11,
				total: 1000,
				active: 1000,
				unlocking: vec![],
			})
		);

		// Unbond almost all of the funds in stash.
		Staking::unbond(Origin::signed(10), 900).unwrap();
		assert_eq!(
			Staking::ledger(&10),
			Some(StakingLedger {
				stash: 11,
				total: 1000,
				active: 100,
				unlocking: vec![UnlockChunk { value: 900, era: 5 }],
			})
		);

		// Re-bond part of the funds unbonded.
		Staking::rebond(Origin::signed(10), 500).unwrap();
		assert_eq!(
			Staking::ledger(&10),
			Some(StakingLedger {
				stash: 11,
				total: 1000,
				active: 600,
				unlocking: vec![UnlockChunk { value: 400, era: 5 }],
			})
		);

		// Re-bond the remainder of the funds unbonded.
		Staking::rebond(Origin::signed(10), 500).unwrap();
		assert_eq!(
			Staking::ledger(&10),
			Some(StakingLedger {
				stash: 11,
				total: 1000,
				active: 1000,
				unlocking: vec![],
			})
		);

		// Unbond parts of the funds in stash.
		Staking::unbond(Origin::signed(10), 300).unwrap();
		Staking::unbond(Origin::signed(10), 300).unwrap();
		Staking::unbond(Origin::signed(10), 300).unwrap();
		assert_eq!(
			Staking::ledger(&10),
			Some(StakingLedger {
				stash: 11,
				total: 1000,
				active: 100,
				unlocking: vec![
					UnlockChunk { value: 300, era: 5 },
					UnlockChunk { value: 300, era: 5 },
					UnlockChunk { value: 300, era: 5 },
				],
			})
		);

		// Re-bond part of the funds unbonded.
		Staking::rebond(Origin::signed(10), 500).unwrap();
		assert_eq!(
			Staking::ledger(&10),
			Some(StakingLedger {
				stash: 11,
				total: 1000,
				active: 600,
				unlocking: vec![UnlockChunk { value: 300, era: 5 }, UnlockChunk { value: 100, era: 5 },],
			})
		);
	})
}

#[test]
fn rebond_is_fifo() {
	// Rebond should proceed by reversing the most recent bond operations.
	ExtBuilder::default().nominate(false).build().execute_with(|| {
		// Set payee to controller. avoids confusion
		assert_ok!(Staking::set_payee(Origin::signed(10), RewardDestination::Controller));

		// Give account 11 some large free balance greater than total
		let _ = Balances::make_free_balance_be(&11, 1000000);

		// confirm that 10 is a normal validator and gets paid at the end of the era.
		mock::start_active_era(1);

		// Initial state of 10
		assert_eq!(
			Staking::ledger(&10),
			Some(StakingLedger {
				stash: 11,
				total: 1000,
				active: 1000,
				unlocking: vec![],
			})
		);

		mock::start_active_era(2);

		// Unbond some of the funds in stash.
		Staking::unbond(Origin::signed(10), 400).unwrap();
		assert_eq!(
			Staking::ledger(&10),
			Some(StakingLedger {
				stash: 11,
				total: 1000,
				active: 600,
				unlocking: vec![UnlockChunk { value: 400, era: 2 + 3 },],
			})
		);

		mock::start_active_era(3);

		// Unbond more of the funds in stash.
		Staking::unbond(Origin::signed(10), 300).unwrap();
		assert_eq!(
			Staking::ledger(&10),
			Some(StakingLedger {
				stash: 11,
				total: 1000,
				active: 300,
				unlocking: vec![
					UnlockChunk { value: 400, era: 2 + 3 },
					UnlockChunk { value: 300, era: 3 + 3 },
				],
			})
		);

		mock::start_active_era(4);

		// Unbond yet more of the funds in stash.
		Staking::unbond(Origin::signed(10), 200).unwrap();
		assert_eq!(
			Staking::ledger(&10),
			Some(StakingLedger {
				stash: 11,
				total: 1000,
				active: 100,
				unlocking: vec![
					UnlockChunk { value: 400, era: 2 + 3 },
					UnlockChunk { value: 300, era: 3 + 3 },
					UnlockChunk { value: 200, era: 4 + 3 },
				],
			})
		);

		// Re-bond half of the unbonding funds.
		Staking::rebond(Origin::signed(10), 400).unwrap();
		assert_eq!(
			Staking::ledger(&10),
			Some(StakingLedger {
				stash: 11,
				total: 1000,
				active: 500,
				unlocking: vec![
					UnlockChunk { value: 400, era: 2 + 3 },
					UnlockChunk { value: 100, era: 3 + 3 },
				],
			})
		);
	})
}

#[test]
fn on_free_balance_zero_stash_removes_validator() {
	// Tests that validator storage items are cleaned up when stash is empty
	// Tests that storage items are untouched when controller is empty
	ExtBuilder::default()
		.existential_deposit(10)
		.minimum_bond(10)
		.build_and_execute(|| {
			// Check the balance of the validator account
			assert_eq!(Balances::free_balance(10), 256);
			// Check the balance of the stash account
			assert_eq!(Balances::free_balance(11), 256000);
			// Check these two accounts are bonded
			assert_eq!(Staking::bonded(&11), Some(10));

			// Set some storage items which we expect to be cleaned up
			// Set payee information
			assert_ok!(Staking::set_payee(Origin::signed(10), RewardDestination::Stash));

			// Check storage items that should be cleaned up
			assert!(<Ledger<Test>>::contains_key(&10));
			assert!(<Bonded<Test>>::contains_key(&11));
			assert!(<Validators<Test>>::contains_key(&11));
			assert_eq!(<<Test as Config>::Rewarder as HandlePayee>::payee(&11), 11);

			// Reduce free_balance of controller to 0
			let _ = Balances::slash(&10, Balance::max_value());

			// Check the balance of the stash account has not been touched
			assert_eq!(Balances::free_balance(11), 256000);
			// Check these two accounts are still bonded
			assert_eq!(Staking::bonded(&11), Some(10));

			// Check storage items have not changed
			assert!(<Ledger<Test>>::contains_key(&10));
			assert!(<Bonded<Test>>::contains_key(&11));
			assert!(<Validators<Test>>::contains_key(&11));
			assert_eq!(<<Test as Config>::Rewarder as HandlePayee>::payee(&11), 11);

			// Reduce free_balance of stash to 0
			let _ = Balances::slash(&11, Balance::max_value());
			// Check total balance of stash. It should be equal to the existential deposit.
			assert_eq!(Balances::total_balance(&11), 10);

			// Reap the stash
			assert_ok!(Staking::reap_stash(Origin::none(), 11));

			// Check storage items do not exist
			assert!(!<Ledger<Test>>::contains_key(&10));
			assert!(!<Bonded<Test>>::contains_key(&11));
			assert!(!<Validators<Test>>::contains_key(&11));
			assert!(!<Nominators<Test>>::contains_key(&11));
			// payee is removed (managed by rewards module)
		});
}

#[test]
fn on_free_balance_zero_stash_removes_nominator() {
	// Tests that nominator storage items are cleaned up when stash is empty
	// Tests that storage items are untouched when controller is empty
	ExtBuilder::default()
		.existential_deposit(10)
		.minimum_bond(10)
		.build_and_execute(|| {
			// Make 10 a nominator
			assert_ok!(Staking::nominate(Origin::signed(10), vec![20]));
			// Check that account 10 is a nominator
			assert!(<Nominators<Test>>::contains_key(11));
			// Check the balance of the nominator account
			assert_eq!(Balances::free_balance(10), 256);
			// Check the balance of the stash account
			assert_eq!(Balances::free_balance(11), 256000);

			// Set payee information
			assert_ok!(Staking::set_payee(Origin::signed(10), RewardDestination::Stash));

			// Check storage items that should be cleaned up
			assert!(<Ledger<Test>>::contains_key(&10));
			assert!(<Bonded<Test>>::contains_key(&11));
			assert!(<Nominators<Test>>::contains_key(&11));
			assert_eq!(<<Test as Config>::Rewarder as HandlePayee>::payee(&11), 11);

			// Reduce free_balance of controller to 0
			let _ = Balances::slash(&10, Balance::max_value());
			// Check total balance of account 10
			assert_eq!(Balances::total_balance(&10), 0);

			// Check the balance of the stash account has not been touched
			assert_eq!(Balances::free_balance(11), 256000);
			// Check these two accounts are still bonded
			assert_eq!(Staking::bonded(&11), Some(10));

			// Check storage items have not changed
			assert!(<Ledger<Test>>::contains_key(&10));
			assert!(<Bonded<Test>>::contains_key(&11));
			assert!(<Nominators<Test>>::contains_key(&11));
			assert_eq!(<<Test as Config>::Rewarder as HandlePayee>::payee(&11), 11);

			// Reduce free_balance of stash to 0
			let _ = Balances::slash(&11, Balance::max_value());
			// Check total balance of stash. Only the minimum balance equal to existential_deposit should remain.
			assert_eq!(Balances::total_balance(&11), 10);

			// Reap the stash
			assert_ok!(Staking::reap_stash(Origin::none(), 11));

			// Check storage items do not exist
			assert!(!<Ledger<Test>>::contains_key(&10));
			assert!(!<Bonded<Test>>::contains_key(&11));
			assert!(!<Validators<Test>>::contains_key(&11));
			// payee is removed (managed by rewards module)
		});
}

#[test]
fn switching_roles() {
	// Test that it should be possible to switch between roles (nominator, validator, idle) with minimal overhead.
	ExtBuilder::default().nominate(false).build_and_execute(|| {
		// Reset reward destination
		for i in &[10, 20] {
			assert_ok!(Staking::set_payee(Origin::signed(*i), RewardDestination::Controller));
		}

		assert_eq_uvec!(validator_controllers(), vec![20, 10]);

		// put some money in account that we'll use.
		for i in 1..7 {
			let _ = Balances::deposit_creating(&i, 5000);
		}

		// add 2 nominators
		assert_ok!(Staking::bond(Origin::signed(1), 2, 2000, RewardDestination::Controller));
		assert_ok!(Staking::nominate(Origin::signed(2), vec![11, 5]));

		assert_ok!(Staking::bond(Origin::signed(3), 4, 500, RewardDestination::Controller));
		assert_ok!(Staking::nominate(Origin::signed(4), vec![21, 1]));

		// add a new validator candidate
		assert_ok!(Staking::bond(Origin::signed(5), 6, 1000, RewardDestination::Controller));
		assert_ok!(Staking::validate(Origin::signed(6), ValidatorPrefs::default()));

		mock::start_active_era(1);

		// with current nominators 10 and 5 have the most stake
		assert_eq_uvec!(validator_controllers(), vec![6, 10]);

		// 2 decides to be a validator. Consequences:
		assert_ok!(Staking::validate(Origin::signed(2), ValidatorPrefs::default()));
		// new stakes:
		// 10: 1000 self vote
		// 20: 1000 self vote + 250 vote
		// 6 : 1000 self vote
		// 2 : 2000 self vote + 250 vote.
		// Winners: 20 and 2

		mock::start_active_era(2);

		assert_eq_uvec!(validator_controllers(), vec![2, 20]);
	});
}

#[test]
fn wrong_vote_is_null() {
	ExtBuilder::default()
		.nominate(false)
		.validator_pool(true)
		.build_and_execute(|| {
			assert_eq_uvec!(validator_controllers(), vec![40, 30]);

			// put some money in account that we'll use.
			for i in 1..3 {
				let _ = Balances::deposit_creating(&i, 5000);
			}

			// add 1 nominators
			assert_ok!(Staking::bond(Origin::signed(1), 2, 2000, RewardDestination::default()));
			assert_ok!(Staking::nominate(
				Origin::signed(2),
				vec![
					11, 21, // good votes
					1, 2, 15, 1000, 25 // crap votes. No effect.
				]
			));

			// new block
			mock::start_active_era(1);

			assert_eq_uvec!(validator_controllers(), vec![20, 10]);
		});
}

#[test]
fn bond_with_no_staked_value() {
	// Behavior when someone bonds with no staked value.
	// Particularly when she votes and the candidate is elected.
	ExtBuilder::default()
		.validator_count(3)
		.minimum_bond(5)
		.nominate(false)
		.minimum_validator_count(1)
		.build()
		.execute_with(|| {
			// Can't bond with 1
			assert_noop!(
				Staking::bond(Origin::signed(1), 2, 1, RewardDestination::Controller),
				Error::<Test>::InsufficientBond,
			);
			// bonded with absolute minimum value possible.
			assert_ok!(Staking::bond(Origin::signed(1), 2, 5, RewardDestination::Controller));
			assert_eq!(Balances::locks(&1)[0].amount, 5);

			// unbonding even 1 will cause all to be unbonded.
			assert_ok!(Staking::unbond(Origin::signed(2), 1));
			assert_eq!(
				Staking::ledger(2),
				Some(StakingLedger {
					stash: 1,
					active: 0,
					total: 5,
					unlocking: vec![UnlockChunk { value: 5, era: 3 }],
				})
			);

			mock::start_active_era(1);
			mock::start_active_era(2);

			// not yet removed.
			assert_ok!(Staking::withdraw_unbonded(Origin::signed(2)));
			assert!(Staking::ledger(2).is_some());
			assert_eq!(Balances::locks(&1)[0].amount, 5);

			mock::start_active_era(3);

			// poof. Account 1 is removed from the staking system.
			assert_ok!(Staking::withdraw_unbonded(Origin::signed(2)));
			assert!(Staking::ledger(2).is_none());
			assert_eq!(Balances::locks(&1).len(), 0);
		});
}

#[test]
fn cannot_nominate_duplicates() {
	ExtBuilder::default()
		.validator_count(2)
		.nominate(false)
		.minimum_validator_count(1)
		.build()
		.execute_with(|| {
			// disable the nominator
			assert_ok!(Staking::chill(Origin::signed(100)));
			// make stakes equal.
			assert_ok!(Staking::bond_extra(Origin::signed(31), 999));

			assert_eq!(
				<Validators<Test>>::iter()
					.map(|(v, _)| (v, Staking::ledger(v - 1).unwrap().total))
					.collect::<Vec<_>>(),
				vec![(31, 1000), (21, 1000), (11, 1000)],
			);
			assert!(<Nominators<Test>>::iter()
				.map(|(n, _)| n)
				.collect::<Vec<_>>()
				.is_empty());

			// give the man some money
			let initial_balance = 1000;
			for i in [1, 2, 3, 4].iter() {
				let _ = Balances::make_free_balance_be(i, initial_balance);
			}

			assert_ok!(Staking::bond(Origin::signed(1), 2, 1000, RewardDestination::Controller));
			assert_noop!(
				Staking::nominate(Origin::signed(2), vec![11, 11, 11, 21, 31]),
				Error::<Test>::DuplicateNominee
			);
		});
}

#[test]
fn bond_with_duplicate_vote_should_be_ignored_by_npos_election_elected() {
	// same as above but ensures that even when the double is being elected, everything is sane.
	ExtBuilder::default()
		.validator_count(2)
		.nominate(false)
		.minimum_validator_count(1)
		.build()
		.execute_with(|| {
			// disable the nominator
			assert_ok!(Staking::chill(Origin::signed(100)));
			// make stakes equal.
			assert_ok!(Staking::bond_extra(Origin::signed(31), 99));

			assert_eq!(
				<Validators<Test>>::iter()
					.map(|(v, _)| (v, Staking::ledger(v - 1).unwrap().total))
					.collect::<Vec<_>>(),
				vec![(31, 100), (21, 1000), (11, 1000)],
			);
			assert!(<Nominators<Test>>::iter()
				.map(|(n, _)| n)
				.collect::<Vec<_>>()
				.is_empty());

			// give the man some money
			let initial_balance = 1000;
			for i in [1, 2, 3, 4].iter() {
				let _ = Balances::make_free_balance_be(i, initial_balance);
			}

			assert_ok!(Staking::bond(Origin::signed(1), 2, 1000, RewardDestination::Controller));
			assert_ok!(Staking::nominate(Origin::signed(2), vec![11, 21, 31,]));

			assert_ok!(Staking::bond(Origin::signed(3), 4, 1000, RewardDestination::Controller));
			assert_ok!(Staking::nominate(Origin::signed(4), vec![21, 31]));

			// winners should be 21 and 31. Otherwise this election is taking duplicates into
			// account.
			let supports = <Test as Config>::ElectionProvider::elect().unwrap().0;
			assert_eq!(
				supports,
				vec![
					(
						21,
						Support {
							total: 1800,
							voters: vec![(21, 1000), (3, 400), (1, 400)]
						}
					),
					(
						31,
						Support {
							total: 2200,
							voters: vec![(31, 1000), (3, 600), (1, 600)]
						}
					)
				],
			);
		});
}

#[test]
fn new_era_elects_correct_number_of_validators() {
	ExtBuilder::default()
		.nominate(true)
		.validator_pool(true)
		.fair(true)
		.validator_count(1)
		.build()
		.execute_with(|| {
			assert_eq!(Staking::validator_count(), 1);
			assert_eq!(validator_controllers().len(), 1);

			Session::on_initialize(System::block_number());

			assert_eq!(validator_controllers().len(), 1);
		})
}

#[test]
fn phragmen_should_not_overflow() {
	ExtBuilder::default().nominate(false).build_and_execute(|| {
		// This is the maximum value that we can have as the outcome of CurrencyToVote.
		type Votes = u64;

		let _ = Staking::chill(Origin::signed(10));
		let _ = Staking::chill(Origin::signed(20));

		bond_validator(3, 2, Votes::max_value() as Balance);
		bond_validator(5, 4, Votes::max_value() as Balance);

		bond_nominator(7, 6, Votes::max_value() as Balance, vec![3, 5]);
		bond_nominator(9, 8, Votes::max_value() as Balance, vec![3, 5]);

		mock::start_active_era(1);

		assert_eq_uvec!(validator_controllers(), vec![4, 2]);

		// We can safely convert back to values within [u64, u128].
		assert!(Staking::eras_stakers(active_era(), 3).total > Votes::max_value() as Balance);
		assert!(Staking::eras_stakers(active_era(), 5).total > Votes::max_value() as Balance);
	})
}

#[test]
fn slashing_validator_does_not_overflow() {
	ExtBuilder::default().build_and_execute(|| {
		let stake = u64::max_value() as Balance * 2;
		let reward_slash = u64::max_value() as Balance * 2;

		// Assert multiplication overflows in balance arithmetic.
		assert!(stake.checked_mul(reward_slash).is_none());
		let _ = Balances::make_free_balance_be(&11, stake);
		let _ = Balances::make_free_balance_be(&2, stake);

		// only slashes out of bonded stake are applied. without this line,
		// it is 0.
		Staking::bond(Origin::signed(2), 20000, stake - 1, RewardDestination::default()).unwrap();
		// Override exposure of 11
		ErasStakers::<Test>::insert(
			0,
			11,
			Exposure {
				total: stake,
				own: 1,
				others: vec![IndividualExposure {
					who: 2,
					value: stake - 1,
				}],
			},
		);

		// Check slashing
		on_offence_now(
			&[OffenceDetails {
				offender: (11, Staking::eras_stakers(Staking::active_era().unwrap().index, 11)),
				reporters: vec![],
			}],
			&[Perbill::from_percent(100)],
		);

		assert_eq!(Balances::total_balance(&11), stake - 1);
		assert_eq!(Balances::total_balance(&2), 1);
	})
}

#[test]
fn unbonded_balance_is_not_slashable() {
	ExtBuilder::default().build_and_execute(|| {
		// total amount staked is slashable.
		assert_eq!(Staking::slashable_balance_of(&11), 1000);

		assert_ok!(Staking::unbond(Origin::signed(10), 800));

		// only the active portion.
		assert_eq!(Staking::slashable_balance_of(&11), 200);
	})
}

#[test]
fn era_is_always_same_length() {
	// This ensures that the sessions is always of the same length if there is no forcing no
	// session changes.
	ExtBuilder::default().build_and_execute(|| {
		let session_per_era = <SessionsPerEra as Get<SessionIndex>>::get();

		mock::start_active_era(1);
		assert_eq!(
			Staking::eras_start_session_index(current_era()).unwrap(),
			session_per_era
		);

		mock::start_active_era(2);
		assert_eq!(
			Staking::eras_start_session_index(current_era()).unwrap(),
			session_per_era * 2u32
		);

		let session = Session::current_index();
		ForceEra::put(Forcing::ForceNew);
		advance_session();
		advance_session();
		assert_eq!(current_era(), 3);
		assert_eq!(Staking::eras_start_session_index(current_era()).unwrap(), session + 2);

		mock::start_active_era(4);
		assert_eq!(
			Staking::eras_start_session_index(current_era()).unwrap(),
			session + 2u32 + session_per_era
		);
	});
}

#[test]
fn offence_forces_new_era() {
	ExtBuilder::default().build_and_execute(|| {
		on_offence_now(
			&[OffenceDetails {
				offender: (11, Staking::eras_stakers(Staking::active_era().unwrap().index, 11)),
				reporters: vec![],
			}],
			&[Perbill::from_percent(5)],
		);

		assert_eq!(Staking::force_era(), Forcing::ForceNew);
	});
}

#[test]
fn offence_ensures_new_era_without_clobbering() {
	ExtBuilder::default().build_and_execute(|| {
		assert_ok!(Staking::force_new_era_always(Origin::root()));
		assert_eq!(Staking::force_era(), Forcing::ForceAlways);

		on_offence_now(
			&[OffenceDetails {
				offender: (11, Staking::eras_stakers(Staking::active_era().unwrap().index, 11)),
				reporters: vec![],
			}],
			&[Perbill::from_percent(5)],
		);

		assert_eq!(Staking::force_era(), Forcing::ForceAlways);
	});
}

#[test]
fn offence_deselects_validator_even_when_slash_is_zero() {
	ExtBuilder::default().build_and_execute(|| {
		assert!(Session::validators().contains(&11));
		assert!(<Validators<Test>>::contains_key(11));

		on_offence_now(
			&[OffenceDetails {
				offender: (11, Staking::eras_stakers(Staking::active_era().unwrap().index, 11)),
				reporters: vec![],
			}],
			&[Perbill::from_percent(0)],
		);

		assert_eq!(Staking::force_era(), Forcing::ForceNew);
		assert!(!<Validators<Test>>::contains_key(11));

		mock::start_active_era(1);

		assert!(!Session::validators().contains(&11));
		assert!(!<Validators<Test>>::contains_key(11));
	});
}

#[test]
fn slashing_performed_according_exposure() {
	// This test checks that slashing is performed according the exposure (or more precisely,
	// historical exposure), not the current balance.
	ExtBuilder::default().build_and_execute(|| {
		assert_eq!(
			Staking::eras_stakers(Staking::active_era().unwrap().index, 11).own,
			1000
		);

		// Handle an offence with a historical exposure.
		on_offence_now(
			&[OffenceDetails {
				offender: (
					11,
					Exposure {
						total: 500,
						own: 500,
						others: vec![],
					},
				),
				reporters: vec![],
			}],
			&[Perbill::from_percent(50)],
		);

		// The stash account should be slashed for 250 (50% of 500).
		assert_eq!(Balances::free_balance(11), 1000 - 250);
	});
}

#[test]
fn slash_in_old_span_does_not_deselect() {
	ExtBuilder::default().build_and_execute(|| {
		mock::start_active_era(1);

		assert!(<Validators<Test>>::contains_key(11));
		assert!(Session::validators().contains(&11));

		on_offence_now(
			&[OffenceDetails {
				offender: (11, Staking::eras_stakers(Staking::active_era().unwrap().index, 11)),
				reporters: vec![],
			}],
			&[Perbill::from_percent(0)],
		);

		assert_eq!(Staking::force_era(), Forcing::ForceNew);
		assert!(!<Validators<Test>>::contains_key(11));

		mock::start_active_era(2);

		Staking::validate(Origin::signed(10), Default::default()).unwrap();
		assert_eq!(Staking::force_era(), Forcing::NotForcing);
		assert!(<Validators<Test>>::contains_key(11));
		assert!(!Session::validators().contains(&11));

		mock::start_active_era(3);

		// this staker is in a new slashing span now, having re-registered after
		// their prior slash.

		on_offence_in_era(
			&[OffenceDetails {
				offender: (11, Staking::eras_stakers(Staking::active_era().unwrap().index, 11)),
				reporters: vec![],
			}],
			&[Perbill::from_percent(0)],
			1,
		);

		// not forcing for zero-slash and previous span.
		assert_eq!(Staking::force_era(), Forcing::NotForcing);
		assert!(<Validators<Test>>::contains_key(11));
		assert!(Session::validators().contains(&11));

		on_offence_in_era(
			&[OffenceDetails {
				offender: (11, Staking::eras_stakers(Staking::active_era().unwrap().index, 11)),
				reporters: vec![],
			}],
			// NOTE: A 100% slash here would clean up the account, causing de-registration.
			&[Perbill::from_percent(95)],
			1,
		);

		// or non-zero.
		assert_eq!(Staking::force_era(), Forcing::NotForcing);
		assert!(<Validators<Test>>::contains_key(11));
		assert!(Session::validators().contains(&11));
	});
}

#[test]
fn reporters_receive_their_slice() {
	// This test verifies that the reporters of the offence receive their slice from the slashed
	// amount.
	ExtBuilder::default().build_and_execute(|| {
		// The reporters' reward is calculated from the total exposure.
		let initial_balance = 1125;

		assert_eq!(
			Staking::eras_stakers(Staking::active_era().unwrap().index, 11).total,
			initial_balance
		);

		on_offence_now(
			&[OffenceDetails {
				offender: (11, Staking::eras_stakers(Staking::active_era().unwrap().index, 11)),
				reporters: vec![1, 2],
			}],
			&[Perbill::from_percent(50)],
		);

		// F1 * (reward_proportion * slash - 0)
		// 50% * (10% * initial_balance / 2)
		let reward = (initial_balance / 20) / 2;
		let reward_each = reward / 2; // split into two pieces.
		assert_eq!(Balances::free_balance(1), 10 + reward_each);
		assert_eq!(Balances::free_balance(2), 20 + reward_each);
	});
}

#[test]
fn subsequent_reports_in_same_span_pay_out_less() {
	// This test verifies that the reporters of the offence receive their slice from the slashed
	// amount, but less and less if they submit multiple reports in one span.
	ExtBuilder::default().build_and_execute(|| {
		// The reporters' reward is calculated from the total exposure.
		let initial_balance = 1125;

		assert_eq!(
			Staking::eras_stakers(Staking::active_era().unwrap().index, 11).total,
			initial_balance
		);

		on_offence_now(
			&[OffenceDetails {
				offender: (11, Staking::eras_stakers(Staking::active_era().unwrap().index, 11)),
				reporters: vec![1],
			}],
			&[Perbill::from_percent(20)],
		);

		// F1 * (reward_proportion * slash - 0)
		// 50% * (10% * initial_balance * 20%)
		let reward = (initial_balance / 5) / 20;
		assert_eq!(Balances::free_balance(1), 10 + reward);

		on_offence_now(
			&[OffenceDetails {
				offender: (11, Staking::eras_stakers(Staking::active_era().unwrap().index, 11)),
				reporters: vec![1],
			}],
			&[Perbill::from_percent(50)],
		);

		let prior_payout = reward;

		// F1 * (reward_proportion * slash - prior_payout)
		// 50% * (10% * (initial_balance / 2) - prior_payout)
		let reward = ((initial_balance / 20) - prior_payout) / 2;
		assert_eq!(Balances::free_balance(1), 10 + prior_payout + reward);
	});
}

#[test]
fn invulnerables_are_not_slashed() {
	// For invulnerable validators no slashing is performed.
	ExtBuilder::default().invulnerables(vec![11]).build_and_execute(|| {
		assert_eq!(Balances::free_balance(11), 1000);
		assert_eq!(Balances::free_balance(21), 2000);

		let exposure = Staking::eras_stakers(Staking::active_era().unwrap().index, 21);
		let initial_balance = Staking::slashable_balance_of(&21);

		let nominator_balances: Vec<_> = exposure.others.iter().map(|o| Balances::free_balance(&o.who)).collect();

		on_offence_now(
			&[
				OffenceDetails {
					offender: (11, Staking::eras_stakers(Staking::active_era().unwrap().index, 11)),
					reporters: vec![],
				},
				OffenceDetails {
					offender: (21, Staking::eras_stakers(Staking::active_era().unwrap().index, 21)),
					reporters: vec![],
				},
			],
			&[Perbill::from_percent(50), Perbill::from_percent(20)],
		);

		// The validator 11 hasn't been slashed, but 21 has been.
		assert_eq!(Balances::free_balance(11), 1000);
		// 2000 - (0.2 * initial_balance)
		assert_eq!(Balances::free_balance(21), 2000 - (2 * initial_balance / 10));

		// ensure that nominators were slashed as well.
		for (initial_balance, other) in nominator_balances.into_iter().zip(exposure.others) {
			assert_eq!(
				Balances::free_balance(&other.who),
				initial_balance - (2 * other.value / 10),
			);
		}
	});
}

#[test]
fn dont_slash_if_fraction_is_zero() {
	// Don't slash if the fraction is zero.
	ExtBuilder::default().build_and_execute(|| {
		assert_eq!(Balances::free_balance(11), 1000);

		on_offence_now(
			&[OffenceDetails {
				offender: (11, Staking::eras_stakers(Staking::active_era().unwrap().index, 11)),
				reporters: vec![],
			}],
			&[Perbill::from_percent(0)],
		);

		// The validator hasn't been slashed. The new era is not forced.
		assert_eq!(Balances::free_balance(11), 1000);
		assert_eq!(Staking::force_era(), Forcing::ForceNew);
	});
}

#[test]
fn only_slash_for_max_in_era() {
	// multiple slashes within one era are only applied if it is more than any previous slash in the
	// same era.
	ExtBuilder::default().build_and_execute(|| {
		assert_eq!(Balances::free_balance(11), 1000);

		on_offence_now(
			&[OffenceDetails {
				offender: (11, Staking::eras_stakers(Staking::active_era().unwrap().index, 11)),
				reporters: vec![],
			}],
			&[Perbill::from_percent(50)],
		);

		// The validator has been slashed and has been force-chilled.
		assert_eq!(Balances::free_balance(11), 500);
		assert_eq!(Staking::force_era(), Forcing::ForceNew);

		on_offence_now(
			&[OffenceDetails {
				offender: (11, Staking::eras_stakers(Staking::active_era().unwrap().index, 11)),
				reporters: vec![],
			}],
			&[Perbill::from_percent(25)],
		);

		// The validator has not been slashed additionally.
		assert_eq!(Balances::free_balance(11), 500);

		on_offence_now(
			&[OffenceDetails {
				offender: (11, Staking::eras_stakers(Staking::active_era().unwrap().index, 11)),
				reporters: vec![],
			}],
			&[Perbill::from_percent(60)],
		);

		// The validator got slashed 10% more.
		assert_eq!(Balances::free_balance(11), 400);
	})
}

#[test]
fn garbage_collection_after_slashing() {
	// ensures that `SlashingSpans` and `SpanSlash` of an account is removed after reaping.
	ExtBuilder::default()
		.existential_deposit(2)
		.minimum_bond(2)
		.build_and_execute(|| {
			assert_eq!(Balances::free_balance(11), 256_000);

			on_offence_now(
				&[OffenceDetails {
					offender: (11, Staking::eras_stakers(Staking::active_era().unwrap().index, 11)),
					reporters: vec![],
				}],
				&[Perbill::from_percent(10)],
			);

			assert_eq!(Balances::free_balance(11), 256_000 - 25_600);
			assert!(<Staking as crate::Store>::SlashingSpans::get(&11).is_some());
			assert_eq!(
				<Staking as crate::Store>::SpanSlash::get(&(11, 0)).amount_slashed(),
				&25_600
			);

			on_offence_now(
				&[OffenceDetails {
					offender: (11, Staking::eras_stakers(Staking::active_era().unwrap().index, 11)),
					reporters: vec![],
				}],
				&[Perbill::from_percent(100)],
			);

			// validator and nominator slash in era are garbage-collected by era change,
			// so we don't test those here.

			// Only the existential deposit would be left
			assert_eq!(Balances::free_balance(11), 2);
			assert_eq!(Balances::total_balance(&11), 2);

			let slashing_spans = <Staking as crate::Store>::SlashingSpans::get(&11).unwrap();
			assert_eq!(slashing_spans.iter().count(), 2);

			assert_ok!(Staking::reap_stash(Origin::none(), 11));

			assert!(<Staking as crate::Store>::SlashingSpans::get(&11).is_none());
			assert_eq!(<Staking as crate::Store>::SpanSlash::get(&(11, 0)).amount_slashed(), &0);
		})
}

#[test]
fn garbage_collection_on_window_pruning() {
	// ensures that `ValidatorSlashInEra` and `NominatorSlashInEra` are cleared after
	// `BondingDuration`.
	ExtBuilder::default().build_and_execute(|| {
		mock::start_active_era(1);

		assert_eq!(Balances::free_balance(11), 1000);
		let now = Staking::active_era().unwrap().index;

		let exposure = Staking::eras_stakers(now, 11);
		assert_eq!(Balances::free_balance(101), 2000);
		let nominated_value = exposure.others.iter().find(|o| o.who == 101).unwrap().value;

		on_offence_now(
			&[OffenceDetails {
				offender: (11, Staking::eras_stakers(now, 11)),
				reporters: vec![],
			}],
			&[Perbill::from_percent(10)],
		);

		assert_eq!(Balances::free_balance(11), 900);
		assert_eq!(Balances::free_balance(101), 2000 - (nominated_value / 10));

		assert!(<Staking as crate::Store>::ValidatorSlashInEra::get(&now, &11).is_some());
		assert!(<Staking as crate::Store>::NominatorSlashInEra::get(&now, &101).is_some());

		// + 1 because we have to exit the bonding window.
		for era in (0..(BondingDuration::get() + 1)).map(|offset| offset + now + 1) {
			assert!(<Staking as crate::Store>::ValidatorSlashInEra::get(&now, &11).is_some());
			assert!(<Staking as crate::Store>::NominatorSlashInEra::get(&now, &101).is_some());

			mock::start_active_era(era);
		}

		assert!(<Staking as crate::Store>::ValidatorSlashInEra::get(&now, &11).is_none());
		assert!(<Staking as crate::Store>::NominatorSlashInEra::get(&now, &101).is_none());
	})
}

#[test]
fn slashing_nominators_by_span_max() {
	ExtBuilder::default().build_and_execute(|| {
		mock::start_active_era(1);
		mock::start_active_era(2);
		mock::start_active_era(3);

		assert_eq!(Balances::free_balance(11), 1000);
		assert_eq!(Balances::free_balance(21), 2000);
		assert_eq!(Balances::free_balance(101), 2000);
		assert_eq!(Staking::slashable_balance_of(&21), 1000);

		let exposure_11 = Staking::eras_stakers(Staking::active_era().unwrap().index, 11);
		let exposure_21 = Staking::eras_stakers(Staking::active_era().unwrap().index, 21);
		let nominated_value_11 = exposure_11.others.iter().find(|o| o.who == 101).unwrap().value;
		let nominated_value_21 = exposure_21.others.iter().find(|o| o.who == 101).unwrap().value;

		on_offence_in_era(
			&[OffenceDetails {
				offender: (11, Staking::eras_stakers(Staking::active_era().unwrap().index, 11)),
				reporters: vec![],
			}],
			&[Perbill::from_percent(10)],
			2,
		);

		assert_eq!(Balances::free_balance(11), 900);

		let slash_1_amount = Perbill::from_percent(10) * nominated_value_11;
		assert_eq!(Balances::free_balance(101), 2000 - slash_1_amount);

		let expected_spans = vec![
			slashing::SlashingSpan {
				index: 1,
				start: 4,
				length: None,
			},
			slashing::SlashingSpan {
				index: 0,
				start: 0,
				length: Some(4),
			},
		];

		let get_span = |account| <Staking as crate::Store>::SlashingSpans::get(&account).unwrap();

		assert_eq!(get_span(11).iter().collect::<Vec<_>>(), expected_spans,);

		assert_eq!(get_span(101).iter().collect::<Vec<_>>(), expected_spans,);

		// second slash: higher era, higher value, same span.
		on_offence_in_era(
			&[OffenceDetails {
				offender: (21, Staking::eras_stakers(Staking::active_era().unwrap().index, 21)),
				reporters: vec![],
			}],
			&[Perbill::from_percent(30)],
			3,
		);

		// 11 was not further slashed, but 21 and 101 were.
		assert_eq!(Balances::free_balance(11), 900);
		assert_eq!(Balances::free_balance(21), 1700);

		let slash_2_amount = Perbill::from_percent(30) * nominated_value_21;
		assert!(slash_2_amount > slash_1_amount);

		// only the maximum slash in a single span is taken.
		assert_eq!(Balances::free_balance(101), 2000 - slash_2_amount);

		// third slash: in same era and on same validator as first, higher
		// in-era value, but lower slash value than slash 2.
		on_offence_in_era(
			&[OffenceDetails {
				offender: (11, Staking::eras_stakers(Staking::active_era().unwrap().index, 11)),
				reporters: vec![],
			}],
			&[Perbill::from_percent(20)],
			2,
		);

		// 11 was further slashed, but 21 and 101 were not.
		assert_eq!(Balances::free_balance(11), 800);
		assert_eq!(Balances::free_balance(21), 1700);

		let slash_3_amount = Perbill::from_percent(20) * nominated_value_21;
		assert!(slash_3_amount < slash_2_amount);
		assert!(slash_3_amount > slash_1_amount);

		// only the maximum slash in a single span is taken.
		assert_eq!(Balances::free_balance(101), 2000 - slash_2_amount);
	});
}

#[test]
fn slashes_are_summed_across_spans() {
	ExtBuilder::default().build_and_execute(|| {
		mock::start_active_era(1);
		mock::start_active_era(2);
		mock::start_active_era(3);

		assert_eq!(Balances::free_balance(21), 2000);
		assert_eq!(Staking::slashable_balance_of(&21), 1000);

		let get_span = |account| <Staking as crate::Store>::SlashingSpans::get(&account).unwrap();

		on_offence_now(
			&[OffenceDetails {
				offender: (21, Staking::eras_stakers(Staking::active_era().unwrap().index, 21)),
				reporters: vec![],
			}],
			&[Perbill::from_percent(10)],
		);

		let expected_spans = vec![
			slashing::SlashingSpan {
				index: 1,
				start: 4,
				length: None,
			},
			slashing::SlashingSpan {
				index: 0,
				start: 0,
				length: Some(4),
			},
		];

		assert_eq!(get_span(21).iter().collect::<Vec<_>>(), expected_spans);
		assert_eq!(Balances::free_balance(21), 1900);

		// 21 has been force-chilled. re-signal intent to validate.
		Staking::validate(Origin::signed(20), Default::default()).unwrap();

		mock::start_active_era(4);

		assert_eq!(Staking::slashable_balance_of(&21), 900);

		on_offence_now(
			&[OffenceDetails {
				offender: (21, Staking::eras_stakers(Staking::active_era().unwrap().index, 21)),
				reporters: vec![],
			}],
			&[Perbill::from_percent(10)],
		);

		let expected_spans = vec![
			slashing::SlashingSpan {
				index: 2,
				start: 5,
				length: None,
			},
			slashing::SlashingSpan {
				index: 1,
				start: 4,
				length: Some(1),
			},
			slashing::SlashingSpan {
				index: 0,
				start: 0,
				length: Some(4),
			},
		];

		assert_eq!(get_span(21).iter().collect::<Vec<_>>(), expected_spans);
		assert_eq!(Balances::free_balance(21), 1810);
	});
}

#[test]
fn deferred_slashes_are_deferred() {
	ExtBuilder::default().slash_defer_duration(2).build_and_execute(|| {
		mock::start_active_era(1);

		assert_eq!(Balances::free_balance(11), 1000);

		let exposure = Staking::eras_stakers(Staking::active_era().unwrap().index, 11);
		assert_eq!(Balances::free_balance(101), 2000);
		let nominated_value = exposure.others.iter().find(|o| o.who == 101).unwrap().value;

		on_offence_now(
			&[OffenceDetails {
				offender: (11, Staking::eras_stakers(Staking::active_era().unwrap().index, 11)),
				reporters: vec![],
			}],
			&[Perbill::from_percent(10)],
		);

		assert_eq!(Balances::free_balance(11), 1000);
		assert_eq!(Balances::free_balance(101), 2000);

		mock::start_active_era(2);

		assert_eq!(Balances::free_balance(11), 1000);
		assert_eq!(Balances::free_balance(101), 2000);

		mock::start_active_era(3);

		assert_eq!(Balances::free_balance(11), 1000);
		assert_eq!(Balances::free_balance(101), 2000);

		// at the start of era 4, slashes from era 1 are processed,
		// after being deferred for at least 2 full eras.
		mock::start_active_era(4);

		assert_eq!(Balances::free_balance(11), 900);
		assert_eq!(Balances::free_balance(101), 2000 - (nominated_value / 10));
	})
}

#[test]
fn remove_deferred() {
	ExtBuilder::default().slash_defer_duration(2).build_and_execute(|| {
		mock::start_active_era(1);

		assert_eq!(Balances::free_balance(11), 1000);

		let exposure = Staking::eras_stakers(Staking::active_era().unwrap().index, 11);
		assert_eq!(Balances::free_balance(101), 2000);
		let nominated_value = exposure.others.iter().find(|o| o.who == 101).unwrap().value;

		on_offence_now(
			&[OffenceDetails {
				offender: (11, exposure.clone()),
				reporters: vec![],
			}],
			&[Perbill::from_percent(10)],
		);

		assert_eq!(Balances::free_balance(11), 1000);
		assert_eq!(Balances::free_balance(101), 2000);

		mock::start_active_era(2);

		on_offence_in_era(
			&[OffenceDetails {
				offender: (11, exposure.clone()),
				reporters: vec![],
			}],
			&[Perbill::from_percent(15)],
			1,
		);

		// fails if empty
		assert_noop!(
			Staking::cancel_deferred_slash(Origin::root(), 1, vec![]),
			Error::<Test>::EmptyTargets
		);

		assert_ok!(Staking::cancel_deferred_slash(Origin::root(), 1, vec![0]));

		assert_eq!(Balances::free_balance(11), 1000);
		assert_eq!(Balances::free_balance(101), 2000);

		mock::start_active_era(3);

		assert_eq!(Balances::free_balance(11), 1000);
		assert_eq!(Balances::free_balance(101), 2000);

		// at the start of era 4, slashes from era 1 are processed,
		// after being deferred for at least 2 full eras.
		mock::start_active_era(4);

		// the first slash for 10% was cancelled, so no effect.
		assert_eq!(Balances::free_balance(11), 1000);
		assert_eq!(Balances::free_balance(101), 2000);

		mock::start_active_era(5);

		let slash_10 = Perbill::from_percent(10);
		let slash_15 = Perbill::from_percent(15);
		let initial_slash = slash_10 * nominated_value;

		let total_slash = slash_15 * nominated_value;
		let actual_slash = total_slash - initial_slash;

		// 5% slash (15 - 10) processed now.
		assert_eq!(Balances::free_balance(11), 950);
		assert_eq!(Balances::free_balance(101), 2000 - actual_slash);
	})
}

#[test]
fn remove_multi_deferred() {
	ExtBuilder::default().slash_defer_duration(2).build_and_execute(|| {
		mock::start_active_era(1);

		assert_eq!(Balances::free_balance(11), 1000);

		let exposure = Staking::eras_stakers(Staking::active_era().unwrap().index, 11);
		assert_eq!(Balances::free_balance(101), 2000);

		on_offence_now(
			&[OffenceDetails {
				offender: (11, exposure.clone()),
				reporters: vec![],
			}],
			&[Perbill::from_percent(10)],
		);

		on_offence_now(
			&[OffenceDetails {
				offender: (21, Staking::eras_stakers(Staking::active_era().unwrap().index, 21)),
				reporters: vec![],
			}],
			&[Perbill::from_percent(10)],
		);

		on_offence_now(
			&[OffenceDetails {
				offender: (11, exposure.clone()),
				reporters: vec![],
			}],
			&[Perbill::from_percent(25)],
		);

		on_offence_now(
			&[OffenceDetails {
				offender: (42, exposure.clone()),
				reporters: vec![],
			}],
			&[Perbill::from_percent(25)],
		);

		on_offence_now(
			&[OffenceDetails {
				offender: (69, exposure.clone()),
				reporters: vec![],
			}],
			&[Perbill::from_percent(25)],
		);

		assert_eq!(<Staking as Store>::UnappliedSlashes::get(&1).len(), 5);

		// fails if list is not sorted
		assert_noop!(
			Staking::cancel_deferred_slash(Origin::root(), 1, vec![2, 0, 4]),
			Error::<Test>::NotSortedAndUnique
		);
		// fails if list is not unique
		assert_noop!(
			Staking::cancel_deferred_slash(Origin::root(), 1, vec![0, 2, 2]),
			Error::<Test>::NotSortedAndUnique
		);
		// fails if bad index
		assert_noop!(
			Staking::cancel_deferred_slash(Origin::root(), 1, vec![1, 2, 3, 4, 5]),
			Error::<Test>::InvalidSlashIndex
		);

		assert_ok!(Staking::cancel_deferred_slash(Origin::root(), 1, vec![0, 2, 4]));

		let slashes = <Staking as Store>::UnappliedSlashes::get(&1);
		assert_eq!(slashes.len(), 2);
		assert_eq!(slashes[0].validator, 21);
		assert_eq!(slashes[1].validator, 42);
	})
}
