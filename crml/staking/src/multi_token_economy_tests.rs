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

use crate as crml_staking;
use frame_support::{parameter_types, traits::OnInitialize};
use pallet_session::historical as pallet_session_historical;
use sp_core::H256;
use sp_runtime::{
	testing::{Header, UintAuthorityId},
	traits::{BlakeTwo256, IdentityLookup},
	ModuleId, Perbill,
};
use sp_staking::SessionIndex;
use std::collections::HashSet;

use crate::mock::{Author11, CurrencyToVoteHandler, TestSessionHandler};
use crate::{
	rewards::{self, HandlePayee, StakerRewardPayment},
	Config, EraIndex, StakerStatus, StakingLedger,
};
use std::cell::RefCell;

const STAKING_ASSET_ID: AssetId = 100;
const REWARD_ASSET_ID: AssetId = 101;
const NEXT_ASSET_ID: AssetId = 102;

/// The AccountId alias in this test module.
type AccountId = u64;
type BlockNumber = u64;
type Balance = u64;
type AssetId = u32;

thread_local! {
	static SESSION: RefCell<(Vec<AccountId>, HashSet<AccountId>)> = RefCell::new(Default::default());
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
		GenericAsset: prml_generic_asset::{Module, Call, Storage, Config<T>, Event<T>},
		Authorship: pallet_authorship::{Module, Call, Storage},
		Staking: crml_staking::{Module, Call, Storage, Config<T>, Event<T>},
		Session: pallet_session::{Module, Call, Storage, Event, Config<T>},
		Historical: pallet_session_historical::{Module},
		Rewards: rewards::{Module, Call, Storage, Config, Event<T>},
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
}
impl frame_system::Config for Test {
	type BaseCallFilter = ();
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type Origin = Origin;
	type Index = u64;
	type Call = Call;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
}

impl prml_generic_asset::Config for Test {
	type Balance = Balance;
	type AssetId = AssetId;
	type Event = Event;
	type WeightInfo = ();
}

parameter_types! {
	pub const Period: BlockNumber = 1;
	pub const Offset: BlockNumber = 0;
	pub const UncleGenerations: u64 = 0;
	pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(25);
}
impl pallet_session::Config for Test {
	type Event = Event;
	type ValidatorId = AccountId;
	type ValidatorIdOf = crate::StashOf<Test>;
	type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
	type NextSessionRotation = ();
	type SessionManager = pallet_session::historical::NoteHistoricalRoot<Test, Staking>;
	type SessionHandler = TestSessionHandler;
	type Keys = UintAuthorityId;
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
	type EventHandler = Rewards;
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
	pub const HistoricalPayoutEras: u16 = 7;
	pub const PayoutSplitThreshold: u32 = 10;
	pub const FiscalEraLength: u32 = 5;
	pub const TreasuryModuleId: ModuleId = ModuleId(*b"py/trsry");
}
impl rewards::Config for Test {
	type CurrencyToReward = prml_generic_asset::SpendingAssetCurrency<Self>;
	type Event = Event;
	type HistoricalPayoutEras = HistoricalPayoutEras;
	type TreasuryModuleId = TreasuryModuleId;
	type PayoutSplitThreshold = PayoutSplitThreshold;
	type FiscalEraLength = FiscalEraLength;
	type WeightInfo = ();
}

parameter_types! {
	pub const SessionsPerEra: SessionIndex = 3;
	pub const BondingDuration: EraIndex = 3;
	pub const BlocksPerEra: BlockNumber = 3;
	pub const SlashDeferDuration: EraIndex = 0;
}
impl Config for Test {
	type Currency = prml_generic_asset::StakingAssetCurrency<Self>;
	type Time = pallet_timestamp::Module<Self>;
	type CurrencyToVote = CurrencyToVoteHandler;
	type Event = Event;
	type Slash = ();
	type SessionsPerEra = SessionsPerEra;
	type BlocksPerEra = BlocksPerEra;
	type SlashDeferDuration = SlashDeferDuration;
	type BondingDuration = BondingDuration;
	type SessionInterface = Self;
	type Rewarder = Rewards;
	type WeightInfo = ();
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

		let _ = prml_generic_asset::GenesisConfig::<Test> {
			endowed_accounts: vec![10, 11],
			initial_balance: 1_000_000_000,
			staking_asset_id: STAKING_ASSET_ID,
			spending_asset_id: REWARD_ASSET_ID,
			assets: vec![STAKING_ASSET_ID, REWARD_ASSET_ID],
			next_asset_id: NEXT_ASSET_ID,
			permissions: vec![],
			asset_meta: vec![],
		}
		.assimilate_storage(&mut storage);

		let _ = crml_staking::GenesisConfig::<Test> {
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

		frame_support::BasicExternalities::execute_with_storage(&mut storage, || {
			for k in &validators {
				frame_system::Module::<Test>::inc_providers(&k);
			}
		});

		let _ = pallet_session::GenesisConfig::<Test> {
			keys: validators.iter().map(|x| (*x, *x, UintAuthorityId(*x))).collect(),
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

fn rotate_to_session(index: SessionIndex) {
	assert!(Session::current_index() <= index);

	let rotations = index - Session::current_index();
	for _i in 0..rotations {
		Timestamp::set_timestamp(Timestamp::now() + 1000);
		Session::rotate_session();
	}
}

fn start_era(era_index: EraIndex) {
	rotate_to_session(era_index * SessionsPerEra::get());
}

#[test]
fn validator_reward_is_not_added_to_staked_amount_in_dual_currency_model() {
	// Rewards go to the correct destination as determined in Payee
	ExtBuilder::default().build().execute_with(|| {
		// Check that account 11 is a validator
		assert!(Staking::current_elected().contains(&11));
		// Check the balance of the validator account
		assert_eq!(GenericAsset::free_balance(STAKING_ASSET_ID, &10), 1_000_000_000);
		// Check the balance of the stash account
		assert_eq!(GenericAsset::free_balance(REWARD_ASSET_ID, &11), 1_000_000_000);
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

		start_era(1);

		// Compute total payout now for whole duration as other parameter won't change
		let total_payout = Rewards::calculate_next_reward_payout();
		assert!(total_payout > 1); // Test is meaningful if reward something
		Rewards::reward_by_ids(vec![(11, 1)]);

		Staking::on_initialize(System::block_number() + 1);

		// Check that RewardDestination is Stash (default)
		assert_eq!(Rewards::payee(&11), 11);
		// Check that reward went to the stash account of validator
		assert_eq!(
			GenericAsset::free_balance(REWARD_ASSET_ID, &11),
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
