// This file is part of Substrate.

// Copyright (C) 2018-2020 Parity Technologies (UK) Ltd.
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

//! Test utilities

use crate as staking;
use crate::rewards::{HandlePayee, RewardCalculation, RewardParts};
use crate::*;
use frame_support::{
	assert_ok, parameter_types,
	traits::{Currency, FindAuthor, Get, OnFinalize, OnInitialize, OneSessionHandler},
	weights::{constants::RocksDbWeight, Weight},
	IterableStorageMap, StorageDoubleMap, StorageValue,
};
use sp_core::H256;
use sp_io;
use sp_npos_elections::{reduce, to_supports, ElectionScore, EvaluateSupport, ExtendedBalance, StakedAssignment};
use sp_runtime::{
	testing::{Header, TestXt, UintAuthorityId},
	traits::{IdentityLookup, One, Zero},
};
use sp_staking::offence::{OffenceDetails, OnOffenceHandler};
use std::{
	cell::RefCell,
	collections::{BTreeMap, HashSet},
};

pub const INIT_TIMESTAMP: u64 = 30_000;
pub const BLOCK_TIME: u64 = 1000;

/// The AccountId alias in this test module.
pub(crate) type AccountId = u64;
pub(crate) type AccountIndex = u64;
pub(crate) type BlockNumber = u64;
pub(crate) type Balance = u128;

thread_local! {
	static SESSION: RefCell<(Vec<AccountId>, HashSet<AccountId>)> = RefCell::new(Default::default());
	static SESSIONS_PER_ERA: RefCell<SessionIndex> = RefCell::new(3);
	static EXISTENTIAL_DEPOSIT: RefCell<Balance> = RefCell::new(0);
	static SLASH_DEFER_DURATION: RefCell<EraIndex> = RefCell::new(0);
	static ELECTION_LOOKAHEAD: RefCell<BlockNumber> = RefCell::new(0);
	static PERIOD: RefCell<BlockNumber> = RefCell::new(1);
	static OFFSET: RefCell<BlockNumber> = RefCell::new(0);
	static MAX_ITERATIONS: RefCell<u32> = RefCell::new(0);
}

/// Another session handler struct to test on_disabled.
pub struct OtherSessionHandler;
impl OneSessionHandler<AccountId> for OtherSessionHandler {
	type Key = UintAuthorityId;

	fn on_genesis_session<'a, I: 'a>(_: I)
	where
		I: Iterator<Item = (&'a AccountId, Self::Key)>,
		AccountId: 'a,
	{
	}

	fn on_new_session<'a, I: 'a>(_: bool, validators: I, _: I)
	where
		I: Iterator<Item = (&'a AccountId, Self::Key)>,
		AccountId: 'a,
	{
		SESSION.with(|x| *x.borrow_mut() = (validators.map(|x| x.0.clone()).collect(), HashSet::new()));
	}

	fn on_disabled(validator_index: usize) {
		SESSION.with(|d| {
			let mut d = d.borrow_mut();
			let value = d.0[validator_index];
			d.1.insert(value);
		})
	}
}

impl sp_runtime::BoundToRuntimeAppPublic for OtherSessionHandler {
	type Public = UintAuthorityId;
}

pub fn is_disabled(controller: AccountId) -> bool {
	let stash = Staking::ledger(&controller).unwrap().stash;
	SESSION.with(|d| d.borrow().1.contains(&stash))
}

pub struct Offset;
impl Get<BlockNumber> for Offset {
	fn get() -> BlockNumber {
		OFFSET.with(|v| *v.borrow())
	}
}

pub struct ExistentialDeposit;
impl Get<Balance> for ExistentialDeposit {
	fn get() -> Balance {
		EXISTENTIAL_DEPOSIT.with(|v| *v.borrow())
	}
}

pub struct SessionsPerEra;
impl Get<SessionIndex> for SessionsPerEra {
	fn get() -> SessionIndex {
		SESSIONS_PER_ERA.with(|v| *v.borrow())
	}
}

impl Get<BlockNumber> for SessionsPerEra {
	fn get() -> BlockNumber {
		SESSIONS_PER_ERA.with(|v| *v.borrow() as BlockNumber)
	}
}

pub struct ElectionLookahead;
impl Get<BlockNumber> for ElectionLookahead {
	fn get() -> BlockNumber {
		ELECTION_LOOKAHEAD.with(|v| *v.borrow())
	}
}

pub struct Period;
impl Get<BlockNumber> for Period {
	fn get() -> BlockNumber {
		PERIOD.with(|v| *v.borrow())
	}
}

pub struct SlashDeferDuration;
impl Get<EraIndex> for SlashDeferDuration {
	fn get() -> EraIndex {
		SLASH_DEFER_DURATION.with(|v| *v.borrow())
	}
}

pub struct MaxIterations;
impl Get<u32> for MaxIterations {
	fn get() -> u32 {
		MAX_ITERATIONS.with(|v| *v.borrow())
	}
}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Module, Call, Config, Storage, Event<T>},
		Timestamp: pallet_timestamp::{Module, Call, Storage, Inherent},
		Balances: pallet_balances::{Module, Call, Storage, Config<T>, Event<T>},
		Session: pallet_session::{Module, Call, Storage, Event, Config<T>},
		Staking: staking::{Module, Call, Storage, Config<T>, Event<T>, ValidateUnsigned},
	}
);

/// Author of block is always 11
pub struct Author11;
impl FindAuthor<AccountId> for Author11 {
	fn find_author<'a, I>(_digests: I) -> Option<AccountId>
	where
		I: 'a + IntoIterator<Item = (frame_support::ConsensusEngineId, &'a [u8])>,
	{
		Some(11)
	}
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub BlockWeights: frame_system::limits::BlockWeights =
		frame_system::limits::BlockWeights::simple_max(
			frame_support::weights::constants::WEIGHT_PER_SECOND * 2
		);
}
impl frame_system::Config for Test {
	type BaseCallFilter = ();
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = RocksDbWeight;
	type Origin = Origin;
	type Index = AccountIndex;
	type Call = Call;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = ::sp_runtime::traits::BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
}

parameter_types! {
	pub const MaxLocks: u32 = 1024;
}
impl pallet_balances::Config for Test {
	type Balance = Balance;
	type Event = Event;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type MaxLocks = MaxLocks;
	type WeightInfo = ();
}

parameter_types! {
	pub const UncleGenerations: u64 = 0;
	pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(25);
}
sp_runtime::impl_opaque_keys! {
	pub struct SessionKeys {
		pub other: OtherSessionHandler,
	}
}
impl pallet_session::Config for Test {
	type Event = Event;
	type ValidatorId = AccountId;
	type ValidatorIdOf = crate::StashOf<Test>;
	type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
	type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
	type SessionManager = pallet_session::historical::NoteHistoricalRoot<Test, Staking>;
	type SessionHandler = (OtherSessionHandler,);
	type Keys = SessionKeys;
	type DisabledValidatorsThreshold = DisabledValidatorsThreshold;
	type WeightInfo = ();
}

impl pallet_session::historical::Config for Test {
	type FullIdentification = crate::Exposure<AccountId, Balance>;
	type FullIdentificationOf = crate::ExposureOf<Test>;
}

impl pallet_authorship::Config for Test {
	type FindAuthor = Author11;
	type UncleGenerations = UncleGenerations;
	type FilterUncle = ();
	type EventHandler = ();
}

parameter_types! {
	pub const MinimumPeriod: u64 = 5;
}
impl pallet_timestamp::Config for Test {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
}

parameter_types! {
	pub const BondingDuration: EraIndex = 3;
	pub const MaxNominatorRewardedPerValidator: u32 = 64;
	pub const UnsignedPriority: u64 = 1 << 20;
	pub const MinSolutionScoreBump: Perbill = Perbill::zero();
	pub OffchainSolutionWeightLimit: Weight = BlockWeights::get().max_block;
}

thread_local! {
	pub static REWARD_REMAINDER_UNBALANCED: RefCell<u128> = RefCell::new(0);
}

impl Config for Test {
	type Currency = Balances;
	type UnixTime = Timestamp;
	type CurrencyToVote = frame_support::traits::SaturatingCurrencyToVote;
	type Event = Event;
	type Slash = ();
	type SessionsPerEra = SessionsPerEra;
	type SlashDeferDuration = SlashDeferDuration;
	type BondingDuration = BondingDuration;
	type SessionInterface = Self;
	type Rewarder = NoopRewarder<Self>;
	type NextNewSession = Session;
	type ElectionLookahead = ElectionLookahead;
	type Call = Call;
	type MaxIterations = MaxIterations;
	type MinSolutionScoreBump = MinSolutionScoreBump;
	type MaxNominatorRewardedPerValidator = MaxNominatorRewardedPerValidator;
	type UnsignedPriority = UnsignedPriority;
	type OffchainSolutionWeightLimit = OffchainSolutionWeightLimit;
	type WeightInfo = ();
}

impl<LocalCall> frame_system::offchain::SendTransactionTypes<LocalCall> for Test
where
	Call: From<LocalCall>,
{
	type OverarchingCall = Call;
	type Extrinsic = Extrinsic;
}

pub type Extrinsic = TestXt<Call, ()>;

pub struct ExtBuilder {
	session_length: BlockNumber,
	election_lookahead: BlockNumber,
	session_per_era: SessionIndex,
	existential_deposit: Balance,
	validator_pool: bool,
	nominate: bool,
	validator_count: u32,
	minimum_validator_count: u32,
	minimum_bond: Balance,
	slash_defer_duration: EraIndex,
	fair: bool,
	num_validators: Option<u32>,
	invulnerables: Vec<AccountId>,
	has_stakers: bool,
	max_offchain_iterations: u32,
	offset: BlockNumber,
	initialize_first_session: bool,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			session_length: 1,
			election_lookahead: 0,
			session_per_era: 3,
			existential_deposit: 1,
			validator_pool: false,
			nominate: true,
			validator_count: 2,
			minimum_validator_count: 0,
			slash_defer_duration: 0,
			minimum_bond: One::one(),
			fair: true,
			num_validators: None,
			invulnerables: vec![],
			has_stakers: true,
			max_offchain_iterations: 0,
			offset: 0,
			initialize_first_session: true,
		}
	}
}

impl ExtBuilder {
	pub fn existential_deposit(mut self, existential_deposit: Balance) -> Self {
		self.existential_deposit = existential_deposit;
		self
	}
	pub fn validator_pool(mut self, validator_pool: bool) -> Self {
		self.validator_pool = validator_pool;
		self
	}
	pub fn nominate(mut self, nominate: bool) -> Self {
		self.nominate = nominate;
		self
	}
	pub fn validator_count(mut self, count: u32) -> Self {
		self.validator_count = count;
		self
	}
	pub fn minimum_validator_count(mut self, count: u32) -> Self {
		self.minimum_validator_count = count;
		self
	}
	pub fn slash_defer_duration(mut self, eras: EraIndex) -> Self {
		self.slash_defer_duration = eras;
		self
	}
	pub fn fair(mut self, is_fair: bool) -> Self {
		self.fair = is_fair;
		self
	}
	pub fn num_validators(mut self, num_validators: u32) -> Self {
		self.num_validators = Some(num_validators);
		self
	}
	pub fn invulnerables(mut self, invulnerables: Vec<AccountId>) -> Self {
		self.invulnerables = invulnerables;
		self
	}
	pub fn session_per_era(mut self, length: SessionIndex) -> Self {
		self.session_per_era = length;
		self
	}
	pub fn election_lookahead(mut self, look: BlockNumber) -> Self {
		self.election_lookahead = look;
		self
	}
	pub fn period(mut self, length: BlockNumber) -> Self {
		self.session_length = length;
		self
	}
	pub fn has_stakers(mut self, has: bool) -> Self {
		self.has_stakers = has;
		self
	}
	pub fn max_offchain_iterations(mut self, iterations: u32) -> Self {
		self.max_offchain_iterations = iterations;
		self
	}
	pub fn offchain_election_ext(self) -> Self {
		self.session_per_era(4).period(5).election_lookahead(3)
	}
	pub fn set_associated_constants(&self) {
		EXISTENTIAL_DEPOSIT.with(|v| *v.borrow_mut() = self.existential_deposit);
		SLASH_DEFER_DURATION.with(|v| *v.borrow_mut() = self.slash_defer_duration);
		SESSIONS_PER_ERA.with(|v| *v.borrow_mut() = self.session_per_era);
		ELECTION_LOOKAHEAD.with(|v| *v.borrow_mut() = self.election_lookahead);
		PERIOD.with(|v| *v.borrow_mut() = self.session_length);
		MAX_ITERATIONS.with(|v| *v.borrow_mut() = self.max_offchain_iterations);
		OFFSET.with(|v| *v.borrow_mut() = self.offset);
	}

	pub fn minimum_bond(mut self, minimum_bond: Balance) -> Self {
		self.minimum_bond = minimum_bond;
		self
	}
	pub fn offset(mut self, offset: BlockNumber) -> Self {
		self.offset = offset;
		self
	}
	pub fn build(self) -> sp_io::TestExternalities {
		sp_tracing::try_init_simple();
		self.set_associated_constants();
		let mut storage = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();
		let balance_factor = if self.existential_deposit > 1 { 256 } else { 1 };

		let num_validators = self.num_validators.unwrap_or(self.validator_count);
		let validators = (0..num_validators)
			.map(|x| ((x + 1) * 10 + 1) as AccountId)
			.collect::<Vec<_>>();

		let _ = pallet_balances::GenesisConfig::<Test> {
			balances: vec![
				(1, 10 * balance_factor),
				(2, 20 * balance_factor),
				(3, 300 * balance_factor),
				(4, 400 * balance_factor),
				(10, balance_factor),
				(11, balance_factor * 1000),
				(20, balance_factor),
				(21, balance_factor * 2000),
				(30, balance_factor),
				(31, balance_factor * 2000),
				(40, balance_factor),
				(41, balance_factor * 2000),
				(50, balance_factor),
				(51, balance_factor * 2000),
				(60, balance_factor),
				(61, balance_factor * 2000),
				(70, balance_factor),
				(71, balance_factor * 2000),
				(80, balance_factor),
				(81, balance_factor * 2000),
				(100, 2000 * balance_factor),
				(101, 2000 * balance_factor),
				// This allows us to have a total_payout different from 0.
				(999, 1_000_000_000_000),
			],
		}
		.assimilate_storage(&mut storage);

		let mut stakers = vec![];
		if self.has_stakers {
			let stake_21 = if self.fair { 1000 } else { 2000 };
			let stake_31 = if self.validator_pool { balance_factor * 1000 } else { 1 };
			let status_41 = if self.validator_pool {
				StakerStatus::<AccountId>::Validator
			} else {
				StakerStatus::<AccountId>::Idle
			};
			let nominated = if self.nominate { vec![11, 21] } else { vec![] };
			stakers = vec![
				// (stash, controller, staked_amount, status)
				(11, 10, balance_factor * 1000, StakerStatus::<AccountId>::Validator),
				(21, 20, stake_21, StakerStatus::<AccountId>::Validator),
				(31, 30, stake_31, StakerStatus::<AccountId>::Validator),
				(41, 40, balance_factor * 1000, status_41),
				// nominator
				(
					101,
					100,
					balance_factor * 500,
					StakerStatus::<AccountId>::Nominator(nominated),
				),
			];
		}
		let _ = crate::GenesisConfig::<Test> {
			stakers,
			validator_count: self.validator_count,
			minimum_bond: self.minimum_bond,
			minimum_validator_count: self.minimum_validator_count,
			invulnerables: self.invulnerables,
			slash_reward_fraction: Perbill::from_percent(10),
			..Default::default()
		}
		.assimilate_storage(&mut storage);

		let _ = pallet_session::GenesisConfig::<Test> {
			keys: validators
				.iter()
				.map(|x| {
					(
						*x,
						*x,
						SessionKeys {
							other: UintAuthorityId(*x as u64),
						},
					)
				})
				.collect(),
		}
		.assimilate_storage(&mut storage);

		let mut ext = sp_io::TestExternalities::from(storage);
		ext.execute_with(|| {
			let validators = Session::validators();
			SESSION.with(|x| *x.borrow_mut() = (validators.clone(), HashSet::new()));
		});

		if self.initialize_first_session {
			// We consider all test to start after timestamp is initialized This must be ensured by
			// having `timestamp::on_initialize` called before `staking::on_initialize`. Also, if
			// session length is 1, then it is already triggered.
			ext.execute_with(|| {
				System::set_block_number(1);
				Session::on_initialize(1);
				Staking::on_initialize(1);
				Timestamp::set_timestamp(INIT_TIMESTAMP);
			});
		}

		ext
	}
	pub fn build_and_execute(self, test: impl FnOnce() -> ()) {
		let mut ext = self.build();
		ext.execute_with(test);
		ext.execute_with(post_conditions);
	}
}

fn post_conditions() {
	check_nominators();
	check_exposures();
	check_ledgers();
}

fn check_ledgers() {
	// check the ledger of all stakers.
	Bonded::<Test>::iter().for_each(|(_, ctrl)| assert_ledger_consistent(ctrl))
}

fn check_exposures() {
	// a check per validator to ensure the exposure struct is always sane.
	let era = active_era();
	ErasStakers::<Test>::iter_prefix_values(era).for_each(|expo| {
		assert_eq!(
			expo.total as u128,
			expo.own as u128 + expo.others.iter().map(|e| e.value as u128).sum::<u128>(),
			"wrong total exposure.",
		);
	})
}

fn check_nominators() {
	// a check per nominator to ensure their entire stake is correctly distributed. Will only kick-
	// in if the nomination was submitted before the current era.
	let era = active_era();
	<Nominators<Test>>::iter()
		.filter_map(|(nominator, nomination)| {
			if nomination.submitted_in > era {
				Some(nominator)
			} else {
				None
			}
		})
		.for_each(|nominator| {
			// must be bonded.
			assert_is_stash(nominator);
			let mut sum = 0;
			Session::validators()
				.iter()
				.map(|v| Staking::eras_stakers(era, v))
				.for_each(|e| {
					let individual = e.others.iter().filter(|e| e.who == nominator).collect::<Vec<_>>();
					let len = individual.len();
					match len {
						0 => { /* not supporting this validator at all. */ }
						1 => sum += individual[0].value,
						_ => panic!("nominator cannot back a validator more than once."),
					};
				});

			let nominator_stake = Staking::slashable_balance_of(&nominator);
			// a nominator cannot over-spend.
			assert!(
				nominator_stake >= sum,
				"failed: Nominator({}) stake({}) >= sum divided({})",
				nominator,
				nominator_stake,
				sum,
			);

			let diff = nominator_stake - sum;
			assert!(diff < 100);
		});
}

fn assert_is_stash(acc: AccountId) {
	assert!(Staking::bonded(&acc).is_some(), "Not a stash.");
}

fn assert_ledger_consistent(ctrl: AccountId) {
	// ensures ledger.total == ledger.active + sum(ledger.unlocking).
	let ledger = Staking::ledger(ctrl).expect("Not a controller.");
	let real_total: Balance = ledger.unlocking.iter().fold(ledger.active, |a, c| a + c.value);
	assert_eq!(real_total, ledger.total);
	assert!(
		ledger.active >= Balances::minimum_balance() || ledger.active == 0,
		"{}: active ledger amount ({}) must be greater than ED {}",
		ctrl,
		ledger.active,
		Balances::minimum_balance()
	);
}

pub(crate) fn active_era() -> EraIndex {
	Staking::active_era().unwrap().index
}

pub(crate) fn current_era() -> EraIndex {
	Staking::current_era().unwrap()
}

pub(crate) fn bond_validator(stash: AccountId, ctrl: AccountId, val: Balance) {
	let _ = Balances::make_free_balance_be(&stash, val);
	let _ = Balances::make_free_balance_be(&ctrl, val);
	assert_ok!(Staking::bond(
		Origin::signed(stash),
		ctrl,
		val,
		RewardDestination::Controller,
	));
	assert_ok!(Staking::validate(Origin::signed(ctrl), ValidatorPrefs::default()));
}

pub(crate) fn bond_nominator(stash: AccountId, ctrl: AccountId, val: Balance, target: Vec<AccountId>) {
	let _ = Balances::make_free_balance_be(&stash, val);
	let _ = Balances::make_free_balance_be(&ctrl, val);
	assert_ok!(Staking::bond(
		Origin::signed(stash),
		ctrl,
		val,
		RewardDestination::Controller,
	));
	assert_ok!(Staking::nominate(Origin::signed(ctrl), target));
}

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
		Timestamp::set_timestamp(System::block_number() * BLOCK_TIME + INIT_TIMESTAMP);
		if b != n {
			Staking::on_finalize(System::block_number());
		}
	}
}

/// Progresses from the current block number (whatever that may be) to the `P * session_index + 1`.
pub(crate) fn start_session(session_index: SessionIndex) {
	let end: u64 = if Offset::get().is_zero() {
		(session_index as u64) * Period::get()
	} else {
		Offset::get() + (session_index.saturating_sub(1) as u64) * Period::get()
	};
	run_to_block(end);
	// session must have progressed properly.
	assert_eq!(
		Session::current_index(),
		session_index,
		"current session index = {}, expected = {}",
		Session::current_index(),
		session_index,
	);
}

/// Go one session forward.
pub(crate) fn advance_session() {
	let current_index = Session::current_index();
	start_session(current_index + 1);
}

/// Progress until the given era.
pub(crate) fn start_active_era(era_index: EraIndex) {
	start_session((era_index * <SessionsPerEra as Get<u32>>::get()).into());
	assert_eq!(active_era(), era_index);
	// One way or another, current_era must have changed before the active era, so they must match
	// at this point.
	assert_eq!(current_era(), active_era());
}

pub(crate) fn validator_controllers() -> Vec<AccountId> {
	Session::validators()
		.into_iter()
		.map(|s| Staking::bonded(&s).expect("no controller for validator"))
		.collect()
}

pub(crate) fn on_offence_in_era(
	offenders: &[OffenceDetails<AccountId, pallet_session::historical::IdentificationTuple<Test>>],
	slash_fraction: &[Perbill],
	era: EraIndex,
) {
	let bonded_eras = crate::BondedEras::get();
	for &(bonded_era, start_session) in bonded_eras.iter() {
		if bonded_era == era {
			let _ = Staking::on_offence(offenders, slash_fraction, start_session).unwrap();
			return;
		} else if bonded_era > era {
			break;
		}
	}

	if Staking::active_era().unwrap().index == era {
		let _ = Staking::on_offence(
			offenders,
			slash_fraction,
			Staking::eras_start_session_index(era).unwrap(),
		)
		.unwrap();
	} else {
		panic!("cannot slash in era {}", era);
	}
}

pub(crate) fn on_offence_now(
	offenders: &[OffenceDetails<AccountId, pallet_session::historical::IdentificationTuple<Test>>],
	slash_fraction: &[Perbill],
) {
	let now = Staking::active_era().unwrap().index;
	on_offence_in_era(offenders, slash_fraction, now)
}

pub(crate) fn add_slash(who: &AccountId) {
	on_offence_now(
		&[OffenceDetails {
			offender: (
				who.clone(),
				Staking::eras_stakers(Staking::active_era().unwrap().index, who.clone()),
			),
			reporters: vec![],
		}],
		&[Perbill::from_percent(10)],
	);
}

// winners will be chosen by simply their unweighted total backing stake. Nominator stake is
// distributed evenly.
pub(crate) fn horrible_npos_solution(do_reduce: bool) -> (CompactAssignments, Vec<ValidatorIndex>, ElectionScore) {
	let mut backing_stake_of: BTreeMap<AccountId, Balance> = BTreeMap::new();

	// self stake
	<Validators<Test>>::iter().for_each(|(who, _p)| {
		*backing_stake_of.entry(who).or_insert(Zero::zero()) += Staking::slashable_balance_of(&who)
	});

	// add nominator stuff
	<Nominators<Test>>::iter().for_each(|(who, nomination)| {
		nomination
			.targets
			.iter()
			.for_each(|v| *backing_stake_of.entry(*v).or_insert(Zero::zero()) += Staking::slashable_balance_of(&who))
	});

	// elect winners
	let mut sorted: Vec<AccountId> = backing_stake_of.keys().cloned().collect();
	sorted.sort_by_key(|x| backing_stake_of.get(x).unwrap());
	let winners: Vec<AccountId> = sorted
		.iter()
		.cloned()
		.take(Staking::validator_count() as usize)
		.collect();

	// create assignments
	let mut staked_assignment: Vec<StakedAssignment<AccountId>> = Vec::new();
	<Nominators<Test>>::iter().for_each(|(who, nomination)| {
		let mut dist: Vec<(AccountId, ExtendedBalance)> = Vec::new();
		nomination.targets.iter().for_each(|v| {
			if winners.iter().find(|w| *w == v).is_some() {
				dist.push((*v, ExtendedBalance::zero()));
			}
		});

		if dist.len() == 0 {
			return;
		}

		// assign real stakes. just split the stake.
		let stake = Staking::slashable_balance_of(&who) as ExtendedBalance;
		let mut sum: ExtendedBalance = Zero::zero();
		let dist_len = dist.len();
		{
			dist.iter_mut().for_each(|(_, w)| {
				let partial = stake / (dist_len as ExtendedBalance);
				*w = partial;
				sum += partial;
			});
		}

		// assign the leftover to last.
		{
			let leftover = stake - sum;
			let last = dist.last_mut().unwrap();
			last.1 += leftover;
		}

		staked_assignment.push(StakedAssignment {
			who,
			distribution: dist,
		});
	});

	// Ensure that this result is worse than seq-phragmen. Otherwise, it should not have been used
	// for testing.
	let score = {
		let (_, _, better_score) = prepare_submission_with(true, true, 0, |_| {});

		let support = to_supports(&winners, &staked_assignment).unwrap();
		let score = (&support).evaluate();

		assert!(sp_npos_elections::is_score_better::<Perbill>(
			better_score,
			score,
			MinSolutionScoreBump::get(),
		));

		score
	};

	if do_reduce {
		reduce(&mut staked_assignment);
	}

	let snapshot_validators = Staking::snapshot_validators().unwrap();
	let snapshot_nominators = Staking::snapshot_nominators().unwrap();
	let nominator_index = |a: &AccountId| -> Option<NominatorIndex> {
		snapshot_nominators
			.iter()
			.position(|x| x == a)
			.map(|i| i as NominatorIndex)
	};
	let validator_index = |a: &AccountId| -> Option<ValidatorIndex> {
		snapshot_validators
			.iter()
			.position(|x| x == a)
			.map(|i| i as ValidatorIndex)
	};

	// convert back to ratio assignment. This takes less space.
	let assignments_reduced =
		sp_npos_elections::assignment_staked_to_ratio::<AccountId, OffchainAccuracy>(staked_assignment);

	let compact = CompactAssignments::from_assignment(assignments_reduced, nominator_index, validator_index).unwrap();

	// winner ids to index
	let winners = winners
		.into_iter()
		.map(|w| validator_index(&w).unwrap())
		.collect::<Vec<_>>();

	(compact, winners, score)
}

/// Note: this should always logically reproduce [`offchain_election::prepare_submission`], yet we
/// cannot do it since we want to have `tweak` injected into the process.
///
/// If the input is being tweaked in a way that the score cannot be compute accurately,
/// `compute_real_score` can be set to true. In this case a `Default` score is returned.
pub(crate) fn prepare_submission_with(
	compute_real_score: bool,
	do_reduce: bool,
	iterations: usize,
	tweak: impl FnOnce(&mut Vec<StakedAssignment<AccountId>>),
) -> (CompactAssignments, Vec<ValidatorIndex>, ElectionScore) {
	// run election on the default stuff.
	let sp_npos_elections::ElectionResult { winners, assignments } =
		Staking::do_phragmen::<OffchainAccuracy>(iterations).unwrap();
	let winners = sp_npos_elections::to_without_backing(winners);

	let mut staked = sp_npos_elections::assignment_ratio_to_staked(assignments, Staking::slashable_balance_of_fn());

	// apply custom tweaks. awesome for testing.
	tweak(&mut staked);

	if do_reduce {
		reduce(&mut staked);
	}

	// convert back to ratio assignment. This takes less space.
	let snapshot_validators = Staking::snapshot_validators().expect("snapshot not created.");
	let snapshot_nominators = Staking::snapshot_nominators().expect("snapshot not created.");
	let nominator_index = |a: &AccountId| -> Option<NominatorIndex> {
		snapshot_nominators.iter().position(|x| x == a).map_or_else(
			|| {
				println!("unable to find nominator index for {:?}", a);
				None
			},
			|i| Some(i as NominatorIndex),
		)
	};
	let validator_index = |a: &AccountId| -> Option<ValidatorIndex> {
		snapshot_validators.iter().position(|x| x == a).map_or_else(
			|| {
				println!("unable to find validator index for {:?}", a);
				None
			},
			|i| Some(i as ValidatorIndex),
		)
	};

	let assignments_reduced = sp_npos_elections::assignment_staked_to_ratio(staked);

	// re-compute score by converting, yet again, into staked type
	let score = if compute_real_score {
		let staked = sp_npos_elections::assignment_ratio_to_staked(
			assignments_reduced.clone(),
			Staking::slashable_balance_of_fn(),
		);

		let support_map = to_supports(winners.as_slice(), staked.as_slice()).unwrap();
		support_map.evaluate()
	} else {
		Default::default()
	};

	let compact = CompactAssignments::from_assignment(assignments_reduced, nominator_index, validator_index)
		.expect("Failed to create compact");

	// winner ids to index
	let winners = winners
		.into_iter()
		.map(|w| validator_index(&w).unwrap())
		.collect::<Vec<_>>();

	(compact, winners, score)
}

#[macro_export]
macro_rules! assert_session_era {
	($session:expr, $era:expr) => {
		assert_eq!(
			Session::current_index(),
			$session,
			"wrong session {} != {}",
			Session::current_index(),
			$session,
		);
		assert_eq!(
			Staking::active_era().unwrap().index,
			$era,
			"wrong active era {} != {}",
			Staking::active_era().unwrap().index,
			$era,
		);
	};
}

pub(crate) fn balances(who: &AccountId) -> (Balance, Balance) {
	(Balances::free_balance(who), Balances::reserved_balance(who))
}

/// A rewarder which does nothing
pub struct NoopRewarder<T: Config>(sp_std::marker::PhantomData<T>);

impl<T: Config> RewardCalculation for NoopRewarder<T> {
	type AccountId = T::AccountId;
	type Balance = BalanceOf<T>;
	fn calculate_total_reward() -> RewardParts<Self::Balance> {
		RewardParts::new(Zero::zero(), Zero::zero(), Perbill::one())
	}
	fn calculate_individual_reward(
		_stash: &Self::AccountId,
		_validator_commission_stake_map: &[(Self::AccountId, Perbill, Exposure<Self::AccountId, Self::Balance>)],
	) -> Self::Balance {
		Zero::zero()
	}
}

impl<T: Config> HandlePayee for NoopRewarder<T> {
	type AccountId = T::AccountId;
	fn set_payee(_stash: &Self::AccountId, _payee: &Self::AccountId) {}
	fn remove_payee(_stash: &Self::AccountId) {}
	fn payee(stash: &Self::AccountId) -> Self::AccountId {
		stash.clone()
	}
}

impl<T: Config> OnEndEra for NoopRewarder<T> {
	type AccountId = T::AccountId;
}
