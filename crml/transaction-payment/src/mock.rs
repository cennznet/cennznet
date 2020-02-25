// Copyright 2018-2020 Parity Technologies (UK) Ltd. and Centrality Investments Ltd.
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

use crate::{Module, Trait};
use cennznet_primitives::{
	traits::{BuyFeeAsset, IsGasMeteredCall},
	types::FeeExchange,
};
use frame_support::{
	additional_traits::DummyDispatchVerifier,
	dispatch::DispatchError,
	impl_outer_dispatch, impl_outer_origin, parameter_types,
	traits::{Currency, Get},
	weights::{DispatchInfo, Weight},
};
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, Convert, IdentityLookup},
	Perbill,
};
use sp_std::cell::RefCell;

pub const VALID_ASSET_TO_BUY_FEE: u32 = 1;
pub const INVALID_ASSET_TO_BUY_FEE: u32 = 2;
// Transfers into this account signal the extrinsic call should be considered gas metered
pub const GAS_METERED_ACCOUNT_ID: u64 = 10;

impl_outer_dispatch! {
	pub enum Call for Runtime where origin: Origin {
		pallet_balances::Balances,
		frame_system::System,
	}
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Runtime;

use frame_system as system;
impl_outer_origin! {
	pub enum Origin for Runtime {}
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: Weight = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::one();
}

type AccountId = u64;

impl frame_system::Trait for Runtime {
	type Origin = Origin;
	type Index = u64;
	type BlockNumber = u64;
	type Call = Call;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = ();
	type BlockHashCount = BlockHashCount;
	type MaximumBlockWeight = MaximumBlockWeight;
	type MaximumBlockLength = MaximumBlockLength;
	type AvailableBlockRatio = AvailableBlockRatio;
	type Doughnut = ();
	type DelegatedDispatchVerifier = DummyDispatchVerifier<Self::Doughnut, Self::AccountId>;
	type Version = ();
	type ModuleToIndex = ();
}

parameter_types! {
	pub const TransferFee: u64 = 0;
	pub const CreationFee: u64 = 0;
	pub const ExistentialDeposit: u64 = 0;
}

impl pallet_balances::Trait for Runtime {
	type Balance = u64;
	type OnFreeBalanceZero = ();
	type OnReapAccount = System;
	type OnNewAccount = ();
	type Event = ();
	type TransferPayment = ();
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type TransferFee = TransferFee;
	type CreationFee = CreationFee;
}

thread_local! {
	static TRANSACTION_BASE_FEE: RefCell<u64> = RefCell::new(0);
	static TRANSACTION_BYTE_FEE: RefCell<u64> = RefCell::new(1);
	static WEIGHT_TO_FEE: RefCell<u64> = RefCell::new(1);
}

pub struct TransactionBaseFee;
impl Get<u64> for TransactionBaseFee {
	fn get() -> u64 {
		TRANSACTION_BASE_FEE.with(|v| *v.borrow())
	}
}

pub struct TransactionByteFee;
impl Get<u64> for TransactionByteFee {
	fn get() -> u64 {
		TRANSACTION_BYTE_FEE.with(|v| *v.borrow())
	}
}

pub struct WeightToFee(u64);
impl Convert<Weight, u64> for WeightToFee {
	fn convert(t: Weight) -> u64 {
		WEIGHT_TO_FEE.with(|v| *v.borrow() * (t as u64))
	}
}

/// Implement a fake BuyFeeAsset for tests
impl BuyFeeAsset for Module<Runtime> {
	type AccountId = u64;
	type Balance = u64;
	type FeeExchange = FeeExchange<<Runtime as Trait>::AssetId, Self::Balance>;
	fn buy_fee_asset(
		who: &Self::AccountId,
		amount: Self::Balance,
		exchange_op: &Self::FeeExchange,
	) -> sp_std::result::Result<Self::Balance, DispatchError> {
		if exchange_op.asset_id() == VALID_ASSET_TO_BUY_FEE {
			if exchange_op.max_payment() == 0 {
				return Err(DispatchError::Other("no money"));
			}
			// buy fee asset at a 1:1 ratio
			let _ = Balances::deposit_into_existing(who, amount)?;
		}
		Ok(amount)
	}
}

impl Trait for Runtime {
	type Balance = u128;
	type AssetId = u32;
	type Currency = pallet_balances::Module<Runtime>;
	type OnTransactionPayment = ();
	type TransactionBaseFee = TransactionBaseFee;
	type TransactionByteFee = TransactionByteFee;
	type WeightToFee = WeightToFee;
	type FeeMultiplierUpdate = ();
	type BuyFeeAsset = Module<Self>;
	type GasMeteredCallResolver = MockCallResolver;
}

pub type Balances = pallet_balances::Module<Runtime>;
pub type System = frame_system::Module<Runtime>;
pub type TransactionPayment = Module<Runtime>;

pub struct ExtBuilder {
	balance_factor: u64,
	base_fee: u64,
	byte_fee: u64,
	weight_to_fee: u64,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			balance_factor: 1,
			base_fee: 0,
			byte_fee: 1,
			weight_to_fee: 1,
		}
	}
}

impl ExtBuilder {
	pub fn fees(mut self, base: u64, byte: u64, weight: u64) -> Self {
		self.base_fee = base;
		self.byte_fee = byte;
		self.weight_to_fee = weight;
		self
	}
	pub fn balance_factor(mut self, factor: u64) -> Self {
		self.balance_factor = factor;
		self
	}
	fn set_constants(&self) {
		TRANSACTION_BASE_FEE.with(|v| *v.borrow_mut() = self.base_fee);
		TRANSACTION_BYTE_FEE.with(|v| *v.borrow_mut() = self.byte_fee);
		WEIGHT_TO_FEE.with(|v| *v.borrow_mut() = self.weight_to_fee);
	}
	pub fn build(self) -> sp_io::TestExternalities {
		self.set_constants();
		let mut t = system::GenesisConfig::default().build_storage::<Runtime>().unwrap();
		pallet_balances::GenesisConfig::<Runtime> {
			balances: vec![
				(1, 10 * self.balance_factor),
				(2, 20 * self.balance_factor),
				(3, 30 * self.balance_factor),
				(4, 40 * self.balance_factor),
				(5, 50 * self.balance_factor),
				(6, 60 * self.balance_factor),
			],
			vesting: vec![],
		}
		.assimilate_storage(&mut t)
		.unwrap();
		t.into()
	}
}

/// create a transaction info struct from weight. Handy to avoid building the whole struct.
pub fn info_from_weight(w: Weight) -> DispatchInfo {
	DispatchInfo {
		weight: w,
		pays_fee: true,
		..Default::default()
	}
}

/// A mock impl of `IsGasMeteredCall`
pub struct MockCallResolver;

impl IsGasMeteredCall for MockCallResolver {
	type Call = Call;
	fn is_gas_metered(call: &Self::Call) -> bool {
		match call {
			Call::Balances(pallet_balances::Call::transfer_keep_alive(who, _)) => &GAS_METERED_ACCOUNT_ID == who,
			_ => false,
		}
	}
}
