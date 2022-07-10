// Copyright 2018-2021 Parity Technologies (UK) Ltd. and Centrality Investments Ltd.
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

use codec::{Decode, Encode};
use crml_generic_asset_rpc_runtime_api;
use pallet_authority_discovery;
use pallet_grandpa::{fg_primitives, AuthorityId as GrandpaId, AuthorityList as GrandpaAuthorityList};
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use pallet_session::{self, historical as session_historical};
use sp_api::impl_runtime_apis;
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_babe;
use sp_core::{crypto::KeyTypeId, OpaqueMetadata};
use sp_runtime::{
	create_runtime_str,
	generic::{self, Era},
	impl_opaque_keys,
	traits::{
		BlakeTwo256, Block as BlockT, Dispatchable, Extrinsic, IdentityLookup, NumberFor, OpaqueKeys,
		PostDispatchInfoOf, SaturatedConversion, Verify,
	},
	transaction_validity::{InvalidTransaction, TransactionSource, TransactionValidity, TransactionValidityError},
	ApplyExtrinsicResult, FixedPointNumber,
};
use sp_std::prelude::*;

#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;
use static_assertions::const_assert;

use crml_staking::rewards as crml_staking_rewards;
pub use crml_staking::StakerStatus;
use crml_support::{log, PrefixedAddressMapping, H160, H256, U256};
pub use frame_support::{
	construct_runtime,
	dispatch::GetDispatchInfo,
	ord_parameter_types, parameter_types,
	traits::{Currency, FindAuthor, Imbalance, KeyOwnerProofSystem, OnUnbalanced, Randomness, U128CurrencyToVote},
	weights::{
		constants::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight, WEIGHT_PER_SECOND},
		DispatchClass, IdentityFee, TransactionPriority, Weight,
	},
	PalletId, StorageValue,
};
use frame_system::{
	limits::{BlockLength, BlockWeights},
	EnsureRoot,
};
pub use pallet_timestamp::Call as TimestampCall;
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;
pub use sp_runtime::{MultiAddress, Perbill, Percent, Permill, Perquintill};

// CENNZnet only imports
use cennznet_primitives::{
	eth::crypto::AuthorityId as EthBridgeId,
	types::{
		AccountId, AssetId, Balance, BlockNumber, CollectionId, Hash, Header, Index, Moment, SerialNumber, SeriesId,
		Signature, TokenId,
	},
};
pub use crml_cennzx::{ExchangeAddressGenerator, FeeRate, PerMillion, PerThousand};
use crml_cennzx_rpc_runtime_api::CennzxResult;
pub use crml_generic_asset::{
	impls::TransferDustImbalance, AllBalances, AssetInfo, Call as GenericAssetCall, SpendingAssetCurrency,
	StakingAssetCurrency,
};
use crml_governance::{ProposalId, ProposalVoteInfo};
use crml_nft::{CollectionInfo, Listing, ListingId, TokenInfo};
use crml_transaction_payment::{FeeDetails, RuntimeDispatchInfo};
pub use crml_transaction_payment::{Multiplier, TargetedFeeAdjustment};
use fp_rpc::TransactionStatus;
use pallet_ethereum::{Call::transact, Transaction as EthereumTransaction};
use pallet_evm::{Account as EVMAccount, EnsureAddressTruncated, EvmConfig, FeeCalculator, Runner};

/// Constant values used within the runtime.
pub mod constants;
use constants::{currency::*, evm::*, time::*};

// Implementations of some helper traits passed into runtime modules as associated types.
pub mod impls;
use impls::{
	CENNZnetOnChargeEVMTransaction, DealWithFees, EthereumFindAuthor, EvmCurrencyScaler, ScheduledPayoutRunner,
	SlashFundsToTreasury, WeightToCpayFee,
};

pub mod precompiles;
use precompiles::{CENNZnetPrecompiles, StateOracleCallbackExecutor};

pub mod runner;
use runner::FeePreferencesRunner;

/// Deprecated host functions required for syncing blocks prior to 2.0 upgrade
pub mod legacy_host_functions;

/// Logging target for runtime level components
pub(crate) const LOG_TARGET: &str = "runtime";

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

/// Wasm binary unwrapped. If built with `SKIP_WASM_BUILD`, the function panics.
#[cfg(feature = "std")]
pub fn wasm_binary_unwrap() -> &'static [u8] {
	WASM_BINARY.expect(
		"Development wasm binary is not available. This means the client is \
						built with `SKIP_WASM_BUILD` flag and it is only usable for \
						production chains. Please rebuild with the flag disabled.",
	)
}

/// Runtime version.
#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: create_runtime_str!("cennznet"),
	impl_name: create_runtime_str!("cennznet-node"),
	authoring_version: 1,
	// Per convention: if the runtime behavior changes, increment `spec_version`
	// and set `impl_version` to equal spec_version. If only runtime
	// implementation changes and behavior does not, then leave `spec_version` as
	// is and increment `impl_version`.
	spec_version: 62,
	impl_version: 0,
	apis: RUNTIME_API_VERSIONS,
	transaction_version: 5,
};

/// The BABE epoch configuration at genesis.
pub const BABE_GENESIS_EPOCH_CONFIG: sp_consensus_babe::BabeEpochConfiguration =
	sp_consensus_babe::BabeEpochConfiguration {
		c: PRIMARY_PROBABILITY,
		allowed_slots: sp_consensus_babe::AllowedSlots::PrimaryAndSecondaryPlainSlots,
	};

/// Native version.
#[cfg(any(feature = "std", test))]
pub fn native_version() -> NativeVersion {
	NativeVersion {
		runtime_version: VERSION,
		can_author_with: Default::default(),
	}
}

/// Prevent state oracle transactions from executing
pub struct StateOracleCallFilter;
impl frame_support::traits::Contains<Call> for StateOracleCallFilter {
	fn contains(call: &Call) -> bool {
		match call {
			Call::EthStateOracle(_) => StateOracleIsActive::get(),
			_ => true,
		}
	}
}

// Configure modules to include in the runtime.

/// We assume that ~10% of the block weight is consumed by `on_initialize` handlers.
/// This is used to limit the maximal weight of a single extrinsic.
const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(10);
/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be used
/// by  Operational  extrinsics.
const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);
/// We allow for 2 seconds of compute with a 5 second average block time.
const MAXIMUM_BLOCK_WEIGHT: Weight = 2 * WEIGHT_PER_SECOND;

parameter_types! {
	pub const BlockHashCount: BlockNumber = 2400;
	pub const Version: RuntimeVersion = VERSION;
	pub RuntimeBlockLength: BlockLength =
		BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
	pub RuntimeBlockWeights: BlockWeights = BlockWeights::builder()
		.base_block(BlockExecutionWeight::get())
		.for_class(DispatchClass::all(), |weights| {
			weights.base_extrinsic = ExtrinsicBaseWeight::get();
		})
		.for_class(DispatchClass::Normal, |weights| {
			weights.max_total = Some(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT);
		})
		.for_class(DispatchClass::Operational, |weights| {
			weights.max_total = Some(MAXIMUM_BLOCK_WEIGHT);
			// Operational transactions have some extra reserved space, so that they
			// are included even if block reached `MAXIMUM_BLOCK_WEIGHT`.
			weights.reserved = Some(
				MAXIMUM_BLOCK_WEIGHT - NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT
			);
		})
		.avg_block_initialization(AVERAGE_ON_INITIALIZE_RATIO)
		.build_or_panic();
	pub const SS58Prefix: u8 = 42;
}

const_assert!(NORMAL_DISPATCH_RATIO.deconstruct() >= AVERAGE_ON_INITIALIZE_RATIO.deconstruct());

impl frame_system::Config for Runtime {
	/// The basic call filter to use in dispatchable.
	type BaseCallFilter = StateOracleCallFilter;
	/// Block & extrinsics weights: base values and limits.
	type BlockWeights = RuntimeBlockWeights;
	/// The maximum length of a block (in bytes).
	type BlockLength = RuntimeBlockLength;
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
	/// The weight of database operations that the runtime can invoke.
	type DbWeight = RocksDbWeight;
	/// Version of the runtime.
	type Version = Version;
	/// Converts a module to the index of the module in `construct_runtime!`.
	type PalletInfo = PalletInfo;
	/// What to do if a new account is created.
	type OnNewAccount = ();
	/// What to do if an account is fully reaped from the system.
	type OnKilledAccount = ();
	/// What to do if an NFT is transferred
	/// The data to be stored in an account.
	type AccountData = ();
	/// Weight information for the extrinsics of this pallet.
	type SystemWeightInfo = frame_system::weights::SubstrateWeight<Runtime>;
	/// This is used as an identifier of the chain. 42 is the generic substrate prefix.
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
}

parameter_types! {
	/// How long listings are open for by default
	pub const DefaultListingDuration: BlockNumber = DAYS * 3;
	/// The maximum length of an attribute value (140 = old tweet limit)
	/// Only applies to string/vec allocated types
	pub const MaxAttributeLength: u8 = 140;
}
impl crml_nft::Config for Runtime {
	type Event = Event;
	type MultiCurrency = GenericAsset;
	type MaxAttributeLength = MaxAttributeLength;
	type DefaultListingDuration = DefaultListingDuration;
	type WeightInfo = ();
	type OnTransferSubscription = TokenApprovals;
}

parameter_types! {
	pub const UncleGenerations: BlockNumber = 5;
}
impl pallet_authorship::Config for Runtime {
	type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Babe>;
	type UncleGenerations = UncleGenerations;
	type FilterUncle = ();
	type EventHandler = (Rewards, ImOnline);
}

parameter_types! {
	pub const EpochDuration: u64 = EPOCH_DURATION_IN_SLOTS;
	pub const ExpectedBlockTime: Moment = MILLISECS_PER_BLOCK;
	pub const ReportLongevity: u64 =
		BondingDuration::get() as u64 * SessionsPerEra::get() as u64 * EpochDuration::get();
}
impl pallet_babe::Config for Runtime {
	type EpochDuration = EpochDuration;
	type ExpectedBlockTime = ExpectedBlockTime;
	type EpochChangeTrigger = pallet_babe::ExternalTrigger;
	type DisabledValidators = Session;
	type KeyOwnerProofSystem = Historical;
	type KeyOwnerProof =
		<Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(KeyTypeId, pallet_babe::AuthorityId)>>::Proof;
	type KeyOwnerIdentification =
		<Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(KeyTypeId, pallet_babe::AuthorityId)>>::IdentificationTuple;
	type HandleEquivocation = pallet_babe::EquivocationHandler<Self::KeyOwnerIdentification, Offences, ReportLongevity>;
	type WeightInfo = ();
	type MaxAuthorities = MaxAuthorities;
}

impl pallet_grandpa::Config for Runtime {
	type Event = Event;
	type Call = Call;
	type KeyOwnerProofSystem = Historical;
	type KeyOwnerProof = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(KeyTypeId, GrandpaId)>>::Proof;
	type KeyOwnerIdentification =
		<Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(KeyTypeId, GrandpaId)>>::IdentificationTuple;
	type HandleEquivocation =
		pallet_grandpa::EquivocationHandler<Self::KeyOwnerIdentification, Offences, ReportLongevity>;
	type WeightInfo = ();
	type MaxAuthorities = MaxAuthorities;
}

parameter_types! {
	pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) *
		RuntimeBlockWeights::get().max_block;
	pub const MaxScheduledPerBlock: u32 = 50;
}
impl pallet_scheduler::Config for Runtime {
	type Event = Event;
	type Origin = Origin;
	type PalletsOrigin = OriginCaller;
	type Call = Call;
	type MaximumWeight = MaximumSchedulerWeight;
	type ScheduleOrigin = EnsureRoot<AccountId>;
	type MaxScheduledPerBlock = MaxScheduledPerBlock;
	type OriginPrivilegeCmp = frame_support::traits::EqualPrivilegeOnly;
	type WeightInfo = ();
}

parameter_types! {
	pub const SessionsPerEra: sp_staking::SessionIndex = SESSIONS_PER_ERA;
	// 28 eras/days for bond to be withdraw
	pub const BondingDuration: crml_staking::EraIndex = 28;
	// 27 eras/days for a slash to be deferrable
	pub const SlashDeferDuration: crml_staking::EraIndex = 27;
	/// the highest n stakers that will receive rewards only
	pub const MaxNominatorRewardedPerValidator: u32 = 128;
	// Allow election solution computation during the entire last session (~10 minutes)
	pub const ElectionLookahead: BlockNumber = EPOCH_DURATION_IN_BLOCKS;
	// maximum phragmen iterations
	pub const MaxIterations: u32 = 10;
	pub MinSolutionScoreBump: Perbill = Perbill::from_rational(5u32, 10_000);
	pub OffchainSolutionWeightLimit: Weight = RuntimeBlockWeights::get()
		.get(DispatchClass::Normal)
		.max_extrinsic.expect("Normal extrinsics have a weight limit configured; qed")
		.saturating_sub(BlockExecutionWeight::get());
	pub const OffendingValidatorsThreshold: Perbill = Perbill::from_percent(17);
	pub OffencesWeightSoftLimit: Weight = Perbill::from_percent(60) *
		RuntimeBlockWeights::get().max_block;
}
impl crml_staking::Config for Runtime {
	type BondingDuration = BondingDuration;
	type Call = Call;
	type Currency = StakingAssetCurrency<Self>;
	type CurrencyToVote = U128CurrencyToVote;
	type Event = Event;
	type ElectionLookahead = ElectionLookahead;
	type MaxIterations = MaxIterations;
	type MaxNominatorRewardedPerValidator = MaxNominatorRewardedPerValidator;
	type MinSolutionScoreBump = MinSolutionScoreBump;
	type NextNewSession = Session;
	type OffchainSolutionWeightLimit = OffchainSolutionWeightLimit;
	type SessionInterface = Self;
	type SessionsPerEra = SessionsPerEra;
	type Slash = SlashFundsToTreasury; // send the slashed funds in CENNZ to the treasury.
	type SlashDeferDuration = SlashDeferDuration;
	type Rewarder = Rewards;
	type UnixTime = Timestamp;
	type UnsignedPriority = StakingUnsignedPriority;
	type OffendingValidatorsThreshold = OffendingValidatorsThreshold;
	type WeightInfo = ();
	// provides deffered offence reporting see (https://github.com/cennznet/cennznet/pull/590)
	type OnOffenceHandler = Staking;
	type SessionHistoricalType = Self;
	type WeightSoftLimit = OffencesWeightSoftLimit;
}

impl_opaque_keys! {
	pub struct SessionKeys {
		pub grandpa: Grandpa,
		pub babe: Babe,
		pub im_online: ImOnline,
		pub authority_discovery: AuthorityDiscovery,
		pub eth_bridge: EthBridge,
	}
}

parameter_types! {
	pub const MaxAuthorities: u32 = 100_000;
}
impl pallet_session::Config for Runtime {
	type Event = Event;
	type ValidatorId = <Self as frame_system::Config>::AccountId;
	type ValidatorIdOf = crml_staking::StashOf<Self>;
	type ShouldEndSession = Babe;
	type NextSessionRotation = Babe;
	type SessionManager = pallet_session::historical::NoteHistoricalRoot<Self, Staking>;
	type SessionHandler = <SessionKeys as OpaqueKeys>::KeyTypeIdProviders;
	type Keys = SessionKeys;
	type WeightInfo = ();
}

parameter_types! {
	pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
}
impl pallet_timestamp::Config for Runtime {
	/// A timestamp: milliseconds since the unix epoch.
	type Moment = u64;
	type OnTimestampSet = Babe;
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
}

impl crml_generic_asset::Config for Runtime {
	type AssetId = AssetId;
	type Balance = Balance;
	type Event = Event;
	type OnDustImbalance = TransferDustImbalance<TreasuryPalletId>;
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
impl crml_transaction_payment::Config for Runtime {
	type AssetId = AssetId;
	type OnChargeTransaction = crml_transaction_payment::CurrencyAdapter<SpendingAssetCurrency<Runtime>, DealWithFees>;
	type TransactionByteFee = TransactionByteFee;
	type WeightToFee = WeightToCpayFee<WeightToCpayFactor>;
	type FeeMultiplierUpdate = TargetedFeeAdjustment<Self, TargetBlockFullness, AdjustmentVariable, MinimumMultiplier>;
	type BuyFeeAsset = Cennzx;
}

pub const fn deposit(items: u32, bytes: u32) -> Balance {
	items as Balance * 15 + (bytes as Balance) * 6
}

impl pallet_sudo::Config for Runtime {
	type Event = Event;
	type Call = Call;
}

parameter_types! {
	/// Max. members of the council
	pub const MaxCouncilSize: u16 = 255;
	pub storage MinimumRegisteredIdentities: u32 = 2;
}
impl crml_governance::Config for Runtime {
	type Call = Call;
	type Currency = SpendingAssetCurrency<Self>;
	type MinimumRegisteredIdentities = MinimumRegisteredIdentities;
	type MaxCouncilSize = MaxCouncilSize;
	type Scheduler = Scheduler;
	type PalletsOrigin = OriginCaller;
	type Event = Event;
	type WeightInfo = ();
	type Registration = impls::RegistrationImplementation<Self>;
	type StakingAmount = Staking;
}

impl pallet_utility::Config for Runtime {
	type Event = Event;
	type Call = Call;
	type PalletsOrigin = OriginCaller;
	type WeightInfo = ();
}

impl pallet_authority_discovery::Config for Runtime {
	type MaxAuthorities = MaxAuthorities;
}

impl frame_system::offchain::SigningTypes for Runtime {
	type Public = <Signature as sp_runtime::traits::Verify>::Signer;
	type Signature = Signature;
}

impl<C> frame_system::offchain::SendTransactionTypes<C> for Runtime
where
	Call: From<C>,
{
	type Extrinsic = UncheckedExtrinsic;
	type OverarchingCall = Call;
}

parameter_types! {
	pub const SessionDuration: BlockNumber = EPOCH_DURATION_IN_BLOCKS as _;
	/// We prioritize im-online heartbeats over election solution submission.
	pub StakingUnsignedPriority: TransactionPriority = Perbill::from_percent(90) * TransactionPriority::max_value();
}

parameter_types! {
	pub const ImOnlineUnsignedPriority: TransactionPriority = TransactionPriority::max_value();
	pub const MaxPeerInHeartbeats: u32 = 10_000;
	pub const MaxPeerDataEncodingSize: u32 = 1_000;
	pub const MaxKeys: u32 = 10_000;
}

impl pallet_im_online::Config for Runtime {
	type AuthorityId = ImOnlineId;
	type Event = Event;
	type ValidatorSet = Historical;
	type NextSessionRotation = Babe;
	type ReportUnresponsiveness = Offences;
	type UnsignedPriority = ImOnlineUnsignedPriority;
	type WeightInfo = pallet_im_online::weights::SubstrateWeight<Runtime>;
	type MaxKeys = MaxKeys;
	type MaxPeerInHeartbeats = MaxPeerInHeartbeats;
	type MaxPeerDataEncodingSize = MaxPeerDataEncodingSize;
}

impl pallet_offences::Config for Runtime {
	type Event = Event;
	type IdentificationTuple = pallet_session::historical::IdentificationTuple<Self>;
	type OnOffenceHandler = Staking;
}

impl pallet_session::historical::Config for Runtime {
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
impl pallet_identity::Config for Runtime {
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
	pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
	pub const BountyUpdatePeriod: BlockNumber = 14 * DAYS;
	pub const MaximumReasonLength: u32 = 16_384;
	pub const MaxApprovals: u32 = 100;
	pub const BountyCuratorDeposit: Permill = Permill::from_percent(50);
	pub const BountyValueMinimum: Balance = 5 * DOLLARS;
}
impl pallet_treasury::Config for Runtime {
	type PalletId = TreasuryPalletId;
	type Currency = SpendingAssetCurrency<Self>;
	// root only is sufficient for launch phase
	type ApproveOrigin = EnsureRoot<AccountId>;
	type RejectOrigin = EnsureRoot<AccountId>;
	type Event = Event;
	type MaxApprovals = MaxApprovals;
	type OnSlash = ();
	type ProposalBond = ProposalBond;
	type ProposalBondMinimum = ProposalBondMinimum;
	type SpendPeriod = SpendPeriod;
	type Burn = Burn;
	type BurnDestination = ();
	type SpendFunds = ();
	type WeightInfo = pallet_treasury::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
	pub const HistoricalPayoutEras: u16 = 7;
	pub const FiscalEraLength: u32 = 365;
}
impl crml_staking_rewards::Config for Runtime {
	type CurrencyToReward = SpendingAssetCurrency<Self>;
	type Event = Event;
	type FiscalEraLength = FiscalEraLength;
	type HistoricalPayoutEras = HistoricalPayoutEras;
	type ScheduledPayoutRunner = ScheduledPayoutRunner<Self>;
	type TreasuryPalletId = TreasuryPalletId;
	type WeightInfo = ();
}

impl crml_cennzx::Config for Runtime {
	type Balance = Balance;
	type AssetId = AssetId;
	type Event = Event;
	type MultiCurrency = GenericAsset;
	type ExchangeAddressFor = ExchangeAddressGenerator<Self>;
	type WeightInfo = ();
}

parameter_types! {
	/// The threshold of notarizations required to approve an Eth deposit
	pub const NotarizationThreshold: Percent = Percent::from_percent(66_u8);
}
impl crml_eth_bridge::Config for Runtime {
	/// Listens for completed eth call jobs
	type EthCallSubscribers = EthStateOracle;
	/// The identifier type for an offchain worker.
	type EthyId = EthBridgeId;
	/// Provides Ethereum JSON-RPC client to the pallet (OCW friendly)
	type EthereumRpcClient = crml_eth_bridge::EthereumRpcClient;
	/// The threshold of positive notarizations to approve an event claim
	type NotarizationThreshold = NotarizationThreshold;
	/// Reports the current validator / notary set
	type AuthoritySet = Historical;
	/// Handle rewards for notaries
	type RewardHandler = Rewards;
	/// Tuple of modules subscribed to bridge events
	type Subscribers = Erc20Peg;
	/// The overarching dispatch call type.
	type Call = Call;
	/// The overarching event type.
	type Event = Event;
	/// Timestamp provider
	type UnixTime = Timestamp;
	/// Reports final session status of an era
	type FinalSessionTracker = Staking;
}

/// Returns min gas price according to network base fee
pub struct MinGasPriceGetter;
impl frame_support::traits::Get<u64> for MinGasPriceGetter {
	fn get() -> u64 {
		BaseFee::min_gas_price().saturated_into()
	}
}
parameter_types! {
	/// The number of blocks a state oracle response can be challenged for
	pub storage ChallengePeriod: BlockNumber = 5;
	/// Minimum bond amount required for a relayer and challenger
	pub storage RelayerBondAmount: Balance = 100_000_000;
	/// Maximum requests allowed per block (Absolute max: 100)
	pub storage MaxRequestsPerBlock: u32 = 30;
	/// Maximum number of active relayers allowed at one time
	pub storage MaxRelayerCount: u32 = 1;
	/// Prevents State Oracle transactions from executing if false
	pub storage StateOracleIsActive: bool = false;
}
impl crml_eth_state_oracle::Config for Runtime {
	type AddressMapping = AddressMappingOf<Self>;
	type ChallengePeriod = ChallengePeriod;
	type ContractExecutor = StateOracleCallbackExecutor<Self>;
	type StateOraclePrecompileAddress = StateOraclePrecompileAddress;
	type EthCallOracle = ();
	type Event = Event;
	type UnixTime = Timestamp;
	type MultiCurrency = GenericAsset;
	type MinGasPrice = MinGasPriceGetter;
	type GasWeightMapping = CENNZnetGasWeightMapping;
	type BuyFeeAsset = Cennzx;
	type RelayerBondAmount = RelayerBondAmount;
	type MaxRequestsPerBlock = MaxRequestsPerBlock;
	type MaxRelayerCount = MaxRelayerCount;
	type StateOracleIsActive = StateOracleIsActive;
}

impl crml_token_approvals::Config for Runtime {
	type MultiCurrency = GenericAsset;
	type IsTokenOwner = Nft;
}

// transaction must have an event/log of the deposit
// i.e. keccack256("Deposit(address,address,uint256,bytes32)")
const DEPOSIT_EVENT_SIGNATURE: [u8; 32] =
	hex_literal::hex!("76bb911c362d5b1feb3058bc7dc9354703e4b6eb9c61cc845f73da880cf62f61");
parameter_types! {
	/// The ERC20 bridge contract deposit event
	pub const DepositEventSignature: [u8; 32] = DEPOSIT_EVENT_SIGNATURE;
	/// The ERC20 peg address
	pub const PegPalletId: PalletId = PalletId(*b"erc20peg");
}
impl crml_erc20_peg::Config for Runtime {
	/// Handles Ethereum events
	type EthBridge = EthBridge;
	type DepositEventSignature = DepositEventSignature;
	/// Runtime currency system
	type MultiCurrency = GenericAsset;
	/// PalletId/Account for this module
	type PegPalletId = PegPalletId;
	/// The overarching event type.
	type Event = Event;
}

parameter_types! {
	/// lower priority than Staking and ImOnline txs
	pub const EcdsaUnsignedPriority: TransactionPriority = TransactionPriority::max_value() / 3;
}
impl crml_eth_wallet::Config for Runtime {
	type Event = Event;
	type Call = Call;
	type AddressMapping = AddressMappingOf<Self>;
	type TransactionFeeHandler = TransactionPayment;
	type Signer = <Signature as Verify>::Signer;
	type UnsignedPriority = EcdsaUnsignedPriority;
}

// Start frontier/EVM stuff

/// Current approximation of the gas/s consumption considering
/// EVM execution over compiled WASM (on 4.4Ghz CPU).
/// Given the 500ms Weight, from which 75% only are used for transactions,
/// the total EVM execution gas limit is: GAS_PER_SECOND * 0.500 * 0.75 ~= 15_000_000.
pub const GAS_PER_SECOND: u64 = 40_000_000;

/// Approximate ratio of the amount of Weight per Gas.
/// u64 works for approximations because Weight is a very small unit compared to gas.
pub const WEIGHT_PER_GAS: u64 = WEIGHT_PER_SECOND / GAS_PER_SECOND;

/// Type alias for the EVM to ss58 address mapping scheme used by CENNZnet runtime
pub type AddressMappingOf<T> = PrefixedAddressMapping<<T as frame_system::Config>::AccountId>;

pub struct CENNZnetGasWeightMapping;

impl pallet_evm::GasWeightMapping for CENNZnetGasWeightMapping {
	fn gas_to_weight(gas: u64) -> Weight {
		gas.saturating_mul(WEIGHT_PER_GAS)
	}
	fn weight_to_gas(weight: Weight) -> u64 {
		u64::try_from(weight.wrapping_div(WEIGHT_PER_GAS)).unwrap_or(u32::MAX as u64)
	}
}

/// This is unused while CENNZnet fullness is inconsistent
pub struct BaseFeeThreshold;
impl pallet_base_fee::BaseFeeThreshold for BaseFeeThreshold {
	fn lower() -> Permill {
		Permill::zero()
	}
	fn ideal() -> Permill {
		// blocks > 5% full trigger fee increase, < 5% full trigger fee decrease
		Permill::from_parts(50_000)
	}
	fn upper() -> Permill {
		Permill::one()
	}
}

parameter_types! {
	/// Floor network base fee per gas
	/// 0.00015 CPAY per gas
	pub const DefaultBaseFeePerGas: u64 = 15_000_000_000_000;
	pub const IsBaseFeeActive: bool = true;
}
impl pallet_base_fee::Config for Runtime {
	type DefaultBaseFeePerGas = DefaultBaseFeePerGas;
	type IsActive = IsBaseFeeActive;
	type Event = Event;
	type Threshold = BaseFeeThreshold;
}

parameter_types! {
	/// Ethereum ChainId
	/// 2999 (local/dev/default)
	/// 3000 (rata)
	/// 3001 (nikau)
	/// 21337 (mainnet)
	/// NB: Configured on live chains via one-time setStorage tx at key `:EthereumChainId:`
	pub storage EthereumChainId: u64 = 2_999;
	pub BlockGasLimit: U256
		= U256::from(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT / WEIGHT_PER_GAS);
	pub PrecompilesValue: CENNZnetPrecompiles<Runtime> = CENNZnetPrecompiles::<_>::new();
}

/// Modified london config with higher contract create fee
const fn cennznet_london() -> EvmConfig {
	let mut c = EvmConfig::london();
	c.gas_transaction_create = 2_000_000;
	c
}

pub static CENNZNET_EVM_CONFIG: EvmConfig = cennznet_london();

impl pallet_evm::Config for Runtime {
	type FeeCalculator = BaseFee;
	type GasWeightMapping = CENNZnetGasWeightMapping;
	type BlockHashMapping = pallet_ethereum::EthereumBlockHashMapping<Self>;
	type CallOrigin = EnsureAddressTruncated;
	type WithdrawOrigin = EnsureAddressTruncated;
	type AddressMapping = AddressMappingOf<Self>;
	type Currency = EvmCurrencyScaler<SpendingAssetCurrency<Self>>;
	type Event = Event;
	type Runner = FeePreferencesRunner<Self, Self>;
	type PrecompilesType = CENNZnetPrecompiles<Self>;
	type PrecompilesValue = PrecompilesValue;
	type ChainId = EthereumChainId;
	type BlockGasLimit = BlockGasLimit;
	type OnChargeTransaction = CENNZnetOnChargeEVMTransaction<Self>;
	type FindAuthor = EthereumFindAuthor<Babe>;
	// internal EVM config
	fn config() -> &'static EvmConfig {
		&CENNZNET_EVM_CONFIG
	}
}

impl pallet_ethereum::Config for Runtime {
	type Event = Event;
	type StateRoot = pallet_ethereum::IntermediateStateRoot;
}

pub struct TransactionConverter;

impl fp_rpc::ConvertTransaction<UncheckedExtrinsic> for TransactionConverter {
	fn convert_transaction(&self, transaction: pallet_ethereum::Transaction) -> UncheckedExtrinsic {
		UncheckedExtrinsic::new_unsigned(pallet_ethereum::Call::<Runtime>::transact { transaction }.into())
	}
}

impl fp_rpc::ConvertTransaction<sp_runtime::OpaqueExtrinsic> for TransactionConverter {
	fn convert_transaction(&self, transaction: pallet_ethereum::Transaction) -> sp_runtime::OpaqueExtrinsic {
		let extrinsic =
			UncheckedExtrinsic::new_unsigned(pallet_ethereum::Call::<Runtime>::transact { transaction }.into());
		let encoded = extrinsic.encode();
		sp_runtime::OpaqueExtrinsic::decode(&mut &encoded[..]).expect("Encoded extrinsic is always valid")
	}
}
// end frontier/EVM stuff

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
				log!(warn, "unable to create signed payload: {:?}", e);
			})
			.ok()?;
		let signature = raw_payload.using_encoded(|payload| C::sign(payload, public))?;
		let (call, extra, _) = raw_payload.deconstruct();
		Some((call, (account, signature.into(), extra)))
	}
}

construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = cennznet_primitives::types::Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		// Give modules fixed indexes in the runtime
		System: frame_system::{Pallet, Call, Storage, Config, Event<T>},
		Scheduler: pallet_scheduler::{Pallet, Call, Storage, Event<T>},
		Babe: pallet_babe::{Pallet, Call, Storage, Config, ValidateUnsigned},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		GenericAsset: crml_generic_asset::{Pallet, Call, Storage, Event<T>, Config<T>},
		Authorship: pallet_authorship::{Pallet, Call, Storage},
		Staking: crml_staking::{Pallet, Call, Storage, Config<T>, Event<T>, ValidateUnsigned},
		Offences: pallet_offences::{Pallet, Storage, Event},
		Session: pallet_session::{Pallet, Call, Storage, Event, Config<T>},
		Grandpa: pallet_grandpa::{Pallet, Call, Storage, Config, Event, ValidateUnsigned},
		ImOnline: pallet_im_online::{Pallet, Call, Storage, Event<T>, ValidateUnsigned, Config<T>},
		AuthorityDiscovery: pallet_authority_discovery::{Pallet, Config},
		Sudo: pallet_sudo::{Pallet, Call, Config<T>, Storage, Event<T>},
		Treasury: pallet_treasury::{Pallet, Call, Storage, Event<T>},
		Utility: pallet_utility::{Pallet, Call, Event},
		Identity: pallet_identity::{Pallet, Call, Storage, Event<T>},
		TransactionPayment: crml_transaction_payment::{Pallet, Storage},
		Historical: session_historical::{Pallet},
		Cennzx: crml_cennzx::{Pallet, Call, Storage, Config<T>, Event<T>},
		Rewards: crml_staking_rewards::{Pallet, Call, Storage, Config, Event<T>},
		Nft: crml_nft::{Pallet, Call, Storage, Event<T>},
		Governance: crml_governance::{Pallet, Call, Storage, Event},
		EthBridge: crml_eth_bridge::{Pallet, Call, Storage, Event, ValidateUnsigned},
		Erc20Peg: crml_erc20_peg::{Pallet, Call, Storage, Config, Event<T>},
		EthStateOracle: crml_eth_state_oracle::{Pallet, Call, Storage, Event<T>},
		EthWallet: crml_eth_wallet::{Pallet, Call, Event<T>, ValidateUnsigned},
		// EVM support
		Ethereum: pallet_ethereum::{Pallet, Call, Storage, Event, Config, Origin},
		EVM: pallet_evm::{Pallet, Config, Call, Storage, Event<T>},
		BaseFee: pallet_base_fee::{Pallet, Call, Storage, Config<T>, Event},
		TokenApprovals: crml_token_approvals::{Pallet, Call, Storage},
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
pub type UncheckedExtrinsic = fp_self_contained::UncheckedExtrinsic<Address, Call, Signature, SignedExtra>;
/// The payload being signed in transactions.
pub type SignedPayload = generic::SignedPayload<Call, SignedExtra>;
/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = fp_self_contained::CheckedExtrinsic<AccountId, Call, SignedExtra, H160>;
/// Executive: handles dispatch to the various modules.
pub type Executive =
	frame_executive::Executive<Runtime, Block, frame_system::ChainContext<Runtime>, Runtime, AllPalletsWithSystem>;

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
			OpaqueMetadata::new(Runtime::metadata().into())
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
	}

	impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
		fn validate_transaction(
			source: TransactionSource,
			tx: <Block as BlockT>::Extrinsic,
			block_hash: <Block as BlockT>::Hash,
		) -> TransactionValidity {
			Executive::validate_transaction(source, tx, block_hash)
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
				c: BABE_GENESIS_EPOCH_CONFIG.c,
				genesis_authorities: Babe::authorities().to_vec(),
				randomness: Babe::randomness(),
				allowed_slots: BABE_GENESIS_EPOCH_CONFIG.allowed_slots,
			}
		}

		fn current_epoch_start() -> sp_consensus_babe::Slot {
			Babe::current_epoch_start()
		}

		fn current_epoch() -> sp_consensus_babe::Epoch {
			Babe::current_epoch()
		}

		fn next_epoch() -> sp_consensus_babe::Epoch {
			Babe::next_epoch()
		}

		fn generate_key_ownership_proof(
			_slot_number: sp_consensus_babe::Slot,
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
			SessionKeys::generate(seed)
		}

		fn decode_session_keys(
			encoded: Vec<u8>,
		) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
			SessionKeys::decode_into_raw_public_keys(&encoded)
		}
	}

	impl fg_primitives::GrandpaApi<Block> for Runtime {
		fn grandpa_authorities() -> GrandpaAuthorityList {
			Grandpa::grandpa_authorities()
		}

		fn current_set_id() -> fg_primitives::SetId {
			Grandpa::current_set_id()
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

	impl cennznet_primitives::eth::EthyApi<Block> for Runtime {
		fn validator_set() -> cennznet_primitives::eth::ValidatorSet<EthBridgeId> {
			EthBridge::validator_set()
		}
	}

	impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index> for Runtime {
		fn account_nonce(account: AccountId) -> Index {
			System::account_nonce(account)
		}
	}

	impl crml_generic_asset_rpc_runtime_api::GenericAssetRuntimeApi<Block, AssetId, Balance, AccountId> for Runtime {
		fn asset_meta() -> Vec<(AssetId, AssetInfo)> {
			GenericAsset::registered_assets()
		}
		fn get_balance(account_id: AccountId, asset_id: AssetId) -> AllBalances<Balance> {
			GenericAsset::get_all_balances(&account_id, asset_id)
		}
	}

	impl crml_governance_rpc_runtime_api::GovernanceRuntimeApi<Block, AccountId> for Runtime {
		fn council() -> Vec<AccountId> {
			Governance::get_council()
		}
		fn proposal_votes() -> Vec<(ProposalId, ProposalVoteInfo)> {
			Governance::get_proposal_votes()
		}
	}

	impl crml_transaction_payment_rpc_runtime_api::TransactionPaymentApi<
		Block,
		Balance,
	> for Runtime {
		fn query_info(uxt: <Block as BlockT>::Extrinsic, len: u32) -> RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_info(uxt, len)
		}
		fn query_fee_details(uxt: <Block as BlockT>::Extrinsic, len: u32) -> FeeDetails<Balance> {
			TransactionPayment::query_fee_details(uxt, len)
		}
	}

	impl crml_nft_rpc_runtime_api::NftApi<
		Block,
		AccountId,
		Runtime,
	> for Runtime {
		fn collected_tokens(collection_id: CollectionId, who: AccountId) -> Vec<TokenId> {
			Nft::collected_tokens(collection_id, &who)
		}
		fn collection_info(collection_id: CollectionId) -> Option<CollectionInfo<AccountId>> {
			Nft::collection_info::<AccountId>(collection_id)
		}
		fn token_info(
		   collection_id: CollectionId,
		   series_id: SeriesId,
		   serial_number: SerialNumber,
		) -> Option<TokenInfo<AccountId>> {
		   Nft::token_info(collection_id, series_id, serial_number)
		}
		fn token_uri(token_id: TokenId) -> Vec<u8> {
			Nft::token_uri(token_id)
		}
		fn collection_listings(
			collection_id: CollectionId,
			offset: u128,
			limit: u16,
		) -> (Option<u128>, Vec<(ListingId, Listing<Runtime>)>) {
			Nft::collection_listings(collection_id, offset, limit)
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

	impl crml_staking_rpc_runtime_api::StakingApi<Block, AccountId> for Runtime {
		fn accrued_payout(stash: &AccountId) -> u64 {
			Staking::accrued_payout(stash) as u64
		}
	}

	impl fp_rpc::ConvertTransactionRuntimeApi<Block> for Runtime {
		fn convert_transaction(transaction: EthereumTransaction) -> <Block as BlockT>::Extrinsic {
			UncheckedExtrinsic::new_unsigned(
				pallet_ethereum::Call::<Runtime>::transact { transaction }.into(),
			)
		}
	}

	impl fp_rpc::EthereumRuntimeRPCApi<Block> for Runtime {
		fn chain_id() -> u64 {
			<Runtime as pallet_evm::Config>::ChainId::get()
		}

		fn account_basic(address: H160) -> EVMAccount {
	  // this balance is scaled up so that eth tooling expecting an 18dp asset
	  // is compatible with the 4dp cpay value
			EVM::account_basic(&address)
		}

		fn gas_price() -> U256 {
			BaseFee::min_gas_price()
		}

		fn account_code_at(address: H160) -> Vec<u8> {
			EVM::account_codes(address)
		}

		fn author() -> H160 {
			<pallet_evm::Pallet<Runtime>>::find_author()
		}

		fn storage_at(address: H160, index: U256) -> H256 {
			let mut tmp = [0u8; 32];
			index.to_big_endian(&mut tmp);
			EVM::account_storages(address, H256::from_slice(&tmp[..]))
		}

		fn call(
			from: H160,
			to: H160,
			data: Vec<u8>,
			value: U256,
			gas_limit: U256,
			max_fee_per_gas: Option<U256>,
			max_priority_fee_per_gas: Option<U256>,
			nonce: Option<U256>,
			estimate: bool,
			access_list: Option<Vec<(H160, Vec<H256>)>>,
		) -> Result<pallet_evm::CallInfo, sp_runtime::DispatchError> {
			let config = if estimate {
				let mut config = <Runtime as pallet_evm::Config>::config().clone();
				config.estimate = true;
				Some(config)
			} else {
				None
			};

			<Runtime as pallet_evm::Config>::Runner::call(
				from,
				to,
				data,
				value,
				gas_limit.low_u64(),
				max_fee_per_gas,
				max_priority_fee_per_gas,
				nonce,
				access_list.unwrap_or_default(),
				config.as_ref().unwrap_or(<Runtime as pallet_evm::Config>::config()),
			).map_err(|err| err.into())
		}

		fn create(
			from: H160,
			data: Vec<u8>,
			value: U256,
			gas_limit: U256,
			max_fee_per_gas: Option<U256>,
			max_priority_fee_per_gas: Option<U256>,
			nonce: Option<U256>,
			estimate: bool,
			access_list: Option<Vec<(H160, Vec<H256>)>>,
		) -> Result<pallet_evm::CreateInfo, sp_runtime::DispatchError> {
			let config = if estimate {
				let mut config = <Runtime as pallet_evm::Config>::config().clone();
				config.estimate = true;
				Some(config)
			} else {
				None
			};

			<Runtime as pallet_evm::Config>::Runner::create(
				from,
				data,
				value,
				gas_limit.low_u64(),
				max_fee_per_gas,
				max_priority_fee_per_gas,
				nonce,
				access_list.unwrap_or_default(),
				config.as_ref().unwrap_or(<Runtime as pallet_evm::Config>::config()),
			).map_err(|err| err.into())
		}

		fn current_transaction_statuses() -> Option<Vec<TransactionStatus>> {
			Ethereum::current_transaction_statuses()
		}

		fn current_block() -> Option<pallet_ethereum::Block> {
			Ethereum::current_block()
		}

		fn current_receipts() -> Option<Vec<pallet_ethereum::Receipt>> {
			Ethereum::current_receipts()
		}

		fn current_all() -> (
			Option<pallet_ethereum::Block>,
			Option<Vec<pallet_ethereum::Receipt>>,
			Option<Vec<TransactionStatus>>
		) {
			(
				Ethereum::current_block(),
				Ethereum::current_receipts(),
				Ethereum::current_transaction_statuses()
			)
		}

		fn extrinsic_filter(
			xts: Vec<<Block as BlockT>::Extrinsic>,
		) -> Vec<EthereumTransaction> {
			xts.into_iter().filter_map(|xt| match xt.0.function {
				Call::Ethereum(transact { transaction }) => Some(transaction),
				_ => None
			}).collect::<Vec<EthereumTransaction>>()
		}

		fn elasticity() -> Option<Permill> {
			Some(BaseFee::elasticity())
		}
	}

	impl cennznet_primitives::txpool::TxPoolRuntimeApi<Block> for Runtime {
		fn extrinsic_filter(
			xts_ready: Vec<<Block as BlockT>::Extrinsic>,
			xts_future: Vec<<Block as BlockT>::Extrinsic>,
		) -> cennznet_primitives::txpool::TxPoolResponse {
			cennznet_primitives::txpool::TxPoolResponse {
				ready: xts_ready
					.into_iter()
					.filter_map(|xt| match xt.0.function {
						Call::Ethereum(transact { transaction }) => Some(transaction),
						_ => None,
					})
					.collect(),
				future: xts_future
					.into_iter()
					.filter_map(|xt| match xt.0.function {
						Call::Ethereum(transact { transaction }) => Some(transaction),
						_ => None,
					})
					.collect(),
			}
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
			add_benchmark!(params, batches, crml_nft, Nft);

			if batches.is_empty() { return Err("Benchmark not found for this pallet.".into()) }
			Ok(batches)
		}
	}
}

impl fp_self_contained::SelfContainedCall for Call {
	type SignedInfo = H160;

	fn is_self_contained(&self) -> bool {
		match self {
			Call::Ethereum(call) => call.is_self_contained(),
			_ => false,
		}
	}

	fn check_self_contained(&self) -> Option<Result<Self::SignedInfo, TransactionValidityError>> {
		match self {
			Call::Ethereum(call) => call.check_self_contained(),
			_ => None,
		}
	}

	fn validate_self_contained(&self, signed_info: &Self::SignedInfo) -> Option<TransactionValidity> {
		match self {
			Call::Ethereum(ref call) => Some(validate_self_contained_inner(&self, &call, signed_info)),
			_ => None,
		}
	}

	fn pre_dispatch_self_contained(&self, info: &Self::SignedInfo) -> Option<Result<(), TransactionValidityError>> {
		match self {
			Call::Ethereum(call) => call.pre_dispatch_self_contained(info),
			_ => None,
		}
	}

	fn apply_self_contained(
		self,
		info: Self::SignedInfo,
	) -> Option<sp_runtime::DispatchResultWithInfo<PostDispatchInfoOf<Self>>> {
		match self {
			call @ Call::Ethereum(pallet_ethereum::Call::transact { .. }) => {
				Some(call.dispatch(Origin::from(pallet_ethereum::RawOrigin::EthereumTransaction(info))))
			}
			_ => None,
		}
	}
}

fn validate_self_contained_inner(
	call: &Call,
	eth_call: &pallet_ethereum::Call<Runtime>,
	signed_info: &<Call as fp_self_contained::SelfContainedCall>::SignedInfo,
) -> TransactionValidity {
	if let pallet_ethereum::Call::transact { ref transaction } = eth_call {
		// Previously, ethereum transactions were contained in an unsigned
		// extrinsic, we now use a new form of dedicated extrinsic defined by
		// frontier, but to keep the same behavior as before, we must perform
		// the controls that were performed on the unsigned extrinsic.
		use sp_runtime::traits::SignedExtension as _;
		let input_len = match transaction {
			pallet_ethereum::Transaction::Legacy(t) => t.input.len(),
			pallet_ethereum::Transaction::EIP2930(t) => t.input.len(),
			pallet_ethereum::Transaction::EIP1559(t) => t.input.len(),
		};
		let extra_validation = SignedExtra::validate_unsigned(call, &call.get_dispatch_info(), input_len)?;
		// Then, do the controls defined by the ethereum pallet.
		let self_contained_validation = eth_call
			.validate_self_contained(signed_info)
			.ok_or(TransactionValidityError::Invalid(InvalidTransaction::BadProof))??;

		Ok(extra_validation.combine_with(self_contained_validation))
	} else {
		Err(TransactionValidityError::Unknown(
			sp_runtime::transaction_validity::UnknownTransaction::CannotLookup,
		))
	}
}
