use super::*;
use crate::mock::{ExtBuilder, Governance, MockStakingAmount, Test};
use frame_support::{assert_noop, assert_ok, traits::OnInitialize};
use sp_runtime::DispatchError;

// Helper function to setup a vector of accounts as council members
fn setup_council_members(accounts: Vec<u64>) {
	accounts.iter().for_each(|a| {
		assert_ok!(Governance::add_council_member(frame_system::RawOrigin::Root.into(), *a));
	});
}

// Helper function to setup a referendum
fn setup_referendum(
	proposal_account: u64,
	voting_account: u64,
	call: Vec<u8>,
	justification_uri: Vec<u8>,
	enactment_delay: u64,
) -> ProposalId {
	let proposal_id = Governance::next_proposal_id();
	setup_council_members(vec![proposal_account, voting_account]);

	assert_ok!(Governance::submit_proposal(
		frame_system::RawOrigin::Signed(proposal_account).into(),
		call,
		justification_uri,
		enactment_delay
	));
	assert_ok!(Governance::vote_on_proposal(
		frame_system::RawOrigin::Signed(voting_account).into(),
		proposal_id,
		true,
	));

	assert_eq!(
		Governance::proposal_status(proposal_id),
		Some(ProposalStatusInfo::ReferendumDeliberation)
	);
	assert_eq!(Governance::referendum_veto_sum(proposal_id), 0);
	proposal_id
}

#[test]
fn add_council_member() {
	ExtBuilder::default().build().execute_with(|| {
		let new_member_account = 3_u64;
		setup_council_members(vec![new_member_account]);
	});
}

#[test]
fn add_council_member_not_enough_staked_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let new_member_account = 1_u64;

		assert_noop!(
			Governance::add_council_member(frame_system::RawOrigin::Root.into(), new_member_account.into()),
			Error::<Test>::NotEnoughStaked
		);
	});
}

#[test]
fn add_council_member_not_enough_registrations_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let new_member_account = 2_u64;

		assert_noop!(
			Governance::add_council_member(frame_system::RawOrigin::Root.into(), new_member_account.into()),
			Error::<Test>::NotEnoughRegistrations
		);
	});
}

#[test]
fn add_council_member_not_root_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let new_member_account = 3_u64;
		let signing_account = 4_u64;
		assert_noop!(
			Governance::add_council_member(
				frame_system::RawOrigin::Signed(signing_account).into(),
				new_member_account.into()
			),
			DispatchError::BadOrigin
		);
	});
}

#[test]
fn set_minimum_stake_bond() {
	ExtBuilder::default().build().execute_with(|| {
		let new_minimum_council_stake: Balance = Balance::from(900_u32);
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

#[test]
fn set_referendum_threshold() {
	ExtBuilder::default().build().execute_with(|| {
		let new_referendum_threshold: Permill = Permill::from_parts(500_000);

		assert_ok!(Governance::set_referendum_threshold(
			frame_system::RawOrigin::Root.into(),
			new_referendum_threshold
		));

		assert_eq!(Governance::referendum_threshold(), new_referendum_threshold);
	});
}

#[test]
fn set_referendum_threshold_not_root_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let new_referendum_threshold: Permill = Permill::from_parts(500_000);
		let signing_account = 4_u64;

		assert_noop!(
			Governance::set_referendum_threshold(
				frame_system::RawOrigin::Signed(signing_account).into(),
				new_referendum_threshold
			),
			DispatchError::BadOrigin
		);
	});
}

#[test]
fn set_minimum_voter_staked_amount() {
	ExtBuilder::default().build().execute_with(|| {
		let new_min_voter_staked_amount: Balance = Balance::from(20_000_u32);

		assert_ok!(Governance::set_minimum_voter_staked_amount(
			frame_system::RawOrigin::Root.into(),
			new_min_voter_staked_amount
		));

		assert_eq!(Governance::min_voter_staked_amount(), new_min_voter_staked_amount);
	});
}

#[test]
fn set_minimum_voter_staked_amount_not_root_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let new_min_voter_staked_amount: Balance = Balance::from(20_000_u32);
		let signing_account = 4_u64;

		assert_noop!(
			Governance::set_minimum_voter_staked_amount(
				frame_system::RawOrigin::Signed(signing_account).into(),
				new_min_voter_staked_amount
			),
			DispatchError::BadOrigin
		);
	});
}

#[test]
fn submit_proposal() {
	ExtBuilder::default().build().execute_with(|| {
		let proposal_account = 3_u64;
		let justification_uri: Vec<u8> = vec![0];
		let enactment_delay = 1;
		let call = "0x123456"; // Invalid call
		let proposal_id = Governance::next_proposal_id();
		setup_council_members(vec![proposal_account]);

		assert_ok!(Governance::submit_proposal(
			frame_system::RawOrigin::Signed(proposal_account).into(),
			call.into(),
			justification_uri.clone(),
			enactment_delay
		));

		// Check storage has been updated
		let expected_proposal = Proposal {
			sponsor: proposal_account,
			justification_uri,
			enactment_delay,
		};
		let mut votes = ProposalVoteInfo::default();
		votes.record_vote(
			Governance::council().binary_search(&proposal_account).unwrap() as u8,
			true,
		);

		assert_eq!(Governance::proposals(proposal_id), Some(expected_proposal));
		assert_eq!(Governance::proposal_votes(proposal_id), votes);
		assert_eq!(
			Governance::proposal_status(proposal_id),
			Some(ProposalStatusInfo::Deliberation)
		);
		assert_eq!(Governance::proposal_calls(proposal_id), Some(call.into()));
		assert_eq!(Governance::next_proposal_id(), proposal_id + 1);
	});
}

#[test]
fn submit_proposal_not_council_member_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let proposal_account = 3_u64;
		let justification_uri: Vec<u8> = vec![0];
		let enactment_delay = 1;
		let call = "0x123456"; // Invalid call

		assert_noop!(
			Governance::submit_proposal(
				frame_system::RawOrigin::Signed(proposal_account).into(),
				call.into(),
				justification_uri,
				enactment_delay
			),
			Error::<Test>::NotCouncilor
		);
	});
}

#[test]
fn vote_yes_on_proposal_should_create_referendum() {
	ExtBuilder::default().build().execute_with(|| {
		let proposal_account = 3_u64;
		let voting_account = 4_u64;
		let justification_uri: Vec<u8> = vec![0];
		let enactment_delay = 1;
		let call = "0x123456"; // Invalid call
		let proposal_id = Governance::next_proposal_id();
		setup_council_members(vec![proposal_account, voting_account]);

		assert_ok!(Governance::submit_proposal(
			frame_system::RawOrigin::Signed(proposal_account).into(),
			call.into(),
			justification_uri.clone(),
			enactment_delay
		));

		assert_ok!(Governance::vote_on_proposal(
			frame_system::RawOrigin::Signed(voting_account).into(),
			proposal_id,
			true,
		));

		assert!(!ProposalVotes::contains_key(proposal_id));
		assert_eq!(
			Governance::proposal_status(proposal_id),
			Some(ProposalStatusInfo::ReferendumDeliberation)
		);
		assert_eq!(Governance::proposal_calls(proposal_id), Some(call.into()));
		assert_eq!(Governance::referendum_veto_sum(proposal_id), 0);
	});
}

#[test]
fn vote_no_on_proposal_should_delete_proposal() {
	ExtBuilder::default().build().execute_with(|| {
		let proposal_account = 3_u64;
		let voting_account = 4_u64;
		let voting_account_2 = 5_u64;
		let justification_uri: Vec<u8> = vec![0];
		let enactment_delay = 1;
		let call = "0x123456"; // Invalid call
		let proposal_id = Governance::next_proposal_id();
		setup_council_members(vec![proposal_account, voting_account, voting_account_2]);

		assert_ok!(Governance::submit_proposal(
			frame_system::RawOrigin::Signed(proposal_account).into(),
			call.into(),
			justification_uri.clone(),
			enactment_delay
		));

		// Two no votes should go over threshold and cause the proposal to fail
		assert_ok!(Governance::vote_on_proposal(
			frame_system::RawOrigin::Signed(voting_account).into(),
			proposal_id,
			false,
		));
		assert_ok!(Governance::vote_on_proposal(
			frame_system::RawOrigin::Signed(voting_account_2).into(),
			proposal_id,
			false,
		));

		// Proposal should be removed
		assert!(!ProposalVotes::contains_key(proposal_id));
		assert!(!ProposalCalls::contains_key(proposal_id));
		assert!(!Proposals::<Test>::contains_key(proposal_id));
		assert_eq!(
			Governance::proposal_status(proposal_id),
			Some(ProposalStatusInfo::Disapproved)
		);
	});
}

#[test]
fn vote_on_proposal_same_account_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let proposal_account = 3_u64;
		let justification_uri: Vec<u8> = vec![0];
		let enactment_delay = 1;
		let call = "0x123456"; // Invalid call
		let proposal_id = Governance::next_proposal_id();
		setup_council_members(vec![proposal_account]);

		assert_ok!(Governance::submit_proposal(
			frame_system::RawOrigin::Signed(proposal_account).into(),
			call.into(),
			justification_uri.clone(),
			enactment_delay
		));

		assert_noop!(
			Governance::vote_on_proposal(
				frame_system::RawOrigin::Signed(proposal_account).into(),
				proposal_id,
				true,
			),
			Error::<Test>::DoubleVote
		);
	});
}

#[test]
fn vote_on_proposal_not_councilor_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let proposal_account = 3_u64;
		let voting_account = 4_u64;
		let justification_uri: Vec<u8> = vec![0];
		let enactment_delay = 1;
		let call = "0x123456"; // Invalid call
		let proposal_id = Governance::next_proposal_id();
		setup_council_members(vec![proposal_account]);

		assert_ok!(Governance::submit_proposal(
			frame_system::RawOrigin::Signed(proposal_account).into(),
			call.into(),
			justification_uri.clone(),
			enactment_delay
		));

		assert_noop!(
			Governance::vote_on_proposal(
				frame_system::RawOrigin::Signed(voting_account).into(),
				proposal_id,
				true,
			),
			Error::<Test>::NotCouncilor
		);
	});
}

#[test]
fn vote_on_proposal_no_proposal_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let proposal_account = 3_u64;
		let voting_account = 4_u64;
		let proposal_id = Governance::next_proposal_id();
		setup_council_members(vec![proposal_account, voting_account]);

		assert_noop!(
			Governance::vote_on_proposal(
				frame_system::RawOrigin::Signed(voting_account).into(),
				proposal_id,
				true,
			),
			Error::<Test>::ProposalMissing
		);
	});
}

#[test]
fn vote_against_referendum() {
	ExtBuilder::default().build().execute_with(|| {
		let proposal_account = 3_u64;
		let voting_account = 4_u64;
		let justification_uri: Vec<u8> = vec![0];
		let enactment_delay = 1;
		let call = "0x123456"; // Invalid call
		let proposal_id = setup_referendum(
			proposal_account,
			voting_account,
			call.into(),
			justification_uri,
			enactment_delay,
		);

		let non_councilor_account = 5_u64;
		assert_ok!(Governance::vote_against_referendum(
			frame_system::RawOrigin::Signed(non_councilor_account).into(),
			proposal_id
		));
		let vote_count = MockStakingAmount::active_balance(&non_councilor_account);
		assert_eq!(
			Governance::referendum_votes(proposal_id, &non_councilor_account),
			vote_count
		);

		// Try a second vote from initial proposal account
		assert_ok!(Governance::vote_against_referendum(
			frame_system::RawOrigin::Signed(proposal_account).into(),
			proposal_id
		));
		let vote_count = MockStakingAmount::active_balance(&proposal_account);
		assert_eq!(Governance::referendum_votes(proposal_id, &proposal_account), vote_count);
	});
}

#[test]
fn vote_against_referendum_not_enough_staked_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let proposal_account = 3_u64;
		let voting_account = 4_u64;
		let justification_uri: Vec<u8> = vec![0];
		let enactment_delay = 1;
		let call = "0x123456"; // Invalid call
		let proposal_id = setup_referendum(
			proposal_account,
			voting_account,
			call.into(),
			justification_uri,
			enactment_delay,
		);

		let non_councilor_account = 1_u64;
		assert_noop!(
			Governance::vote_against_referendum(
				frame_system::RawOrigin::Signed(non_councilor_account).into(),
				proposal_id
			),
			Error::<Test>::NotEnoughStaked
		);
		assert!(!ReferendumVotes::<Test>::contains_key(
			proposal_id,
			&non_councilor_account
		));
	});
}

#[test]
fn vote_against_referendum_not_enough_registrations_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let proposal_account = 3_u64;
		let voting_account = 4_u64;
		let justification_uri: Vec<u8> = vec![0];
		let enactment_delay = 1;
		let call = "0x123456"; // Invalid call
		let proposal_id = setup_referendum(
			proposal_account,
			voting_account,
			call.into(),
			justification_uri,
			enactment_delay,
		);

		let non_councilor_account = 2_u64;
		assert_noop!(
			Governance::vote_against_referendum(
				frame_system::RawOrigin::Signed(non_councilor_account).into(),
				proposal_id
			),
			Error::<Test>::NotEnoughRegistrations
		);
		assert!(!ReferendumVotes::<Test>::contains_key(
			proposal_id,
			&non_councilor_account
		));
	});
}

#[test]
fn vote_against_non_existent_referendum_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let non_councilor_account = 2_u64;
		assert_noop!(
			Governance::vote_against_referendum(frame_system::RawOrigin::Signed(non_councilor_account).into(), 0),
			Error::<Test>::ProposalMissing
		);
	});
}

#[test]
fn end_referendum() {
	ExtBuilder::default().build().execute_with(|| {
		let proposal_account = 3_u64;
		let voting_account = 4_u64;
		let justification_uri: Vec<u8> = vec![0];
		let enactment_delay = 1;
		let new_account = 5_u64;
		let call: <Test as Config>::Call = (Call::add_council_member::<Test> {
			new_member: new_account,
		})
		.into();
		let call = call.encode();
		let proposal_id = setup_referendum(
			proposal_account,
			voting_account,
			call,
			justification_uri,
			enactment_delay,
		);

		let end_block = 22000;
		Governance::on_initialize(end_block);

		assert_eq!(
			Governance::proposal_status(proposal_id),
			Some(ProposalStatusInfo::ApprovedWaitingEnactment)
		);

		//Voting against referendum after it ends should fail
		let non_councilor_account = 5_u64;
		assert_noop!(
			Governance::vote_against_referendum(
				frame_system::RawOrigin::Signed(non_councilor_account).into(),
				proposal_id
			),
			Error::<Test>::ReferendumNotDeliberating
		);

		// Manually call enact referendum
		assert_ok!(Governance::enact_referendum(
			frame_system::RawOrigin::Root.into(),
			proposal_id
		));

		// Check that storage has changed and call enacted
		assert_eq!(Governance::council(), vec![3, 4, 5]);
		assert_eq!(
			Governance::proposal_status(proposal_id),
			Some(ProposalStatusInfo::ApprovedEnacted(true))
		);
		assert!(!ProposalCalls::contains_key(proposal_id));
		assert!(!<Proposals<Test>>::contains_key(proposal_id));
		assert!(!ReferendumVetoSum::contains_key(proposal_id));
		assert!(!<ReferendumStartTime<Test>>::contains_key(proposal_id));
	});
}

#[test]
fn end_referendum_with_0_enactment_delay() {
	ExtBuilder::default().build().execute_with(|| {
		let proposal_account = 3_u64;
		let voting_account = 4_u64;
		let justification_uri: Vec<u8> = vec![0];
		let enactment_delay = 0;
		let call = "0x8c041f021cbd2d43530a44705ad088af313e18f80b53ef16b36177cd4b77b846f2a5f07c";
		let proposal_id = setup_referendum(
			proposal_account,
			voting_account,
			call.into(),
			justification_uri,
			enactment_delay,
		);

		let end_block = 22000;
		Governance::on_initialize(end_block);
		assert_eq!(
			Governance::proposal_status(proposal_id),
			Some(ProposalStatusInfo::ApprovedWaitingEnactment)
		);
	});
}
