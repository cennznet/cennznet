// Copyright 2018-2019 Parity Technologies (UK) Ltd. and Centrality Investments Ltd.
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

use authority_discovery_primitives::{AuthorityId as EncodedAuthorityId, Signature as EncodedSignature};
use babe_primitives::{AuthorityId as BabeId, AuthoritySignature as BabeSignature};
use cennznet_primitives::types::{
	AccountId, AccountIndex, AssetId, Balance, BlockNumber, Doughnut, Hash, Index, Moment, Signature,
};
use cennznut::{CENNZnut, Domain, Validate, ValidationErr};
use client::{
	block_builder::api::{self as block_builder_api, CheckInherentsResult, InherentData},
	impl_runtime_apis, runtime_api as client_api,
};
use codec::{Decode, Encode};
use generic_asset::{SpendingAssetCurrency, StakingAssetCurrency};
use grandpa::fg_primitives;
use grandpa::{AuthorityId as GrandpaId, AuthorityWeight as GrandpaWeight};
use im_online::sr25519::AuthorityId as ImOnlineId;
use primitives::u32_trait::{_1, _2, _3, _4};
use primitives::OpaqueMetadata;
use rstd::prelude::*;
use sr_primitives::curve::PiecewiseLinear;
use sr_primitives::traits::{
	self, BlakeTwo256, Block as BlockT, DoughnutApi, NumberFor, SaturatedConversion, StaticLookup,
};
use sr_primitives::transaction_validity::TransactionValidity;
use sr_primitives::weights::Weight;
use sr_primitives::{create_runtime_str, generic, impl_opaque_keys, key_types, ApplyResult, Perbill, Permill};
use support::{additional_traits, construct_runtime, parameter_types, traits::Randomness};
use system::offchain::TransactionSubmitter;
#[cfg(any(feature = "std", test))]
use version::NativeVersion;
use version::RuntimeVersion;

pub use cennzx_spot::{ExchangeAddressGenerator, FeeRate, PerMilli, PerMillion};
pub use contracts::Gas;
pub use generic_asset::Call as GenericAssetCall;

#[cfg(any(feature = "std", test))]
pub use sr_primitives::BuildStorage;
pub use staking::StakerStatus;
pub use support::StorageValue;
pub use timestamp::Call as TimestampCall;

pub use sylo::device as sylo_device;
pub use sylo::e2ee as sylo_e2ee;
pub use sylo::groups as sylo_groups;
pub use sylo::inbox as sylo_inbox;
pub use sylo::response as sylo_response;
pub use sylo::vault as sylo_vault;

/// Implementations of some helper traits passed into runtime modules as associated types.
pub mod impls;
use impls::{CurrencyToVoteHandler, FeeMultiplierUpdateHandler, WeightToFee};

/// Constant values used within the runtime.
pub mod constants;
use constants::{currency::*, time::*};

mod doughnut;

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
	spec_version: 26,
	impl_version: 26,
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

type CennznetDoughnut = prml_doughnut::PlugDoughnut<Doughnut, Runtime>;

impl system::Trait for Runtime {
	type Origin = Origin;
	type Call = Call;
	type Index = Index;
	type BlockNumber = BlockNumber;
	type Hash = Hash;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = Indices;
	type Header = generic::Header<BlockNumber, BlakeTwo256>;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type Doughnut = CennznetDoughnut;
	type DelegatedDispatchVerifier = prml_doughnut::PlugDoughnutDispatcher<Runtime>;
	type MaximumBlockWeight = MaximumBlockWeight;
	type MaximumBlockLength = MaximumBlockLength;
	type AvailableBlockRatio = AvailableBlockRatio;
	type Version = Version;
}

impl prml_doughnut::DoughnutRuntime for Runtime {
	type AccountId = <Self as system::Trait>::AccountId;
	type Call = Call;
	type Doughnut = <Self as system::Trait>::Doughnut;
	type TimestampProvider = timestamp::Module<Runtime>;
}

impl utility::Trait for Runtime {
	type Event = Event;
	type Call = Call;
}

impl cennzx_spot::Trait for Runtime {
	type Call = Call;
	type Event = Event;
	type ExchangeAddressGenerator = ExchangeAddressGenerator<Self>;
	type BalanceToUnsignedInt = Balance;
	type UnsignedIntToBalance = Balance;
}

impl attestation::Trait for Runtime {
	type Event = Event;
}

impl sylo::groups::Trait for Runtime {}
impl sylo::e2ee::Trait for Runtime {
	type Event = Event;
}
impl sylo::device::Trait for Runtime {
	type Event = Event;
}
impl sylo::response::Trait for Runtime {}
impl sylo::inbox::Trait for Runtime {}
impl sylo::vault::Trait for Runtime {}

parameter_types! {
	pub const EpochDuration: u64 = EPOCH_DURATION_IN_SLOTS;
	pub const ExpectedBlockTime: Moment = MILLISECS_PER_BLOCK;
}
impl babe::Trait for Runtime {
	type EpochDuration = EpochDuration;
	type ExpectedBlockTime = ExpectedBlockTime;
	type EpochChangeTrigger = babe::ExternalTrigger;
}

impl indices::Trait for Runtime {
	type AccountIndex = AccountIndex;
	type IsDeadAccount = ();
	type ResolveHint = indices::SimpleResolveHint<Self::AccountId, Self::AccountIndex>;
	type Event = Event;
}

impl generic_asset::Trait for Runtime {
	type Balance = Balance;
	type AssetId = AssetId;
	type Event = Event;
}

parameter_types! {
	pub const TransactionBaseFee: Balance = 1 * CENTS;
	pub const TransactionByteFee: Balance = 10 * MILLICENTS;
}

impl transaction_payment::Trait for Runtime {
	type Balance = Balance;
	type AssetId = AssetId;
	type Currency = SpendingAssetCurrency<Self>;
	type OnTransactionPayment = ();
	type TransactionBaseFee = TransactionBaseFee;
	type TransactionByteFee = TransactionByteFee;
	type WeightToFee = WeightToFee;
	type FeeMultiplierUpdate = FeeMultiplierUpdateHandler;
	type BuyFeeAsset = CennzxSpot;
}

parameter_types! {
	pub const MinimumPeriod: Moment = SLOT_DURATION / 2;
}
impl timestamp::Trait for Runtime {
	type Moment = Moment;
	type OnTimestampSet = Babe;
	type MinimumPeriod = MinimumPeriod;
}

parameter_types! {
	pub const UncleGenerations: BlockNumber = 5;
}

impl authorship::Trait for Runtime {
	type FindAuthor = session::FindAccountFromAuthorIndex<Self, Babe>;
	type UncleGenerations = UncleGenerations;
	type FilterUncle = ();
	type EventHandler = Staking;
}

type SessionHandlers = (Grandpa, Babe, ImOnline, AuthorityDiscovery);

impl_opaque_keys! {
	pub struct SessionKeys {
		#[id(key_types::GRANDPA)]
		pub grandpa: GrandpaId,
		#[id(key_types::BABE)]
		pub babe: BabeId,
		#[id(key_types::IM_ONLINE)]
		pub im_online: ImOnlineId,
	}
}

// NOTE: `SessionHandler` and `SessionKeys` are co-dependent: One key will be used for each handler.
// The number and order of items in `SessionHandler` *MUST* be the same number and order of keys in
// `SessionKeys`.
// TODO: Introduce some structure to tie these together to make it a bit less of a footgun. This
// should be easy, since OneSessionHandler trait provides the `Key` as an associated type. #2858
parameter_types! {
	pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(17);
}

impl session::Trait for Runtime {
	type OnSessionEnding = Staking;
	type SessionHandler = SessionHandlers;
	type ShouldEndSession = Babe;
	type Event = Event;
	type Keys = SessionKeys;
	type ValidatorId = <Self as system::Trait>::AccountId;
	type ValidatorIdOf = staking::StashOf<Self>;
	type SelectInitialValidators = Staking;
	type DisabledValidatorsThreshold = DisabledValidatorsThreshold;
}

impl session::historical::Trait for Runtime {
	type FullIdentification = staking::Exposure<AccountId, Balance>;
	type FullIdentificationOf = staking::ExposureOf<Runtime>;
}

srml_staking_reward_curve::build! {
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
	pub const SessionsPerEra: sr_staking_primitives::SessionIndex = 6;
	pub const BondingDuration: staking::EraIndex = 24 * 28;
	pub const RewardCurve: &'static PiecewiseLinear<'static> = &REWARD_CURVE;
}

impl staking::Trait for Runtime {
	type Currency = StakingAssetCurrency<Self>;
	type RewardCurrency = SpendingAssetCurrency<Self>;
	type CurrencyToReward = Balance;
	type Time = Timestamp;
	type CurrencyToVote = CurrencyToVoteHandler;
	type OnRewardMinted = Treasury;
	type Event = Event;
	type Slash = Treasury; // send the slashed funds to the treasury.
	type Reward = (); // rewards are minted from the void
	type SessionsPerEra = SessionsPerEra;
	type BondingDuration = BondingDuration;
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
}

impl democracy::Trait for Runtime {
	type Proposal = Call;
	type Event = Event;
	type Currency = StakingAssetCurrency<Self>;
	type EnactmentPeriod = EnactmentPeriod;
	type LaunchPeriod = LaunchPeriod;
	type VotingPeriod = VotingPeriod;
	type EmergencyVotingPeriod = EmergencyVotingPeriod;
	type MinimumDeposit = MinimumDeposit;
	/// A straight majority of the council can decide what their next motion is.
	type ExternalOrigin = collective::EnsureProportionAtLeast<_1, _2, AccountId, CouncilCollective>;
	/// A super-majority can have the next scheduled referendum be a straight majority-carries vote.
	type ExternalMajorityOrigin = collective::EnsureProportionAtLeast<_3, _4, AccountId, CouncilCollective>;
	/// A unanimous council can have the next scheduled referendum be a straight default-carries
	/// (NTB) vote.
	type ExternalDefaultOrigin = collective::EnsureProportionAtLeast<_1, _1, AccountId, CouncilCollective>;
	/// Two thirds of the technical committee can have an ExternalMajority/ExternalDefault vote
	/// be tabled immediately and with a shorter voting/enactment period.
	type FastTrackOrigin = collective::EnsureProportionAtLeast<_2, _3, AccountId, TechnicalCollective>;
	// To cancel a proposal which has been passed, 2/3 of the council must agree to it.
	type CancellationOrigin = collective::EnsureProportionAtLeast<_2, _3, AccountId, CouncilCollective>;
	// Any single technical committee member may veto a coming council proposal, however they can
	// only do it once and it lasts only for the cooloff period.
	type VetoOrigin = collective::EnsureMember<AccountId, TechnicalCollective>;
	type CooloffPeriod = CooloffPeriod;
}

type CouncilCollective = collective::Instance1;
impl collective::Trait<CouncilCollective> for Runtime {
	type Origin = Origin;
	type Proposal = Call;
	type Event = Event;
}

parameter_types! {
	pub const CandidacyBond: Balance = 10 * DOLLARS;
	pub const VotingBond: Balance = 1 * DOLLARS;
}

impl elections_phragmen::Trait for Runtime {
	type Event = Event;
	type Currency = StakingAssetCurrency<Self>;
	type CurrencyToVote = CurrencyToVoteHandler;
	type CandidacyBond = CandidacyBond;
	type VotingBond = VotingBond;
	type LoserCandidate = ();
	type BadReport = ();
	type KickedMember = ();
	type ChangeMembers = Council;
}

type TechnicalCollective = collective::Instance2;
impl collective::Trait<TechnicalCollective> for Runtime {
	type Origin = Origin;
	type Proposal = Call;
	type Event = Event;
}

impl membership::Trait<membership::Instance1> for Runtime {
	type Event = Event;
	type AddOrigin = collective::EnsureProportionMoreThan<_1, _2, AccountId, CouncilCollective>;
	type RemoveOrigin = collective::EnsureProportionMoreThan<_1, _2, AccountId, CouncilCollective>;
	type SwapOrigin = collective::EnsureProportionMoreThan<_1, _2, AccountId, CouncilCollective>;
	type ResetOrigin = collective::EnsureProportionMoreThan<_1, _2, AccountId, CouncilCollective>;
	type MembershipInitialized = TechnicalCommittee;
	type MembershipChanged = TechnicalCommittee;
}

parameter_types! {
	pub const ProposalBond: Permill = Permill::from_percent(5);
	pub const ProposalBondMinimum: Balance = 1 * DOLLARS;
	pub const SpendPeriod: BlockNumber = 1 * DAYS;
	pub const Burn: Permill = Permill::from_percent(50);
}

impl treasury::Trait for Runtime {
	type Currency = StakingAssetCurrency<Self>;
	type ApproveOrigin = collective::EnsureMembers<_4, AccountId, CouncilCollective>;
	type RejectOrigin = collective::EnsureMembers<_2, AccountId, CouncilCollective>;
	type Event = Event;
	type MintedForSpending = ();
	type ProposalRejection = ();
	type ProposalBond = ProposalBond;
	type ProposalBondMinimum = ProposalBondMinimum;
	type SpendPeriod = SpendPeriod;
	type Burn = Burn;
}

parameter_types! {
	pub const ContractTransferFee: Balance = 1 * CENTS;
	pub const ContractCreationFee: Balance = 1 * CENTS;
	pub const ContractTransactionBaseFee: Balance = 1 * CENTS;
	pub const ContractTransactionByteFee: Balance = 10 * MILLICENTS;
	pub const ContractFee: Balance = 1 * CENTS;
	pub const TombstoneDeposit: Balance = 1 * DOLLARS;
	pub const RentByteFee: Balance = 1 * DOLLARS;
	pub const RentDepositOffset: Balance = 1000 * DOLLARS;
	pub const SurchargeReward: Balance = 150 * DOLLARS;
}

impl contracts::Trait for Runtime {
	type Currency = SpendingAssetCurrency<Self>;
	type Time = Timestamp;
	type Randomness = RandomnessCollectiveFlip;
	type Call = Call;
	type Event = Event;
	type DetermineContractAddress = contracts::SimpleAddressDeterminator<Runtime>;
	type ComputeDispatchFee = contracts::DefaultDispatchFeeComputor<Runtime>;
	type TrieIdGenerator = contracts::TrieIdFromParentCounter<Runtime>;
	type GasPayment = ();
	type RentPayment = ();
	type SignedClaimHandicap = contracts::DefaultSignedClaimHandicap;
	type TombstoneDeposit = TombstoneDeposit;
	type StorageSizeOffset = contracts::DefaultStorageSizeOffset;
	type RentByteFee = RentByteFee;
	type RentDepositOffset = RentDepositOffset;
	type SurchargeReward = SurchargeReward;
	type TransferFee = ContractTransferFee;
	type CreationFee = ContractCreationFee;
	type TransactionBaseFee = ContractTransactionBaseFee;
	type TransactionByteFee = ContractTransactionByteFee;
	type ContractFee = ContractFee;
	type CallBaseFee = contracts::DefaultCallBaseFee;
	type InstantiateBaseFee = contracts::DefaultInstantiateBaseFee;
	type MaxDepth = contracts::DefaultMaxDepth;
	type MaxValueSize = contracts::DefaultMaxValueSize;
	type BlockGasLimit = contracts::DefaultBlockGasLimit;
}

impl sudo::Trait for Runtime {
	type Event = Event;
	type Proposal = Call;
}

type SubmitTransaction = TransactionSubmitter<ImOnlineId, Runtime, UncheckedExtrinsic>;

impl im_online::Trait for Runtime {
	type AuthorityId = ImOnlineId;
	type Call = Call;
	type Event = Event;
	type SubmitTransaction = SubmitTransaction;
	type ReportUnresponsiveness = Offences;
}

impl offences::Trait for Runtime {
	type Event = Event;
	type IdentificationTuple = session::historical::IdentificationTuple<Self>;
	type OnOffenceHandler = Staking;
}

impl authority_discovery::Trait for Runtime {
	type AuthorityId = BabeId;
}

impl grandpa::Trait for Runtime {
	type Event = Event;
}

parameter_types! {
	pub const WindowSize: BlockNumber = 101;
	pub const ReportLatency: BlockNumber = 1000;
}

impl finality_tracker::Trait for Runtime {
	type OnFinalizationStalled = Grandpa;
	type WindowSize = WindowSize;
	type ReportLatency = ReportLatency;
}

impl system::offchain::CreateTransaction<Runtime, UncheckedExtrinsic> for Runtime {
	type Signature = Signature;

	fn create_transaction<F: system::offchain::Signer<AccountId, Self::Signature>>(
		call: Call,
		account: AccountId,
		index: Index,
	) -> Option<(Call, <UncheckedExtrinsic as traits::Extrinsic>::SignaturePayload)> {
		let period = 1 << 8;
		let current_block = System::block_number().saturated_into::<u64>();
		let tip = 0;
		let extra: SignedExtra = (
			None,
			system::CheckVersion::<Runtime>::new(),
			system::CheckGenesis::<Runtime>::new(),
			system::CheckEra::<Runtime>::from(generic::Era::mortal(period, current_block)),
			system::CheckNonce::<Runtime>::from(index),
			system::CheckWeight::<Runtime>::new(),
			transaction_payment::ChargeTransactionPayment::<Runtime>::from(tip, None),
			Default::default(),
		);
		let raw_payload = SignedPayload::new(call, extra).ok()?;
		let signature = F::sign(account.clone(), &raw_payload)?;
		let address = Indices::unlookup(account);
		let (call, extra, _) = raw_payload.deconstruct();
		Some((call, (address, signature, extra)))
	}
}

/// Verify a Doughnut proof authorizes method dispatch given some input parameters
impl additional_traits::DelegatedDispatchVerifier<CennznetDoughnut> for Runtime {
	const DOMAIN: &'static str = "cennznet";

	fn verify_dispatch(doughnut: &CennznetDoughnut, module: &str, method: &str) -> Result<(), &'static str> {
		let mut domain = doughnut
			.get_domain(Self::DOMAIN)
			.ok_or("CENNZnut does not grant permission for cennznet domain")?;
		let cennznut: CENNZnut = Decode::decode(&mut domain).map_err(|_| "Bad CENNZnut encoding")?;

		// Extract Module name from <prefix>-<Module_name>
		let module_offset = module.find('-').ok_or("error during module name segmentation")? + 1;
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
		System: system::{Module, Call, Storage, Config, Event},
		Utility: utility::{Module, Call, Event},
		Babe: babe::{Module, Call, Storage, Config, Inherent(Timestamp)},
		Timestamp: timestamp::{Module, Call, Storage, Inherent},
		Authorship: authorship::{Module, Call, Storage, Inherent},
		Attestation: attestation::{Module, Call, Storage, Event<T>},
		Indices: indices,
		TransactionPayment: transaction_payment::{Module, Storage},
		GenericAsset: generic_asset::{Module, Call, Storage, Event<T>, Config<T>, Fee},
		Staking: staking::{default, OfflineWorker},
		Session: session::{Module, Call, Storage, Event, Config<T>},
		Democracy: democracy::{Module, Call, Storage, Config, Event<T>},
		Council: collective::<Instance1>::{Module, Call, Storage, Origin<T>, Event<T>, Config<T>},
		TechnicalCommittee: collective::<Instance2>::{Module, Call, Storage, Origin<T>, Event<T>, Config<T>},
		Elections: elections_phragmen::{Module, Call, Storage, Event<T>, Config<T>},
		TechnicalMembership: membership::<Instance1>::{Module, Call, Storage, Event<T>, Config<T>},
		FinalityTracker: finality_tracker::{Module, Call, Inherent},
		Grandpa: grandpa::{Module, Call, Storage, Config, Event},
		Treasury: treasury::{Module, Call, Storage, Event<T>},
		Contracts: contracts,
		Sudo: sudo,
		ImOnline: im_online::{Module, Call, Storage, Event<T>, ValidateUnsigned, Config<T>},
		AuthorityDiscovery: authority_discovery::{Module, Call, Config<T>},
		Offences: offences::{Module, Call, Storage, Event},
		RandomnessCollectiveFlip: randomness_collective_flip::{Module, Call, Storage},
		SyloGroups: sylo_groups::{Module, Call, Storage},
		SyloE2EE: sylo_e2ee::{Module, Call, Event<T>, Storage},
		SyloDevice: sylo_device::{Module, Call, Event<T>, Storage},
		SyloInbox: sylo_inbox::{Module, Call, Storage},
		SyloResponse: sylo_response::{Module, Call, Storage},
		SyloVault: sylo_vault::{Module, Call, Storage},
		CennzxSpot: cennzx_spot::{Module, Call, Storage, Config<T>, Event<T>},
	}
);

/// The address format for describing accounts.
pub type Address = <Indices as StaticLookup>::Source;
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
	system::CheckVersion<Runtime>,
	system::CheckGenesis<Runtime>,
	system::CheckEra<Runtime>,
	system::CheckNonce<Runtime>,
	system::CheckWeight<Runtime>,
	transaction_payment::ChargeTransactionPayment<Runtime>,
	contracts::CheckBlockGasLimit<Runtime>,
);
/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic = generic::UncheckedExtrinsic<Address, Call, Signature, SignedExtra>;
/// The payload being signed in transactions.
pub type SignedPayload = generic::SignedPayload<Call, SignedExtra>;
/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, Call, SignedExtra>;
/// Executive: handles dispatch to the various modules.
pub type Executive = executive::Executive<Runtime, Block, system::ChainContext<Runtime>, Runtime, AllModules>;

impl_runtime_apis! {
	impl client_api::Core<Block> for Runtime {
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

	impl client_api::Metadata<Block> for Runtime {
		fn metadata() -> OpaqueMetadata {
			Runtime::metadata().into()
		}
	}

	impl block_builder_api::BlockBuilder<Block> for Runtime {
		fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyResult {
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

	impl client_api::TaggedTransactionQueue<Block> for Runtime {
		fn validate_transaction(tx: <Block as BlockT>::Extrinsic) -> TransactionValidity {
			Executive::validate_transaction(tx)
		}
	}

	impl offchain_primitives::OffchainWorkerApi<Block> for Runtime {
		fn offchain_worker(number: NumberFor<Block>) {
			Executive::offchain_worker(number)
		}
	}

	impl fg_primitives::GrandpaApi<Block> for Runtime {
		fn grandpa_authorities() -> Vec<(GrandpaId, GrandpaWeight)> {
			Grandpa::grandpa_authorities()
		}
	}

	impl babe_primitives::BabeApi<Block> for Runtime {
		fn configuration() -> babe_primitives::BabeConfiguration {
			// The choice of `c` parameter (where `1 - c` represents the
			// probability of a slot being empty), is done in accordance to the
			// slot duration and expected target block time, for safely
			// resisting network delays of maximum two seconds.
			// <https://research.web3.foundation/en/latest/polkadot/BABE/Babe/#6-practical-results>
			babe_primitives::BabeConfiguration {
				slot_duration: Babe::slot_duration(),
				epoch_length: EpochDuration::get(),
				c: PRIMARY_PROBABILITY,
				genesis_authorities: Babe::authorities(),
				randomness: Babe::randomness(),
				secondary_slots: true,
			}
		}
	}

	impl authority_discovery_primitives::AuthorityDiscoveryApi<Block> for Runtime {
		fn authorities() -> Vec<EncodedAuthorityId> {
			AuthorityDiscovery::authorities().into_iter()
				.map(|id| id.encode())
				.map(EncodedAuthorityId)
				.collect()
		}

		fn sign(payload: &Vec<u8>) -> Option<(EncodedSignature, EncodedAuthorityId)> {
			AuthorityDiscovery::sign(payload).map(|(sig, id)| {
				(EncodedSignature(sig.encode()), EncodedAuthorityId(id.encode()))
			})
		}

		fn verify(payload: &Vec<u8>, signature: &EncodedSignature, authority_id: &EncodedAuthorityId) -> bool {
			let signature = match BabeSignature::decode(&mut &signature.0[..]) {
				Ok(s) => s,
				_ => return false,
			};

			let authority_id = match BabeId::decode(&mut &authority_id.0[..]) {
				Ok(id) => id,
				_ => return false,
			};

			AuthorityDiscovery::verify(payload, signature, authority_id)
		}
	}

	impl system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index> for Runtime {
		fn account_nonce(account: AccountId) -> Index {
			System::account_nonce(account)
		}
	}

	impl contracts_rpc_runtime_api::ContractsApi<Block, AccountId, Balance> for Runtime {
		fn call(
			origin: AccountId,
			dest: AccountId,
			value: Balance,
			gas_limit: u64,
			input_data: Vec<u8>,
		) -> contracts_rpc_runtime_api::ContractExecResult {
			use contracts_rpc_runtime_api::ContractExecResult;

			let exec_result = Contracts::bare_call(
				origin,
				dest.into(),
				value,
				gas_limit,
				input_data,
			);
			match exec_result {
				Ok(v) => ContractExecResult::Success {
					status: v.status,
					data: v.data,
				},
				Err(_) => ContractExecResult::Error,
			}
		}
	}

	impl substrate_session::SessionKeys<Block> for Runtime {
		fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
			let seed = seed.as_ref().map(|s| rstd::str::from_utf8(&s).expect("Seed is an utf8 string"));
			SessionKeys::generate(seed)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use sr_primitives::app_crypto::RuntimeAppPublic;
	use system::offchain::SubmitSignedTransaction;

	fn is_submit_signed_transaction<T, Signer>(_arg: T)
	where
		T: SubmitSignedTransaction<
			Runtime,
			Call,
			Extrinsic = UncheckedExtrinsic,
			CreateTransaction = Runtime,
			Signer = Signer,
		>,
		Signer: RuntimeAppPublic + From<AccountId>,
		Signer::Signature: Into<Signature>,
	{
	}

	#[test]
	fn validate_bounds() {
		let x = SubmitTransaction::default();
		is_submit_signed_transaction(x);
	}
}
