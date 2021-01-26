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

//! The CENNZnet runtime. This can be compiled with ``#[no_std]`, ready for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

use codec::Encode;

use pallet_authority_discovery;
use pallet_grandpa::fg_primitives;
use pallet_grandpa::{AuthorityId as GrandpaId, AuthorityList as GrandpaAuthorityList};
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use pallet_session;
use pallet_session::historical as session_historical;
use pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo;
use prml_generic_asset_rpc_runtime_api;
use sp_api::impl_runtime_apis;
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_babe;
use sp_core::{crypto::KeyTypeId, OpaqueMetadata};
use sp_runtime::{
	create_runtime_str,
	generic::{self, Era},
	impl_opaque_keys,
	traits::{
		BlakeTwo256, Block as BlockT, Extrinsic, IdentityLookup, NumberFor, OpaqueKeys, SaturatedConversion,
		Saturating, Verify,
	},
	transaction_validity::{TransactionSource, TransactionValidity},
	ApplyExtrinsicResult, FixedPointNumber,
};
use sp_std::prelude::*;
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;

use crml_staking::rewards as crml_staking_rewards;
pub use crml_staking::StakerStatus;
pub use frame_support::{
	construct_runtime, debug,
	dispatch::marker::PhantomData,
	ord_parameter_types, parameter_types,
	traits::{KeyOwnerProofSystem, Randomness},
	weights::{
		constants::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight, WEIGHT_PER_SECOND},
		IdentityFee, TransactionPriority, Weight,
	},
	StorageValue,
};
use frame_system::EnsureRoot;
pub use pallet_timestamp::Call as TimestampCall;
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;
pub use sp_runtime::{ModuleId, Perbill, Percent, Permill, Perquintill};

// CENNZnet only imports
use cennznet_primitives::types::{AccountId, AssetId, Balance, BlockNumber, Hash, Header, Index, Moment, Signature};
pub use crml_cennzx::{ExchangeAddressGenerator, FeeRate, PerMillion, PerThousand};
use crml_cennzx_rpc_runtime_api::CennzxResult;
pub use crml_sylo::device as sylo_device;
pub use crml_sylo::e2ee as sylo_e2ee;
pub use crml_sylo::groups as sylo_groups;
pub use crml_sylo::inbox as sylo_inbox;
pub use crml_sylo::payment as sylo_payment;
pub use crml_sylo::response as sylo_response;
pub use crml_sylo::vault as sylo_vault;
pub use crml_transaction_payment::{Multiplier, TargetedFeeAdjustment};
pub use prml_generic_asset::{AssetInfo, Call as GenericAssetCall, SpendingAssetCurrency, StakingAssetCurrency};

/// Constant values used within the runtime.
pub mod constants;
use constants::{currency::*, time::*};

// Implementations of some helper traits passed into runtime modules as associated types.
pub mod impls;
use impls::{CurrencyToVoteHandler, FeePayerResolver, RootMemberOnly, SlashFundsToTreasury, WeightToCpayFee};

/// Deprecated host functions required for syncing blocks prior to 2.0 upgrade
pub mod legacy_host_functions;

/// Weights for CENNZnet runtime modules (crml packages)
mod weights;

use crate::opaque::SessionKeys;

/// Runtime version.
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: create_runtime_str!("cennznet"),
	impl_name: create_runtime_str!("cennznet-node"),
	authoring_version: 1,
	// Per convention: if the runtime behavior changes, increment `spec_version`
	// and set `impl_version` to equal spec_version. If only runtime
	// implementation changes and behavior does not, then leave `spec_version` as
	// is and increment `impl_version`.
	spec_version: 38,
	impl_version: 38,
	apis: RUNTIME_API_VERSIONS,
	transaction_version: 5,
};

/// Native version.
#[cfg(any(feature = "std", test))]
pub fn native_version() -> NativeVersion {
	NativeVersion {
		runtime_version: VERSION,
		can_author_with: Default::default(),
	}
}

// Configure modules to include in the runtime.

const AVERAGE_ON_INITIALIZE_WEIGHT: Perbill = Perbill::from_percent(10);
parameter_types! {
	pub const BlockHashCount: BlockNumber = 2400;
	/// We allow for 2 seconds of compute with a 5 second average block time.
	pub const MaximumBlockWeight: Weight = 2 * WEIGHT_PER_SECOND;
	pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
	/// Assume 10% of weight for average on_initialize calls.
	pub MaximumExtrinsicWeight: Weight =
		AvailableBlockRatio::get().saturating_sub(AVERAGE_ON_INITIALIZE_WEIGHT)
		* MaximumBlockWeight::get();
	pub const MaximumBlockLength: u32 = 5 * 1024 * 1024;
	pub const Version: RuntimeVersion = VERSION;
}
// const_assert!(AvailableBlockRatio::get().deconstruct() >= AVERAGE_ON_INITIALIZE_WEIGHT.deconstruct());

impl frame_system::Trait for Runtime {
	/// The basic call filter to use in dispatchable.
	type BaseCallFilter = ();
	/// The identifier used to distinguish between accounts.
	type AccountId = AccountId;
	/// The aggregated dispatch type that is available for extrinsics.
	type Call = Call;
	/// The lookup mechanism to get account ID from whatever is passed in dispatchers.
	type Lookup = IdentityLookup<AccountId>;
	/// The index type for storing how many extrinsics an account has signed.
	type Index = Index;
	/// The index type for blocks.
	type BlockNumber = BlockNumber;
	/// The type for hashing blocks and tries.
	type Hash = Hash;
	/// The hashing algorithm used.
	type Hashing = BlakeTwo256;
	/// The header type.
	type Header = generic::Header<BlockNumber, BlakeTwo256>;
	/// The ubiquitous event type.
	type Event = Event;
	/// The ubiquitous origin type.
	type Origin = Origin;
	/// Maximum number of block number to block hash mappings to keep (oldest pruned first).
	type BlockHashCount = BlockHashCount;
	/// Maximum weight of each block.
	type MaximumBlockWeight = MaximumBlockWeight;
	/// The weight of database operations that the runtime can invoke.
	type DbWeight = RocksDbWeight;
	/// The weight of the overhead invoked on the block import process, independent of the
	/// extrinsics included in that block.
	type BlockExecutionWeight = BlockExecutionWeight;
	/// The base weight of any extrinsic processed by the runtime, independent of the
	/// logic of that extrinsic. (Signature verification, nonce increment, fee, etc...)
	type ExtrinsicBaseWeight = ExtrinsicBaseWeight;
	/// The maximum weight that a single extrinsic of `Normal` dispatch class can have,
	/// independent of the logic of that extrinsics. (Roughly max block weight - average on
	/// initialize cost).
	type MaximumExtrinsicWeight = MaximumExtrinsicWeight;
	/// Maximum size of all encoded transactions (in bytes) that are allowed in one block.
	type MaximumBlockLength = MaximumBlockLength;
	/// Portion of the block weight that is available to all normal transactions.
	type AvailableBlockRatio = AvailableBlockRatio;
	/// Version of the runtime.
	type Version = Version;
	/// Converts a module to the index of the module in `construct_runtime!`.
	///
	/// This type is being generated by `construct_runtime!`.
	type PalletInfo = PalletInfo;
	/// What to do if a new account is created.
	type OnNewAccount = ();
	/// What to do if an account is fully reaped from the system.
	type OnKilledAccount = ();
	/// The data to be stored in an account.
	type AccountData = ();
	/// Weight information for the extrinsics of this pallet.
	type SystemWeightInfo = ();
}

impl crml_nft::Trait for Runtime {
	type Event = Event;
	type ClassId = u32;
	type TokenId = u32;
	type WeightInfo = ();
}

parameter_types! {
	pub const UncleGenerations: BlockNumber = 5;
}
impl pallet_authorship::Trait for Runtime {
	type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Babe>;
	type UncleGenerations = UncleGenerations;
	type FilterUncle = ();
	type EventHandler = (Staking, ImOnline);
}

parameter_types! {
	pub const EpochDuration: u64 = EPOCH_DURATION_IN_SLOTS;
	pub const ExpectedBlockTime: Moment = MILLISECS_PER_BLOCK;
}
impl pallet_babe::Trait for Runtime {
	type EpochDuration = EpochDuration;
	type ExpectedBlockTime = ExpectedBlockTime;
	type EpochChangeTrigger = pallet_babe::ExternalTrigger;
	type KeyOwnerProofSystem = Historical;
	type KeyOwnerProof =
		<Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(KeyTypeId, pallet_babe::AuthorityId)>>::Proof;
	type KeyOwnerIdentification =
		<Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(KeyTypeId, pallet_babe::AuthorityId)>>::IdentificationTuple;
	type HandleEquivocation = pallet_babe::EquivocationHandler<Self::KeyOwnerIdentification, Offences>;
	type WeightInfo = ();
}

impl pallet_grandpa::Trait for Runtime {
	type Event = Event;
	type Call = Call;
	type KeyOwnerProofSystem = ();
	type KeyOwnerProof = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(KeyTypeId, GrandpaId)>>::Proof;
	type KeyOwnerIdentification =
		<Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(KeyTypeId, GrandpaId)>>::IdentificationTuple;
	type HandleEquivocation = ();
	type WeightInfo = ();
}

parameter_types! {
	pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) * MaximumBlockWeight::get();
	pub const MaxScheduledPerBlock: u32 = 50;
}
impl pallet_scheduler::Trait for Runtime {
	type Event = Event;
	type Origin = Origin;
	type PalletsOrigin = OriginCaller;
	type Call = Call;
	type MaximumWeight = MaximumSchedulerWeight;
	type ScheduleOrigin = EnsureRoot<AccountId>;
	type MaxScheduledPerBlock = MaxScheduledPerBlock;
	type WeightInfo = ();
}

parameter_types! {
	pub const SessionsPerEra: sp_staking::SessionIndex = SESSIONS_PER_ERA;
	pub const BlocksPerEra: BlockNumber = EPOCH_DURATION_IN_BLOCKS * SESSIONS_PER_ERA;
	pub const BondingDuration: crml_staking::EraIndex = 24 * 28;
	pub const SlashDeferDuration: crml_staking::EraIndex = 24 * 7; // 1/4 the bonding duration.
}
impl crml_staking::Trait for Runtime {
	type Currency = StakingAssetCurrency<Self>;
	type Time = Timestamp;
	type CurrencyToVote = CurrencyToVoteHandler;
	type Event = Event;
	type Slash = SlashFundsToTreasury; // send the slashed funds in CENNZ to the treasury.
	type SessionsPerEra = SessionsPerEra;
	type BlocksPerEra = BlocksPerEra;
	type BondingDuration = BondingDuration;
	type SlashDeferDuration = SlashDeferDuration;
	type SessionInterface = Self;
	type Rewarder = Rewards;
	type WeightInfo = ();
}

parameter_types! {
	pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(17);
}
impl pallet_session::Trait for Runtime {
	type Event = Event;
	type ValidatorId = <Self as frame_system::Trait>::AccountId;
	type ValidatorIdOf = crml_staking::StashOf<Self>;
	type ShouldEndSession = Babe;
	type NextSessionRotation = Babe;
	type SessionManager = pallet_session::historical::NoteHistoricalRoot<Self, Staking>;
	type SessionHandler = <SessionKeys as OpaqueKeys>::KeyTypeIdProviders;
	type Keys = SessionKeys;
	type DisabledValidatorsThreshold = DisabledValidatorsThreshold;
	type WeightInfo = ();
}

parameter_types! {
	pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
}
impl pallet_timestamp::Trait for Runtime {
	/// A timestamp: milliseconds since the unix epoch.
	type Moment = u64;
	type OnTimestampSet = Babe;
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
}

impl prml_generic_asset::Trait for Runtime {
	type AssetId = AssetId;
	type Balance = Balance;
	type Event = Event;
	type WeightInfo = ();
}

parameter_types! {
	pub const TransactionByteFee: Balance = 100 * MICROS;
	pub const TargetBlockFullness: Perquintill = Perquintill::from_percent(25);
	// weight:cpay/0.005%
	// optimising for a GA transfer fee of ~1.0000 CPAY
	pub const WeightToCpayFactor: Perbill = Perbill::from_parts(1_500);
	// `1/50_000` comes from  halving substrate's: `1/100,000` config.
	// due to CENNZnet having a blocktime ~2x slower.
	// We do this to constrain fee adjustment to the recommended +/-23% fee adjustment per day
	pub AdjustmentVariable: Multiplier = Multiplier::saturating_from_rational(1, 50_000);
	pub MinimumMultiplier: Multiplier = Multiplier::saturating_from_rational(1, 500_000_000u128);
}
impl crml_transaction_payment::Trait for Runtime {
	type AssetId = AssetId;
	type Currency = SpendingAssetCurrency<Self>;
	type OnTransactionPayment = Rewards;
	type TransactionByteFee = TransactionByteFee;
	type WeightToFee = WeightToCpayFee<WeightToCpayFactor>;
	type FeeMultiplierUpdate = TargetedFeeAdjustment<Self, TargetBlockFullness, AdjustmentVariable, MinimumMultiplier>;
	type BuyFeeAsset = Cennzx;
	type FeePayer = FeePayerResolver;
}

pub const fn deposit(items: u32, bytes: u32) -> Balance {
	items as Balance * 15 + (bytes as Balance) * 6
}

parameter_types! {
	// One storage item; key size is 32; value is size 4+4+16+32 bytes = 56 bytes.
	pub const DepositBase: Balance = deposit(1, 88);
	// Additional storage item size of 32 bytes.
	pub const DepositFactor: Balance = deposit(0, 32);
	pub const MaxSignatories: u16 = 100;
}
impl pallet_multisig::Trait for Runtime {
	type Event = Event;
	type Call = Call;
	type Currency = SpendingAssetCurrency<Self>;
	type DepositBase = DepositBase;
	type DepositFactor = DepositFactor;
	type MaxSignatories = MaxSignatories;
	type WeightInfo = ();
}

impl pallet_sudo::Trait for Runtime {
	type Event = Event;
	type Call = Call;
}

impl pallet_utility::Trait for Runtime {
	type Event = Event;
	type Call = Call;
	type WeightInfo = ();
}

impl pallet_authority_discovery::Trait for Runtime {}

parameter_types! {
	pub const SessionDuration: BlockNumber = EPOCH_DURATION_IN_BLOCKS as _;
	pub const ImOnlineUnsignedPriority: TransactionPriority = TransactionPriority::max_value();
	pub const StakingUnsignedPriority: TransactionPriority = TransactionPriority::max_value() / 2;
}
impl pallet_im_online::Trait for Runtime {
	type AuthorityId = ImOnlineId;
	type Event = Event;
	type SessionDuration = SessionDuration;
	type ReportUnresponsiveness = Offences;
	type UnsignedPriority = ImOnlineUnsignedPriority;
	type WeightInfo = ();
}

parameter_types! {
	pub WindowSize: BlockNumber = pallet_finality_tracker::DEFAULT_WINDOW_SIZE.into();
	pub ReportLatency: BlockNumber = pallet_finality_tracker::DEFAULT_REPORT_LATENCY.into();
}
impl pallet_finality_tracker::Trait for Runtime {
	type OnFinalizationStalled = ();
	type WindowSize = WindowSize;
	type ReportLatency = ReportLatency;
}

parameter_types! {
	pub OffencesWeightSoftLimit: Weight = Perbill::from_percent(60) * MaximumBlockWeight::get();
}
impl pallet_offences::Trait for Runtime {
	type Event = Event;
	type IdentificationTuple = pallet_session::historical::IdentificationTuple<Self>;
	type OnOffenceHandler = Staking;
	type WeightSoftLimit = OffencesWeightSoftLimit;
}

impl pallet_session::historical::Trait for Runtime {
	type FullIdentification = crml_staking::Exposure<AccountId, Balance>;
	type FullIdentificationOf = crml_staking::ExposureOf<Self>;
}

parameter_types! {
	// Minimum 4 CENTS/byte
	pub const BasicDeposit: Balance = deposit(1, 258);
	pub const FieldDeposit: Balance = deposit(0, 66);
	pub const SubAccountDeposit: Balance = deposit(1, 53);
	pub const MaxSubAccounts: u32 = 100;
	pub const MaxAdditionalFields: u32 = 100;
	pub const MaxRegistrars: u32 = 20;
}
impl pallet_identity::Trait for Runtime {
	type Event = Event;
	type Currency = SpendingAssetCurrency<Self>;
	type BasicDeposit = BasicDeposit;
	type FieldDeposit = FieldDeposit;
	type SubAccountDeposit = SubAccountDeposit;
	type MaxSubAccounts = MaxSubAccounts;
	type MaxAdditionalFields = MaxAdditionalFields;
	type MaxRegistrars = MaxRegistrars;
	type Slashed = ();
	type ForceOrigin = EnsureRoot<AccountId>;
	type RegistrarOrigin = EnsureRoot<AccountId>;
	type WeightInfo = ();
}

parameter_types! {
	pub const ProposalBond: Permill = Permill::from_percent(5);
	pub const ProposalBondMinimum: Balance = 1 * DOLLARS;
	pub const SpendPeriod: BlockNumber = 1 * DAYS;
	pub const Burn: Permill = Permill::from_percent(50);
	pub const TipCountdown: BlockNumber = 1 * DAYS;
	pub const TipFindersFee: Percent = Percent::from_percent(20);
	pub const TipReportDepositBase: Balance = 1 * DOLLARS;
	pub const DataDepositPerByte: Balance = 1 * MICROS;
	pub const BountyDepositBase: Balance = 1 * DOLLARS;
	pub const BountyDepositPayoutDelay: BlockNumber = 1 * DAYS;
	pub const TreasuryModuleId: ModuleId = ModuleId(*b"py/trsry");
	pub const BountyUpdatePeriod: BlockNumber = 14 * DAYS;
	pub const MaximumReasonLength: u32 = 16384;
	pub const BountyCuratorDeposit: Permill = Permill::from_percent(50);
	pub const BountyValueMinimum: Balance = 5 * DOLLARS;
}
impl pallet_treasury::Trait for Runtime {
	type ModuleId = TreasuryModuleId;
	type Currency = SpendingAssetCurrency<Self>;
	// root only is sufficient for launch phase
	type ApproveOrigin = EnsureRoot<AccountId>;
	type RejectOrigin = EnsureRoot<AccountId>;
	type Tippers = RootMemberOnly<Self>;
	type TipCountdown = TipCountdown;
	type TipFindersFee = TipFindersFee;
	type TipReportDepositBase = TipReportDepositBase;
	type DataDepositPerByte = DataDepositPerByte;
	type Event = Event;
	type OnSlash = ();
	type ProposalBond = ProposalBond;
	type ProposalBondMinimum = ProposalBondMinimum;
	type SpendPeriod = SpendPeriod;
	type Burn = Burn;
	type BountyDepositBase = BountyDepositBase;
	type BountyDepositPayoutDelay = BountyDepositPayoutDelay;
	type BountyUpdatePeriod = BountyUpdatePeriod;
	type BountyCuratorDeposit = BountyCuratorDeposit;
	type BountyValueMinimum = BountyValueMinimum;
	type MaximumReasonLength = MaximumReasonLength;
	type BurnDestination = ();
	type WeightInfo = ();
}

parameter_types! {
	pub const HistoricalPayoutEras: u16 = 7;
	pub const PayoutSplitThreshold: u32 = 1000;
	pub const FiscalEraLength: u32 = 365;
}
impl crml_staking_rewards::Trait for Runtime {
	type CurrencyToReward = SpendingAssetCurrency<Self>;
	type Event = Event;
	type HistoricalPayoutEras = HistoricalPayoutEras;
	type TreasuryModuleId = TreasuryModuleId;
	type PayoutSplitThreshold = PayoutSplitThreshold;
	type FiscalEraLength = FiscalEraLength;
	type WeightInfo = ();
}

impl crml_sylo::e2ee::Trait for Runtime {
	type WeightInfo = ();
}
impl crml_sylo::payment::Trait for Runtime {
	type WeightInfo = ();
}
impl crml_sylo::device::Trait for Runtime {}
impl crml_sylo::inbox::Trait for Runtime {
	type WeightInfo = ();
}
impl crml_sylo::response::Trait for Runtime {
	type WeightInfo = ();
}
impl crml_sylo::vault::Trait for Runtime {
	type WeightInfo = ();
}
impl crml_sylo::groups::Trait for Runtime {
	type WeightInfo = ();
}

impl crml_cennzx::Trait for Runtime {
	type AssetId = AssetId;
	type Event = Event;
	type MultiCurrency = GenericAsset;
	type ExchangeAddressFor = ExchangeAddressGenerator<Self>;
	type WeightInfo = weights::crml_cennzx::WeightInfo;
}

impl prml_attestation::Trait for Runtime {
	type Event = Event;
	type WeightInfo = ();
}

/// Submits a transaction with the node's public and signature type. Adheres to the signed extension
/// format of the chain.
impl<LocalCall> frame_system::offchain::CreateSignedTransaction<LocalCall> for Runtime
where
	Call: From<LocalCall>,
{
	fn create_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
		call: Call,
		public: <Signature as Verify>::Signer,
		account: AccountId,
		nonce: Index,
	) -> Option<(Call, <UncheckedExtrinsic as Extrinsic>::SignaturePayload)> {
		let tip = 0;
		// take the biggest period possible.
		let period = BlockHashCount::get()
			.checked_next_power_of_two()
			.map(|c| c / 2)
			.unwrap_or(2) as u64;
		let current_block = System::block_number()
			.saturated_into::<u64>()
			// The `System::block_number` is initialized with `n+1`,
			// so the actual block number is `n`.
			.saturating_sub(1);
		let era = Era::mortal(period, current_block);
		let extra = (
			frame_system::CheckSpecVersion::<Runtime>::new(),
			frame_system::CheckTxVersion::<Runtime>::new(),
			frame_system::CheckGenesis::<Runtime>::new(),
			frame_system::CheckEra::<Runtime>::from(era),
			frame_system::CheckNonce::<Runtime>::from(nonce),
			frame_system::CheckWeight::<Runtime>::new(),
			crml_transaction_payment::ChargeTransactionPayment::<Runtime>::from(tip, None),
		);
		let raw_payload = SignedPayload::new(call, extra)
			.map_err(|e| {
				debug::warn!("Unable to create signed payload: {:?}", e);
			})
			.ok()?;
		let signature = raw_payload.using_encoded(|payload| C::sign(payload, public))?;
		let (call, extra, _) = raw_payload.deconstruct();
		Some((call, (account, signature.into(), extra)))
	}
}

impl frame_system::offchain::SigningTypes for Runtime {
	type Public = <Signature as Verify>::Signer;
	type Signature = Signature;
}

impl<C> frame_system::offchain::SendTransactionTypes<C> for Runtime
where
	Call: From<C>,
{
	type Extrinsic = UncheckedExtrinsic;
	type OverarchingCall = Call;
}

construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = opaque::Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		// Give modules fixed indexes in the runtime
		System: frame_system::{Module, Call, Storage, Config, Event<T>} = 0,
		Scheduler: pallet_scheduler::{Module, Call, Storage, Event<T>} = 1,
		Babe: pallet_babe::{Module, Call, Storage, Config, Inherent, ValidateUnsigned} = 2,
		Timestamp: pallet_timestamp::{Module, Call, Storage, Inherent} = 3,
		GenericAsset: prml_generic_asset::{Module, Call, Storage, Event<T>, Config<T>} = 4,
		Authorship: pallet_authorship::{Module, Call, Storage} = 5,
		Staking: crml_staking::{Module, Call, Storage, Config<T>, Event<T>} = 6,
		Offences: pallet_offences::{Module, Call, Storage, Event} = 7,
		Session: pallet_session::{Module, Call, Storage, Event, Config<T>} = 8,
		FinalityTracker: pallet_finality_tracker::{Module, Call, Storage, Inherent} = 9,
		Grandpa: pallet_grandpa::{Module, Call, Storage, Config, Event, ValidateUnsigned} = 10,
		ImOnline: pallet_im_online::{Module, Call, Storage, Event<T>, ValidateUnsigned, Config<T>} = 11,
		AuthorityDiscovery: pallet_authority_discovery::{Module, Call, Config} = 12,
		// Governance Modules (Sudo initially)
		Sudo: pallet_sudo::{Module, Call, Config<T>, Storage, Event<T>} = 13,
		// Democracy: pallet_democracy::{Module, Call, Storage, Config, Event<T>}
		// Council: pallet_collective::<Instance1>::{Module, Call, Storage, Origin<T>, Event<T>, Config<T>}
		// TechnicalCommittee: pallet_collective::<Instance2>::{Module, Call, Storage, Origin<T>, Event<T>, Config<T>}
		// TechnicalMembership: pallet_membership::<Instance1>::{Module, Call, Storage, Event<T>, Config<T>}
		Treasury: pallet_treasury::{Module, Call, Storage, Event<T>} = 14,
		Utility: pallet_utility::{Module, Call, Event} = 15,
		Identity: pallet_identity::{Module, Call, Storage, Event<T>} = 16,
		TransactionPayment: crml_transaction_payment::{Module, Storage} = 17,
		Multisig: pallet_multisig::{Module, Call, Storage, Event<T>} = 18,
		RandomnessCollectiveFlip: pallet_randomness_collective_flip::{Module, Storage} = 19,
		Historical: session_historical::{Module} = 20,
		Cennzx: crml_cennzx::{Module, Call, Storage, Config<T>, Event<T>} = 21,
		// TODO: these should all be in one module
		SyloGroups: sylo_groups::{Module, Call, Storage} = 22,
		SyloE2EE: sylo_e2ee::{Module, Call, Storage} = 23,
		SyloDevice: sylo_device::{Module, Call, Storage} = 24,
		SyloInbox: sylo_inbox::{Module, Call, Storage} = 25,
		SyloResponse: sylo_response::{Module, Call, Storage} = 26,
		SyloVault: sylo_vault::{Module, Call, Storage} = 27,
		SyloPayment: sylo_payment::{Module, Call, Storage} = 28,
		Attestation: prml_attestation::{Module, Call, Storage, Event<T>} = 29,
		Rewards: crml_staking_rewards::{Module, Call, Storage, Config, Event<T>} = 30,
		Nft: crml_nft::{Module, Call, Storage, Event<T>} = 31,
	}
);

/// The address format for describing accounts.
pub type Address = AccountId;
/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;
/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;
/// The SignedExtension to the basic transaction logic.
pub type SignedExtra = (
	frame_system::CheckSpecVersion<Runtime>,
	frame_system::CheckTxVersion<Runtime>,
	frame_system::CheckGenesis<Runtime>,
	frame_system::CheckEra<Runtime>,
	frame_system::CheckNonce<Runtime>,
	frame_system::CheckWeight<Runtime>,
	crml_transaction_payment::ChargeTransactionPayment<Runtime>,
);

/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic = generic::UncheckedExtrinsic<Address, Call, Signature, SignedExtra>;
/// The payload being signed in transactions.
pub type SignedPayload = generic::SignedPayload<Call, SignedExtra>;
/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, Call, SignedExtra>;
/// Executive: handles dispatch to the various modules.
pub type Executive =
	frame_executive::Executive<Runtime, Block, frame_system::ChainContext<Runtime>, Runtime, AllModules>;

impl_runtime_apis! {
	impl sp_api::Core<Block> for Runtime {
		fn version() -> RuntimeVersion {
			VERSION
		}

		fn execute_block(block: Block) {
			Executive::execute_block(block)
		}

		fn initialize_block(header: &<Block as BlockT>::Header) {
			Executive::initialize_block(header)
		}
	}

	impl sp_api::Metadata<Block> for Runtime {
		fn metadata() -> OpaqueMetadata {
			Runtime::metadata().into()
		}
	}

	impl sp_block_builder::BlockBuilder<Block> for Runtime {
		fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
			Executive::apply_extrinsic(extrinsic)
		}

		fn finalize_block() -> <Block as BlockT>::Header {
			Executive::finalize_block()
		}

		fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
			data.create_extrinsics()
		}

		fn check_inherents(
			block: Block,
			data: sp_inherents::InherentData,
		) -> sp_inherents::CheckInherentsResult {
			data.check_extrinsics(&block)
		}

		fn random_seed() -> <Block as BlockT>::Hash {
			RandomnessCollectiveFlip::random_seed()
		}
	}

	impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
		fn validate_transaction(
			source: TransactionSource,
			tx: <Block as BlockT>::Extrinsic,
		) -> TransactionValidity {
			Executive::validate_transaction(source, tx)
		}
	}

	impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
		fn offchain_worker(header: &<Block as BlockT>::Header) {
			Executive::offchain_worker(header)
		}
	}

	impl sp_authority_discovery::AuthorityDiscoveryApi<Block> for Runtime {
		fn authorities() -> Vec<AuthorityDiscoveryId> {
			AuthorityDiscovery::authorities()
		}
	}

	impl sp_consensus_babe::BabeApi<Block> for Runtime {
		fn configuration() -> sp_consensus_babe::BabeGenesisConfiguration {
			// The choice of `c` parameter (where `1 - c` represents the
			// probability of a slot being empty), is done in accordance to the
			// slot duration and expected target block time, for safely
			// resisting network delays of maximum two seconds.
			// <https://research.web3.foundation/en/latest/polkadot/BABE/Babe/#6-practical-results>
			sp_consensus_babe::BabeGenesisConfiguration {
				slot_duration: Babe::slot_duration(),
				epoch_length: EpochDuration::get(),
				c: PRIMARY_PROBABILITY,
				genesis_authorities: Babe::authorities(),
				randomness: Babe::randomness(),
				allowed_slots: sp_consensus_babe::AllowedSlots::PrimaryAndSecondaryPlainSlots,
			}
		}

		fn current_epoch_start() -> sp_consensus_babe::SlotNumber {
			Babe::current_epoch_start()
		}

		fn generate_key_ownership_proof(
			_slot_number: sp_consensus_babe::SlotNumber,
			authority_id: sp_consensus_babe::AuthorityId,
		) -> Option<sp_consensus_babe::OpaqueKeyOwnershipProof> {
			use codec::Encode;

			Historical::prove((sp_consensus_babe::KEY_TYPE, authority_id))
				.map(|p| p.encode())
				.map(sp_consensus_babe::OpaqueKeyOwnershipProof::new)
		}

		fn submit_report_equivocation_unsigned_extrinsic(
			equivocation_proof: sp_consensus_babe::EquivocationProof<<Block as BlockT>::Header>,
			key_owner_proof: sp_consensus_babe::OpaqueKeyOwnershipProof,
		) -> Option<()> {
			let key_owner_proof = key_owner_proof.decode()?;

			Babe::submit_unsigned_equivocation_report(
				equivocation_proof,
				key_owner_proof,
			)
		}
	}

	impl sp_session::SessionKeys<Block> for Runtime {
		fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
			opaque::SessionKeys::generate(seed)
		}

		fn decode_session_keys(
			encoded: Vec<u8>,
		) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
			opaque::SessionKeys::decode_into_raw_public_keys(&encoded)
		}
	}

	impl fg_primitives::GrandpaApi<Block> for Runtime {
		fn grandpa_authorities() -> GrandpaAuthorityList {
			Grandpa::grandpa_authorities()
		}

		fn submit_report_equivocation_unsigned_extrinsic(
			_equivocation_proof: fg_primitives::EquivocationProof<
				<Block as BlockT>::Hash,
				NumberFor<Block>,
			>,
			_key_owner_proof: fg_primitives::OpaqueKeyOwnershipProof,
		) -> Option<()> {
			None
		}

		fn generate_key_ownership_proof(
			_set_id: fg_primitives::SetId,
			_authority_id: GrandpaId,
		) -> Option<fg_primitives::OpaqueKeyOwnershipProof> {
			// NOTE: this is the only implementation possible since we've
			// defined our key owner proof type as a bottom type (i.e. a type
			// with no values).
			None
		}
	}

	impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index> for Runtime {
		fn account_nonce(account: AccountId) -> Index {
			System::account_nonce(account)
		}
	}

	impl prml_generic_asset_rpc_runtime_api::AssetMetaApi<Block, AssetId> for Runtime {
		fn asset_meta() -> Vec<(AssetId, AssetInfo)> {
			GenericAsset::registered_assets()
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance> for Runtime {
		fn query_info(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_info(uxt, len)
		}
	}


	impl crml_cennzx_rpc_runtime_api::CennzxApi<
		Block,
		AssetId,
		Balance,
		AccountId,
	> for Runtime {
		fn buy_price(
			buy_asset: AssetId,
			buy_amount: Balance,
			sell_asset: AssetId,
		) -> CennzxResult<Balance> {
			let result = Cennzx::get_buy_price(buy_asset, buy_amount, sell_asset);
			match result {
				Ok(value) => CennzxResult::Success(value),
				Err(_) => CennzxResult::Error,
			}
		}

		fn sell_price(
			sell_asset: AssetId,
			sell_amount: Balance,
			buy_asset: AssetId,
		) -> CennzxResult<Balance> {
			let result = Cennzx::get_sell_price(sell_asset, sell_amount, buy_asset);
			match result {
				Ok(value) => CennzxResult::Success(value),
				Err(_) => CennzxResult::Error,
			}
		}

		fn liquidity_value(
			account: AccountId,
			asset_id: AssetId,
		) -> (Balance, Balance, Balance) {
			let value = Cennzx::account_liquidity_value(&account, asset_id);
			(value.liquidity, value.core, value.asset)
		}

		fn liquidity_price(
			asset_id: AssetId,
			liquidity_to_buy: Balance
		) -> (Balance, Balance) {
			let value = Cennzx::liquidity_price(asset_id, liquidity_to_buy);
			(value.core, value.asset)
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl frame_benchmarking::Benchmark<Block> for Runtime {
		fn dispatch_benchmark(
			config: frame_benchmarking::BenchmarkConfig
		) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
			use frame_benchmarking::{Benchmarking, BenchmarkBatch, add_benchmark, TrackedStorageKey};

			let whitelist: Vec<TrackedStorageKey> = vec![
				// Block Number
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef702a5c1b19ab7a04f536c519aca4983ac").to_vec().into(),
				// Total Issuance
				hex_literal::hex!("c2261276cc9d1f8598ea4b6a74b15c2f57c875e4cff74148e4628f264b974c80").to_vec().into(),
				// Execution Phase
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef7ff553b5a9862a516939d82b3d3d8661a").to_vec().into(),
				// Event Count
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef70a98fdbe9ce6c55837576c60c7af3850").to_vec().into(),
				// System Events
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7").to_vec().into(),
				// Treasury Account
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef7b99d880ec681799c0cf30e8886371da95ecffd7b6c0f78751baa9d281e0bfa3a6d6f646c70792f74727372790000000000000000000000000000000000000000").to_vec().into(),
			];

			let mut batches = Vec::<BenchmarkBatch>::new();
			let params = (&config, &whitelist);

			add_benchmark!(params, batches, crml_cennzx, Cennzx);
			add_benchmark!(params, batches, sylo_payment, SyloPayment);
			add_benchmark!(params, batches, sylo_response, SyloResponse);
			add_benchmark!(params, batches, sylo_inbox, SyloInbox);
			add_benchmark!(params, batches, sylo_vault, SyloVault);
			add_benchmark!(params, batches, sylo_e2ee, SyloE2EE);
			add_benchmark!(params, batches, sylo_groups, SyloGroups);
			add_benchmark!(params, batches, crml_staking_rewards, Rewards);

			if batches.is_empty() { return Err("Benchmark not found for this pallet.".into()) }
			Ok(batches)
		}
	}
}

/// Opaque types. These are used by the CLI to instantiate machinery that don't need to know
/// the specifics of the runtime. They can then be made to be agnostic over specific formats
/// of data like extrinsics, allowing for them to continue syncing the network through upgrades
/// to even the core data structures.
pub mod opaque {
	use super::*;

	pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;

	/// Opaque block header type.
	pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
	/// Opaque block type.
	pub type Block = generic::Block<Header, UncheckedExtrinsic>;
	/// Opaque block identifier type.
	pub type BlockId = generic::BlockId<Block>;

	impl_opaque_keys! {
		pub struct SessionKeys {
			pub grandpa: Grandpa,
			pub babe: Babe,
			pub im_online: ImOnline,
			pub authority_discovery: AuthorityDiscovery,
		}
	}
}
