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

use crate as crml_nft;
use cennznet_primitives::types::{AssetId, Balance};
use frame_support::{parameter_types, traits::OnUnbalanced};
use prml_generic_asset::{AccountData, CheckedImbalance, NegativeImbalance};
use prml_support::MultiCurrencyAccounting;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{AccountIdConversion, BlakeTwo256, IdentityLookup},
	ModuleId,
};
use sp_std::mem;

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
		GenericAsset: prml_generic_asset::{Module, Call, Storage, Config<T>, Event<T>},
		Nft: crml_nft::{Module, Call, Storage, Event<T>},
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
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
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type BlockHashCount = BlockHashCount;
	type Event = Event;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = AccountData<u32>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
}

parameter_types! {
	pub const TreasuryModuleId: ModuleId = ModuleId(*b"py/trsry");
}
pub struct TransferImbalanceToTreasury;
impl OnUnbalanced<NegativeImbalance<Test>> for TransferImbalanceToTreasury {
	fn on_nonzero_unbalanced(imbalance: NegativeImbalance<Test>) {
		let treasury_account_id = TreasuryModuleId::get().into_account();
		let deposit_imbalance =
			GenericAsset::deposit_creating(&treasury_account_id, Some(imbalance.asset_id()), imbalance.amount());
		mem::forget(deposit_imbalance);
		mem::forget(imbalance);
	}
}
impl prml_generic_asset::Config for Test {
	type AssetId = AssetId;
	type Balance = Balance;
	type Event = Event;
	type AccountStore = System;
	type OnDustImbalance = TransferImbalanceToTreasury;
	type WeightInfo = ();
}

parameter_types! {
	pub const DefaultListingDuration: u64 = 5;
	pub const MaxAttributeLength: u8 = 140;
}
impl crate::Trait for Test {
	type Event = Event;
	type MultiCurrency = GenericAsset;
	type MaxAttributeLength = MaxAttributeLength;
	type DefaultListingDuration = DefaultListingDuration;
	type WeightInfo = ();
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
