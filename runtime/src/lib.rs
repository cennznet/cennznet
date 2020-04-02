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
#![allow(array_into_iter)]

use cennznet_primitives::types::{AccountId, AssetId, Balance, BlockNumber, Hash, Index, Moment, Signature};
use cennznut::{CENNZnut, Domain, Validate, ValidationErr};
use codec::Decode;
pub use crml_cennzx_spot::{ExchangeAddressGenerator, FeeRate, PerMilli, PerMillion};
use crml_cennzx_spot_rpc_runtime_api::CennzxSpotResult;
use frame_support::{
	additional_traits::{self, MultiCurrencyAccounting},
	construct_runtime, debug, parameter_types,
	traits::{Randomness, SplitTwoWays},
	weights::Weight,
};
use frame_system::offchain::TransactionSubmitter;
pub use pallet_contracts::Gas;
use pallet_contracts_rpc_runtime_api::ContractExecResult;
pub use pallet_generic_asset::Call as GenericAssetCall;
use pallet_generic_asset::{SpendingAssetCurrency, StakingAssetCurrency};
use pallet_grandpa::fg_primitives;
use pallet_grandpa::AuthorityList as GrandpaAuthorityList;
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo;
use sp_api::impl_runtime_apis;
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_core::u32_trait::{_0, _1, _2, _4};
use sp_core::OpaqueMetadata;
use sp_inherents::{CheckInherentsResult, InherentData};
use sp_runtime::curve::PiecewiseLinear;
use sp_runtime::traits::{
	self, BlakeTwo256, Block as BlockT, IdentityLookup, OpaqueKeys, PlugDoughnutApi, SaturatedConversion,
};
use sp_runtime::transaction_validity::{TransactionSource, TransactionValidity};
use sp_runtime::{create_runtime_str, generic, impl_opaque_keys, ApplyExtrinsicResult, Perbill, Percent, Permill};
use sp_std::prelude::*;
#[cfg(any(feature = "std", test))]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;

pub use crml_staking::StakerStatus;
pub use frame_support::StorageValue;
pub use pallet_timestamp::Call as TimestampCall;
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;

pub use crml_sylo::device as sylo_device;
pub use crml_sylo::e2ee as sylo_e2ee;
pub use crml_sylo::groups as sylo_groups;
pub use crml_sylo::inbox as sylo_inbox;
pub use crml_sylo::response as sylo_response;
pub use crml_sylo::vault as sylo_vault;

/// Implementations of some helper traits passed into runtime modules as associated types.
pub mod impls;
use impls::{
	CurrencyToVoteHandler, GasHandler, GasMeteredCallResolver, LinearWeightToFee, SplitToAllValidators,
	TargetedFeeAdjustment,
};

/// Constant values used within the runtime.
pub mod constants;
use constants::{currency::*, time::*};

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

/// Runtime version.
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: create_runtime_str!("cennznet"),
	impl_name: create_runtime_str!("cennznet-node"),
	authoring_version: 1,
	// Per convention: if the runtime behavior changes, increment spec_version
	// and set impl_version to equal spec_version. If only runtime
	// implementation changes and behavior does not, then leave spec_version as
	// is and increment impl_version.
	spec_version: 29,
	impl_version: 29,
	apis: RUNTIME_API_VERSIONS,
};

/// Native version.
#[cfg(any(feature = "std", test))]
pub fn native_version() -> NativeVersion {
	NativeVersion {
		runtime_version: VERSION,
		can_author_with: Default::default(),
	}
}

parameter_types! {
	pub const BlockHashCount: BlockNumber = 250;
	pub const MaximumBlockWeight: Weight = 1_000_000_000;
	pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
	pub const MaximumBlockLength: u32 = 5 * 1024 * 1024;
	pub const Version: RuntimeVersion = VERSION;
}

pub type CennznetDoughnut = prml_doughnut::PlugDoughnut<Runtime>;

impl frame_system::Trait for Runtime {
	type Origin = Origin;
	type Call = Call;
	type Index = Index;
	type BlockNumber = BlockNumber;
	type Hash = Hash;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<AccountId>;
	type Header = generic::Header<BlockNumber, BlakeTwo256>;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type Doughnut = prml_doughnut::PlugDoughnut<Runtime>;
	type DelegatedDispatchVerifier = Runtime;
	type MaximumBlockWeight = MaximumBlockWeight;
	type MaximumBlockLength = MaximumBlockLength;
	type AvailableBlockRatio = AvailableBlockRatio;
	type Version = Version;
	type ModuleToIndex = ModuleToIndex;
}

parameter_types! {
	// One storage item; value is size 4+4+16+32 bytes = 56 bytes.
	pub const MultisigDepositBase: Balance = 30 * CENTS;
	// Additional storage item size of 32 bytes.
	pub const MultisigDepositFactor: Balance = 5 * CENTS;
	pub const MaxSignatories: u16 = 100;
}

impl prml_doughnut::DoughnutRuntime for Runtime {
	type AccountId = <Self as frame_system::Trait>::AccountId;
	type Call = Call;
	type Doughnut = <Self as frame_system::Trait>::Doughnut;
	type TimestampProvider = pallet_timestamp::Module<Runtime>;
}

impl pallet_utility::Trait for Runtime {
	type Event = Event;
	type Call = Call;
	type Currency = StakingAssetCurrency<Self>;
	type MultisigDepositBase = MultisigDepositBase;
	type MultisigDepositFactor = MultisigDepositFactor;
	type MaxSignatories = MaxSignatories;
}

impl crml_cennzx_spot::Trait for Runtime {
	type Call = Call;
	type Event = Event;
	type ExchangeAddressGenerator = ExchangeAddressGenerator<Self>;
	type BalanceToUnsignedInt = Balance;
	type UnsignedIntToBalance = Balance;
}

impl prml_attestation::Trait for Runtime {
	type Event = Event;
}

impl crml_sylo::groups::Trait for Runtime {}
impl crml_sylo::e2ee::Trait for Runtime {
	type Event = Event;
}
impl crml_sylo::device::Trait for Runtime {
	type Event = Event;
}
impl crml_sylo::response::Trait for Runtime {}
impl crml_sylo::inbox::Trait for Runtime {}
impl crml_sylo::vault::Trait for Runtime {}

parameter_types! {
	pub const EpochDuration: u64 = EPOCH_DURATION_IN_SLOTS;
	pub const ExpectedBlockTime: Moment = MILLISECS_PER_BLOCK;
}
impl pallet_babe::Trait for Runtime {
	type EpochDuration = EpochDuration;
	type ExpectedBlockTime = ExpectedBlockTime;
	type EpochChangeTrigger = pallet_babe::ExternalTrigger;
}

impl pallet_generic_asset::Trait for Runtime {
	type Balance = Balance;
	type AssetId = AssetId;
	type Event = Event;
}

parameter_types! {
	pub const TransactionBaseFee: Balance = 1 * CENTS;
	pub const TransactionByteFee: Balance = 10 * MILLICENTS;
	// setting this to zero will disable the weight fee.
	pub const WeightFeeCoefficient: Balance = 1_000;
	// for a sane configuration, this should always be less than `AvailableBlockRatio`.
	pub const TargetBlockFullness: Perbill = Perbill::from_percent(25);
}

pub type PositiveImbalance = <GenericAsset as MultiCurrencyAccounting>::PositiveImbalance;
pub type NegativeImbalance = <GenericAsset as MultiCurrencyAccounting>::NegativeImbalance;

pub type DealWithFees = SplitTwoWays<
	Balance,
	NegativeImbalance,
	_0,
	Treasury,
	_1,
	SplitToAllValidators, // 100% goes to elected validators
>;

impl crml_transaction_payment::Trait for Runtime {
	type Balance = Balance;
	type AssetId = AssetId;
	type Currency = SpendingAssetCurrency<Self>;
	type OnTransactionPayment = DealWithFees;
	type TransactionBaseFee = TransactionBaseFee;
	type TransactionByteFee = TransactionByteFee;
	type WeightToFee = LinearWeightToFee<WeightFeeCoefficient>;
	type FeeMultiplierUpdate = TargetedFeeAdjustment<TargetBlockFullness>;
	type BuyFeeAsset = CennzxSpot;
	type GasMeteredCallResolver = GasMeteredCallResolver;
}

parameter_types! {
	pub const MinimumPeriod: Moment = SLOT_DURATION / 2;
}
impl pallet_timestamp::Trait for Runtime {
	type Moment = Moment;
	type OnTimestampSet = Babe;
	type MinimumPeriod = MinimumPeriod;
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

impl_opaque_keys! {
	pub struct SessionKeys {
		pub grandpa: Grandpa,
		pub babe: Babe,
		pub im_online: ImOnline,
		pub authority_discovery: AuthorityDiscovery,
	}
}

parameter_types! {
	pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(17);
}

impl pallet_session::Trait for Runtime {
	type SessionManager = Staking;
	type SessionHandler = <SessionKeys as OpaqueKeys>::KeyTypeIdProviders;
	type ShouldEndSession = Babe;
	type Event = Event;
	type Keys = SessionKeys;
	type ValidatorId = <Self as frame_system::Trait>::AccountId;
	type ValidatorIdOf = crml_staking::StashOf<Self>;
	type DisabledValidatorsThreshold = DisabledValidatorsThreshold;
}

impl pallet_session::historical::Trait for Runtime {
	type FullIdentification = crml_staking::Exposure<AccountId, Balance>;
	type FullIdentificationOf = crml_staking::ExposureOf<Runtime>;
}

crml_staking_reward_curve::build! {
	const REWARD_CURVE: PiecewiseLinear<'static> = curve!(
		min_inflation: 0_025_000,
		max_inflation: 0_100_000,
		ideal_stake: 0_500_000,
		falloff: 0_050_000,
		max_piece_count: 40,
		test_precision: 0_005_000,
	);
}

parameter_types! {
	pub const SessionsPerEra: sp_staking::SessionIndex = 6;
	pub const BondingDuration: crml_staking::EraIndex = 24 * 28;
	pub const SlashDeferDuration: crml_staking::EraIndex = 24 * 7; // 1/4 the bonding duration.
	pub const RewardCurve: &'static PiecewiseLinear<'static> = &REWARD_CURVE;
}

impl crml_staking::Trait for Runtime {
	type Currency = StakingAssetCurrency<Self>;
	type RewardCurrency = SpendingAssetCurrency<Self>;
	type Time = Timestamp;
	type CurrencyToVote = CurrencyToVoteHandler;
	type RewardRemainder = Treasury;
	type Event = Event;
	type Slash = Treasury; // send the slashed funds to the treasury.
	type Reward = (); // rewards are minted from the void
	type SessionsPerEra = SessionsPerEra;
	type BondingDuration = BondingDuration;
	type SlashDeferDuration = SlashDeferDuration;
	type SessionInterface = Self;
	type RewardCurve = RewardCurve;
}

parameter_types! {
	pub const LaunchPeriod: BlockNumber = 28 * 24 * 60 * MINUTES;
	pub const VotingPeriod: BlockNumber = 28 * 24 * 60 * MINUTES;
	pub const EmergencyVotingPeriod: BlockNumber = 3 * 24 * 60 * MINUTES;
	pub const MinimumDeposit: Balance = 100 * DOLLARS;
	pub const EnactmentPeriod: BlockNumber = 30 * 24 * 60 * MINUTES;
	pub const CooloffPeriod: BlockNumber = 28 * 24 * 60 * MINUTES;
	// One cent: $10,000 / MB
	pub const PreimageByteDeposit: Balance = 1 * CENTS;
}

parameter_types! {
	pub const CouncilMotionDuration: BlockNumber = 5 * DAYS;
}
type CouncilCollective = pallet_collective::Instance1;
impl pallet_collective::Trait<CouncilCollective> for Runtime {
	type Origin = Origin;
	type Proposal = Call;
	type Event = Event;
	type MotionDuration = CouncilMotionDuration;
}

parameter_types! {
	pub const CandidacyBond: Balance = 10 * DOLLARS;
	pub const VotingBond: Balance = 1 * DOLLARS;
	pub const TermDuration: BlockNumber = 7 * DAYS;
	pub const DesiredMembers: u32 = 13;
	pub const DesiredRunnersUp: u32 = 7;
}

impl pallet_elections_phragmen::Trait for Runtime {
	type Event = Event;
	type Currency = StakingAssetCurrency<Self>;
	type CurrencyToVote = CurrencyToVoteHandler;
	type CandidacyBond = CandidacyBond;
	type VotingBond = VotingBond;
	type TermDuration = TermDuration;
	type DesiredMembers = DesiredMembers;
	type DesiredRunnersUp = DesiredRunnersUp;
	type LoserCandidate = ();
	type BadReport = ();
	type KickedMember = ();
	type ChangeMembers = Council;
}

parameter_types! {
	pub const TechnicalMotionDuration: BlockNumber = 5 * DAYS;
}
type TechnicalCollective = pallet_collective::Instance2;
impl pallet_collective::Trait<TechnicalCollective> for Runtime {
	type Origin = Origin;
	type Proposal = Call;
	type Event = Event;
	type MotionDuration = TechnicalMotionDuration;
}

impl pallet_membership::Trait<pallet_membership::Instance1> for Runtime {
	type Event = Event;
	type AddOrigin = pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, Self::Doughnut, CouncilCollective>;
	type RemoveOrigin =
		pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, Self::Doughnut, CouncilCollective>;
	type SwapOrigin = pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, Self::Doughnut, CouncilCollective>;
	type ResetOrigin =
		pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, Self::Doughnut, CouncilCollective>;
	type PrimeOrigin =
		pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, Self::Doughnut, CouncilCollective>;
	type MembershipInitialized = TechnicalCommittee;
	type MembershipChanged = TechnicalCommittee;
}

parameter_types! {
	pub const ProposalBond: Permill = Permill::from_percent(5);
	pub const ProposalBondMinimum: Balance = 1 * DOLLARS;
	pub const SpendPeriod: BlockNumber = 1 * DAYS;
	pub const Burn: Permill = Permill::from_percent(50);
	pub const TipCountdown: BlockNumber = 1 * DAYS;
	pub const TipFindersFee: Percent = Percent::from_percent(20);
	pub const TipReportDepositBase: Balance = 1 * DOLLARS;
	pub const TipReportDepositPerByte: Balance = 1 * CENTS;
}

impl pallet_treasury::Trait for Runtime {
	type Currency = StakingAssetCurrency<Self>;
	type ApproveOrigin = pallet_collective::EnsureMembers<_4, AccountId, Self::Doughnut, CouncilCollective>;
	type RejectOrigin = pallet_collective::EnsureMembers<_2, AccountId, Self::Doughnut, CouncilCollective>;
	type Event = Event;
	type ProposalRejection = ();
	type ProposalBond = ProposalBond;
	type ProposalBondMinimum = ProposalBondMinimum;
	type SpendPeriod = SpendPeriod;
	type Burn = Burn;
	type Tippers = Elections;
	type TipCountdown = TipCountdown;
	type TipFindersFee = TipFindersFee;
	type TipReportDepositBase = TipReportDepositBase;
	type TipReportDepositPerByte = TipReportDepositPerByte;
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

impl pallet_contracts::Trait for Runtime {
	type Currency = SpendingAssetCurrency<Self>;
	type Time = Timestamp;
	type Randomness = RandomnessCollectiveFlip;
	type Call = Call;
	type Event = Event;
	type DetermineContractAddress = pallet_contracts::SimpleAddressDeterminer<Runtime>;
	type ComputeDispatchFee = pallet_contracts::DefaultDispatchFeeComputor<Runtime>;
	type TrieIdGenerator = pallet_contracts::TrieIdFromParentCounter<Runtime>;
	type GasPayment = ();
	type GasHandler = GasHandler;
	type RentPayment = ();
	type SignedClaimHandicap = pallet_contracts::DefaultSignedClaimHandicap;
	type TombstoneDeposit = TombstoneDeposit;
	type StorageSizeOffset = pallet_contracts::DefaultStorageSizeOffset;
	type RentByteFee = RentByteFee;
	type RentDepositOffset = RentDepositOffset;
	type SurchargeReward = SurchargeReward;
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

impl pallet_sudo::Trait for Runtime {
	type Event = Event;
	type Call = Call;
}

/// A runtime transaction submitter.
pub type SubmitTransaction = TransactionSubmitter<ImOnlineId, Runtime, UncheckedExtrinsic>;

parameter_types! {
	pub const SessionDuration: BlockNumber = EPOCH_DURATION_IN_SLOTS as _;
}

impl pallet_im_online::Trait for Runtime {
	type AuthorityId = ImOnlineId;
	type Call = Call;
	type Event = Event;
	type SubmitTransaction = SubmitTransaction;
	type ReportUnresponsiveness = Offences;
	type SessionDuration = SessionDuration;
}

impl pallet_offences::Trait for Runtime {
	type Event = Event;
	type IdentificationTuple = pallet_session::historical::IdentificationTuple<Self>;
	type OnOffenceHandler = Staking;
}

impl pallet_authority_discovery::Trait for Runtime {}

impl pallet_grandpa::Trait for Runtime {
	type Event = Event;
}

parameter_types! {
	pub const WindowSize: BlockNumber = 101;
	pub const ReportLatency: BlockNumber = 1000;
}

impl pallet_finality_tracker::Trait for Runtime {
	type OnFinalizationStalled = ();
	type WindowSize = WindowSize;
	type ReportLatency = ReportLatency;
}

impl frame_system::offchain::CreateTransaction<Runtime, UncheckedExtrinsic> for Runtime {
	type Public = <Signature as traits::Verify>::Signer;
	type Signature = Signature;

	fn create_transaction<TSigner: frame_system::offchain::Signer<Self::Public, Self::Signature>>(
		call: Call,
		public: Self::Public,
		account: AccountId,
		index: Index,
	) -> Option<(Call, <UncheckedExtrinsic as traits::Extrinsic>::SignaturePayload)> {
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
		let tip = 0;
		let extra: SignedExtra = (
			None,
			frame_system::CheckVersion::<Runtime>::new(),
			frame_system::CheckGenesis::<Runtime>::new(),
			frame_system::CheckEra::<Runtime>::from(generic::Era::mortal(period, current_block)),
			frame_system::CheckNonce::<Runtime>::from(index),
			frame_system::CheckWeight::<Runtime>::new(),
			crml_transaction_payment::ChargeTransactionPayment::<Runtime>::from(tip, None),
			Default::default(),
		);
		let raw_payload = SignedPayload::new(call, extra)
			.map_err(|e| {
				debug::warn!("Unable to create signed payload: {:?}", e);
			})
			.ok()?;
		let signature = TSigner::sign(public, &raw_payload)?;
		let (call, extra, _) = raw_payload.deconstruct();
		Some((call, (account, signature, extra)))
	}
}

/// Verify a Doughnut proof authorizes method dispatch given some input parameters
impl additional_traits::DelegatedDispatchVerifier for Runtime {
	type Doughnut = <Self as frame_system::Trait>::Doughnut;
	type AccountId = <Self as frame_system::Trait>::AccountId;

	const DOMAIN: &'static str = "cennznet";

	fn verify_dispatch(doughnut: &Self::Doughnut, module: &str, method: &str) -> Result<(), &'static str> {
		let mut domain = doughnut
			.get_domain(Self::DOMAIN)
			.ok_or("CENNZnut does not grant permission for cennznet domain")?;
		let cennznut: CENNZnut = Decode::decode(&mut domain).map_err(|_| "Bad CENNZnut encoding")?;

		// Extract Module name from <prefix>-<Module_name>
		let module_offset = module.find('-').ok_or("error during module name segmentation")? + 1;
		if module_offset <= 1 || module_offset >= module.len() {
			return Err("error during module name segmentation");
		}
		match cennznut.validate(&module[module_offset..], method, &[]) {
			Ok(r) => Ok(r),
			Err(ValidationErr::ConstraintsInterpretation) => Err("error while interpreting constraints"),
			Err(ValidationErr::NoPermission(Domain::Method)) => Err("CENNZnut does not grant permission for method"),
			Err(ValidationErr::NoPermission(Domain::Module)) => Err("CENNZnut does not grant permission for module"),
			Err(ValidationErr::NoPermission(Domain::MethodArguments)) => {
				Err("CENNZnut does not grant permission for method arguments")
			}
		}
	}
}

construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = cennznet_primitives::types::Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		System: frame_system::{Module, Call, Storage, Config, Event},
		Utility: pallet_utility::{Module, Call, Storage, Event<T>},
		Babe: pallet_babe::{Module, Call, Storage, Config, Inherent(Timestamp)},
		Timestamp: pallet_timestamp::{Module, Call, Storage, Inherent},
		Authorship: pallet_authorship::{Module, Call, Storage, Inherent},
		Attestation: prml_attestation::{Module, Call, Storage, Event<T>},
		TransactionPayment: crml_transaction_payment::{Module, Storage},
		GenericAsset: pallet_generic_asset::{Module, Call, Storage, Event<T>, Config<T>},
		Staking: crml_staking::{Module, Call, Config<T>, Storage, Event<T>},
		Session: pallet_session::{Module, Call, Storage, Event, Config<T>},
		Council: pallet_collective::<Instance1>::{Module, Call, Storage, Origin<T>, Event<T>, Config<T>},
		TechnicalCommittee: pallet_collective::<Instance2>::{Module, Call, Storage, Origin<T>, Event<T>, Config<T>},
		Elections: pallet_elections_phragmen::{Module, Call, Storage, Event<T>},
		TechnicalMembership: pallet_membership::<Instance1>::{Module, Call, Storage, Event<T>, Config<T>},
		FinalityTracker: pallet_finality_tracker::{Module, Call, Inherent},
		Grandpa: pallet_grandpa::{Module, Call, Storage, Config, Event},
		Treasury: pallet_treasury::{Module, Call, Storage, Config, Event<T>},
		Contracts: pallet_contracts::{Module, Call, Config<T>, Storage, Event<T>},
		Sudo: pallet_sudo::{Module, Call, Config<T>, Storage, Event<T>},
		ImOnline: pallet_im_online::{Module, Call, Storage, Event<T>, ValidateUnsigned, Config<T>},
		AuthorityDiscovery: pallet_authority_discovery::{Module, Call, Config},
		Offences: pallet_offences::{Module, Call, Storage, Event},
		RandomnessCollectiveFlip: pallet_randomness_collective_flip::{Module, Call, Storage},
		SyloGroups: sylo_groups::{Module, Call, Storage},
		SyloE2EE: sylo_e2ee::{Module, Call, Event<T>, Storage},
		SyloDevice: sylo_device::{Module, Call, Event<T>, Storage},
		SyloInbox: sylo_inbox::{Module, Call, Storage},
		SyloResponse: sylo_response::{Module, Call, Storage},
		SyloVault: sylo_vault::{Module, Call, Storage},
		CennzxSpot: crml_cennzx_spot::{Module, Call, Storage, Config<T>, Event<T>},
	}
);

/// The address format for describing accounts.
pub type Address = AccountId;
/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;
/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;
/// The `SignedExtension` payload for transactions in the plug runtime.
/// It can contain a doughnut delegation proof as it's second value.
pub type SignedExtra = (
	Option<CennznetDoughnut>,
	frame_system::CheckVersion<Runtime>,
	frame_system::CheckGenesis<Runtime>,
	frame_system::CheckEra<Runtime>,
	frame_system::CheckNonce<Runtime>,
	frame_system::CheckWeight<Runtime>,
	crml_transaction_payment::ChargeTransactionPayment<Runtime>,
	pallet_contracts::CheckBlockGasLimit<Runtime>,
);
/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic = generic::UncheckedExtrinsic<AccountId, Call, Signature, SignedExtra>;
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

		fn inherent_extrinsics(data: InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
			data.create_extrinsics()
		}

		fn check_inherents(block: Block, data: InherentData) -> CheckInherentsResult {
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

	impl fg_primitives::GrandpaApi<Block> for Runtime {
		fn grandpa_authorities() -> GrandpaAuthorityList {
			Grandpa::grandpa_authorities()
		}
	}

	impl sp_consensus_babe::BabeApi<Block> for Runtime {
		fn configuration() -> sp_consensus_babe::BabeConfiguration {
			// The choice of `c` parameter (where `1 - c` represents the
			// probability of a slot being empty), is done in accordance to the
			// slot duration and expected target block time, for safely
			// resisting network delays of maximum two seconds.
			// <https://research.web3.foundation/en/latest/polkadot/BABE/Babe/#6-practical-results>
			sp_consensus_babe::BabeConfiguration {
				slot_duration: Babe::slot_duration(),
				epoch_length: EpochDuration::get(),
				c: PRIMARY_PROBABILITY,
				genesis_authorities: Babe::authorities(),
				randomness: Babe::randomness(),
				secondary_slots: true,
			}
		}

		fn current_epoch_start() -> sp_consensus_babe::SlotNumber {
			Babe::current_epoch_start()
		}
	}

	impl sp_authority_discovery::AuthorityDiscoveryApi<Block> for Runtime {
		fn authorities() -> Vec<AuthorityDiscoveryId> {
			AuthorityDiscovery::authorities()
		}
	}

	impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index> for Runtime {
		fn account_nonce(account: AccountId) -> Index {
			System::account_nonce(account)
		}
	}

	impl pallet_contracts_rpc_runtime_api::ContractsApi<Block, AccountId, Balance, BlockNumber> for Runtime {
		fn call(
			origin: AccountId,
			dest: AccountId,
			value: Balance,
			gas_limit: u64,
			input_data: Vec<u8>,
		) -> ContractExecResult {
			let exec_result = Contracts::bare_call(
				origin,
				dest.into(),
				value,
				gas_limit,
				input_data,
				None,
			);
			match exec_result {
				Ok(v) => ContractExecResult::Success {
					status: v.status,
					data: v.data,
				},
				Err(_) => ContractExecResult::Error,
			}
		}

		fn get_storage(
			address: AccountId,
			key: [u8; 32],
		) -> pallet_contracts_primitives::GetStorageResult {
			Contracts::get_storage(address, key)
		}

		fn rent_projection(
			address: AccountId,
		) -> pallet_contracts_primitives::RentProjectionResult<BlockNumber> {
			Contracts::rent_projection(address)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<
		Block,
		Balance,
		UncheckedExtrinsic,
	> for Runtime {
		fn query_info(uxt: UncheckedExtrinsic, len: u32) -> RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_info(uxt, len)
		}
	}

	impl crml_cennzx_spot_rpc_runtime_api::CennzxSpotApi<
		Block,
		AssetId,
		Balance,
	> for Runtime {
		fn buy_price(
			buy_asset: AssetId,
			buy_amount: Balance,
			sell_asset: AssetId,
		) -> CennzxSpotResult<Balance> {
			let result = CennzxSpot::get_buy_price(buy_asset, buy_amount, sell_asset);
			match result {
				Ok(value) => CennzxSpotResult::Success(value),
				Err(_) => CennzxSpotResult::Error,
			}
		}

		fn sell_price(
			sell_asset: AssetId,
			sell_amount: Balance,
			buy_asset: AssetId,
		) -> CennzxSpotResult<Balance> {
			let result = CennzxSpot::get_sell_price(sell_asset, sell_amount, buy_asset);
			match result {
				Ok(value) => CennzxSpotResult::Success(value),
				Err(_) => CennzxSpotResult::Error,
			}
		}
	}

	impl sp_session::SessionKeys<Block> for Runtime {
		fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
			SessionKeys::generate(seed)
		}

		fn decode_session_keys(
			encoded: Vec<u8>,
		) -> Option<Vec<(Vec<u8>, sp_core::crypto::KeyTypeId)>> {
			SessionKeys::decode_into_raw_public_keys(&encoded)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_system::offchain::{SignAndSubmitTransaction, SubmitSignedTransaction};

	#[test]
	fn validate_transaction_submitter_bounds() {
		fn is_submit_signed_transaction<T>()
		where
			T: SubmitSignedTransaction<Runtime, Call>,
		{
		}

		fn is_sign_and_submit_transaction<T>()
		where
			T: SignAndSubmitTransaction<
				Runtime,
				Call,
				Extrinsic = UncheckedExtrinsic,
				CreateTransaction = Runtime,
				Signer = ImOnlineId,
			>,
		{
		}

		is_submit_signed_transaction::<SubmitTransaction>();
		is_sign_and_submit_transaction::<SubmitTransaction>();
	}
}
