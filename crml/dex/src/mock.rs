#![cfg(test)]

use super::*;
use frame_support::{construct_runtime, ord_parameter_types, parameter_types, traits::Everything};
use frame_system::EnsureSignedBy;
use primitives::{AssetId, Balance, TradingPair};
use sp_core::H256;
use sp_runtime::{testing::Header, traits::IdentityLookup};

pub type BlockNumber = u64;
pub type AccountId = u128;
pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const pBTC: AssetId = 101;
pub const pETH: AssetId = 102;
pub const pUSDC: AssetId = 103;
pub const PLUG: AssetId = 1;

parameter_types! {
	pub static pUSDCBTCPair: TradingPair = TradingPair::from_token_asset_ids(pUSDC, pBTC).unwrap();
	pub static pUSDCETHPair: TradingPair = TradingPair::from_token_asset_ids(pUSDC, pETH).unwrap();
	pub static pETHBTCPair: TradingPair = TradingPair::from_token_asset_ids(pETH, pBTC).unwrap();
}

mod dex {
	pub use super::super::*;
}

parameter_types! {
	pub const BlockHashCount: BlockNumber = 250;
}

impl frame_system::Config for Runtime {
	type Origin = Origin;
	type Index = u64;
	type BlockNumber = BlockNumber;
	type Call = Call;
	type Hash = H256;
	type Hashing = ::sp_runtime::traits::BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type BlockWeights = ();
	type BlockLength = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type DbWeight = ();
	type BaseCallFilter = Everything;
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
}

parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
	pub const MaxReserves: u32 = 50;
}

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = [u8; 8];
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type MaxLocks = ();
	type WeightInfo = ();
}

pub type PalletBalances = pallet_balances::Pallet<Runtime>;

ord_parameter_types! {
	pub const ListingOrigin: AccountId = 3;
}

parameter_types! {
	pub const GetExchangeFee: (u32, u32) = (1, 100);
	pub const TradingPathLimit: u32 = 3;
	pub const DEXPalletId: PalletId = PalletId(*b"plug/dex");
}

impl plug_utils::Config for Runtime {
	type Event = Event;
	type Assets = Assets;
	type Currency = Balances;
	type AdminOrigin = EnsureSignedBy<ListingOrigin, AccountId>;
}

parameter_types! {
	pub const AssetDeposit: u64 = 1;
	pub const ApprovalDeposit: u64 = 1;
	pub const StringLimit: u32 = 50;
	pub const MetadataDepositBase: u64 = 1;
	pub const MetadataDepositPerByte: u64 = 1;
}

impl pallet_assets::Config for Runtime {
	type Event = Event;
	type Balance = u128;
	type AssetId = u64;
	type Currency = Balances;
	type ForceOrigin = frame_system::EnsureRoot<AccountId>;
	type AssetDeposit = AssetDeposit;
	type MetadataDepositBase = MetadataDepositBase;
	type MetadataDepositPerByte = MetadataDepositPerByte;
	type ApprovalDeposit = ApprovalDeposit;
	type StringLimit = StringLimit;
	type Freezer = ();
	type WeightInfo = ();
	type Extra = ();
}

impl Config for Runtime {
	type Event = Event;
	type GetExchangeFee = GetExchangeFee;
	type TradingPathLimit = TradingPathLimit;
	type DEXPalletId = DEXPalletId;
	type WeightInfo = ();
	type ListingOrigin = EnsureSignedBy<ListingOrigin, AccountId>;
}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Storage, Config, Event<T>},
		DexModule: dex::{Pallet, Storage, Call, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Util: plug_utils::{Pallet, Call, Event<T>},
		Assets: pallet_assets::{Pallet, Call, Storage, Event<T>},
	}
);

pub struct ExtBuilder;

impl Default for ExtBuilder {
	fn default() -> Self {
		ExtBuilder
	}
}

/*
impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		t.into()
	}
}
*/

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| Assets::create(Origin::signed(1), pUSDC, 1, 1));
		ext.execute_with(|| Assets::create(Origin::signed(1), pETH, 1, 1));
		ext.execute_with(|| Assets::create(Origin::signed(1), pBTC, 1, 1));
		ext.execute_with(|| {
			Assets::create(
				Origin::signed(1),
				pUSDCBTCPair::get().get_dex_share_asset_id().unwrap(),
				1,
				1,
			)
		});
		ext.execute_with(|| {
			Assets::create(
				Origin::signed(1),
				pUSDCBTCPair::get().get_dex_share_asset_id().unwrap(),
				1,
				1,
			)
		});
		ext.execute_with(|| {
			Assets::create(
				Origin::signed(1),
				pUSDCBTCPair::get().get_dex_share_asset_id().unwrap(),
				1,
				1,
			)
		});
		ext
	}
}
