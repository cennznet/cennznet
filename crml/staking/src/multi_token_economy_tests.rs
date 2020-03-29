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

// Test for staking rewards in a multi-token economic model
// i.e. The token at stake is not necessarily the token that is rewarded to validators
// Sadly we need to re-mock everything here just to alter the `RewardCurrency`,
// apart from that this file is simplified copy of `mock.rs`

use frame_support::{impl_outer_origin, parameter_types};
use sp_core::H256;
use sp_runtime::{
	curve::PiecewiseLinear,
	testing::{Header, UintAuthorityId},
	traits::{IdentityLookup, OnInitialize},
	Perbill,
};
use sp_staking::SessionIndex;
use std::collections::HashSet;

use crate::mock::{
	Author11, CurrencyToVoteHandler, ExistentialDeposit, SlashDeferDuration, TestSessionHandler, SESSION,
};
use crate::{inflation, EraIndex, GenesisConfig, Module, RewardDestination, StakerStatus, StakingLedger, Trait};

const REWARD_ASSET_ID: u32 = 101;
const STAKING_ASSET_ID: u32 = 100;

/// The AccountId alias in this test module.
pub type AccountId = u64;
pub type BlockNumber = u64;
pub type Balance = u64;

use frame_system as system;
impl_outer_origin! {
	pub enum Origin for Test {}
}

// Workaround for https://github.com/rust-lang/rust/issues/26925 . Remove when sorted.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Test;
parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: u32 = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::one();
}
impl frame_system::Trait for Test {
	type Origin = Origin;
	type Index = u64;
	type BlockNumber = BlockNumber;
	type Call = ();
	type Hash = H256;
	type Hashing = sp_runtime::traits::BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = ();
	type BlockHashCount = BlockHashCount;
	type MaximumBlockWeight = MaximumBlockWeight;
	type AvailableBlockRatio = AvailableBlockRatio;
	type MaximumBlockLength = MaximumBlockLength;
	type Version = ();
	type ModuleToIndex = ();
	type Doughnut = ();
	type DelegatedDispatchVerifier = ();
}
parameter_types! {
	pub const TransferFee: Balance = 0;
	pub const CreationFee: Balance = 0;
}
impl pallet_balances::Trait for Test {
	type Balance = Balance;
	type OnReapAccount = System;
	type OnNewAccount = ();
	type Event = ();
	type TransferPayment = ();
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type CreationFee = CreationFee;
}
impl pallet_generic_asset::Trait for Test {
	type Balance = u64;
	type AssetId = u32;
	type Event = ();
}
parameter_types! {
	pub const Period: BlockNumber = 1;
	pub const Offset: BlockNumber = 0;
	pub const UncleGenerations: u64 = 0;
	pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(25);
}
impl pallet_session::Trait for Test {
	type SessionManager = Staking;
	type Keys = UintAuthorityId;
	type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
	type SessionHandler = TestSessionHandler;
	type Event = ();
	type ValidatorId = AccountId;
	type ValidatorIdOf = crate::StashOf<Test>;
	type DisabledValidatorsThreshold = DisabledValidatorsThreshold;
}

impl pallet_session::historical::Trait for Test {
	type FullIdentification = crate::Exposure<AccountId, Balance>;
	type FullIdentificationOf = crate::ExposureOf<Test>;
}
impl pallet_authorship::Trait for Test {
	type FindAuthor = Author11;
	type UncleGenerations = UncleGenerations;
	type FilterUncle = ();
	type EventHandler = Module<Test>;
}
parameter_types! {
	pub const MinimumPeriod: u64 = 5;
}
impl pallet_timestamp::Trait for Test {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
}
crml_staking_reward_curve::build! {
	const I_NPOS: PiecewiseLinear<'static> = curve!(
		min_inflation: 0_025_000,
		max_inflation: 0_100_000,
		ideal_stake: 0_500_000,
		falloff: 0_050_000,
		max_piece_count: 40,
		test_precision: 0_005_000,
	);
}
parameter_types! {
	pub const SessionsPerEra: SessionIndex = 3;
	pub const BondingDuration: EraIndex = 3;
	pub const RewardCurve: &'static PiecewiseLinear<'static> = &I_NPOS;
}
impl Trait for Test {
	type Currency = pallet_generic_asset::StakingAssetCurrency<Self>;
	type RewardCurrency = pallet_generic_asset::SpendingAssetCurrency<Self>;
	type Time = pallet_timestamp::Module<Self>;
	type CurrencyToVote = CurrencyToVoteHandler;
	type RewardRemainder = ();
	type Event = ();
	type Slash = ();
	type Reward = ();
	type SessionsPerEra = SessionsPerEra;
	type SlashDeferDuration = SlashDeferDuration;
	type SlashCancelOrigin = frame_system::EnsureRoot<Self::AccountId, ()>;
	type BondingDuration = BondingDuration;
	type SessionInterface = Self;
	type RewardCurve = RewardCurve;
}

pub struct ExtBuilder {
	validator_count: u32,
	minimum_validator_count: u32,
	num_validators: Option<u32>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			validator_count: 2,
			minimum_validator_count: 0,
			num_validators: None,
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut storage = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

		let num_validators = self.num_validators.unwrap_or(self.validator_count);
		let validators = (0..num_validators)
			.map(|x| ((x + 1) * 10 + 1) as u64)
			.collect::<Vec<_>>();

		let _ = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

		let _ = pallet_generic_asset::GenesisConfig::<Test> {
			endowed_accounts: vec![10, 11],
			initial_balance: 1_000_000_000,
			staking_asset_id: STAKING_ASSET_ID,
			spending_asset_id: REWARD_ASSET_ID,
			assets: vec![STAKING_ASSET_ID, REWARD_ASSET_ID],
			next_asset_id: 102,
		}
		.assimilate_storage(&mut storage);

		let _ = GenesisConfig::<Test> {
			minimum_bond: 1,
			current_era: 0,
			stakers: vec![
				// (stash, controller, staked_amount, status)
				(11, 10, 500_000, StakerStatus::<AccountId>::Validator),
			],
			validator_count: self.validator_count,
			minimum_validator_count: self.minimum_validator_count,
			slash_reward_fraction: Perbill::from_percent(10),
			..Default::default()
		}
		.assimilate_storage(&mut storage);

		let _ = pallet_session::GenesisConfig::<Test> {
			keys: validators.iter().map(|x| (*x, UintAuthorityId(*x))).collect(),
		}
		.assimilate_storage(&mut storage);

		let mut t = sp_io::TestExternalities::new(storage);
		t.execute_with(|| {
			let validators = Session::validators();
			SESSION.with(|x| *x.borrow_mut() = (validators.clone(), HashSet::new()));
		});
		t
	}
}

pub type System = frame_system::Module<Test>;
pub type GenericAsset = pallet_generic_asset::Module<Test>;
pub type Session = pallet_session::Module<Test>;
pub type Timestamp = pallet_timestamp::Module<Test>;
pub type Staking = Module<Test>;

pub fn start_session(session_index: SessionIndex) {
	// Compensate for session delay
	let session_index = session_index + 1;
	for i in Session::current_index()..session_index {
		System::set_block_number((i + 1).into());
		Timestamp::set_timestamp(System::block_number() * 1000);
		Session::on_initialize(System::block_number());
	}

	assert_eq!(Session::current_index(), session_index);
}

pub fn start_era(era_index: EraIndex) {
	start_session((era_index * 3).into());
	assert_eq!(Staking::current_era(), era_index);
}

pub fn current_total_payout_for_duration(duration: u64) -> u64 {
	inflation::compute_total_payout(
		<Test as Trait>::RewardCurve::get(),
		<Module<Test>>::slot_stake() * 2,
		GenericAsset::total_issuance(&STAKING_ASSET_ID),
		duration,
	)
	.0
}

#[test]
fn validator_reward_is_not_added_to_staked_amount_in_dual_currency_model() {
	// Rewards go to the correct destination as determined in Payee
	ExtBuilder::default().build().execute_with(|| {
		// Check that account 11 is a validator
		assert!(Staking::current_elected().contains(&11));
		// Check the balance of the validator account
		assert_eq!(GenericAsset::free_balance(&STAKING_ASSET_ID, &10), 1_000_000_000);
		// Check the balance of the stash account
		assert_eq!(GenericAsset::free_balance(&REWARD_ASSET_ID, &11), 1_000_000_000);
		// Check how much is at stake
		assert_eq!(
			Staking::ledger(&10),
			Some(StakingLedger {
				stash: 11,
				total: 500_000,
				active: 500_000,
				unlocking: vec![],
			})
		);

		// Compute total payout now for whole duration as other parameter won't change
		let total_payout = current_total_payout_for_duration(3000);
		assert!(total_payout > 1); // Test is meaningfull if reward something
		<Module<Test>>::reward_by_ids(vec![(11, 1)]);

		start_era(1);

		// Check that RewardDestination is Stash (default)
		assert_eq!(Staking::payee(&11), RewardDestination::Stash);
		// Check that reward went to the stash account of validator
		assert_eq!(
			GenericAsset::free_balance(&REWARD_ASSET_ID, &11),
			1_000_000_000 + total_payout
		);
		// Check that amount at stake has NOT changed
		assert_eq!(
			Staking::ledger(&10),
			Some(StakingLedger {
				stash: 11,
				total: 500_000,
				active: 500_000,
				unlocking: vec![],
			})
		);
		// Check total issuance
		let total_issuance = 1_000_000_000 * 2; // one stash and controller accounts
		assert_eq!(GenericAsset::total_issuance(STAKING_ASSET_ID), total_issuance);
		assert_eq!(
			GenericAsset::total_issuance(REWARD_ASSET_ID),
			total_issuance + total_payout
		);
	})
}
