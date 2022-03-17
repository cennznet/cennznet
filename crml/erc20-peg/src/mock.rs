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

use crate as crml_erc20_peg;
use cennznet_primitives::types::{AccountId, AssetId, Balance};
use crml_generic_asset::impls::TransferDustImbalance;
use crml_support::H256;
use crml_support::{EthAbiCodec, EventClaimVerifier, H160};
use frame_support::{pallet_prelude::*, parameter_types, PalletId};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

pub const CENNZ_ASSET_ID: AssetId = 16000;
pub const CPAY_ASSET_ID: AssetId = 16001;
pub const NEXT_ASSET_ID: AssetId = 17000;

pub const STAKING_ASSET_ID: AssetId = CENNZ_ASSET_ID;
pub const SPENDING_ASSET_ID: AssetId = CPAY_ASSET_ID;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Module, Call, Config, Storage, Event<T>},
		GenericAsset: crml_generic_asset::{Module, Call, Storage, Config<T>, Event<T>},
		Erc20Peg: crml_erc20_peg::{Module, Call, Storage, Event<T>},
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
	type Hash = sp_core::H256;
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
	type OnDustImbalance = TransferDustImbalance<TreasuryPalletId>;
	type WeightInfo = ();
}

parameter_types! {
	pub const DefaultListingDuration: u64 = 5;
	pub const MaxAttributeLength: u8 = 140;
	pub const PegPalletId: PalletId = PalletId(*b"py/erc20");
	pub const DepositEventSignature: [u8; 32] = hex_literal::hex!("76bb911c362d5b1feb3058bc7dc9354703e4b6eb9c61cc845f73da880cf62f61");
}
impl crate::Config for Test {
	type DepositEventSignature = DepositEventSignature;
	type Event = Event;
	type EthBridge = MockEthBridge;
	type MultiCurrency = GenericAsset;
	type PegPalletId = PegPalletId;
}

/// Mock ethereum bridge
pub struct MockEthBridge;

impl EventClaimVerifier for MockEthBridge {
	/// Submit an event claim
	fn submit_event_claim(
		_contract_address: &H160,
		_event_signature: &H256,
		_tx_hash: &H256,
		_event_data: &[u8],
	) -> Result<u64, DispatchError> {
		Ok(1)
	}

	/// Generate proof of the given message
	/// Returns a unique proof Id on success
	fn generate_event_proof<M: EthAbiCodec>(_message: &M) -> Result<u64, DispatchError> {
		Ok(2)
	}
}

#[derive(Default)]
pub struct ExtBuilder;

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

		crml_generic_asset::GenesisConfig::<Test> {
			assets: vec![CENNZ_ASSET_ID, CPAY_ASSET_ID],
			initial_balance: 0,
			endowed_accounts: vec![],
			next_asset_id: NEXT_ASSET_ID,
			staking_asset_id: STAKING_ASSET_ID,
			spending_asset_id: SPENDING_ASSET_ID,
			permissions: vec![],
			asset_meta: vec![],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| {
			System::initialize(&1, &[0u8; 32].into(), &Default::default(), frame_system::InitKind::Full);
		});

		ext
	}
}
