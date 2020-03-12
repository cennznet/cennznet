// Copyright (C) 2020 Centrality Investments Limited
// This file is part of CENNZnet.
//
// CENNZnet is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// CENNZnet is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with CENNZnet.  If not, see <http://www.gnu.org/licenses/>.

#![allow(dead_code)]
use cennznet_cli::chain_spec::{get_authority_keys_from_seed, AuthorityKeys};
use cennznet_primitives::types::{AccountId, AssetId, Balance, BlockNumber, Hash};
use cennznet_runtime::constants::{asset::*, currency::*};
use cennznet_runtime::impls::{
	CurrencyToVoteHandler, FeeMultiplierUpdateHandler, GasHandler, GasMeteredCallResolver, LinearWeightToFee,
};
use cennznet_runtime::{
	Call, CennzxSpot, DealWithFees, ExchangeAddressGenerator, RandomnessCollectiveFlip, StakerStatus, VERSION,
};
use cennznet_testing::keyring::*;
use crml_cennzx_spot::{FeeRate, PerMilli, PerMillion};
use crml_staking::EraIndex;
use std::{cell::RefCell, collections::HashSet};

use core::convert::TryFrom;
use frame_support::{
	impl_outer_origin, parameter_types,
	traits::{FindAuthor, Get},
	weights::Weight,
};
use pallet_contracts::{Gas, Schedule};
use pallet_generic_asset::{SpendingAssetCurrency, StakingAssetCurrency};
use sp_core::crypto::key_types;
use sp_runtime::testing::{Header, UintAuthorityId};
use sp_runtime::{
	curve::PiecewiseLinear,
	traits::{IdentityLookup, OpaqueKeys},
	KeyTypeId, Perbill,
};
use sp_staking::SessionIndex;

pub const GENESIS_HASH: [u8; 32] = [69u8; 32];
pub const SPEC_VERSION: u32 = VERSION.spec_version;

pub type System = frame_system::Module<Test>;
pub type GenericAsset = pallet_generic_asset::Module<Test>;
pub type Session = pallet_session::Module<Test>;
pub type Timestamp = pallet_timestamp::Module<Test>;
pub type Staking = crml_staking::Module<Test>;

fn generate_initial_authorities(n: usize) -> Vec<AuthorityKeys> {
	assert!(n > 0 && n < 7); // because there are 6 pre-defined accounts
	let accounts = vec!["Alice", "Bob", "Charlie", "Dave", "Eve", "Ferdie"];
	accounts
		.iter()
		.take(n)
		.map(|s| get_authority_keys_from_seed(s))
		.collect()
}

// get all validators (stash account , controller account)
pub fn validators(n: usize) -> Vec<(AccountId, AccountId)> {
	assert!(n > 0 && n < 7); // because there are 6 pre-defined accounts
	generate_initial_authorities(n)
		.iter()
		.map(|x| (x.0.clone(), x.1.clone()))
		.collect()
}

/// Author of block is always `Alice`
pub struct AuthorAlice;
impl FindAuthor<AccountId> for AuthorAlice {
	fn find_author<'a, I>(_digests: I) -> Option<AccountId>
	where
		I: 'a + IntoIterator<Item = (frame_support::ConsensusEngineId, &'a [u8])>,
	{
		Some(validators(1).0)
	}
}

thread_local! {
	pub(crate) static SESSION: RefCell<(Vec<AccountId>, HashSet<AccountId>)> = RefCell::new(Default::default());
	static SLASH_DEFER_DURATION: RefCell<EraIndex> = RefCell::new(0);
}

pub struct TestSessionHandler;
impl pallet_session::SessionHandler<AccountId> for TestSessionHandler {
	const KEY_TYPE_IDS: &'static [KeyTypeId] = &[key_types::DUMMY];

	fn on_genesis_session<Ks: OpaqueKeys>(_validators: &[(AccountId, Ks)]) {}

	fn on_new_session<Ks: OpaqueKeys>(
		_changed: bool,
		validators: &[(AccountId, Ks)],
		_queued_validators: &[(AccountId, Ks)],
	) {
		SESSION.with(|x| *x.borrow_mut() = (validators.iter().map(|x| x.0.clone()).collect(), HashSet::new()));
	}

	fn on_disabled(validator_index: usize) {
		SESSION.with(|d| {
			let mut d = d.borrow_mut();
			let value = d.0[validator_index];
			d.1.insert(value);
		})
	}
}

pub struct SlashDeferDuration;
impl Get<EraIndex> for SlashDeferDuration {
	fn get() -> EraIndex {
		SLASH_DEFER_DURATION.with(|v| *v.borrow())
	}
}

impl_outer_origin! {
	pub enum Origin for Test where system = frame_system {}
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Test;

parameter_types! {
	pub const BlockHashCount: BlockNumber = 250;
	pub const MaximumBlockWeight: Weight = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::one();
}

impl frame_system::Trait for Test {
	type Origin = Origin;
	type Index = u64;
	type BlockNumber = BlockNumber;
	type Call = Call;
	type Hash = Hash;
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

impl pallet_generic_asset::Trait for Test {
	type Balance = Balance;
	type AssetId = AssetId;
	type Event = ();
}

parameter_types! {
	pub const TransactionBaseFee: Balance = 1 * CENTS;
	pub const TransactionByteFee: Balance = 10 * MILLICENTS;
	// setting this to zero will disable the weight fee.
	pub const WeightFeeCoefficient: Balance = 1_000;
}

impl crml_transaction_payment::Trait for Test {
	type Balance = Balance;
	type AssetId = AssetId;
	type Currency = SpendingAssetCurrency<Self>;
	type OnTransactionPayment = DealWithFees;
	type TransactionBaseFee = TransactionBaseFee;
	type TransactionByteFee = TransactionByteFee;
	type WeightToFee = LinearWeightToFee<WeightFeeCoefficient>;
	type FeeMultiplierUpdate = FeeMultiplierUpdateHandler;
	type BuyFeeAsset = CennzxSpot;
	type GasMeteredCallResolver = GasMeteredCallResolver;
}

parameter_types! {
	pub const Period: BlockNumber = 1;
	pub const Offset: BlockNumber = 0;
	pub const UncleGenerations: BlockNumber = 0;
	pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(25);
}
impl pallet_session::Trait for Test {
	type SessionManager = Staking;
	type Keys = UintAuthorityId;
	type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
	type SessionHandler = TestSessionHandler;
	type Event = ();
	type ValidatorId = AccountId;
	type ValidatorIdOf = crml_staking::StashOf<Self>;
	type DisabledValidatorsThreshold = DisabledValidatorsThreshold;
}

impl pallet_session::historical::Trait for Test {
	type FullIdentification = crml_staking::Exposure<AccountId, Balance>;
	type FullIdentificationOf = crml_staking::ExposureOf<Test>;
}
impl pallet_authorship::Trait for Test {
	type FindAuthor = AuthorAlice;
	type UncleGenerations = UncleGenerations;
	type FilterUncle = ();
	type EventHandler = Staking;
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
impl crml_staking::Trait for Test {
	type Currency = StakingAssetCurrency<Self>;
	type RewardCurrency = SpendingAssetCurrency<Self>;
	type CurrencyToReward = Balance;
	type Time = pallet_timestamp::Module<Self>;
	type CurrencyToVote = CurrencyToVoteHandler;
	type RewardRemainder = ();
	type Event = ();
	type Slash = ();
	type Reward = ();
	type SessionsPerEra = SessionsPerEra;
	type BondingDuration = BondingDuration;
	type SlashDeferDuration = SlashDeferDuration;
	type SlashCancelOrigin = frame_system::EnsureRoot<Self::AccountId, ()>;
	type SessionInterface = Self;
	type RewardCurve = RewardCurve;
}
impl crml_cennzx_spot::Trait for Test {
	type Call = Call;
	type Event = ();
	type ExchangeAddressGenerator = ExchangeAddressGenerator<Self>;
	type BalanceToUnsignedInt = Balance;
	type UnsignedIntToBalance = Balance;
}
parameter_types! {
	pub const ContractTransferFee: Balance = 1 * NANOCENTS;
	pub const ContractCreationFee: Balance = 1 * MICROCENTS;
	pub const ContractTransactionBaseFee: Balance = 1 * NANOCENTS;
	pub const ContractTransactionByteFee: Balance = 10 * MICROCENTS;
	pub const ContractFee: Balance = 1 * CENTS;
	pub const TombstoneDeposit: Balance = 1 * DOLLARS;
	pub const RentByteFee: Balance = 1 * DOLLARS;
	pub const RentDepositOffset: Balance = 1000 * DOLLARS;
	pub const SurchargeReward: Balance = 150 * DOLLARS;
	pub const BlockGasLimit: u64 = 100 * DOLLARS as u64;
}
impl pallet_contracts::Trait for Test {
	type Currency = SpendingAssetCurrency<Self>;
	type Time = Timestamp;
	type Randomness = RandomnessCollectiveFlip;
	type Call = Call;
	type Event = ();
	type DetermineContractAddress = pallet_contracts::SimpleAddressDeterminator<Test>;
	type ComputeDispatchFee = pallet_contracts::DefaultDispatchFeeComputor<Test>;
	type TrieIdGenerator = pallet_contracts::TrieIdFromParentCounter<Test>;
	type GasPayment = ();
	type GasHandler = GasHandler;
	type RentPayment = ();
	type SignedClaimHandicap = pallet_contracts::DefaultSignedClaimHandicap;
	type TombstoneDeposit = TombstoneDeposit;
	type StorageSizeOffset = pallet_contracts::DefaultStorageSizeOffset;
	type RentByteFee = RentByteFee;
	type RentDepositOffset = RentDepositOffset;
	type SurchargeReward = SurchargeReward;
	type TransferFee = ContractTransferFee;
	type CreationFee = ContractCreationFee;
	type TransactionBaseFee = ContractTransactionBaseFee;
	type TransactionByteFee = ContractTransactionByteFee;
	type ContractFee = ContractFee;
	type CallBaseFee = pallet_contracts::DefaultCallBaseFee;
	type InstantiateBaseFee = pallet_contracts::DefaultInstantiateBaseFee;
	type MaxDepth = pallet_contracts::DefaultMaxDepth;
	type MaxValueSize = pallet_contracts::DefaultMaxValueSize;
	type BlockGasLimit = BlockGasLimit;
}

pub struct ExtBuilder {
	initial_balance: Balance,
	gas_price: Balance,
	// Configurable prices for certain gas metered operations
	gas_sandbox_data_read_cost: Gas,
	gas_regular_op_cost: Gas,
	// Configurable fields for staking module tests
	stash: Balance,
	validator_count: usize,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			initial_balance: 0,
			gas_price: 0,
			gas_sandbox_data_read_cost: 0_u64,
			gas_regular_op_cost: 0_u64,
			stash: 0,
			validator_count: 3,
		}
	}
}

impl ExtBuilder {
	pub fn initial_balance(mut self, initial_balance: Balance) -> Self {
		self.initial_balance = initial_balance;
		self
	}
	pub fn gas_price(mut self, gas_price: Balance) -> Self {
		self.gas_price = gas_price;
		self
	}
	pub fn gas_sandbox_data_read_cost<T: Into<Gas>>(mut self, cost: T) -> Self {
		self.gas_sandbox_data_read_cost = cost.into();
		self
	}
	pub fn gas_regular_op_cost<T: Into<Gas>>(mut self, cost: T) -> Self {
		self.gas_regular_op_cost = cost.into();
		self
	}
	pub fn stash(mut self, stash: Balance) -> Self {
		self.stash = stash;
		self
	}
	pub fn validator_count(mut self, count: usize) -> Self {
		self.validator_count = count;
		self
	}
	pub fn build(self) -> sp_io::TestExternalities {
		let mut endowed_accounts = vec![alice(), bob(), charlie(), dave(), eve(), ferdie()];
		let initial_validators = validators(self.validator_count);
		let stash_accounts: Vec<_> = initial_validators.iter().map(|x| x.0.clone()).collect();
		endowed_accounts.extend(stash_accounts);

		let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();
		crml_cennzx_spot::GenesisConfig::<Test> {
			fee_rate: FeeRate::<PerMillion>::try_from(FeeRate::<PerMilli>::from(3u128)).unwrap(),
			core_asset_id: CENTRAPAY_ASSET_ID,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		// Configure the gas schedule
		let mut gas_price_schedule = Schedule::default();
		gas_price_schedule.sandbox_data_read_cost = self.gas_sandbox_data_read_cost;
		gas_price_schedule.regular_op_cost = self.gas_regular_op_cost;

		pallet_contracts::GenesisConfig::<Test> {
			current_schedule: gas_price_schedule,
			gas_price: self.gas_price,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		pallet_generic_asset::GenesisConfig::<Test> {
			assets: vec![
				CENNZ_ASSET_ID,
				CENTRAPAY_ASSET_ID,
				PLUG_ASSET_ID,
				SYLO_ASSET_ID,
				CERTI_ASSET_ID,
				ARDA_ASSET_ID,
			],
			initial_balance: self.initial_balance,
			endowed_accounts: endowed_accounts,
			next_asset_id: NEXT_ASSET_ID,
			staking_asset_id: STAKING_ASSET_ID,
			spending_asset_id: SPENDING_ASSET_ID,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		crml_staking::GenesisConfig::<Test> {
			current_era: 0,
			validator_count: initial_validators.len() as u32 * 2,
			minimum_validator_count: initial_validators.len() as u32,
			stakers: initial_validators
				.iter()
				.map(|x| (x.0.clone(), x.1.clone(), self.stash, StakerStatus::Validator))
				.collect(),
			slash_reward_fraction: Perbill::from_percent(10),
			..Default::default()
		}
		.assimilate_storage(&mut t)
		.unwrap();

		pallet_session::GenesisConfig::<Test> {
			keys: initial_validators
				.iter()
				.map(|x| (x.0.clone(), UintAuthorityId(x.0)))
				.collect::<Vec<_>>(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}

/// Test contracts
pub mod contracts {

	/// Contract WABT for reading 32 bytes from memory
	pub const CONTRACT_READ_32_BYTES: &str = r#"
	(module
		(import "env" "ext_scratch_read" (func $ext_scratch_read (param i32 i32 i32)))
		(import "env" "memory" (memory 1 1))
		(func (export "deploy"))
		(func (export "call")
			(call $ext_scratch_read
				(i32.const 0)
				(i32.const 0)
				(i32.const 4)
			)
		)

		;; 32 bytes for reading
		(data (i32.const 4)
			"\09\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00"
		)
	)"#;

	/// Contract WABT for a contract which will fail during execution
	pub const CONTRACT_WITH_TRAP: &str = r#"
	(module
		(import "env" "ext_scratch_read" (func $ext_scratch_read (param i32 i32 i32)))
		(import "env" "memory" (memory 1 1))
		(func (export "deploy"))
		(func (export "call")
			unreachable
		)
	)"#;

	/// Contract WABT for a contract which dispatches a generic asset transfer of CENNZ to charlie
	pub const CONTRACT_WITH_GA_TRANSFER: &str = r#"
	(module
		(import "env" "ext_dispatch_call" (func $ext_dispatch_call (param i32 i32)))
		(import "env" "memory" (memory 1 1))
		(func (export "call")
			(call $ext_dispatch_call
				(i32.const 8) ;; Pointer to the start of encoded call buffer
				(i32.const 42) ;; Length of the buffer
			)
		)
		(func (export "deploy"))
		(data (i32.const 8) "\06\01\01\FA\90\B5\AB\20\5C\69\74\C9\EA\84\1B\E6\88\86\46\33\DC\9C\A8\A3\57\84\3E\EA\CF\23\14\64\99\65\FE\22\07\00\10\A5\D4\E8")
	)"#;
}
