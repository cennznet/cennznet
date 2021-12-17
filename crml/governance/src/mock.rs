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

use crate as crml_governance;
use cennznet_primitives::types::{AssetId, Balance};
use crml_generic_asset::StakingAssetCurrency;
use crml_support::StakingAmount;
use frame_support::{parameter_types, traits::RegistrationInfo, weights::Weight, PalletId};
use frame_system::EnsureRoot;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	Perbill,
};

pub type AccountId = u64;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Module, Call, Config, Storage, Event<T>},
		Scheduler: pallet_scheduler::{Module, Call, Config, Storage, Event<T>},
		GenericAsset: crml_generic_asset::{Module, Call, Storage, Config<T>, Event<T>},
		Governance: crml_governance::{Module, Call, Storage, Event},
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub BlockWeights: frame_system::limits::BlockWeights =
		frame_system::limits::BlockWeights::simple_max(1_000_000);
}
impl frame_system::Config for Test {
	type BlockWeights = ();
	type BlockLength = ();
	type BaseCallFilter = ();
	type Origin = Origin;
	type Index = u64;
	type BlockNumber = u64;
	type Call = Call;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type BlockHashCount = BlockHashCount;
	type Event = Event;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
}

parameter_types! {
	pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
}
impl crml_generic_asset::Config for Test {
	type AssetId = AssetId;
	type Balance = Balance;
	type Event = Event;
	type OnDustImbalance = ();
	type WeightInfo = ();
}

parameter_types! {
	pub const MaxScheduledPerBlock: u32 = 50;
	pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) * 100;

}
impl pallet_scheduler::Config for Test {
	type Event = Event;
	type Origin = Origin;
	type PalletsOrigin = OriginCaller;
	type Call = Call;
	type MaximumWeight = MaximumSchedulerWeight;
	type ScheduleOrigin = EnsureRoot<AccountId>;
	type MaxScheduledPerBlock = ();
	type WeightInfo = ();
}

pub struct MockStakingAmount;
impl StakingAmount for MockStakingAmount {
	type AccountId = AccountId;
	type Balance = Balance;

	fn active_balance(controller: &Self::AccountId) -> Self::Balance {
		match controller {
			1 => 1_000_000,
			2 => 20_000_000,
			3 => 30_000_000,
			_ => 0,
		}
	}

	fn count_nominators() -> u32 {
		1
	}
}

pub struct MockRegistrationImplementation;
impl RegistrationInfo for MockRegistrationImplementation {
	type AccountId = AccountId;
	/// Registration information for an identity
	fn registered_identity_count(who: &Self::AccountId) -> u32 {
		match who {
			1 => 2,
			2 => 1,
			3 => 3,
			_ => 0,
		}
	}
}

parameter_types! {
	pub const DefaultListingDuration: u64 = 5;
	pub const MaxAttributeLength: u8 = 140;
	pub const MaxCouncilSize: u16 = 2;
}
impl crate::Config for Test {
	type Call = Call;
	type Currency = StakingAssetCurrency<Self>;
	type MaxCouncilSize = MaxCouncilSize;
	type Scheduler = Scheduler;
	type PalletsOrigin = OriginCaller;
	type Event = Event;
	type WeightInfo = ();
	type Registration = MockRegistrationImplementation;
	type StakingAmount = MockStakingAmount;
}

#[derive(Default)]
pub struct ExtBuilder;

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut ext: sp_io::TestExternalities = frame_system::GenesisConfig::default()
			.build_storage::<Test>()
			.unwrap()
			.into();

		ext.execute_with(|| {
			System::initialize(&1, &[0u8; 32].into(), &Default::default(), frame_system::InitKind::Full);
		});

		ext
	}
}
