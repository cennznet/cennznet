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

use cennznet_primitives::types::{AccountId, AssetId, Balance, BlockNumber, Hash, Index};
use cennznet_testing::keyring::{alice, bob, charlie};
use crml_scaling::{Module, Trait};
use frame_support::{
	additional_traits::DummyDispatchVerifier, impl_outer_dispatch, impl_outer_origin, parameter_types, weights::Weight,
};
use sp_core::crypto::AccountId32;
use sp_runtime::{
	generic,
	traits::{BlakeTwo256, IdentityLookup},
	Perbill,
};

pub const STAKING_ASSET_ID: AssetId = 16000;
pub const SPENDING_ASSET_ID: AssetId = 16001;
pub const PLUG_ASSET_ID: AssetId = 16002;
pub const NEXT_ASSET_ID: AssetId = 17000;

parameter_types! {
	pub const ScaleDownFactor: Balance = 1000_000_000_000;
	pub const BlockHashCount: u32 = 250;
	pub const MaximumBlockWeight: Weight = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::one();
}

use frame_system as system;
impl_outer_origin! {
	pub enum Origin for Test {}
}

impl_outer_dispatch! {
	pub enum Call for Test where origin: Origin {
		crml_scaling::Scaling,
	}
}

#[derive(Clone, Eq, PartialEq)]
pub struct Test;

pub type Scaling = Module<Test>;

impl Trait for Test {
	type ScaleDownFactor = ScaleDownFactor;
}
impl pallet_generic_asset::Trait for Test {
	type Balance = Balance;
	type AssetId = AssetId;
	type Event = ();
}
impl frame_system::Trait for Test {
	type Origin = Origin;
	type Call = ();
	type Index = Index;
	type BlockNumber = BlockNumber;
	type Hash = Hash;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<AccountId>;
	type Header = generic::Header<BlockNumber, BlakeTwo256>;
	type Event = ();
	type BlockHashCount = BlockHashCount;
	type Doughnut = ();
	type DelegatedDispatchVerifier = DummyDispatchVerifier<Self::Doughnut, Self::AccountId>;
	type MaximumBlockWeight = MaximumBlockWeight;
	type MaximumBlockLength = MaximumBlockLength;
	type AvailableBlockRatio = AvailableBlockRatio;
	type Version = ();
	type ModuleToIndex = ();
}
impl pallet_sudo::Trait for Test {
	type Event = ();
	type Call = Call;
}

pub struct ExtBuilder {
	sudoer: AccountId32,
}
impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			sudoer: Default::default(),
		}
	}
}
impl ExtBuilder {
	pub fn sudoer(mut self, key: AccountId32) -> Self {
		self.sudoer = key;
		self
	}
	pub fn build(self) -> sp_io::TestExternalities {
		let accounts = vec![alice(), bob(), charlie()];

		let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

		pallet_generic_asset::GenesisConfig::<Test> {
			assets: vec![STAKING_ASSET_ID, SPENDING_ASSET_ID, PLUG_ASSET_ID],
			initial_balance: Default::default(),
			endowed_accounts: accounts,
			next_asset_id: NEXT_ASSET_ID,
			staking_asset_id: STAKING_ASSET_ID,
			spending_asset_id: SPENDING_ASSET_ID,
			permissions: vec![
				(SPENDING_ASSET_ID, self.sudoer.clone()),
				(STAKING_ASSET_ID, self.sudoer.clone()),
				(PLUG_ASSET_ID, self.sudoer.clone()),
			],
			asset_meta: vec![],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		pallet_sudo::GenesisConfig::<Test> { key: alice() }
			.assimilate_storage(&mut t)
			.unwrap();

		t.into()
	}
}
