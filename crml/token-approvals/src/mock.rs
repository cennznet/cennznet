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

use crate as token_approvals;
use cennznet_primitives::types::{AccountId, AssetId, Balance, TokenId};
use crml_generic_asset::impls::TransferDustImbalance;
use crml_support::IsTokenOwner;
use frame_support::{parameter_types, PalletId};
use hex_literal::hex;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		GenericAsset: crml_generic_asset::{Pallet, Call, Storage, Config<T>, Event<T>},
		Nft: crml_nft::{Pallet, Call, Storage, Event<T>},
		TokenApprovals: token_approvals::{Pallet, Call, Storage},
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
}
impl frame_system::Config for Test {
	type BlockWeights = ();
	type BlockLength = ();
	type BaseCallFilter = frame_support::traits::Everything;
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
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

parameter_types! {
	pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
}
impl crml_generic_asset::Config for Test {
	type AssetId = AssetId;
	type Balance = Balance;
	type Event = Event;
	type OnDustImbalance = TransferDustImbalance<TreasuryPalletId>;
	type WeightInfo = ();
}

parameter_types! {
	pub const DefaultListingDuration: u64 = 5;
	pub const MaxAttributeLength: u8 = 140;
}
impl crml_nft::Config for Test {
	type Event = Event;
	type MultiCurrency = GenericAsset;
	type MaxAttributeLength = MaxAttributeLength;
	type DefaultListingDuration = DefaultListingDuration;
	type WeightInfo = ();
	type OnTransferSubscription = TokenApprovals;
}

pub struct MockTokenOwner;
impl IsTokenOwner for MockTokenOwner {
	type AccountId = AccountId;

	fn check_ownership(account: &Self::AccountId, token_id: &TokenId) -> bool {
		let test_account = AccountId::from(hex!("63766d3a00000000000000a86e122edbdcba4bf24a2abf89f5c230b37df49d4a"));
		if account == &test_account && token_id == &(0u32, 0u32, 0u32) {
			return true;
		}
		return false;
	}
}

impl crate::Config for Test {
	type MultiCurrency = GenericAsset;
	type IsTokenOwner = MockTokenOwner;
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
			System::initialize(&1, &[0u8; 32].into(), &Default::default());
		});

		ext
	}
}
