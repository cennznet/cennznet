// Copyright 2017-2020 Parity Technologies (UK) Ltd. and Centrality Investments Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

//! Tests for the module.

use super::*;
use crate::Store;
use frame_support::{
	assert_noop, assert_ok,
	traits::{Currency, OnInitialize, ReservableCurrency},
	StorageMap,
};
use frame_system::{EventRecord, Phase};
use mock::*;
use pallet_balances::Error as BalancesError;
use sp_runtime::{assert_eq_error_rate, traits::BadOrigin};
use sp_staking::offence::OffenceDetails;
use substrate_test_utils::assert_eq_uvec;

#[test]
fn force_unstake_works() {
	// Verifies initial conditions of mock
	ExtBuilder::default().build().execute_with(|| {
		// Account 11 is stashed and locked, and account 10 is the controller
		assert_eq!(Staking::bonded(&11), Some(10));
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
fn basic_setup_works() {
	// Verifies initial conditions of mock
	ExtBuilder::default().build().execute_with(|| {
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
		assert_eq!(
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
			Staking::stakers(11),
			Exposure {
				total: 1125,
				own: 1000,
				others: vec![IndividualExposure { who: 101, value: 125 }]
			}
		);
		assert_eq!(
			Staking::stakers(21),
			Exposure {
				total: 1375,
				own: 1000,
				others: vec![IndividualExposure { who: 101, value: 375 }]
			}
		);
		// initial slot_stake
		assert_eq!(Staking::slot_stake(), 1125);

		// The number of validators required.
		assert_eq!(Staking::validator_count(), 2);

		// Initial Era and session
		assert_eq!(Staking::current_era(), 0);

		// Account 10 has `balance_factor` free balance
		assert_eq!(Balances::free_balance(&10), 1);
		assert_eq!(Balances::free_balance(&10), 1);

		// New era is not being forced
		assert_eq!(Staking::force_era(), Forcing::NotForcing);

		// All exposures must be correct.
		check_exposure_all();
		check_nominator_all();
	});
}

#[test]
fn change_controller_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Staking::bonded(&11), Some(10));

		assert!(<Validators<Test>>::iter()
			.map(|(c, _)| c)
			.collect::<Vec<u64>>()
			.contains(&11));
		// 10 can control 11 who is initially a validator.
		assert_ok!(Staking::chill(Origin::signed(10)));
		assert!(!<Validators<Test>>::iter()
			.map(|(c, _)| c)
			.collect::<Vec<u64>>()
			.contains(&11));

		assert_ok!(Staking::set_controller(Origin::signed(11), 5));

		start_era(1);

		assert_noop!(
			Staking::validate(Origin::signed(10), ValidatorPrefs::default()),
			Error::<Test>::NotController,
		);
		assert_ok!(Staking::validate(Origin::signed(5), ValidatorPrefs::default()));
	})
}

#[test]
fn rewards_should_work() {
	// should check that:
	// * rewards get recorded per session
	// * rewards get paid per Era
	// * Check that nominators are also rewarded
	ExtBuilder::default().nominate(false).build().execute_with(|| {
		// Init some balances
		let _ = Balances::make_free_balance_be(&2, 500);

		let init_balance_2 = Balances::total_balance(&2);
		let init_balance_10 = Balances::total_balance(&10);
		let init_balance_11 = Balances::total_balance(&11);

		// Set payee to controller
		assert_ok!(Staking::set_payee(Origin::signed(10), RewardDestination::Controller));

		// Initial config should be correct
		assert_eq!(Staking::current_era(), 0);
		assert_eq!(Session::current_index(), 0);

		// Add a dummy nominator.
		//
		// Equal division indicates that the reward will be equally divided among validator and
		// nominator.
		<Stakers<Test>>::insert(
			&11,
			Exposure {
				own: 500,
				total: 1000,
				others: vec![IndividualExposure { who: 2, value: 500 }],
			},
		);

		assert_eq!(Rewards::payee(&2), 2);
		assert_eq!(Rewards::payee(&11), 10);

		start_session(1);

		assert_eq!(Staking::current_era(), 0);
		assert_eq!(Session::current_index(), 1);

		Rewards::reward_by_ids(vec![(11, 50)]);
		Rewards::reward_by_ids(vec![(11, 50)]);
		// This is the second validator of the current elected set.
		Rewards::reward_by_ids(vec![(21, 50)]);

		// Compute total payout now for whole duration as other parameter won't change
		let total_payout: Balance = Rewards::calculate_next_reward_payout();
		assert!(total_payout > 10); // Test is meaningful if reward something

		// No reward yet
		assert_eq!(Balances::total_balance(&2), init_balance_2);
		assert_eq!(Balances::total_balance(&10), init_balance_10);
		assert_eq!(Balances::total_balance(&11), init_balance_11);

		start_era(1);
		Staking::on_initialize(System::block_number() + 1);

		// 11 validator has 2/3 of the total rewards and half half for it and its nominator
		assert_eq_error_rate!(Balances::total_balance(&2), init_balance_2 + total_payout / 3, 1);
		assert_eq_error_rate!(Balances::total_balance(&10), init_balance_10 + total_payout / 3, 1);
		assert_eq!(Balances::total_balance(&11), init_balance_11);
	});
}

#[test]
fn multi_era_reward_should_work() {
	// Should check that:
	// The value of current_session_reward is set at the end of each era, based on
	// slot_stake and session_reward.
	ExtBuilder::default().nominate(false).build().execute_with(|| {
		let init_balance_10 = Balances::total_balance(&10);

		// Set payee to controller
		assert_ok!(Staking::set_payee(Origin::signed(10), RewardDestination::Controller));

		// Compute now as other parameter won't change
		let total_payout_0: Balance = Rewards::calculate_next_reward_payout();
		assert!(total_payout_0 > 10); // Test is meaningful if reward something
		Rewards::reward_by_ids(vec![(11, 1)]);

		start_session(0);
		start_session(1);
		start_session(2);
		start_session(3);

		Staking::on_initialize(System::block_number() + 1);

		assert_eq!(Staking::current_era(), 1);
		assert_eq!(Balances::total_balance(&10), init_balance_10 + total_payout_0);

		start_session(5);

		let total_payout_1: Balance = Rewards::calculate_next_reward_payout();
		assert!(total_payout_1 > 10); // Test is meaningful if reward something
		Rewards::reward_by_ids(vec![(11, 101)]);

		// new era is triggered here.
		start_session(6);

		Staking::on_initialize(System::block_number() + 1);

		// pay time
		assert_eq!(
			Balances::total_balance(&10),
			init_balance_10 + total_payout_0 + total_payout_1
		);
	});
}

#[test]
fn staking_should_work() {
	// should test:
	// * new validators can be added to the default set
	// * new ones will be chosen per era
	// * either one can unlock the stash and back-down from being a validator via `chill`ing.
	ExtBuilder::default()
		.nominate(false)
		.fair(false) // to give 20 more staked value
		.build()
		.execute_with(|| {
			Timestamp::set_timestamp(1); // Initialize time.

			// remember + compare this along with the test.
			assert_eq_uvec!(validator_controllers(), vec![20, 10]);

			// put some money in account that we'll use.
			for i in 1..5 {
				let _ = Balances::make_free_balance_be(&i, 2000);
			}

			// --- Block 1:
			start_session(1);
			// add a new candidate for being a validator. account 3 controlled by 4.
			assert_ok!(Staking::bond(Origin::signed(3), 4, 1500, RewardDestination::Controller));
			assert_ok!(Staking::validate(Origin::signed(4), ValidatorPrefs::default()));

			// No effects will be seen so far.
			assert_eq_uvec!(validator_controllers(), vec![20, 10]);

			// --- Block 2:
			start_session(2);

			// No effects will be seen so far. Era has not been yet triggered.
			assert_eq_uvec!(validator_controllers(), vec![20, 10]);

			// --- Block 3: the validators will now be queued.
			start_session(3);
			assert_eq!(Staking::current_era(), 1);

			// --- Block 4: the validators will now be changed.
			start_session(4);

			assert_eq_uvec!(validator_controllers(), vec![20, 4]);
			// --- Block 4: Unstake 4 as a validator, freeing up the balance stashed in 3
			// 4 will chill
			Staking::chill(Origin::signed(4)).unwrap();

			// --- Block 5: nothing. 4 is still there.
			start_session(5);
			assert_eq_uvec!(validator_controllers(), vec![20, 4]);

			// --- Block 6: 4 will not be a validator.
			start_session(7);
			assert_eq_uvec!(validator_controllers(), vec![20, 10]);

			// Note: the stashed value of 4 is still lock
			assert_eq!(
				Staking::ledger(&4),
				Some(StakingLedger {
					stash: 3,
					total: 1500,
					active: 1500,
					unlocking: vec![]
				})
			);
			// e.g. it cannot spend more than 500 that it has free from the total 2000
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

			start_era(1);

			// Previous set is selected. NO election algorithm is even executed.
			assert_eq_uvec!(validator_controllers(), vec![30, 20, 10]);

			// But the exposure is updated in a simple way. No external votes exists.
			// This is purely self-vote.
			assert_eq!(Staking::stakers(10).others.len(), 0);
			assert_eq!(Staking::stakers(20).others.len(), 0);
			assert_eq!(Staking::stakers(30).others.len(), 0);
			check_exposure_all();
			check_nominator_all();
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

			let _ = Staking::chill(Origin::signed(10));

			// trigger era
			start_era(1);

			// Previous ones are elected. chill is invalidates. TODO: #2494
			assert_eq_uvec!(validator_controllers(), vec![10, 20, 30, 40]);
			// Though the validator preferences has been removed.
			assert!(Staking::validators(11) != prefs);
		});
}

#[test]
fn nominators_also_get_slashed() {
	// A nominator should be slashed if the validator they nominated is slashed
	// Here is the breakdown of roles:
	// 10 - is the controller of 11
	// 11 - is the stash.
	// 2 - is the nominator of 20, 10
	ExtBuilder::default().nominate(false).build().execute_with(|| {
		assert_eq!(Staking::validator_count(), 2);

		// Set payee to controller
		assert_ok!(Staking::set_payee(Origin::signed(10), RewardDestination::Controller));

		// give the man some money.
		let initial_balance = 1000;
		for i in [1, 2, 3, 10].iter() {
			let _ = Balances::make_free_balance_be(i, initial_balance);
		}

		// 2 will nominate for 10, 20
		let nominator_stake = 500;
		assert_ok!(Staking::bond(
			Origin::signed(1),
			2,
			nominator_stake,
			RewardDestination::default()
		));
		assert_ok!(Staking::nominate(Origin::signed(2), vec![20, 10]));

		let total_payout: Balance = Rewards::calculate_next_reward_payout();
		assert!(total_payout > 100); // Test is meaningful if reward something
		Rewards::reward_by_ids(vec![(11, 1)]);

		// new era, pay rewards,
		start_era(1);

		Staking::on_initialize(System::block_number() + 1);

		// Nominator stash didn't collect any.
		assert_eq!(Balances::total_balance(&2), initial_balance);

		// 10 goes offline
		on_offence_now(
			&[OffenceDetails {
				offender: (11, Staking::stakers(&11)),
				reporters: vec![],
			}],
			&[Perbill::from_percent(5)],
		);
		let expo = Staking::stakers(11);
		let slash_value = 50;
		let total_slash = expo.total.min(slash_value);
		let validator_slash = expo.own.min(total_slash);
		let nominator_slash = nominator_stake.min(total_slash - validator_slash);

		// initial + first era reward + slash
		assert_eq!(Balances::total_balance(&11), initial_balance - validator_slash);
		assert_eq!(Balances::total_balance(&2), initial_balance - nominator_slash);
		check_exposure_all();
		check_nominator_all();
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
	ExtBuilder::default().build().execute_with(|| {
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
	ExtBuilder::default().build().execute_with(|| {
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
fn session_and_eras_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Staking::current_era(), 0);

		start_session(1);
		assert_eq!(Session::current_index(), 1);
		assert_eq!(Staking::current_era(), 0);

		start_session(2);
		assert_eq!(Session::current_index(), 2);
		assert_eq!(Staking::current_era(), 0);

		// An era change kicks in.
		start_session(3);
		assert_eq!(Session::current_index(), 3);
		assert_eq!(Staking::current_era(), 1);

		// Another era change.
		start_session(6);
		assert_eq!(Session::current_index(), 6);
		assert_eq!(Staking::current_era(), 2);

		// No era change.
		start_session(7);
		assert_eq!(Session::current_index(), 7);
		assert_eq!(Staking::current_era(), 2);

		// Another era change.
		start_session(9);
		assert_eq!(Session::current_index(), 9);
		assert_eq!(Staking::current_era(), 3);
	});
}

#[test]
fn forcing_new_era_works() {
	ExtBuilder::default().build().execute_with(|| {
		// normal flow of session.
		assert_eq!(Staking::current_era(), 0);
		start_session(1);
		assert_eq!(Staking::current_era(), 0);
		start_session(2);
		assert_eq!(Staking::current_era(), 0);
		start_session(3);
		assert_eq!(Staking::current_era(), 1);

		// no era change.
		ForceEra::put(Forcing::ForceNone);
		start_session(4);
		assert_eq!(Staking::current_era(), 1);
		start_session(5);
		assert_eq!(Staking::current_era(), 1);
		start_session(6);
		assert_eq!(Staking::current_era(), 1);
		start_session(7);
		assert_eq!(Staking::current_era(), 1);

		// back to normal.
		// this immediately starts a new session.
		ForceEra::put(Forcing::NotForcing);
		start_session(8);
		assert_eq!(Staking::current_era(), 2);
		start_session(9);
		assert_eq!(Staking::current_era(), 2);

		// forceful change
		ForceEra::put(Forcing::ForceAlways);
		start_session(10);
		assert_eq!(Staking::current_era(), 3);
		start_session(11);
		assert_eq!(Staking::current_era(), 4);
		start_session(12);
		assert_eq!(Staking::current_era(), 5);

		// just one forceful change
		ForceEra::put(Forcing::ForceNew);
		start_session(13);
		assert_eq!(Staking::current_era(), 6);

		assert_eq!(ForceEra::get(), Forcing::NotForcing);
		start_session(14);
		assert_eq!(Staking::current_era(), 6);
	});
}

#[test]
fn cannot_transfer_staked_balance() {
	// Tests that a stash account cannot transfer funds
	ExtBuilder::default().nominate(false).build().execute_with(|| {
		// Confirm account 11 is stashed
		assert_eq!(Staking::bonded(&11), Some(10));
		// Confirm account 11 has some free balance
		assert_eq!(Balances::free_balance(11), 1000);
		// Confirm account 11 (via controller 10) is totally staked
		assert_eq!(Staking::stakers(&11).total, 1000);
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
	ExtBuilder::default()
		.nominate(false)
		.fair(true)
		.build()
		.execute_with(|| {
			// Confirm account 21 is stashed
			assert_eq!(Staking::bonded(&21), Some(20));
			// Confirm account 21 has some free balance
			assert_eq!(Balances::free_balance(21), 2000);
			// Confirm account 21 (via controller 20) is totally staked
			assert_eq!(Staking::stakers(&21).total, 1000);
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
	ExtBuilder::default().build().execute_with(|| {
		// Confirm account 11 is stashed
		assert_eq!(Staking::bonded(&11), Some(10));
		// Confirm account 11 has some free balance
		assert_eq!(Balances::free_balance(11), 1000);
		// Confirm account 11 (via controller 10) is totally staked
		assert_eq!(Staking::stakers(&11).own, 1000);
		// Confirm account 11 cannot transfer as a result
		assert_noop!(
			Balances::reserve(&11, 1),
			BalancesError::<Test, _>::LiquidityRestrictions
		);

		// Give account 11 extra free balance
		let _ = Balances::make_free_balance_be(&11, 10000);
		// Confirm account 11 can now reserve balance
		assert_ok!(Balances::reserve(&11, 1));
	});
}

#[test]
fn reward_destination_works() {
	// Rewards go to the correct destination as determined in Payee
	ExtBuilder::default().nominate(false).build().execute_with(|| {
		// Check that account 11 is a validator
		assert!(Staking::current_elected().contains(&11));
		// Check the balance of the validator account
		assert_eq!(Balances::free_balance(10), 1);
		// Check the balance of the stash account
		assert_eq!(Balances::free_balance(11), 1000);
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

		// Compute total payout now for whole duration as other parameter won't change
		let total_payout_0: Balance = Rewards::calculate_next_reward_payout();
		assert!(total_payout_0 > 100); // Test is meaningful if reward something
		Rewards::reward_by_ids(vec![(11, 1)]);

		start_era(1);
		Staking::on_initialize(System::block_number() + 1);

		// Check that RewardDestination is Stash (default)
		assert_eq!(Rewards::payee(&11), 11);
		// Check that reward went to the stash account of validator
		assert_eq!(Balances::free_balance(11), 1000 + total_payout_0);

		// Compute total payout now for whole duration as other parameter won't change
		let total_payout_1: Balance = Rewards::calculate_next_reward_payout();
		assert!(total_payout_1 > 100); // Test is meaningful if reward something
		Rewards::reward_by_ids(vec![(11, 1)]);

		start_era(2);
		Staking::on_initialize(System::block_number() + 1);

		// Check that RewardDestination is Stash
		assert_eq!(Rewards::payee(&11), 11);
		// Check that reward went to the stash account
		assert_eq!(Balances::free_balance(11), 1000 + total_payout_0 + total_payout_1);
		// Record this value
		let recorded_stash_balance = 1000 + total_payout_0 + total_payout_1;

		// Change RewardDestination to Controller
		<<Test as Trait>::Rewarder as HandlePayee>::set_payee(&11, &10);

		// Check controller balance
		assert_eq!(Balances::free_balance(10), 1);

		// Compute total payout now for whole duration as other parameter won't change
		let total_payout_2: Balance = Rewards::calculate_next_reward_payout();
		assert!(total_payout_2 > 100); // Test is meaningful if reward something
		Rewards::reward_by_ids(vec![(11, 1)]);

		start_era(3);
		Staking::on_initialize(System::block_number() + 1);

		// Check that RewardDestination is Controller
		assert_eq!(Rewards::payee(&11), 10);
		// Check that reward went to the controller account
		assert_eq!(Balances::free_balance(10), 1 + total_payout_2);
		// Check that amount in staked account is NOT increased.
		assert_eq!(Balances::free_balance(11), recorded_stash_balance);
	});
}

#[test]
fn reward_destination_controller_is_replaced_with_account() {
	ExtBuilder::default().minimum_bond(5).build().execute_with(|| {
		let stash = 100_u64;
		let controller = 101_u64;

		// bond with controller as payee
		assert_ok!(Staking::bond(
			Origin::signed(stash),
			controller,
			Staking::minimum_bond(),
			RewardDestination::Controller
		));
		assert_eq!(Rewards::payee(&stash), controller);

		// explicit set controller as payee
		assert_ok!(Staking::set_payee(
			Origin::signed(controller),
			RewardDestination::Controller
		));
		assert_eq!(Rewards::payee(&stash), controller);
	});
}

#[test]
fn validator_payment_prefs_work() {
	// Test that validator preferences are correctly honored
	// Note: unstake threshold is being directly tested in slashing tests.
	// This test will focus on validator payment.
	ExtBuilder::default().build().execute_with(|| {
		// Initial config
		let stash_initial_balance = Balances::total_balance(&11);

		// check the balance of a validator accounts.
		assert_eq!(Balances::total_balance(&10), 1);
		// check the balance of a validator's stash accounts.
		assert_eq!(Balances::total_balance(&11), stash_initial_balance);
		// and the nominator (to-be)
		let nominator_balance = Balances::total_balance(&2);

		// add a dummy nominator.
		<Stakers<Test>>::insert(
			&11,
			Exposure {
				own: nominator_balance, // equal division indicates that the reward will be equally divided among validator and nominator.
				total: 2 * nominator_balance,
				others: vec![IndividualExposure {
					who: 2,
					value: nominator_balance,
				}],
			},
		);
		<Validators<Test>>::insert(
			&11,
			ValidatorPrefs {
				commission: Perbill::from_percent(50),
			},
		);

		// Compute total payout now for whole duration as other parameter won't change
		let total_payout_0: Balance = Rewards::calculate_next_reward_payout();
		assert!(total_payout_0 > 100); // Test is meaningful if reward something
		Rewards::reward_by_ids(vec![(11, 1)]);

		start_era(1);
		Staking::on_initialize(System::block_number() + 1);

		let commission = total_payout_0 / 2;
		// whats left to be shared is the sum of 3 rounds minus the validator's cut.
		let shared_cut = total_payout_0 - commission;
		// Validator's payee is Staked account, 11, reward will be paid here.
		assert_eq!(
			Balances::total_balance(&11),
			stash_initial_balance + commission + shared_cut / 2
		);
		// Controller account will not get any reward.
		assert_eq!(Balances::total_balance(&10), 1);
		// Rest of the reward will be shared and paid to the nominator in stake.
		assert_eq!(Balances::total_balance(&2), nominator_balance + shared_cut / 2);

		check_exposure_all();
		check_nominator_all();
	});
}

#[test]
fn bond_extra_works() {
	// Tests that extra `free_balance` in the stash can be added to stake
	// NOTE: this tests only verifies `StakingLedger` for correct updates
	// See `bond_extra_and_withdraw_unbonded_works` for more details and updates on `Exposure`.
	ExtBuilder::default().build().execute_with(|| {
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
		assert_ok!(Staking::bond_extra(Origin::signed(11), u64::max_value()));
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
	ExtBuilder::default().nominate(false).build().execute_with(|| {
		// Set payee to controller. avoids confusion
		assert_ok!(Staking::set_payee(Origin::signed(10), RewardDestination::Controller));

		// Give account 11 some large free balance greater than total
		let _ = Balances::make_free_balance_be(&11, 1000000);

		// Initial config should be correct
		assert_eq!(Staking::current_era(), 0);
		assert_eq!(Session::current_index(), 0);

		// check the balance of a validator accounts.
		assert_eq!(Balances::total_balance(&10), 1);

		// confirm that 10 is a normal validator and gets paid at the end of the era.
		start_era(1);

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
			Staking::stakers(&11),
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
			Staking::stakers(&11),
			Exposure {
				total: 1000 + 100,
				own: 1000 + 100,
				others: vec![]
			}
		);

		// trigger next era.
		start_era(2);
		assert_eq!(Staking::current_era(), 2);

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
			Staking::stakers(&11),
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
				}]
			})
		);

		// Attempting to free the balances now will fail. 2 eras need to pass.
		Staking::withdraw_unbonded(Origin::signed(10)).unwrap();
		assert_eq!(
			Staking::ledger(&10),
			Some(StakingLedger {
				stash: 11,
				total: 1000 + 100,
				active: 100,
				unlocking: vec![UnlockChunk {
					value: 1000,
					era: 2 + 3
				}]
			})
		);

		// trigger next era.
		start_era(3);

		// nothing yet
		Staking::withdraw_unbonded(Origin::signed(10)).unwrap();
		assert_eq!(
			Staking::ledger(&10),
			Some(StakingLedger {
				stash: 11,
				total: 1000 + 100,
				active: 100,
				unlocking: vec![UnlockChunk {
					value: 1000,
					era: 2 + 3
				}]
			})
		);

		// trigger next era.
		start_era(5);

		Staking::withdraw_unbonded(Origin::signed(10)).unwrap();
		// Now the value is free and the staking ledger is updated.
		assert_eq!(
			Staking::ledger(&10),
			Some(StakingLedger {
				stash: 11,
				total: 100,
				active: 100,
				unlocking: vec![]
			})
		);
	})
}

#[test]
fn too_many_unbond_calls_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		// locked at era 0 until 3
		for _ in 0..MAX_UNLOCKING_CHUNKS - 1 {
			assert_ok!(Staking::unbond(Origin::signed(10), 1));
		}

		start_era(1);

		// locked at era 1 until 4
		assert_ok!(Staking::unbond(Origin::signed(10), 1));
		// can't do more.
		assert_noop!(Staking::unbond(Origin::signed(10), 1), Error::<Test>::NoMoreChunks);

		start_era(3);

		assert_noop!(Staking::unbond(Origin::signed(10), 1), Error::<Test>::NoMoreChunks);
		// free up.
		assert_ok!(Staking::withdraw_unbonded(Origin::signed(10)));

		// Can add again.
		assert_ok!(Staking::unbond(Origin::signed(10), 1));
		assert_eq!(Staking::ledger(&10).unwrap().unlocking.len(), 2);
	})
}

#[test]
fn it_should_unbond_all_active_stake_when_remainder_is_below_minimum_bond() {
	ExtBuilder::default().minimum_bond(5).build().execute_with(|| {
		// `ExtBuilder` auto configures some IDs < 100 with funds
		// making tests less clear
		let stash = 100_u64;
		let controller = 101_u64;
		let initial_bond = Staking::minimum_bond() + 50;

		assert!(Balances::deposit_into_existing(&stash, initial_bond).is_ok());

		// Bond with slightly more than the minimum bond
		assert_ok!(Staking::bond(
			Origin::signed(stash),
			controller,
			initial_bond,
			RewardDestination::Controller
		));
		assert_eq!(
			Staking::ledger(&controller),
			Some(StakingLedger {
				stash,
				total: initial_bond,
				active: initial_bond,
				unlocking: vec![],
			})
		);
		// Unbond just enough to put the active stash below the minimum bond amount
		assert_ok!(Staking::unbond(Origin::signed(controller), 51));
		assert_eq!(
			Staking::ledger(&controller),
			Some(StakingLedger {
				stash,
				total: initial_bond,
				active: Zero::zero(),
				unlocking: vec![UnlockChunk {
					value: initial_bond,
					era: 3
				}],
			})
		);
	});
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
		start_era(1);

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

		start_era(2);
		assert_eq!(Staking::current_era(), 2);

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
				unlocking: vec![UnlockChunk { value: 900, era: 2 + 3 },]
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
				unlocking: vec![]
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
				]
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
				unlocking: vec![UnlockChunk { value: 300, era: 5 }, UnlockChunk { value: 100, era: 5 },]
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
		start_era(1);

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

		start_era(2);

		// Unbond some of the funds in stash.
		Staking::unbond(Origin::signed(10), 400).unwrap();
		assert_eq!(
			Staking::ledger(&10),
			Some(StakingLedger {
				stash: 11,
				total: 1000,
				active: 600,
				unlocking: vec![UnlockChunk { value: 400, era: 2 + 3 },]
			})
		);

		start_era(3);

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
				]
			})
		);

		start_era(4);

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
				]
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
				]
			})
		);
	})
}

#[test]
fn slot_stake_is_least_staked_validator_and_exposure_defines_maximum_punishment() {
	// Test that slot_stake is determined by the least staked validator
	// Test that slot_stake is the maximum punishment that can happen to a validator
	ExtBuilder::default()
		.nominate(false)
		.fair(false)
		.build()
		.execute_with(|| {
			// Confirm validator count is 2
			assert_eq!(Staking::validator_count(), 2);
			// Confirm account 10 and 20 are validators
			assert!(<Validators<Test>>::contains_key(&11) && <Validators<Test>>::contains_key(&21));

			assert_eq!(Staking::stakers(&11).total, 1000);
			assert_eq!(Staking::stakers(&21).total, 2000);

			// We confirm initialized slot_stake is this value
			assert_eq!(Staking::slot_stake(), Staking::stakers(&11).total);

			// Now lets lower account 20 stake
			<Stakers<Test>>::insert(
				&21,
				Exposure {
					total: 69,
					own: 69,
					others: vec![],
				},
			);
			assert_eq!(Staking::stakers(&21).total, 69);
			<Ledger<Test>>::insert(
				&20,
				StakingLedger {
					stash: 22,
					total: 69,
					active: 69,
					unlocking: vec![],
				},
			);

			// Compute total payout now for whole duration as other parameter won't change
			let total_payout_0: Balance = Rewards::calculate_next_reward_payout();
			assert!(total_payout_0 > 100); // Test is meaningful if reward something
			Rewards::reward_by_ids(vec![(11, 1)]);
			Rewards::reward_by_ids(vec![(21, 1)]);

			// New era --> rewards are paid to stash account --> stakes are not change
			start_era(1);
			Staking::on_initialize(System::block_number() + 1);

			// -- balances not change
			assert_eq!(Staking::stakers(&11).total, 1000);
			assert_eq!(Staking::stakers(&21).total, 69);

			// -- stash account balance + reward
			assert_eq!(Balances::free_balance(&11), 1000 + total_payout_0 / 2);
			assert_eq!(Balances::free_balance(&21), 2000 + total_payout_0 / 2);

			// -- slot stake should NOT increased.
			assert_eq!(Staking::slot_stake(), 69);

			check_exposure_all();
			check_nominator_all();
		});
}

#[test]
fn on_free_balance_zero_stash_removes_validator() {
	// Tests that validator storage items are cleaned up when stash is empty
	// Tests that storage items are untouched when controller is empty
	ExtBuilder::default().existential_deposit(10).build().execute_with(|| {
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
		assert_eq!(Rewards::payee(&11), 11);

		// Reduce free_balance of controller to 0
		let _ = Balances::slash(&10, u64::max_value());

		// Check the balance of the stash account has not been touched
		assert_eq!(Balances::free_balance(11), 256000);
		// Check these two accounts are still bonded
		assert_eq!(Staking::bonded(&11), Some(10));

		// Check storage items have not changed
		assert!(<Ledger<Test>>::contains_key(&10));
		assert!(<Bonded<Test>>::contains_key(&11));
		assert!(<Validators<Test>>::contains_key(&11));
		assert_eq!(Rewards::payee(&11), 11);

		// Reduce free_balance of stash to 0
		let _ = Balances::slash(&11, u64::max_value());
		// Check total balance of stash
		assert_eq!(Balances::total_balance(&11), 0);

		// Reap the stash
		assert_ok!(Staking::reap_stash(Origin::none(), 11));

		// Check storage items do not exist
		assert!(!<Ledger<Test>>::contains_key(&10));
		assert!(!<Bonded<Test>>::contains_key(&11));
		assert!(!<Validators<Test>>::contains_key(&11));
		assert!(!<Nominators<Test>>::contains_key(&11));
	});
}

#[test]
fn on_free_balance_zero_stash_removes_nominator() {
	// Tests that nominator storage items are cleaned up when stash is empty
	// Tests that storage items are untouched when controller is empty
	ExtBuilder::default().existential_deposit(10).build().execute_with(|| {
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
		assert_eq!(Rewards::payee(&11), 11);

		// Reduce free_balance of controller to 0
		let _ = Balances::slash(&10, u64::max_value());
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
		assert_eq!(Rewards::payee(&11), 11);

		// Reduce free_balance of stash to 0
		let _ = Balances::slash(&11, u64::max_value());
		// Check total balance of stash
		assert_eq!(Balances::total_balance(&11), 0);

		// Reap the stash
		assert_ok!(Staking::reap_stash(Origin::none(), 11));

		// Check storage items do not exist
		assert!(!<Ledger<Test>>::contains_key(&10));
		assert!(!<Bonded<Test>>::contains_key(&11));
		assert!(!<Validators<Test>>::contains_key(&11));
		assert!(!<Nominators<Test>>::contains_key(&11));
	});
}

#[test]
fn switching_roles() {
	// Test that it should be possible to switch between roles (nominator, validator, idle) with minimal overhead.
	ExtBuilder::default().nominate(false).build().execute_with(|| {
		Timestamp::set_timestamp(1); // Initialize time.

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

		// new block
		start_session(2);

		// no change
		assert_eq_uvec!(validator_controllers(), vec![20, 10]);

		// new block
		start_session(3);

		// no change
		assert_eq_uvec!(validator_controllers(), vec![20, 10]);

		// new block --> ne era --> new validators
		start_session(4);

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

		start_session(5);
		assert_eq_uvec!(validator_controllers(), vec![6, 10]);

		start_session(6);
		assert_eq_uvec!(validator_controllers(), vec![6, 10]);

		// ne era
		start_session(7);
		assert_eq_uvec!(validator_controllers(), vec![2, 20]);

		check_exposure_all();
		check_nominator_all();
	});
}

#[test]
fn wrong_vote_is_null() {
	ExtBuilder::default()
		.nominate(false)
		.validator_pool(true)
		.build()
		.execute_with(|| {
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
			start_era(1);
			advance_session();

			assert_eq_uvec!(validator_controllers(), vec![20, 10]);
		});
}

#[test]
fn bond_with_no_staked_value() {
	// Behavior when someone bonds with no staked value.
	// Particularly when she votes and the candidate is elected.
	ExtBuilder::default()
		.validator_count(3)
		.existential_deposit(5)
		.nominate(false)
		.minimum_validator_count(1)
		.minimum_bond(3)
		.build()
		.execute_with(|| {
			// Can't bond with value < minimum bond
			assert_noop!(
				Staking::bond(Origin::signed(1), 2, 1, RewardDestination::Controller),
				Error::<Test>::InsufficientBond,
			);
			// bonded with absolute minimum value possible.
			assert_ok!(Staking::bond(
				Origin::signed(1),
				2,
				Staking::minimum_bond(),
				RewardDestination::Controller
			));
			assert_eq!(Balances::locks(&1)[0].amount, Staking::minimum_bond());

			// unbond leaving active < minimum bond will cause all to be unbonded.
			assert_ok!(Staking::unbond(Origin::signed(2), 1));
			assert_eq!(
				Staking::ledger(2),
				Some(StakingLedger {
					stash: 1,
					active: 0,
					total: Staking::minimum_bond(),
					unlocking: vec![UnlockChunk {
						value: Staking::minimum_bond(),
						era: 3
					}]
				})
			);

			start_era(1);
			start_era(2);

			// not yet removed.
			assert_ok!(Staking::withdraw_unbonded(Origin::signed(2)));
			assert!(Staking::ledger(2).is_some());
			assert_eq!(Balances::locks(&1)[0].amount, Staking::minimum_bond());

			start_era(3);

			// poof. Account 1 is removed from the staking system.
			assert_ok!(Staking::withdraw_unbonded(Origin::signed(2)));
			assert!(Staking::ledger(2).is_none());
			assert_eq!(Balances::locks(&1).len(), 0);
		});
}

#[test]
fn bond_with_little_staked_value_bounded_by_slot_stake() {
	// Behavior when someone bonds with little staked value.
	// Particularly when she votes and the candidate is elected.
	ExtBuilder::default()
		.validator_count(3)
		.nominate(false)
		.minimum_validator_count(1)
		.build()
		.execute_with(|| {
			// setup
			assert_ok!(Staking::chill(Origin::signed(30)));
			assert_ok!(Staking::set_payee(Origin::signed(10), RewardDestination::Controller));
			let init_balance_2 = Balances::free_balance(&2);
			let init_balance_10 = Balances::free_balance(&10);

			// Stingy validator.
			assert_ok!(Staking::bond(
				Origin::signed(1),
				2,
				Staking::minimum_bond(),
				RewardDestination::Controller
			));
			assert_ok!(Staking::validate(Origin::signed(2), ValidatorPrefs::default()));

			let total_payout_0: Balance = Rewards::calculate_next_reward_payout();
			assert!(total_payout_0 > 100); // Test is meaningful if reward something
			reward_all_elected();

			start_era(1);
			Staking::on_initialize(System::block_number() + 1);
			advance_session();

			// 2 is elected.
			// and fucks up the slot stake.
			assert_eq_uvec!(validator_controllers(), vec![20, 10, 2]);
			assert_eq!(Staking::slot_stake(), 1);

			// Old ones are rewarded.
			assert_eq!(Balances::free_balance(10), init_balance_10 + total_payout_0 / 3);
			// no rewards paid to 2. This was initial election.
			assert_eq!(Balances::free_balance(2), init_balance_2);

			let total_payout_1: Balance = Rewards::calculate_next_reward_payout();
			assert!(total_payout_1 > 100); // Test is meaningful if reward something
			reward_all_elected();
			start_era(2);
			Staking::on_initialize(System::block_number() + 1);

			assert_eq_uvec!(validator_controllers(), vec![20, 10, 2]);
			assert_eq!(Staking::slot_stake(), 1);

			assert_eq!(Balances::free_balance(2), init_balance_2 + total_payout_1 / 3);
			assert_eq!(
				Balances::free_balance(&10),
				init_balance_10 + total_payout_0 / 3 + total_payout_1 / 3,
			);
			check_exposure_all();
			check_nominator_all();
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

			System::set_block_number(1);
			Session::on_initialize(System::block_number());

			assert_eq!(validator_controllers().len(), 1);
			check_exposure_all();
			check_nominator_all();
		})
}

#[test]
fn phragmen_should_not_overflow_validators() {
	ExtBuilder::default().nominate(false).build().execute_with(|| {
		let _ = Staking::chill(Origin::signed(10));
		let _ = Staking::chill(Origin::signed(20));

		bond_validator(3, 2, u64::max_value());
		bond_validator(5, 4, u64::max_value());

		bond_nominator(7, 6, u64::max_value() / 2, vec![3, 5]);
		bond_nominator(9, 8, u64::max_value() / 2, vec![3, 5]);

		start_era(1);
		advance_session();

		assert_eq_uvec!(validator_controllers(), vec![4, 2]);

		// This test will fail this. Will saturate.
		// check_exposure_all();
		assert_eq!(Staking::stakers(3).total, u64::max_value());
		assert_eq!(Staking::stakers(5).total, u64::max_value());
	})
}

#[test]
fn phragmen_should_not_overflow_nominators() {
	ExtBuilder::default().nominate(false).build().execute_with(|| {
		let _ = Staking::chill(Origin::signed(10));
		let _ = Staking::chill(Origin::signed(20));

		bond_validator(3, 2, u64::max_value() / 2);
		bond_validator(5, 4, u64::max_value() / 2);

		bond_nominator(7, 6, u64::max_value(), vec![3, 5]);
		bond_nominator(9, 8, u64::max_value(), vec![3, 5]);

		start_era(1);
		advance_session();

		assert_eq_uvec!(validator_controllers(), vec![4, 2]);

		// Saturate.
		assert_eq!(Staking::stakers(3).total, u64::max_value());
		assert_eq!(Staking::stakers(5).total, u64::max_value());
	})
}

#[test]
fn phragmen_should_not_overflow_ultimate() {
	ExtBuilder::default().nominate(false).build().execute_with(|| {
		bond_validator(3, 2, u64::max_value());
		bond_validator(5, 4, u64::max_value());

		bond_nominator(7, 6, u64::max_value(), vec![3, 5]);
		bond_nominator(9, 8, u64::max_value(), vec![3, 5]);

		start_era(1);
		advance_session();

		assert_eq_uvec!(validator_controllers(), vec![4, 2]);

		// Saturate.
		assert_eq!(Staking::stakers(3).total, u64::max_value());
		assert_eq!(Staking::stakers(5).total, u64::max_value());
	})
}

#[test]
fn reward_validator_slashing_validator_doesnt_overflow() {
	ExtBuilder::default().build().execute_with(|| {
		let stake = u32::max_value() as u64 * 2;
		let reward_slash = u32::max_value() as u64 * 2;

		// Assert multiplication overflows in balance arithmetic.
		assert!(stake.checked_mul(reward_slash).is_none());

		// Set staker
		let _ = Balances::make_free_balance_be(&11, stake);
		<Stakers<Test>>::insert(
			&11,
			Exposure {
				total: stake,
				own: stake,
				others: vec![],
			},
		);

		// Check reward
		let _ = Balances::deposit_into_existing(&11, stake);
		assert_eq!(Balances::total_balance(&11), stake * 2);

		// Set staker
		let _ = Balances::make_free_balance_be(&11, stake);
		let _ = Balances::make_free_balance_be(&2, stake);

		// only slashes out of bonded stake are applied. without this line,
		// it is 0.
		Staking::bond(Origin::signed(2), 20000, stake - 1, RewardDestination::default()).unwrap();
		<Stakers<Test>>::insert(
			&11,
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
				offender: (11, Staking::stakers(&11)),
				reporters: vec![],
			}],
			&[Perbill::from_percent(100)],
		);

		assert_eq!(Balances::total_balance(&11), stake - 1);
		assert_eq!(Balances::total_balance(&2), 1);
	})
}

#[test]
fn unbonded_balance_is_still_slashable() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Staking::slashable_balance_of(&11), 1000);
		assert_ok!(Staking::unbond(Origin::signed(10), 800));
		// active amount + unlocking amounts are still slashable
		assert_eq!(Staking::slashable_balance_of(&11), 1000);
	})
}

#[test]
fn era_is_always_same_length() {
	// This ensures that the sessions is always of the same length if there is no forcing no
	// session changes.
	ExtBuilder::default().build().execute_with(|| {
		start_era(1);
		assert_eq!(Staking::current_era_start_session_index(), SessionsPerEra::get());

		start_era(2);
		assert_eq!(Staking::current_era_start_session_index(), SessionsPerEra::get() * 2);

		let session = Session::current_index();
		ForceEra::put(Forcing::ForceNew);
		advance_session();
		assert_eq!(Staking::current_era(), 3);
		assert_eq!(Staking::current_era_start_session_index(), session + 1);

		start_era(4);
		assert_eq!(
			Staking::current_era_start_session_index(),
			session + SessionsPerEra::get() + 1
		);
	});
}

#[test]
fn offence_forces_new_era() {
	ExtBuilder::default().build().execute_with(|| {
		on_offence_now(
			&[OffenceDetails {
				offender: (11, Staking::stakers(&11)),
				reporters: vec![],
			}],
			&[Perbill::from_percent(5)],
		);

		assert_eq!(Staking::force_era(), Forcing::ForceNew);
	});
}

#[test]
fn offence_ensures_new_era_without_clobbering() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Staking::force_new_era_always(Origin::root()));

		on_offence_now(
			&[OffenceDetails {
				offender: (11, Staking::stakers(&11)),
				reporters: vec![],
			}],
			&[Perbill::from_percent(5)],
		);

		assert_eq!(Staking::force_era(), Forcing::ForceAlways);
	});
}

#[test]
fn offence_deselects_validator_when_slash_is_zero() {
	ExtBuilder::default().build().execute_with(|| {
		assert!(<Validators<Test>>::contains_key(11));
		on_offence_now(
			&[OffenceDetails {
				offender: (11, Staking::stakers(&11)),
				reporters: vec![],
			}],
			&[Perbill::from_percent(0)],
		);
		assert_eq!(Staking::force_era(), Forcing::ForceNew);
		assert!(!<Validators<Test>>::contains_key(11));
	});
}

#[test]
fn slashing_performed_according_exposure() {
	// This test checks that slashing is performed according the exposure (or more precisely,
	// historical exposure), not the current balance.
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Staking::stakers(&11).own, 1000);

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
	ExtBuilder::default().build().execute_with(|| {
		start_era(1);

		assert!(<Validators<Test>>::contains_key(11));
		on_offence_now(
			&[OffenceDetails {
				offender: (11, Staking::stakers(&11)),
				reporters: vec![],
			}],
			&[Perbill::from_percent(0)],
		);
		assert_eq!(Staking::force_era(), Forcing::ForceNew);
		assert!(!<Validators<Test>>::contains_key(11));

		start_era(2);

		Staking::validate(Origin::signed(10), Default::default()).unwrap();
		assert_eq!(Staking::force_era(), Forcing::NotForcing);
		assert!(<Validators<Test>>::contains_key(11));

		start_era(3);

		// this staker is in a new slashing span now, having re-registered after
		// their prior slash.

		on_offence_in_era(
			&[OffenceDetails {
				offender: (11, Staking::stakers(&11)),
				reporters: vec![],
			}],
			&[Perbill::from_percent(0)],
			1,
		);

		// not for zero-slash.
		assert_eq!(Staking::force_era(), Forcing::NotForcing);
		assert!(<Validators<Test>>::contains_key(11));

		on_offence_in_era(
			&[OffenceDetails {
				offender: (11, Staking::stakers(&11)),
				reporters: vec![],
			}],
			// NOTE: A 100% slash here would clean up the account, causing de-registration.
			&[Perbill::from_percent(95)],
			1,
		);

		// or non-zero.
		assert_eq!(Staking::force_era(), Forcing::NotForcing);
		assert!(<Validators<Test>>::contains_key(11));
		assert_ledger_consistent(11);
	});
}

#[test]
fn reporters_receive_their_slice() {
	// This test verifies that the reporters of the offence receive their slice from the slashed
	// amount.
	ExtBuilder::default().build().execute_with(|| {
		// The reporters' reward is calculated from the total exposure.
		let initial_balance = 1125;

		assert_eq!(Staking::stakers(&11).total, initial_balance);

		on_offence_now(
			&[OffenceDetails {
				offender: (11, Staking::stakers(&11)),
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
		assert_ledger_consistent(11);
	});
}

#[test]
fn subsequent_reports_in_same_span_pay_out_less() {
	// This test verifies that the reporters of the offence receive their slice from the slashed
	// amount.
	ExtBuilder::default().build().execute_with(|| {
		// The reporters' reward is calculated from the total exposure.
		let initial_balance = 1125;

		assert_eq!(Staking::stakers(&11).total, initial_balance);

		on_offence_now(
			&[OffenceDetails {
				offender: (11, Staking::stakers(&11)),
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
				offender: (11, Staking::stakers(&11)),
				reporters: vec![1],
			}],
			&[Perbill::from_percent(50)],
		);

		let prior_payout = reward;

		// F1 * (reward_proportion * slash - prior_payout)
		// 50% * (10% * (initial_balance / 2) - prior_payout)
		let reward = ((initial_balance / 20) - prior_payout) / 2;
		assert_eq!(Balances::free_balance(1), 10 + prior_payout + reward);
		assert_ledger_consistent(11);
	});
}

#[test]
fn invulnerables_are_not_slashed() {
	// For invulnerable validators no slashing is performed.
	ExtBuilder::default().invulnerables(vec![11]).build().execute_with(|| {
		assert_eq!(Balances::free_balance(11), 1000);
		assert_eq!(Balances::free_balance(21), 2000);

		let exposure = Staking::stakers(&21);
		let initial_balance = Staking::slashable_balance_of(&21);

		let nominator_balances: Vec<_> = exposure.others.iter().map(|o| Balances::free_balance(&o.who)).collect();

		on_offence_now(
			&[
				OffenceDetails {
					offender: (11, Staking::stakers(&11)),
					reporters: vec![],
				},
				OffenceDetails {
					offender: (21, Staking::stakers(&21)),
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
		assert_ledger_consistent(11);
		assert_ledger_consistent(21);
	});
}

#[test]
fn invulnerables_can_be_set() {
	// Invulnerable validator set can be modified
	ExtBuilder::default().invulnerables(vec![21]).build().execute_with(|| {
		System::set_block_number(1);

		assert_eq!(Balances::free_balance(11), 1000);
		assert_eq!(Balances::free_balance(21), 2000);

		//Changing the invulnerables set from [21] to [11].
		let _ = Staking::set_invulnerables(Origin::root(), vec![11]);
		assert_eq!(
			System::events(),
			vec![EventRecord {
				phase: Phase::Initialization,
				event: mock::MetaEvent::staking(RawEvent::SetInvulnerables(vec![11])),
				topics: vec![],
			}]
		);

		let exposure = Staking::stakers(&21);
		let initial_balance = Staking::slashable_balance_of(&21);

		let nominator_balances: Vec<_> = exposure.others.iter().map(|o| Balances::free_balance(&o.who)).collect();

		on_offence_now(
			&[
				OffenceDetails {
					offender: (11, Staking::stakers(&11)),
					reporters: vec![],
				},
				OffenceDetails {
					offender: (21, Staking::stakers(&21)),
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
		assert_ledger_consistent(11);
		assert_ledger_consistent(21);
	});
}

#[test]
fn invulnerables_can_only_be_set_by_root() {
	// Invulnerable validator set can be modified
	ExtBuilder::default().invulnerables(vec![11]).build().execute_with(|| {
		assert_eq!(Staking::invulnerables(), vec![11]);

		//try to change the invulnerable set without root access
		let _ = Staking::set_invulnerables(Origin::signed(21), vec![21]);
		assert_eq!(Staking::invulnerables(), vec![11]);

		//Changing the invulnerables with root access.
		let _ = Staking::set_invulnerables(Origin::root(), vec![21, 11]);
		assert_eq!(Staking::invulnerables(), vec![21, 11]);
	});
}

#[test]
fn invulnerables_be_empty() {
	ExtBuilder::default()
		.invulnerables(vec![11, 21])
		.build()
		.execute_with(|| {
			assert_eq!(Staking::invulnerables(), vec![11, 21]);

			//Changing the invulnerables with root access.
			let invulnerables: Vec<AccountId> = vec![];
			let _ = Staking::set_invulnerables(Origin::root(), invulnerables.clone());
			assert_eq!(Staking::invulnerables(), invulnerables);
		});
}

#[test]
fn dont_slash_if_fraction_is_zero() {
	// Don't slash if the fraction is zero.
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Balances::free_balance(11), 1000);

		on_offence_now(
			&[OffenceDetails {
				offender: (11, Staking::stakers(&11)),
				reporters: vec![],
			}],
			&[Perbill::from_percent(0)],
		);

		// The validator hasn't been slashed. The new era is not forced.
		assert_eq!(Balances::free_balance(11), 1000);
		assert_ledger_consistent(11);
	});
}

#[test]
fn only_slash_for_max_in_era() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Balances::free_balance(11), 1000);

		on_offence_now(
			&[OffenceDetails {
				offender: (11, Staking::stakers(&11)),
				reporters: vec![],
			}],
			&[Perbill::from_percent(50)],
		);

		// The validator has been slashed and has been force-chilled.
		assert_eq!(Balances::free_balance(11), 500);
		assert_eq!(Staking::force_era(), Forcing::ForceNew);

		on_offence_now(
			&[OffenceDetails {
				offender: (11, Staking::stakers(&11)),
				reporters: vec![],
			}],
			&[Perbill::from_percent(25)],
		);

		// The validator has not been slashed additionally.
		assert_eq!(Balances::free_balance(11), 500);

		on_offence_now(
			&[OffenceDetails {
				offender: (11, Staking::stakers(&11)),
				reporters: vec![],
			}],
			&[Perbill::from_percent(60)],
		);

		// The validator got slashed 10% more.
		assert_eq!(Balances::free_balance(11), 400);
		assert_ledger_consistent(11);
	})
}

#[test]
fn garbage_collection_after_slashing() {
	ExtBuilder::default().existential_deposit(2).build().execute_with(|| {
		assert_eq!(Balances::free_balance(11), 256_000);

		on_offence_now(
			&[OffenceDetails {
				offender: (11, Staking::stakers(&11)),
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
				offender: (11, Staking::stakers(&11)),
				reporters: vec![],
			}],
			&[Perbill::from_percent(100)],
		);

		// validator and nominator slash in era are garbage-collected by era change,
		// so we don't test those here.

		assert_eq!(Balances::free_balance(11), 0);
		assert_eq!(Balances::total_balance(&11), 0);

		let slashing_spans = <Staking as crate::Store>::SlashingSpans::get(&11).unwrap();
		assert_eq!(slashing_spans.iter().count(), 2);

		assert_ok!(Staking::reap_stash(Origin::none(), 11));

		assert!(<Staking as crate::Store>::SlashingSpans::get(&11).is_none());
		assert_eq!(<Staking as crate::Store>::SpanSlash::get(&(11, 0)).amount_slashed(), &0);
	})
}

#[test]
fn garbage_collection_on_window_pruning() {
	ExtBuilder::default().build().execute_with(|| {
		start_era(1);

		assert_eq!(Balances::free_balance(11), 1000);

		let exposure = Staking::stakers(&11);
		assert_eq!(Balances::free_balance(101), 2000);
		let nominated_value = exposure.others.iter().find(|o| o.who == 101).unwrap().value;

		on_offence_now(
			&[OffenceDetails {
				offender: (11, Staking::stakers(&11)),
				reporters: vec![],
			}],
			&[Perbill::from_percent(10)],
		);

		let now = Staking::current_era();

		assert_eq!(Balances::free_balance(11), 900);
		assert_eq!(Balances::free_balance(101), 2000 - (nominated_value / 10));

		assert!(<Staking as crate::Store>::ValidatorSlashInEra::get(&now, &11).is_some());
		assert!(<Staking as crate::Store>::NominatorSlashInEra::get(&now, &101).is_some());

		// + 1 because we have to exit the bonding window.
		for era in (0..(BondingDuration::get() + 1)).map(|offset| offset + now + 1) {
			assert!(<Staking as crate::Store>::ValidatorSlashInEra::get(&now, &11).is_some());
			assert!(<Staking as crate::Store>::NominatorSlashInEra::get(&now, &101).is_some());

			start_era(era);
		}

		assert!(<Staking as crate::Store>::ValidatorSlashInEra::get(&now, &11).is_none());
		assert!(<Staking as crate::Store>::NominatorSlashInEra::get(&now, &101).is_none());
	})
}

#[test]
fn slashing_nominators_by_span_max() {
	ExtBuilder::default().build().execute_with(|| {
		start_era(1);
		start_era(2);
		start_era(3);

		assert_eq!(Balances::free_balance(11), 1000);
		assert_eq!(Balances::free_balance(21), 2000);
		assert_eq!(Balances::free_balance(101), 2000);
		assert_eq!(Staking::slashable_balance_of(&21), 1000);

		let exposure_11 = Staking::stakers(&11);
		let exposure_21 = Staking::stakers(&21);
		assert_eq!(Balances::free_balance(101), 2000);
		let nominated_value_11 = exposure_11.others.iter().find(|o| o.who == 101).unwrap().value;
		let nominated_value_21 = exposure_21.others.iter().find(|o| o.who == 101).unwrap().value;

		on_offence_in_era(
			&[OffenceDetails {
				offender: (11, Staking::stakers(&11)),
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
				offender: (21, Staking::stakers(&21)),
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
				offender: (11, Staking::stakers(&11)),
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
	ExtBuilder::default().build().execute_with(|| {
		start_era(1);
		start_era(2);
		start_era(3);

		assert_eq!(Balances::free_balance(21), 2000);
		assert_eq!(Staking::slashable_balance_of(&21), 1000);

		let get_span = |account| <Staking as crate::Store>::SlashingSpans::get(&account).unwrap();

		on_offence_now(
			&[OffenceDetails {
				offender: (21, Staking::stakers(&21)),
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

		start_era(4);

		assert_eq!(Staking::slashable_balance_of(&21), 900);

		on_offence_now(
			&[OffenceDetails {
				offender: (21, Staking::stakers(&21)),
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
	ExtBuilder::default().slash_defer_duration(2).build().execute_with(|| {
		start_era(1);

		assert_eq!(Balances::free_balance(11), 1000);

		let exposure = Staking::stakers(&11);
		assert_eq!(Balances::free_balance(101), 2000);
		let nominated_value = exposure.others.iter().find(|o| o.who == 101).unwrap().value;

		on_offence_now(
			&[OffenceDetails {
				offender: (11, Staking::stakers(&11)),
				reporters: vec![],
			}],
			&[Perbill::from_percent(10)],
		);

		assert_eq!(Balances::free_balance(11), 1000);
		assert_eq!(Balances::free_balance(101), 2000);

		start_era(2);

		assert_eq!(Balances::free_balance(11), 1000);
		assert_eq!(Balances::free_balance(101), 2000);

		start_era(3);

		assert_eq!(Balances::free_balance(11), 1000);
		assert_eq!(Balances::free_balance(101), 2000);

		// at the start of era 4, slashes from era 1 are processed,
		// after being deferred for at least 2 full eras.
		start_era(4);

		assert_eq!(Balances::free_balance(11), 900);
		assert_eq!(Balances::free_balance(101), 2000 - (nominated_value / 10));
	})
}

#[test]
fn remove_deferred() {
	ExtBuilder::default().slash_defer_duration(2).build().execute_with(|| {
		start_era(1);

		assert_eq!(Balances::free_balance(11), 1000);

		let exposure = Staking::stakers(&11);
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

		start_era(2);

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

		start_era(3);

		assert_eq!(Balances::free_balance(11), 1000);
		assert_eq!(Balances::free_balance(101), 2000);

		// at the start of era 4, slashes from era 1 are processed,
		// after being deferred for at least 2 full eras.
		start_era(4);

		// the first slash for 10% was cancelled, so no effect.
		assert_eq!(Balances::free_balance(11), 1000);
		assert_eq!(Balances::free_balance(101), 2000);

		start_era(5);

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
	ExtBuilder::default().slash_defer_duration(2).build().execute_with(|| {
		start_era(1);

		assert_eq!(Balances::free_balance(11), 1000);

		let exposure = Staking::stakers(&11);
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
				offender: (21, Staking::stakers(&21)),
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

#[test]
fn slash_kicks_validators_not_nominators() {
	ExtBuilder::default().build().execute_with(|| {
		start_era(1);

		assert_eq!(Balances::free_balance(11), 1000);

		let exposure = Staking::stakers(&11);
		assert_eq!(Balances::free_balance(101), 2000);
		let nominated_value = exposure.others.iter().find(|o| o.who == 101).unwrap().value;

		on_offence_now(
			&[OffenceDetails {
				offender: (11, exposure.clone()),
				reporters: vec![],
			}],
			&[Perbill::from_percent(10)],
		);

		assert_eq!(Balances::free_balance(11), 900);
		assert_eq!(Balances::free_balance(101), 2000 - (nominated_value / 10));

		// This is the best way to check that the validator was chilled; `get` will
		// return default value.
		for (stash, _) in <Staking as Store>::Validators::iter() {
			assert!(stash != 11);
		}

		let nominations = <Staking as Store>::Nominators::get(&101).unwrap();

		// and make sure that the vote will be ignored even if the validator
		// re-registers.
		let last_slash = <Staking as Store>::SlashingSpans::get(&11)
			.unwrap()
			.last_nonzero_slash();
		assert!(nominations.submitted_in < last_slash);
	});
}

#[test]
fn zero_slash_keeps_nominators() {
	ExtBuilder::default().build().execute_with(|| {
		start_era(1);

		assert_eq!(Balances::free_balance(11), 1000);

		let exposure = Staking::stakers(&11);
		assert_eq!(Balances::free_balance(101), 2000);

		on_offence_now(
			&[OffenceDetails {
				offender: (11, exposure.clone()),
				reporters: vec![],
			}],
			&[Perbill::from_percent(0)],
		);

		assert_eq!(Balances::free_balance(11), 1000);
		assert_eq!(Balances::free_balance(101), 2000);

		// This is the best way to check that the validator was chilled; `get` will
		// return default value.
		for (stash, _) in <<Staking as Store>::Validators>::iter() {
			assert!(stash != 11);
		}

		let nominations = <Staking as Store>::Nominators::get(&101).unwrap();

		// and make sure that the vote will not be ignored, because the slash was
		// zero.
		let last_slash = <Staking as Store>::SlashingSpans::get(&11)
			.unwrap()
			.last_nonzero_slash();
		assert!(nominations.submitted_in >= last_slash);
	});
}

#[test]
fn show_that_max_commission_is_100_percent() {
	ExtBuilder::default().build().execute_with(|| {
		let prefs = ValidatorPrefs {
			commission: Perbill::from_fraction(1.5),
		};

		assert_ok!(Staking::validate(Origin::signed(10), prefs.clone()));

		let stored_prefs = <Staking as Store>::Validators::get(&11);

		let total_rewards: u32 = 1_000;
		let expected_rewards: u32 = 1_000;

		assert_eq!(stored_prefs.commission * total_rewards, expected_rewards);
	});
}

#[test]
fn set_minimum_bond_works() {
	ExtBuilder::default().minimum_bond(7357).build().execute_with(|| {
		System::set_block_number(1);
		// Non-root accounts cannot set minimum bond
		assert_noop!(Staking::set_minimum_bond(Origin::signed(1), 123), BadOrigin);
		assert_eq!(Staking::minimum_bond(), 7357);
		assert_eq!(System::events(), vec![]);

		// Root accounts can set minimum bond
		assert_ok!(Staking::set_minimum_bond(Origin::root(), 537));
		assert_eq!(Staking::minimum_bond(), 537);
		assert_eq!(
			System::events(),
			vec![EventRecord {
				phase: Phase::Initialization,
				event: mock::MetaEvent::staking(RawEvent::SetMinimumBond(537)),
				topics: vec![],
			}]
		);
	});
}

#[test]
#[should_panic(expected = "Minimum bond must be greater than zero.")]
fn minimum_bond_in_genesis_config_must_be_greater_than_zero() {
	ExtBuilder::default().minimum_bond(0).build().execute_with(|| {});
}

#[test]
fn unbond_works_when_remaining_active_is_more_than_minimum_bond() {
	ExtBuilder::default().minimum_bond(5).build().execute_with(|| {
		// `ExtBuilder` auto configures some IDs < 100 with funds
		// making tests less clear
		let stash = 100_u64;
		let controller = 101_u64;
		let initial_bond = Staking::minimum_bond() + 50;

		assert!(Balances::deposit_into_existing(&stash, initial_bond).is_ok());

		// Bond with slightly more than the minimum bond
		assert_ok!(Staking::bond(
			Origin::signed(stash),
			controller,
			initial_bond,
			RewardDestination::Controller
		));
		assert_eq!(
			Staking::ledger(&controller),
			Some(StakingLedger {
				stash,
				total: initial_bond,
				active: initial_bond,
				unlocking: vec![],
			})
		);
		let unbond_amount = 40;
		assert_ok!(Staking::unbond(Origin::signed(controller), unbond_amount));
		assert_eq!(
			Staking::ledger(&controller),
			Some(StakingLedger {
				stash,
				total: initial_bond,
				active: initial_bond - unbond_amount,
				unlocking: vec![UnlockChunk {
					value: unbond_amount,
					era: 3
				}],
			})
		);
	});
}

#[test]
fn unbond_should_remove_validator_from_future_era_when_remaining_bond_is_less_than_minimum_bond() {
	ExtBuilder::default().minimum_bond(5).build().execute_with(|| {
		// `ExtBuilder` auto configures some IDs < 100 with funds
		// making tests less clear
		let stash = 100_u64;
		let controller = 101_u64;
		let initial_bond = Staking::minimum_bond() + 50;

		assert!(Balances::deposit_into_existing(&stash, initial_bond).is_ok());

		// Bond with slightly more than the minimum bond
		assert_ok!(Staking::bond(
			Origin::signed(stash),
			controller,
			initial_bond,
			RewardDestination::Controller
		));
		assert_ok!(Staking::validate(Origin::signed(controller), ValidatorPrefs::default()));
		// Unbond just enough to put the active stash below the minimum bond amount
		assert_ok!(Staking::unbond(Origin::signed(controller), 51));

		// Stash will not be considered as a validator next era
		assert!(!<Staking as crate::Store>::Validators::contains_key(stash));

		// Stash cannot validate again with inactive bond
		assert_noop!(
			Staking::validate(Origin::signed(controller), ValidatorPrefs::default()),
			Error::<Test>::InsufficientBond,
		);
	});
}

#[test]
fn unbond_should_remove_nominator_from_future_era_when_remaining_bond_is_less_than_minimum_bond() {
	ExtBuilder::default().minimum_bond(5).build().execute_with(|| {
		// `ExtBuilder` auto configures some IDs < 100 with funds
		// making tests less clear
		let stash = 100_u64;
		let controller = 101_u64;
		let initial_bond = Staking::minimum_bond() + 50;

		assert!(Balances::deposit_into_existing(&stash, initial_bond).is_ok());

		// Bond with slightly more than the minimum bond
		assert_ok!(Staking::bond(
			Origin::signed(stash),
			controller,
			initial_bond,
			RewardDestination::Controller
		));
		assert_ok!(Staking::nominate(Origin::signed(controller), vec![stash]));
		// Unbond just enough to put the active stash below the minimum bond amount
		assert_ok!(Staking::unbond(Origin::signed(controller), 51));

		// Stash will not be considered as a nominator next era
		assert!(!<Staking as crate::Store>::Nominators::contains_key(stash));

		// Stash cannot nominate again with inactive bond
		assert_noop!(
			Staking::nominate(Origin::signed(controller), vec![1, 2, 3, 4, 5]),
			Error::<Test>::InsufficientBond,
		);
	});
}

#[test]
fn validators_with_insufficient_active_bond_are_not_elected() {
	ExtBuilder::default()
		.minimum_bond(1_000)
		.validator_count(5)
		.minimum_validator_count(3)
		.simple()
		.execute_with(|| {
			// Check some initial parameters are set
			assert_eq!(<Staking as crate::Store>::ValidatorCount::get(), 5);
			assert_eq!(<Staking as crate::Store>::MinimumValidatorCount::get(), 3);

			// Setup
			// 1) Fund and bond stashes
			// 2) Sign up to validate
			let stashes = vec![1, 2, 3, 4, 5];
			let controllers = vec![11, 22, 33, 44, 55];
			let initial_bond = Staking::minimum_bond() + 50;

			for (stash, controller) in stashes.iter().zip(controllers.iter()) {
				let _ = Balances::deposit_creating(&stash, initial_bond + controller);
				// Bond with slightly more than the minimum bond
				assert_ok!(Staking::bond(
					Origin::signed(*stash),
					controller.clone(),
					initial_bond,
					RewardDestination::Controller
				));

				assert_ok!(Staking::validate(
					Origin::signed(*controller),
					ValidatorPrefs::default()
				));
			}

			// Run an initial election, every stash should be elected
			let mut initial_elected = Staking::select_validators().1.expect("some were elected");
			initial_elected.sort();
			assert_eq!(initial_elected, stashes);

			// Unbond 2 validators and run another election, they should not be elected as their active stash
			// is now `0` and additionally should have been removed from the validator candidacy storage.
			assert_ok!(Staking::unbond(Origin::signed(controllers[0]), 51));
			assert_ok!(Staking::unbond(Origin::signed(controllers[1]), 51));

			// The unbonded validators are not elected
			let mut next_elected = Staking::select_validators().1.expect("some were elected");
			next_elected.sort();
			assert_eq!(next_elected[..], stashes[2..]);
		});
}

#[test]
fn nominations_with_insufficient_active_bond_are_ignored_during_election() {
	ExtBuilder::default()
		.minimum_bond(1_000)
		.validator_count(3)
		.minimum_validator_count(3)
		.simple()
		.execute_with(|| {
			// Scenario:
			// - 5 validators are bonded equally ids: [1,2,3,4,5]
			// - The validator count is set to 3 (only 3/5 will be selected in the next election)
			//-  2 nominators bond and nominate validator ids: [1, 2]
			// - The nominators backing should ensure [1, 2] are elected
			// - The nominators unbond
			// - The nominators backing is no longer considered

			// Check some initial parameters are set
			assert_eq!(<Staking as crate::Store>::ValidatorCount::get(), 3);
			assert_eq!(<Staking as crate::Store>::MinimumValidatorCount::get(), 3);

			// Setup
			// 1) Fund and bond stashes
			// 2) Sign up to validate
			let stashes = vec![1, 2, 3, 4, 5];
			let controllers = vec![11, 22, 33, 44, 55];
			let initial_bond = Staking::minimum_bond() + 50;

			for (stash, controller) in stashes.iter().zip(controllers.iter()) {
				let _ = Balances::deposit_creating(&stash, initial_bond + controller);
				// Bond with slightly more than the minimum bond
				assert_ok!(Staking::bond(
					Origin::signed(*stash),
					controller.clone(),
					// bond less as stash ID increases
					// This ensures a deterministic candidate order at election time
					// i.e at election time preference is: 1 > 2 > 3 > 4 > 5
					initial_bond - stash,
					RewardDestination::Controller
				));

				assert_ok!(Staking::validate(
					Origin::signed(*controller),
					ValidatorPrefs::default()
				));
			}

			// 1) Fund and bond stashes
			// 2) Sign up to nominate
			let nominator_stashes = vec![100, 200];
			let nominator_controllers = vec![101, 202];
			for (stash, controller) in nominator_stashes.iter().zip(nominator_controllers.iter()) {
				let _ = Balances::deposit_creating(&stash, initial_bond + controller);
				// Bond with slightly more than the minimum bond
				assert_ok!(Staking::bond(
					Origin::signed(*stash),
					controller.clone(),
					initial_bond * 2, // Give the nominators a big sway at election time
					RewardDestination::Controller
				));

				assert_ok!(Staking::nominate(
					Origin::signed(*controller),
					vec![stashes[3], stashes[4]]
				));
			}

			// Run an initial election, stashes[3], stashes[4] should be elected with the largest total backing
			// via nomination.
			let initial_elected = Staking::select_validators().1.expect("some were elected");
			assert!(initial_elected.contains(&stashes[3]));
			assert!(initial_elected.contains(&stashes[4]));

			// Unbond the nominators
			assert_ok!(Staking::unbond(
				Origin::signed(nominator_controllers[0]),
				initial_bond * 2
			));
			assert_ok!(Staking::unbond(
				Origin::signed(nominator_controllers[1]),
				initial_bond * 2
			));

			// The unbonded nominations are not considered in subsequent elections
			// stashes with highest bond from validators should be elected instead.
			let mut next_elected = Staking::select_validators().1.expect("some were elected");
			next_elected.sort();
			assert_eq!(next_elected[..], stashes[..3]);
		});
}

#[test]
fn nominate_with_empty_or_duplicate_candidates_fails() {
	ExtBuilder::default()
		.minimum_bond(1_000)
		.validator_count(3)
		.minimum_validator_count(3)
		.simple()
		.execute_with(|| {
			let (nominator_stash, nominator_controller) = (100_u64, 101);

			// Setup: fund and bond nominator
			let _ = Balances::deposit_creating(&nominator_stash, Staking::minimum_bond());
			assert_ok!(Staking::bond(
				Origin::signed(nominator_stash),
				nominator_controller,
				Staking::minimum_bond(),
				RewardDestination::Controller,
			));

			// Nominate no candidates
			assert_noop!(
				Staking::nominate(Origin::signed(nominator_controller), vec![]),
				Error::<Test>::EmptyTargets
			);

			// Nominate selecting candidate `1` multiple times
			assert_noop!(
				Staking::nominate(Origin::signed(nominator_controller), vec![1, 1, 1, 2, 3]),
				Error::<Test>::DuplicateNominee
			);
		});
}

#[test]
fn payout_to_any_account_works() {
	ExtBuilder::default().has_stakers(false).build().execute_with(|| {
		let balance = 1000;
		let reward_account_id = 42;
		// Create a validator:
		bond_validator(11, 10, balance); // Default(64)

		// Create a stash/controller pair
		bond_nominator(1234, 1337, 100, vec![11]);

		// Update payout location
		assert_ok!(Staking::set_payee(
			Origin::signed(1337),
			RewardDestination::Account(reward_account_id)
		));

		// Reward Destination account doesn't exist
		let init_balance = Balances::free_balance(reward_account_id);

		mock::start_era(1);
		Rewards::reward_by_ids(vec![(11, 1)]);
		// Compute total payout now for whole duration as other parameter won't change
		let total_payout_0: Balance = Rewards::calculate_next_reward_payout();
		assert!(total_payout_0 > 100); // Test is meaningful if reward something
		mock::start_era(2);

		Staking::on_initialize(System::block_number() + 1);

		// Payment is successful
		assert!(Balances::free_balance(reward_account_id) > init_balance);
	})
}

#[test]
fn migration_to_v2_works() {
	use super::*;
	use frame_support::{traits::OnRuntimeUpgrade, Hashable};

	ExtBuilder::default().build().execute_with(|| {
		frame_support::migration::put_storage_value(
			b"Staking",
			b"Payee",
			&0u64.twox_64_concat(),
			RewardDestination::<AccountId>::Stash,
		);
		frame_support::migration::put_storage_value(
			b"Staking",
			b"Payee",
			&1u64.twox_64_concat(),
			RewardDestination::<AccountId>::Controller,
		);
		frame_support::migration::put_storage_value(
			b"Staking",
			b"Payee",
			&2u64.twox_64_concat(),
			RewardDestination::<AccountId>::Account(5),
		);
		frame_support::migration::put_storage_value(b"Staking", b"BlockBonding", b"", true);

		StorageVersion::put(0);
		bond_validator(1, 4, 10);

		assert_eq!(StorageVersion::get(), Releases::V0 as u32);

		Staking::on_runtime_upgrade();

		assert_eq!(StorageVersion::get(), Releases::V1 as u32);

		assert_eq!(Rewards::payee(&0u64), 0);
		assert_eq!(Rewards::payee(&1u64), 4);
		assert_eq!(Rewards::payee(&2u64), 5);

		assert!(!frame_support::migration::have_storage_value(b"Staking", b"Payee", b"",));

		assert!(!frame_support::migration::have_storage_value(
			b"Staking",
			b"BlockBonding",
			b"",
		));

		assert_eq!(crate::rewards::Payee::<Test>::iter().count(), 2);
	});
}
