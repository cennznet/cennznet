//! The CENNZnet runtime. This can be compiled with ``#[no_std]`, ready for Wasm.

#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit.
#![recursion_limit = "512"]

use cennznet_primitives::{
	AccountId, AccountIndex, AuthorityId, AuthoritySignature, Balance, BlockNumber, CennznetExtrinsic, Hash, Index,
	Signature,
};
#[cfg(feature = "std")]
use council::seats as council_seats;
use council::{motions as council_motions, voting as council_voting};
use grandpa::fg_primitives::{self, ScheduledChange};
use rstd::prelude::*;
use runtime_primitives::traits::{
	AuthorityIdFor, BlakeTwo256, Block as BlockT, Checkable, Convert, DigestFor, NumberFor, StaticLookup,
};
use runtime_primitives::transaction_validity::TransactionValidity;
use runtime_primitives::{create_runtime_str, generic, ApplyResult};
use substrate_client::impl_runtime_apis;
use substrate_client::{
	block_builder::api::{self as block_builder_api, CheckInherentsResult, InherentData},
	runtime_api as client_api,
};
use substrate_primitives::OpaqueMetadata;
use support::construct_runtime;
use support::traits::Currency;

#[cfg(any(feature = "std", test))]
use version::NativeVersion;
use version::RuntimeVersion;

use generic_asset::{SpendingAssetCurrency, StakingAssetCurrency};

pub use consensus::Call as ConsensusCall;
#[cfg(any(feature = "std", test))]
pub use runtime_primitives::BuildStorage;
pub use runtime_primitives::{Perbill, Permill};
pub use staking::StakerStatus;
pub use support::StorageValue;
pub use timestamp::Call as TimestampCall;

pub use cennzx_spot::{ExchangeAddressGenerator, FeeRate};
pub use contract::Schedule;
pub use sylo::device as sylo_device;
pub use sylo::e2ee as sylo_e2ee;
pub use sylo::groups as sylo_groups;
pub use sylo::inbox as sylo_inbox;
pub use sylo::response as sylo_response;
pub use sylo::vault as sylo_vault;

mod fee;

/// Runtime version.
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: create_runtime_str!("cennznet"),
	impl_name: create_runtime_str!("centrality-cennznet"),
	authoring_version: 1,
	spec_version: 24,
	impl_version: 24,
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

pub struct CurrencyToVoteHandler;

impl CurrencyToVoteHandler {
	fn factor() -> u128 {
		(<StakingAssetCurrency<Runtime>>::total_issuance() / u64::max_value() as u128).max(1)
	}
}

impl Convert<u128, u64> for CurrencyToVoteHandler {
	fn convert(x: u128) -> u64 {
		(x / Self::factor()) as u64
	}
}

impl Convert<u128, u128> for CurrencyToVoteHandler {
	fn convert(x: u128) -> u128 {
		x * Self::factor()
	}
}

impl system::Trait for Runtime {
	type Origin = Origin;
	type Index = Index;
	type BlockNumber = BlockNumber;
	type Hash = Hash;
	type Hashing = BlakeTwo256;
	type Digest = generic::Digest<Log>;
	type AccountId = AccountId;
	type Lookup = Indices;
	type Header = generic::Header<BlockNumber, BlakeTwo256, Log>;
	type Event = Event;
	type Log = Log;
}

impl aura::Trait for Runtime {
	type HandleReport = aura::StakingSlasher<Runtime>;
}

impl consensus::Trait for Runtime {
	type Log = Log;
	type SessionKey = AuthorityId;

	// the aura module handles offline-reports internally
	// rather than using an explicit report system.
	type InherentOfflineReport = ();
}

impl indices::Trait for Runtime {
	type AccountIndex = AccountIndex;
	type IsDeadAccount = ();
	type ResolveHint = indices::SimpleResolveHint<Self::AccountId, Self::AccountIndex>;
	type Event = Event;
}

impl timestamp::Trait for Runtime {
	type Moment = u64;
	type OnTimestampSet = Aura;
}

impl session::Trait for Runtime {
	type ConvertAccountIdToSessionKey = ();
	type OnSessionChange = (Staking, grandpa::SyncedAuthorities<Runtime>);
	type Event = Event;
}

impl staking::Trait for Runtime {
	type Currency = StakingAssetCurrency<Self>;
	type RewardCurrency = SpendingAssetCurrency<Self>;
	type CurrencyToReward = Balance;
	type CurrencyToVote = CurrencyToVoteHandler;
	type OnRewardMinted = ();
	type Event = Event;
	type Slash = ();
	type Reward = ();
}

impl democracy::Trait for Runtime {
	type Currency = StakingAssetCurrency<Self>;
	type Proposal = Call;
	type Event = Event;
}

impl council::Trait for Runtime {
	type Event = Event;
	type BadPresentation = ();
	type BadReaper = ();
}

impl council::voting::Trait for Runtime {
	type Event = Event;
}

impl council::motions::Trait for Runtime {
	type Origin = Origin;
	type Proposal = Call;
	type Event = Event;
}

impl contract::Trait for Runtime {
	type Currency = SpendingAssetCurrency<Self>;
	type Call = Call;
	type Event = Event;
	type Gas = u64;
	type DetermineContractAddress = contract::SimpleAddressDeterminator<Runtime>;
	type ComputeDispatchFee = contract::DefaultDispatchFeeComputor<Runtime>;
	type TrieIdGenerator = contract::TrieIdFromParentCounter<Runtime>;
	type GasPayment = ();
}

impl sudo::Trait for Runtime {
	type Event = Event;
	type Proposal = Call;
}

impl grandpa::Trait for Runtime {
	type Log = Log;
	type SessionKey = AuthorityId;
	type Event = Event;
}

impl generic_asset::Trait for Runtime {
	type Balance = Balance;
	type AssetId = u32;
	type Event = Event;
}

impl fees::Trait for Runtime {
	type Event = Event;
	type Currency = SpendingAssetCurrency<Self>;
	type BuyFeeAsset = cennzx_spot::Module<Self>;
	type OnFeeCharged = rewards::Module<Self>;
	type Fee = Fee;
}

impl rewards::Trait for Runtime {}

impl cennzx_spot::Trait for Runtime {
	type Call = Call;
	type Event = Event;
	type ExchangeAddressGenerator = ExchangeAddressGenerator<Self>;
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

construct_runtime!(
	pub enum Runtime with Log(InternalLog: DigestItem<Hash, AuthorityId, AuthoritySignature>) where
		Block = Block,
		NodeBlock = cennznet_primitives::Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		System: system::{default, Log(ChangesTrieRoot)},
		Aura: aura::{Module, Inherent(Timestamp)},
		Timestamp: timestamp::{Module, Call, Storage, Config<T>, Inherent},
		GenericAsset: generic_asset::{Module, Call, Storage, Config<T>, Event<T>, Fee},
		Consensus: consensus::{Module, Call, Storage, Config<T>, Log(AuthoritiesChange), Inherent},
		Indices: indices,
		Session: session,
		Staking: staking,
		Democracy: democracy,
		Council: council::{Module, Call, Storage, Event<T>},
		CouncilVoting: council_voting,
		CouncilMotions: council_motions::{Module, Call, Storage, Event<T>, Origin},
		CouncilSeats: council_seats::{Config<T>},
		Grandpa: grandpa::{Module, Call, Storage, Config<T>, Log(), Event<T>},
		Contract: contract::{Module, Call, Storage, Config<T>, Event<T>},
		Sudo: sudo,
		Fees: fees::{Module, Call, Fee, Storage, Config<T>, Event<T>},
		Rewards: rewards::{Module, Call, Storage, Config<T>},
		Attestation: attestation::{Module, Call, Storage, Event<T>},
		CennzxSpot: cennzx_spot::{Module, Call, Storage, Config<T>, Event<T>},
		SyloGroups: sylo_groups::{Module, Call, Storage},
		SyloE2EE: sylo_e2ee::{Module, Call, Event<T>, Storage},
		SyloDevice: sylo_device::{Module, Call, Event<T>, Storage},
		SyloInbox: sylo_inbox::{Module, Call, Storage},
		SyloResponse: sylo_response::{Module, Call, Storage},
		SyloVault: sylo_vault::{Module, Call, Storage},
	}
);

pub type Address = <Indices as StaticLookup>::Source;
/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256, Log>;
/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;
/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;
/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic = CennznetExtrinsic<AccountId, Address, Index, Call, Signature, Balance>;
/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = <<Block as BlockT>::Extrinsic as Checkable<system::ChainContext<Runtime>>>::Checked;
/// A type that handles payment for extrinsic fees
pub type ExtrinsicFeePayment = fee::ExtrinsicFeeCharger;
/// Executive: handles dispatch to the various modules.
pub type Executive =
	executive::Executive<Runtime, Block, system::ChainContext<Runtime>, ExtrinsicFeePayment, AllModules>;

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

		fn authorities() -> Vec<AuthorityIdFor<Block>> {
			panic!("Deprecated, please use `AuthoritiesApi`.")
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
			System::random_seed()
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
		fn grandpa_pending_change(digest: &DigestFor<Block>)
			-> Option<ScheduledChange<NumberFor<Block>>>
		{
			for log in digest.logs.iter().filter_map(|l| match l {
				Log(InternalLog::grandpa(grandpa_signal)) => Some(grandpa_signal),
				_=> None
			}) {
				if let Some(change) = Grandpa::scrape_digest_change(log) {
					return Some(change);
				}
			}
			None
		}

		fn grandpa_forced_change(digest: &DigestFor<Block>)
			-> Option<(NumberFor<Block>, ScheduledChange<NumberFor<Block>>)>
		{
			for log in digest.logs.iter().filter_map(|l| match l {
				Log(InternalLog::grandpa(grandpa_signal)) => Some(grandpa_signal),
				_ => None
			}) {
				if let Some(change) = Grandpa::scrape_digest_forced_change(log) {
					return Some(change);
				}
			}
			None
		}

		fn grandpa_authorities() -> Vec<(AuthorityId, u64)> {
			Grandpa::grandpa_authorities()
		}
	}

	impl consensus_aura::AuraApi<Block> for Runtime {
		fn slot_duration() -> u64 {
			Aura::slot_duration()
		}
	}

	impl consensus_authorities::AuthoritiesApi<Block> for Runtime {
		fn authorities() -> Vec<AuthorityIdFor<Block>> {
			Consensus::authorities()
		}
	}
}
