use super::*;
use crate::mock::{ExtBuilder, Governance, Test};
use frame_support::{assert_noop, assert_ok};
use sp_runtime::DispatchError;

#[test]
fn add_council_member() {
	ExtBuilder::default().build().execute_with(|| {
		let account = 3_u64;

		assert_ok!(Governance::add_council_member(
			frame_system::RawOrigin::Root.into(),
			account.into()
		));
	});
}

#[test]
fn add_council_member_not_enough_staked_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let account = 1_u64;

		assert_noop!(
			Governance::add_council_member(frame_system::RawOrigin::Root.into(), account.into()),
			Error::<Test>::NotEnoughStaked
		);
	});
}

#[test]
fn add_council_member_not_enough_registrations_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let account = 2_u64;

		assert_noop!(
			Governance::add_council_member(frame_system::RawOrigin::Root.into(), account.into()),
			Error::<Test>::NotEnoughRegistrations
		);
	});
}

#[test]
fn add_council_member_not_root_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let account = 3_u64;
		let signing_account = 4_u64;
		assert_noop!(
			Governance::add_council_member(frame_system::RawOrigin::Signed(signing_account).into(), account.into()),
			DispatchError::BadOrigin
		);
	});
}

#[test]
fn set_minimum_stake_bond() {
	ExtBuilder::default().build().execute_with(|| {
		let new_minimum_council_stake: Balance = Balance::from(500_000_u32);
		let account = 1_u64;

		assert_noop!(
			Governance::add_council_member(frame_system::RawOrigin::Root.into(), account.into()),
			Error::<Test>::NotEnoughStaked
		);

		assert_ok!(Governance::set_minimum_council_stake(
			frame_system::RawOrigin::Root.into(),
			new_minimum_council_stake
		));

		assert_ok!(Governance::add_council_member(
			frame_system::RawOrigin::Root.into(),
			account.into()
		));
	});
}

#[test]
fn set_minimum_stake_bond_not_root_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let new_minimum_council_stake: Balance = Balance::from(10_000_u32);
		let signing_account = 4_u64;

		assert_noop!(
			Governance::set_minimum_council_stake(
				frame_system::RawOrigin::Signed(signing_account).into(),
				new_minimum_council_stake
			),
			DispatchError::BadOrigin
		);
	});
}
