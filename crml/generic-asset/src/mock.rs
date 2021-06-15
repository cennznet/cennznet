// Copyright 2019-2021
//     by  Centrality Investments Ltd.
//     and Parity Technologies (UK) Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

//! Mocks for the module.

#![cfg(test)]

use super::*;
use crate::{self as crml_generic_asset, NegativeImbalance, PositiveImbalance};
use frame_support::parameter_types;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{AccountIdConversion, BlakeTwo256, IdentityLookup},
	ModuleId,
};
use sp_std::mem;

// test accounts
pub const ALICE: u64 = 1;
pub const BOB: u64 = 2;
pub const CHARLIE: u64 = 3;

// staking asset id
pub const STAKING_ASSET_ID: u32 = 16000;
// spending asset id
pub const SPENDING_ASSET_ID: u32 = 16001;
// pre-existing asset 1
pub const TEST1_ASSET_ID: u32 = 16003;
// pre-existing asset 2
pub const TEST2_ASSET_ID: u32 = 16004;
// default next asset id
pub const ASSET_ID: u32 = 1000;
// initial issuance for creating new asset
pub const INITIAL_ISSUANCE: u64 = 1_000_000;
// initial balance for setting free balance
pub const INITIAL_BALANCE: u64 = 100;
// lock identifier
pub const ID_1: LockIdentifier = *b"1       ";
// lock identifier
pub const ID_2: LockIdentifier = *b"2       ";

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
	}
);

pub type PositiveImbalanceOf = PositiveImbalance<Test>;
pub type NegativeImbalanceOf = NegativeImbalance<Test>;

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
	type AccountData = ();
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
		let treasury_balance = GenericAsset::free_balance(imbalance.asset_id(), &treasury_account_id);
		GenericAsset::set_free_balance(
			imbalance.asset_id(),
			&treasury_account_id,
			treasury_balance + imbalance.amount(),
		);
		mem::forget(imbalance);
	}
}

impl Config for Test {
	type Balance = u64;
	type AssetId = u32;
	type Event = Event;
	type OnDustImbalance = TransferImbalanceToTreasury;
	type WeightInfo = ();
}

// Build storage for generic asset with some default values
pub(crate) fn new_test_ext(
	assets: Vec<u32>,
	endowed_accounts: Vec<u64>,
	initial_balance: u64,
	permissions: Vec<(u32, u64)>,
	next_asset_id: u32,
) -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

	crml_generic_asset::GenesisConfig::<Test> {
		assets,
		endowed_accounts,
		initial_balance,
		next_asset_id,
		staking_asset_id: STAKING_ASSET_ID,
		spending_asset_id: SPENDING_ASSET_ID,
		permissions,
		asset_meta: vec![
			(TEST1_ASSET_ID, AssetInfo::new(b"TST1".to_vec(), 1, 3)),
			(TEST2_ASSET_ID, AssetInfo::new(b"TST 2".to_vec(), 2, 5)),
		],
	}
	.assimilate_storage(&mut t)
	.unwrap();

	t.into()
}

pub(crate) fn new_test_ext_with_default() -> sp_io::TestExternalities {
	new_test_ext(vec![0], vec![], 0, vec![], ASSET_ID)
}
pub(crate) fn new_test_ext_with_balance(
	asset_id: u32,
	account_id: u64,
	initial_balance: u64,
) -> sp_io::TestExternalities {
	new_test_ext(vec![asset_id], vec![account_id], initial_balance, vec![], ASSET_ID)
}

pub(crate) fn new_test_ext_with_next_asset_id(next_asset_id: u32) -> sp_io::TestExternalities {
	new_test_ext(vec![0], vec![], 0, vec![], next_asset_id)
}

pub(crate) fn new_test_ext_with_permissions(permissions: Vec<(u32, u64)>) -> sp_io::TestExternalities {
	new_test_ext(vec![0], vec![], 0, permissions, TEST2_ASSET_ID + 1)
}
