// Copyright 2017-2021 Parity Technologies (UK) Ltd. and Centrality Investments Ltd.
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

//! # Staking Module
//!
//! The Staking module is used to manage funds at stake by network maintainers.
//!
//! - [`staking::Config`](./trait.Config.html)
//! - [`Call`](./enum.Call.html)
//! - [`Module`](./struct.Module.html)
//!
//! ## Overview
//!
//! The Staking module is the means by which a set of network maintainers (known as _authorities_
//! in some contexts and _validators_ in others) are chosen based upon those who voluntarily place
//! funds under deposit. Under deposit, those funds are rewarded under normal operation but are
//! held at pain of _slash_ (expropriation) should the staked maintainer be found not to be
//! discharging its duties properly.
//!
//! ### Terminology
//! <!-- Original author of paragraph: @gavofyork -->
//!
//! - Staking: The process of locking up funds for some time, placing them at risk of slashing
//! (loss) in order to become a rewarded maintainer of the network.
//! - Validating: The process of running a node to actively maintain the network, either by
//! producing blocks or guaranteeing finality of the chain.
//! - Nominating: The process of placing staked funds behind one or more validators in order to
//! share in any reward, and punishment, they take.
//! - Stash account: The account holding an owner's funds used for staking.
//! - Controller account: The account that controls an owner's funds for staking.
//! - Era: A (whole) number of sessions, which is the period that the validator set (and each
//! validator's active nominator set) is recalculated and where rewards are paid out.
//! - Slash: The punishment of a staker by reducing its funds.
//!
//! ### Goals
//! <!-- Original author of paragraph: @gavofyork -->
//!
//! The staking system in Substrate NPoS is designed to make the following possible:
//!
//! - Stake funds that are controlled by a cold wallet.
//! - Withdraw some, or deposit more, funds without interrupting the role of an entity.
//! - Switch between roles (nominator, validator, idle) with minimal overhead.
//!
//! ### Scenarios
//!
//! #### Staking
//!
//! Almost any interaction with the Staking module requires a process of _**bonding**_ (also known
//! as being a _staker_). To become *bonded*, a fund-holding account known as the _stash account_,
//! which holds some or all of the funds that become frozen in place as part of the staking process,
//! is paired with an active **controller** account, which issues instructions on how they shall be
//! used.
//!
//! An account pair can become bonded using the [`bond`](./enum.Call.html#variant.bond) call.
//!
//! Stash accounts can change their associated controller using the
//! [`set_controller`](./enum.Call.html#variant.set_controller) call.
//!
//! There are three possible roles that any staked account pair can be in: `Validator`, `Nominator`
//! and `Idle` (defined in [`StakerStatus`](./enum.StakerStatus.html)). There are three
//! corresponding instructions to change between roles, namely:
//! [`validate`](./enum.Call.html#variant.validate), [`nominate`](./enum.Call.html#variant.nominate),
//! and [`chill`](./enum.Call.html#variant.chill).
//!
//! #### Validating
//!
//! A **validator** takes the role of either validating blocks or ensuring their finality,
//! maintaining the veracity of the network. A validator should avoid both any sort of malicious
//! misbehavior and going offline. Bonded accounts that state interest in being a validator do NOT
//! get immediately chosen as a validator. Instead, they are declared as a _candidate_ and they
//! _might_ get elected at the _next era_ as a validator. The result of the election is determined
//! by nominators and their votes.
//!
//! An account can become a validator candidate via the
//! [`validate`](./enum.Call.html#variant.validate) call.
//!
//! #### Nomination
//!
//! A **nominator** does not take any _direct_ role in maintaining the network, instead, it votes on
//! a set of validators  to be elected. Once interest in nomination is stated by an account, it
//! takes effect at the next election round. The funds in the nominator's stash account indicate the
//! _weight_ of its vote. Both the rewards and any punishment that a validator earns are shared
//! between the validator and its nominators. This rule incentivizes the nominators to NOT vote for
//! the misbehaving/offline validators as much as possible, simply because the nominators will also
//! lose funds if they vote poorly.
//!
//! An account can become a nominator via the [`nominate`](enum.Call.html#variant.nominate) call.
//!
//! #### Rewards and Slash
//!
//! The **reward and slashing** procedure is the core of the Staking module, attempting to _embrace
//! valid behavior_ while _punishing any misbehavior or lack of availability_.
//!
//! Slashing can occur at any point in time, once misbehavior is reported. Once slashing is
//! determined, a value is deducted from the balance of the validator and all the nominators who
//! voted for this validator (values are deducted from the _stash_ account of the slashed entity).
//!
//! Slashing logic is further described in the documentation of the `slashing` module.
//!
//! Similar to slashing, rewards are also shared among a validator and its associated nominators.
//! Yet, the reward funds are not always transferred to the stash account and can be configured.
//! See [Reward Calculation](#reward-calculation) for more details.
//!
//! #### Chilling
//!
//! Finally, any of the roles above can choose to step back temporarily and just chill for a while.
//! This means that if they are a nominator, they will not be considered as voters anymore and if
//! they are validators, they will no longer be a candidate for the next election.
//!
//! An account can step back via the [`chill`](enum.Call.html#variant.chill) call.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! The dispatchable functions of the Staking module enable the steps needed for entities to accept
//! and change their role, alongside some helper functions to get/set the metadata of the module.
//!
//! ### Public Functions
//!
//! The Staking module contains many public storage items and (im)mutable functions.
//!
//! ## Usage
//!
//! ## Implementation Details
//!
//! ### Slot Stake
//!
//! The term [`SlotStake`](./struct.Module.html#method.slot_stake) will be used throughout this
//! section. It refers to a value calculated at the end of each era, containing the _minimum value
//! at stake among all validators._ Note that a validator's value at stake might be a combination
//! of the validator's own stake and the votes it received. See [`Exposure`](./struct.Exposure.html)
//! for more details.
//!
//! ### Reward Calculation
//!
//! The validator and its nominator split their reward as following:
//!
//! The validator can declare an amount, named
//! [`commission`](./struct.ValidatorPrefs.html#structfield.commission), that does not
//! get shared with the nominators at each reward payout through its
//! [`ValidatorPrefs`](./struct.ValidatorPrefs.html). This value gets deducted from the total reward
//! that is paid to the validator and its nominators. The remaining portion is split among the
//! validator and all of the nominators that nominated the validator, proportional to the value
//! staked behind this validator (_i.e._ dividing the
//! [`own`](./struct.Exposure.html#structfield.own) or
//! [`others`](./struct.Exposure.html#structfield.others) by
//! [`total`](./struct.Exposure.html#structfield.total) in [`Exposure`](./struct.Exposure.html)).
//!
//! All entities who receive a reward have the option to choose their reward destination
//! through the [`Payee`](./struct.Payee.html) storage item (see
//! [`set_payee`](enum.Call.html#variant.set_payee)), to be one of the following:
//!
//! - Controller account, (obviously) not increasing the staked value.
//! - Stash account, not increasing the staked value.
//! - Stash account, also increasing the staked value.
//!
//! ### Additional Fund Management Operations
//!
//! Any funds already placed into stash can be the target of the following operations:
//!
//! The controller account can free a portion (or all) of the funds using the
//! [`unbond`](enum.Call.html#variant.unbond) call. Note that the funds are not immediately
//! accessible. Instead, a duration denoted by [`BondingDuration`](./struct.BondingDuration.html)
//! (in number of eras) must pass until the funds can actually be removed. Once the
//! `BondingDuration` is over, the [`withdraw_unbonded`](./enum.Call.html#variant.withdraw_unbonded)
//! call can be used to actually withdraw the funds.
//!
//! Note that there is a limitation to the number of fund-chunks that can be scheduled to be
//! unlocked in the future via [`unbond`](enum.Call.html#variant.unbond). In case this maximum
//! (`MAX_UNLOCKING_CHUNKS`) is reached, the bonded account _must_ first wait until a successful
//! call to `withdraw_unbonded` to remove some of the chunks.
//!
//! ### Election Algorithm
//!
//! The current election algorithm is implemented based on Phragmén.
//! The reference implementation can be found
//! [here](https://github.com/w3f/consensus/tree/master/NPoS).
//!
//! The election algorithm, aside from electing the validators with the most stake value and votes,
//! tries to divide the nominator votes among candidates in an equal manner. To further assure this,
//! an optional post-processing can be applied that iteratively normalizes the nominator staked
//! values until the total difference among votes of a particular nominator are less than a
//! threshold.
//!
//! ## GenesisConfig
//!
//! The Staking module depends on the [`GenesisConfig`](./struct.GenesisConfig.html).
//!
//! ## Related Modules
//!
//! - [Balances](../pallet_balances/index.html): Used to manage values at stake.
//! - [Session](../pallet_session/index.html): Used to manage sessions. Also, a list of new validators
//! is stored in the Session module's `Validators` at the end of each era.

#![recursion_limit = "128"]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

mod migration;
mod offchain_election;
pub mod rewards;
pub use rewards::{HandlePayee, OnEndEra, RewardCalculation};

mod slashing;
pub use slashing::REWARD_F1;

use codec::{Decode, Encode, HasCompact};
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage,
	dispatch::{DispatchErrorWithPostInfo, DispatchResult, DispatchResultWithPostInfo, WithPostDispatchInfo},
	ensure,
	traits::{
		Currency, CurrencyToVote, EstimateNextNewSession, Get, IsSubType, LockIdentifier, LockableCurrency,
		OnUnbalanced, UnixTime, WithdrawReasons,
	},
	weights::{
		constants::{WEIGHT_PER_MICROS, WEIGHT_PER_NANOS},
		Weight,
	},
	IterableStorageMap,
};
use frame_system::{self as system, ensure_none, ensure_root, ensure_signed, offchain::SendTransactionTypes};
use pallet_session::historical;
use pallet_staking::WeightInfo;
use sp_npos_elections::{
	generate_solution_type, is_score_better, seq_phragmen, to_supports, Assignment, CompactSolution,
	ElectionResult as PrimitiveElectionResult, ElectionScore, EvaluateSupport, ExtendedBalance, PerThing128, Supports,
	VoteWeight,
};
use sp_runtime::{
	traits::{AtLeast32Bit, CheckedSub, Convert, Dispatchable, Saturating, Zero},
	transaction_validity::{
		InvalidTransaction, TransactionPriority, TransactionSource, TransactionValidity, TransactionValidityError,
		ValidTransaction,
	},
	DispatchError, InnerOf, PerU16, Perbill, RuntimeDebug, SaturatedConversion,
};
#[cfg(feature = "std")]
use sp_runtime::{Deserialize, Serialize};
use sp_staking::{
	offence::{Offence, OffenceDetails, OffenceError, OnOffenceHandler, ReportOffence},
	SessionIndex,
};
use sp_std::{collections::btree_set::BTreeSet, convert::TryInto, iter::FromIterator, mem::size_of, prelude::*, vec};

const STAKING_ID: LockIdentifier = *b"staking ";
const MAX_UNLOCKING_CHUNKS: usize = 32;
pub const MAX_NOMINATIONS: usize = <CompactAssignments as CompactSolution>::LIMIT;

pub(crate) const LOG_TARGET: &'static str = "staking";

// syntactic sugar for logging.
#[macro_export]
macro_rules! log {
	($level:tt, $patter:expr $(, $values:expr)* $(,)?) => {
		log::$level!(
			target: crate::LOG_TARGET,
			$patter $(, $values)*
		)
	};
}

// Note: Maximum nomination limit is set here -- 16.
generate_solution_type!(
	#[compact]
	pub struct CompactAssignments::<NominatorIndex, ValidatorIndex, OffchainAccuracy>(16)
);

/// Data type used to index nominators in the compact type
pub type NominatorIndex = u32;

/// Data type used to index validators in the compact type.
pub type ValidatorIndex = u16;

// Ensure the size of both ValidatorIndex and NominatorIndex. They both need to be well below usize.
static_assertions::const_assert!(size_of::<ValidatorIndex>() <= size_of::<usize>());
static_assertions::const_assert!(size_of::<NominatorIndex>() <= size_of::<usize>());
static_assertions::const_assert!(size_of::<ValidatorIndex>() <= size_of::<u32>());
static_assertions::const_assert!(size_of::<NominatorIndex>() <= size_of::<u32>());

/// Maximum number of stakers that can be stored in a snapshot.
pub(crate) const MAX_VALIDATORS: usize = ValidatorIndex::max_value() as usize;
pub(crate) const MAX_NOMINATORS: usize = NominatorIndex::max_value() as usize;

/// Counter for the number of eras that have passed.
pub type EraIndex = u32;

/// Accuracy used for on-chain election.
pub type ChainAccuracy = Perbill;

/// Accuracy used for off-chain election. This better be small.
pub type OffchainAccuracy = PerU16;

/// The result of an election round.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct ElectionResult<AccountId, Balance: HasCompact> {
	/// Flat list of validators who have been elected.
	elected_stashes: Vec<AccountId>,
	/// Flat list of new exposures, to be updated in the [`Exposure`] storage.
	exposures: Vec<(AccountId, Exposure<AccountId, Balance>)>,
	/// Type of the result. This is kept on chain only to track and report the best score's
	/// submission type. An optimisation could remove this.
	compute: ElectionCompute,
}

/// The status of the upcoming (offchain) election.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum ElectionStatus<BlockNumber> {
	/// Nothing has and will happen for now. submission window is not open.
	Closed,
	/// The submission window has been open since the contained block number.
	Open(BlockNumber),
}

/// Some indications about the size of the election. This must be submitted with the solution.
///
/// Note that these values must reflect the __total__ number, not only those that are present in the
/// solution. In short, these should be the same size as the size of the values dumped in
/// `SnapshotValidators` and `SnapshotNominators`.
#[derive(PartialEq, Eq, Clone, Copy, Encode, Decode, RuntimeDebug, Default)]
pub struct ElectionSize {
	/// Number of validators in the snapshot of the current election round.
	#[codec(compact)]
	pub validators: ValidatorIndex,
	/// Number of nominators in the snapshot of the current election round.
	#[codec(compact)]
	pub nominators: NominatorIndex,
}

impl<BlockNumber: PartialEq> ElectionStatus<BlockNumber> {
	fn is_open_at(&self, n: BlockNumber) -> bool {
		*self == Self::Open(n)
	}

	fn is_closed(&self) -> bool {
		match self {
			Self::Closed => true,
			_ => false,
		}
	}

	fn is_open(&self) -> bool {
		!self.is_closed()
	}
}

impl<BlockNumber> Default for ElectionStatus<BlockNumber> {
	fn default() -> Self {
		Self::Closed
	}
}

/// Indicates the initial status of a staker (used by genesis config only).
#[derive(RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum StakerStatus<AccountId> {
	/// Chilling.
	Idle,
	/// Declared desire in validating or already participating in it.
	Validator,
	/// Nominating for a group of other stakers.
	Nominator(Vec<AccountId>),
}

/// A destination account for payment.
#[derive(PartialEq, Eq, Copy, Clone, Encode, Decode, RuntimeDebug)]
pub enum RewardDestination<AccountId> {
	/// Pay into the stash account, not increasing the amount at stake.
	Stash,
	/// Pay into the controller account.
	Controller,
	/// Pay into a specified account.
	Account(AccountId),
}

impl<AccountId> Default for RewardDestination<AccountId> {
	fn default() -> Self {
		RewardDestination::Stash
	}
}

/// Preference of what happens regarding validation.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct ValidatorPrefs {
	/// Reward that validator takes up-front; only the rest is split between themselves and
	/// nominators.
	#[codec(compact)]
	pub commission: Perbill,
}

impl Default for ValidatorPrefs {
	fn default() -> Self {
		ValidatorPrefs {
			commission: Default::default(),
		}
	}
}

/// Just a Balance/BlockNumber tuple to encode when a chunk of funds will be unlocked.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct UnlockChunk<Balance: HasCompact> {
	/// Amount of funds to be unlocked.
	#[codec(compact)]
	value: Balance,
	/// Era number at which point it'll be unlocked.
	#[codec(compact)]
	era: EraIndex,
}

/// The ledger of a (bonded) stash.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct StakingLedger<AccountId, Balance: HasCompact> {
	/// The stash account whose balance is actually locked and at stake.
	pub stash: AccountId,
	/// The total amount of the stash's balance (`active` plus any `unlocking` balances).
	#[codec(compact)]
	pub total: Balance,
	/// The amount of the stash's balance that will be at stake in any forthcoming
	/// eras. i.e it will affect voting power in the next election.
	/// It could lessen in the current round after unbonding.
	#[codec(compact)]
	pub active: Balance,
	/// Any balance that has been unbonded and becoming free, which may eventually be transferred out
	/// of the stash (assuming it doesn't get slashed first).
	pub unlocking: Vec<UnlockChunk<Balance>>,
}

impl<AccountId, Balance: HasCompact + Copy + Saturating + AtLeast32Bit> StakingLedger<AccountId, Balance> {
	/// The amount of stash's funds slashable as of right now.
	/// It should remain slashable until the bonding duration expires and it is withdrawn.
	fn slashable_balance(&self) -> Balance {
		self.total
	}
	/// Remove entries from `unlocking` that are sufficiently old and reduce the
	/// total by the sum of their balances.
	fn consolidate_unlocked(self, current_era: EraIndex) -> Self {
		let mut total = self.total;
		let unlocking = self
			.unlocking
			.into_iter()
			.filter(|chunk| {
				if chunk.era > current_era {
					true
				} else {
					total = total.saturating_sub(chunk.value);
					false
				}
			})
			.collect();
		Self {
			total,
			active: self.active,
			stash: self.stash,
			unlocking,
		}
	}

	/// Re-bond funds that were scheduled for unlocking.
	fn rebond(mut self, value: Balance) -> Self {
		let mut rebonded_total: Balance = Zero::zero();

		while let Some(last) = self.unlocking.last_mut() {
			let remaining = value - rebonded_total;

			if last.value <= remaining {
				rebonded_total += last.value;
				self.active += last.value;
				self.unlocking.pop();
			} else {
				rebonded_total += remaining;
				self.active += remaining;
				last.value -= remaining;
			}

			if rebonded_total >= value {
				break;
			}
		}

		self
	}
}

impl<AccountId, Balance> StakingLedger<AccountId, Balance>
where
	Balance: AtLeast32Bit + Saturating + Copy,
{
	/// Slash the validator for a given amount of balance. This can grow the value
	/// of the slash in the case that the validator has less than `minimum_balance`
	/// active funds. Returns the amount of funds actually slashed.
	///
	/// Slashes from `active` funds first, and then `unlocking`, starting with the
	/// chunks that are closest to unlocking.
	fn slash(&mut self, mut value: Balance, minimum_balance: Balance) -> Balance {
		let total_before_slash = self.total;
		let total_after_slash = &mut self.slashable_balance();
		let active = &mut self.active;

		value = Self::apply_slash(total_after_slash, active, value, minimum_balance);

		let i = self
			.unlocking
			.iter_mut()
			.map(|chunk| {
				value = Self::apply_slash(total_after_slash, &mut chunk.value, value, minimum_balance);
				chunk.value
			})
			.take_while(|value| value.is_zero()) // take all fully-consumed chunks out.
			.count();

		// kill all drained chunks.
		let _ = self.unlocking.drain(..i);

		self.total = *total_after_slash;
		// return the slashed amount
		total_before_slash.saturating_sub(*total_after_slash)
	}

	/// Apply slash to a target set of funds
	///
	/// Ensures dust isn't left in the balance of the `target_funds`
	/// Returns the remainder of `slash` if the full `slash` was not applied
	fn apply_slash(
		total_funds: &mut Balance,
		target_funds: &mut Balance,
		slash: Balance,
		minimum_balance: Balance,
	) -> Balance {
		let slash_from_target = (slash).min(*target_funds);
		let mut slash_remainder = slash;

		if !slash_from_target.is_zero() {
			slash_remainder = slash - slash_from_target;
			*total_funds = total_funds.saturating_sub(slash_from_target);
			*target_funds -= slash_from_target;

			// don't leave a dust balance in the staking system.
			if *target_funds <= minimum_balance {
				*total_funds = total_funds.saturating_sub(*target_funds);
				*target_funds = Zero::zero();
			}
		}
		slash_remainder
	}
}

/// A record of the nominations made by a specific account.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct Nominations<AccountId> {
	/// The targets of nomination.
	pub targets: Vec<AccountId>,
	/// The era the nominations were submitted.
	pub submitted_in: EraIndex,
}

/// The amount of exposure (to slashing) than an individual nominator has.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Encode, Decode, RuntimeDebug)]
pub struct IndividualExposure<AccountId, Balance: HasCompact> {
	/// The stash account of the nominator in question.
	who: AccountId,
	/// Amount of funds exposed.
	#[codec(compact)]
	value: Balance,
}

impl<AccountId, Balance: HasCompact> IndividualExposure<AccountId, Balance> {
	pub fn new(who: AccountId, value: Balance) -> Self {
		Self { who, value }
	}
}

/// A snapshot of the stake backing a single validator in the system.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct Exposure<AccountId, Balance: HasCompact> {
	/// The total balance backing this validator.
	#[codec(compact)]
	pub total: Balance,
	/// The validator's own stash that is exposed.
	#[codec(compact)]
	pub own: Balance,
	/// The portions of nominators stashes that are exposed.
	pub others: Vec<IndividualExposure<AccountId, Balance>>,
}

/// A pending slash record. The value of the slash has been computed but not applied yet,
/// rather deferred for several eras.
#[derive(Encode, Decode, Default, RuntimeDebug)]
pub struct UnappliedSlash<AccountId, Balance: HasCompact> {
	/// The stash ID of the offending validator.
	validator: AccountId,
	/// The validator's own slash.
	own: Balance,
	/// All other slashed stakers and amounts.
	others: Vec<(AccountId, Balance)>,
	/// Reporters of the offence; bounty payout recipients.
	reporters: Vec<AccountId>,
	/// The amount of payout.
	payout: Balance,
}

/// Indicate how an election round was computed.
#[derive(PartialEq, Eq, Clone, Copy, Encode, Decode, RuntimeDebug)]
pub enum ElectionCompute {
	/// Result was forcefully computed on chain at the end of the session.
	OnChain,
	/// Result was submitted and accepted to the chain via a signed transaction.
	Signed,
	/// Result was submitted and accepted to the chain via an unsigned transaction (by an
	/// authority).
	Unsigned,
}

pub type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as system::Config>::AccountId>>::Balance;
type NegativeImbalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;

/// Information regarding the active era (era in used in session).
#[derive(Encode, Decode, RuntimeDebug)]
pub struct ActiveEraInfo {
	/// Index of era.
	pub index: EraIndex,
	/// Moment of start expressed as millisecond from `$UNIX_EPOCH`.
	///
	/// Start can be none if start hasn't been set for the era yet,
	/// Start is set on the first on_finalize of the era to guarantee usage of `Time`.
	start: Option<u64>,
}

/// Means for interacting with a specialized version of the `session` trait.
///
/// This is needed because `Staking` sets the `ValidatorIdOf` of the `pallet_session::Config`
pub trait SessionInterface<AccountId>: frame_system::Config {
	/// Disable a given validator by stash ID.
	///
	/// Returns `true` if new era should be forced at the end of this session.
	/// This allows preventing a situation where there is too many validators
	/// disabled and block production stalls.
	fn disable_validator(validator: &AccountId) -> Result<bool, ()>;
	/// Get the validators from session.
	fn validators() -> Vec<AccountId>;
	/// Prune historical session tries up to but not including the given index.
	fn prune_historical_up_to(up_to: SessionIndex);
}

impl<T: Config> SessionInterface<<T as frame_system::Config>::AccountId> for T
where
	T: pallet_session::Config<ValidatorId = <T as frame_system::Config>::AccountId>,
	T: pallet_session::historical::Config<
		FullIdentification = Exposure<<T as frame_system::Config>::AccountId, BalanceOf<T>>,
		FullIdentificationOf = ExposureOf<T>,
	>,
	T::SessionHandler: pallet_session::SessionHandler<<T as frame_system::Config>::AccountId>,
	T::SessionManager: pallet_session::SessionManager<<T as frame_system::Config>::AccountId>,
	T::ValidatorIdOf: Convert<<T as frame_system::Config>::AccountId, Option<<T as frame_system::Config>::AccountId>>,
{
	fn disable_validator(validator: &<T as frame_system::Config>::AccountId) -> Result<bool, ()> {
		<pallet_session::Module<T>>::disable(validator)
	}

	fn validators() -> Vec<<T as frame_system::Config>::AccountId> {
		<pallet_session::Module<T>>::validators()
	}

	fn prune_historical_up_to(up_to: SessionIndex) {
		<pallet_session::historical::Module<T>>::prune_up_to(up_to);
	}
}

pub trait Config: frame_system::Config + SendTransactionTypes<Call<Self>> {
	/// The staking balance.
	type Currency: LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;

	/// Time used for computing era duration.
	///
	/// It is guaranteed to start being called from the first `on_finalize`. Thus value at genesis
	/// is not used.
	type UnixTime: UnixTime;

	/// The overarching call type.
	type Call: Dispatchable + From<Call<Self>> + IsSubType<Call<Self>> + Clone;

	/// Convert a balance into a number used for election calculation. This must fit into a `u64`
	/// but is allowed to be sensibly lossy. The `u64` is used to communicate with the
	/// [`sp_npos_elections`] crate which accepts u64 numbers and does operations in 128.
	/// Consequently, the backward convert is used convert the u128s from sp-elections back to a
	/// [`BalanceOf`].
	type CurrencyToVote: CurrencyToVote<BalanceOf<Self>>;

	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;

	/// Handler for the unbalanced reduction when slashing a staker.
	type Slash: OnUnbalanced<NegativeImbalanceOf<Self>>;

	/// Number of sessions per era.
	type SessionsPerEra: Get<SessionIndex>;

	/// Something that can estimate the next session change, accurately or as a best effort guess.
	type NextNewSession: EstimateNextNewSession<Self::BlockNumber>;

	/// Number of eras that staked funds must remain bonded for.
	type BondingDuration: Get<EraIndex>;

	/// Number of eras that slashes are deferred by, after computation. This
	/// should be less than the bonding duration. Set to 0 if slashes should be
	/// applied immediately, without opportunity for intervention.
	type SlashDeferDuration: Get<EraIndex>;

	/// Interface for interacting with a session module.
	type SessionInterface: self::SessionInterface<Self::AccountId>;

	/// Handles payout for validator rewards
	type Rewarder: HandlePayee<AccountId = Self::AccountId>
		+ OnEndEra<AccountId = Self::AccountId>
		+ RewardCalculation<AccountId = Self::AccountId, Balance = BalanceOf<Self>>;

	/// The number of blocks before the end of the era from which election submissions are allowed.
	///
	/// Setting this to zero will disable the offchain compute and only on-chain seq-phragmen will
	/// be used.
	///
	/// This is bounded by being within the last session. Hence, setting it to a value more than the
	/// length of a session will be pointless.
	type ElectionLookahead: Get<Self::BlockNumber>;

	/// Maximum number of balancing iterations to run in the offchain submission.
	///
	/// If set to 0, balance_solution will not be executed at all.
	type MaxIterations: Get<u32>;

	/// The threshold of improvement that should be provided for a new solution to be accepted.
	type MinSolutionScoreBump: Get<Perbill>;

	/// The maximum number of nominators rewarded for each validator.
	///
	/// For each validator only the `$MaxNominatorRewardedPerValidator` biggest stakers can claim
	/// their reward. This used to limit the i/o cost for the nominator payout.
	type MaxNominatorRewardedPerValidator: Get<u32>;

	/// A configuration for base priority of unsigned transactions.
	///
	/// This is exposed so that it can be tuned for particular runtime, when
	/// multiple pallets send unsigned transactions.
	type UnsignedPriority: Get<TransactionPriority>;

	/// Maximum weight that the unsigned transaction can have.
	///
	/// Chose this value with care. On one hand, it should be as high as possible, so the solution
	/// can contain as many nominators/validators as possible. On the other hand, it should be small
	/// enough to fit in the block.
	type OffchainSolutionWeightLimit: Get<Weight>;

	/// Extrinsic weight info
	type WeightInfo: WeightInfo;
}

/// Mode of era-forcing.
#[derive(Copy, Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum Forcing {
	/// Not forcing anything - just let whatever happen.
	NotForcing,
	/// Force a new era, then reset to `NotForcing` as soon as it is done.
	ForceNew,
	/// Avoid a new era indefinitely.
	ForceNone,
	/// Force a new era at the end of all sessions indefinitely.
	ForceAlways,
}

impl Default for Forcing {
	fn default() -> Self {
		Forcing::NotForcing
	}
}

// A value placed in storage that represents the current version of the Staking storage. This value
// is used by the `on_runtime_upgrade` logic to determine whether we run storage migration logic.
// This should match directly with the semantic versions of the Rust crate.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug)]
enum Releases {
	/// storage version pre-runtime v38
	V0 = 0,
	/// storage version runtime v38
	V1 = 1,
	/// storage version runtime v39
	V2 = 2,
}

impl Default for Releases {
	fn default() -> Self {
		Releases::V2
	}
}

impl From<u32> for Releases {
	fn from(val: u32) -> Self {
		match val {
			0 => Releases::V0,
			1 => Releases::V1,
			2 | _ => Releases::V2,
		}
	}
}

decl_storage! {
	trait Store for Module<T: Config> as Staking {
		/// Minimum amount to bond.
		MinimumBond get(fn minimum_bond) config(): BalanceOf<T>;

		/// Number of eras to keep in history.
		///
		/// Information is kept for eras in `[current_era - history_depth; current_era]`.
		///
		/// Must be more than the number of eras delayed by session otherwise. I.e. active era must
		/// always be in history. I.e. `active_era > current_era - history_depth` must be
		/// guaranteed.
		HistoryDepth get(fn history_depth) config(): u32 = 32;

		/// The ideal number of staking participants.
		pub ValidatorCount get(fn validator_count) config(): u32;

		/// Minimum number of staking participants before emergency conditions are imposed.
		pub MinimumValidatorCount get(fn minimum_validator_count) config(): u32;

		/// Any validators that may never be slashed or forcibly kicked. It's a Vec since they're
		/// easy to initialize and the performance hit is minimal (we expect no more than four
		/// invulnerables) and restricted to testnets.
		pub Invulnerables get(fn invulnerables) config(): Vec<T::AccountId>;

		/// Map from all locked "stash" accounts to the controller account.
		pub Bonded get(fn bonded): map hasher(twox_64_concat) T::AccountId => Option<T::AccountId>;

		/// Map from all (unlocked) "controller" accounts to the info regarding the staking.
		pub Ledger get(fn ledger):
			map hasher(blake2_128_concat) T::AccountId
			=> Option<StakingLedger<T::AccountId, BalanceOf<T>>>;

		/// The map from (wannabe) validator stash key to the preferences of that validator.
		pub Validators get(fn validators):
		map hasher(twox_64_concat) T::AccountId => ValidatorPrefs;

		/// The map from nominator stash key to the set of stash keys of all validators to nominate.
		pub Nominators get(fn nominators):
			map hasher(twox_64_concat) T::AccountId => Option<Nominations<T::AccountId>>;

		/// The current era index.
		pub CurrentEra get(fn current_era): Option<EraIndex>;

		/// The active era information, it holds index and start.
		///
		/// The active era is the era currently rewarded.
		/// Validator set of this era must be equal to `SessionInterface::validators`.
		pub ActiveEra get(fn active_era): Option<ActiveEraInfo>;

		/// The session index at which the era start for the last `HISTORY_DEPTH` eras.
		pub ErasStartSessionIndex get(fn eras_start_session_index):
			map hasher(twox_64_concat) EraIndex => Option<SessionIndex>;

		/// Exposure of validator at era.
		///
		/// This is keyed first by the era index to allow bulk deletion and then the stash account.
		///
		/// Is it removed after `HISTORY_DEPTH` eras.
		/// If stakers hasn't been set or has been removed then empty exposure is returned.
		pub ErasStakers get(fn eras_stakers):
			double_map hasher(twox_64_concat) EraIndex, hasher(twox_64_concat) T::AccountId
			=> Exposure<T::AccountId, BalanceOf<T>>;

		/// Clipped Exposure of validator at era.
		///
		/// This is similar to [`ErasStakers`] but number of nominators exposed is reduced to the
		/// `T::MaxNominatorRewardedPerValidator` biggest stakers.
		/// (Note: the field `total` and `own` of the exposure remains unchanged).
		/// This is used to limit the i/o cost for the nominator payout.
		///
		/// This is keyed fist by the era index to allow bulk deletion and then the stash account.
		///
		/// Is it removed after `HISTORY_DEPTH` eras.
		/// If stakers hasn't been set or has been removed then empty exposure is returned.
		pub ErasStakersClipped get(fn eras_stakers_clipped):
			double_map hasher(twox_64_concat) EraIndex, hasher(twox_64_concat) T::AccountId
			=> Exposure<T::AccountId, BalanceOf<T>>;

		/// Similar to `ErasStakers`, this holds the preferences of validators.
		///
		/// This is keyed first by the era index to allow bulk deletion and then the stash account.
		///
		/// Is it removed after `HISTORY_DEPTH` eras.
		// If prefs hasn't been set or has been removed then 0 commission is returned.
		pub ErasValidatorPrefs get(fn eras_validator_prefs):
		double_map hasher(twox_64_concat) EraIndex, hasher(twox_64_concat) T::AccountId
		=> ValidatorPrefs;

		/// The total amount staked for the last `HISTORY_DEPTH` eras.
		/// If total hasn't been set or has been removed then 0 stake is returned.
		pub ErasTotalStake get(fn eras_total_stake):
			map hasher(twox_64_concat) EraIndex => BalanceOf<T>;

		/// True if the next session change will be a new era regardless of index.
		pub ForceEra get(fn force_era) config(): Forcing;

		/// The percentage of the slash that is distributed to reporters.
		///
		/// The rest of the slashed value is handled by the `Slash`.
		pub SlashRewardFraction get(fn slash_reward_fraction) config(): Perbill;

		/// All unapplied slashes that are queued for later.
		pub UnappliedSlashes:
			map hasher(twox_64_concat) EraIndex => Vec<UnappliedSlash<T::AccountId, BalanceOf<T>>>;

		/// A mapping from still-bonded eras to the first session index of that era.
		///
		/// Must contains information for eras for the range:
		/// `[active_era - bounding_duration; active_era]`
		BondedEras: Vec<(EraIndex, SessionIndex)>;

		/// All slashing events on validators, mapped by era to the highest slash proportion
		/// and slash value of the era.
		ValidatorSlashInEra:
			double_map hasher(twox_64_concat) EraIndex, hasher(twox_64_concat) T::AccountId
			=> Option<(Perbill, BalanceOf<T>)>;

		/// All slashing events on nominators, mapped by era to the highest slash value of the era.
		NominatorSlashInEra:
			double_map hasher(twox_64_concat) EraIndex, hasher(twox_64_concat) T::AccountId
			=> Option<BalanceOf<T>>;

		/// Slashing spans for stash accounts.
		SlashingSpans: map hasher(twox_64_concat) T::AccountId => Option<slashing::SlashingSpans>;

		/// Records information about the maximum slash of a stash within a slashing span,
		/// as well as how much reward has been paid out.
		SpanSlash:
			map hasher(twox_64_concat) (T::AccountId, slashing::SpanIndex)
			=> slashing::SpanRecord<BalanceOf<T>>;

		/// The earliest era for which we have a pending, unapplied slash.
		EarliestUnappliedSlash: Option<EraIndex>;

		/// Snapshot of validators at the beginning of the current election window. This should only
		/// have a value when [`EraElectionStatus`] == `ElectionStatus::Open(_)`.
		pub SnapshotValidators get(fn snapshot_validators): Option<Vec<T::AccountId>>;

		/// Snapshot of nominators at the beginning of the current election window. This should only
		/// have a value when [`EraElectionStatus`] == `ElectionStatus::Open(_)`.
		pub SnapshotNominators get(fn snapshot_nominators): Option<Vec<T::AccountId>>;

		/// The next validator set. At the end of an era, if this is available (potentially from the
		/// result of an offchain worker), it is immediately used. Otherwise, the on-chain election
		/// is executed.
		pub QueuedElected get(fn queued_elected): Option<ElectionResult<T::AccountId, BalanceOf<T>>>;

		/// The score of the current [`QueuedElected`].
		pub QueuedScore get(fn queued_score): Option<ElectionScore>;

		/// Flag to control the execution of the offchain election. When `Open(_)`, we accept
		/// solutions to be submitted.
		pub EraElectionStatus get(fn era_election_status): ElectionStatus<T::BlockNumber>;

		/// True if the next **planned** session is final (last in the era). Note that this does not take era
		/// forcing into account.
		pub IsPlannedSessionFinal get(fn is_planned_session_final): bool = false;

		/// True if the active session is final (last in the era). Note that this does not take era
		/// forcing into account
		pub IsCurrentSessionFinal get(fn is_current_session_final): bool = false;

		/// Same as `will_era_be_forced()` but persists to `end_era`
		WasEndEraForced get(fn was_end_era_forced): bool = false;

		/// True if network has been upgraded to this version.
		/// Storage version of the pallet.
		///
		/// This is set to v2 for new networks.
		StorageVersion build(|_: &GenesisConfig<T>| Releases::V2 as u32): u32;
	}
	add_extra_genesis {
		config(stakers):
			Vec<(T::AccountId, T::AccountId, BalanceOf<T>, StakerStatus<T::AccountId>)>;
		build(|config: &GenesisConfig<T>| {
			assert!(config.minimum_bond > Zero::zero(), "Minimum bond must be greater than zero.");
			assert!(config.history_depth > 1, "history depth must be greater than 1");
			for &(ref stash, ref controller, balance, ref status) in &config.stakers {
				assert!(
					T::Currency::free_balance(&stash) >= balance,
					"Stash does not have enough balance to bond."
				);
				let _ = <Module<T>>::bond(
					T::Origin::from(Some(stash.clone()).into()),
					controller.clone(),
					balance,
					RewardDestination::Stash,
				);
				let _ = match status {
					StakerStatus::Validator => {
						<Module<T>>::validate(
							T::Origin::from(Some(controller.clone()).into()),
							Default::default(),
						)
					},
					StakerStatus::Nominator(votes) => {
						<Module<T>>::nominate(
							T::Origin::from(Some(controller.clone()).into()),
							votes.to_vec(),
						)
					},
					_ => Ok(()),
				};
			}
		});
	}
}

decl_event!(
	pub enum Event<T> where Balance = BalanceOf<T>, <T as frame_system::Config>::AccountId {
		/// One validator (and its nominators) has been slashed by the given amount.
		Slash(AccountId, Balance),
		/// The validator is invulnerable, so it has NOT been slashed.
		InvulnerableNotSlashed(AccountId, Perbill),
		/// Minimum bond amount is changed.
		SetMinimumBond(Balance),
		/// An old slashing report from a prior era was discarded because it could
		/// not be processed.
		OldSlashingReportDiscarded(SessionIndex),
		/// A new set of stakers was elected with the given \[compute\].
		StakingElection(ElectionCompute),
		/// A new solution for the upcoming election has been stored. \[compute\]
		SolutionStored(ElectionCompute),
		/// A new set of validators are marked to be invulnerable
		SetInvulnerables(Vec<AccountId>),
		/// An account has bonded this amount. \[stash, amount\]
		///
		/// NOTE: This event is only emitted when funds are bonded via a dispatchable. Notably,
		/// it will not be emitted for staking rewards when they are added to stake.
		Bonded(AccountId, Balance),
		/// An account has unbonded this amount. \[stash, amount\]
		Unbonded(AccountId, Balance),
		/// An account has called `withdraw_unbonded` and removed unbonding chunks worth `Balance`
		/// from the unlocking queue. \[stash, amount\]
		Withdrawn(AccountId, Balance),
	}
);

decl_error! {
	/// Error for the staking module.
	pub enum Error for Module<T: Config> {
		/// Not a controller account.
		NotController,
		/// Not a stash account.
		NotStash,
		/// Stash is already bonded.
		AlreadyBonded,
		/// Controller is already paired.
		AlreadyPaired,
		/// Targets cannot be empty.
		EmptyTargets,
		/// Slash record index out of bounds.
		InvalidSlashIndex,
		/// Can not bond with value less than minimum balance.
		InsufficientBond,
		/// User does not have enough free balance to bond this amount
		InsufficientFreeBalance,
		/// Can not schedule more unlock chunks.
		NoMoreChunks,
		/// Can not rebond without unlocking chunks.
		NoUnlockChunk,
		/// Attempting to target a stash that still has funds.
		FundedTarget,
		/// Items are not sorted and unique.
		NotSortedAndUnique,
		/// Cannot nominate the same account multiple times
		DuplicateNominee,
		/// The submitted result is received out of the open window.
		OffchainElectionEarlySubmission,
		/// The submitted result is not as good as the one stored on chain.
		OffchainElectionWeakSubmission,
		/// The snapshot data of the current window is missing.
		SnapshotUnavailable,
		/// Incorrect number of winners were presented.
		OffchainElectionBogusWinnerCount,
		/// One of the submitted winners is not an active candidate on chain (index is out of range
		/// in snapshot).
		OffchainElectionBogusWinner,
		/// Error while building the assignment type from the compact. This can happen if an index
		/// is invalid, or if the weights _overflow_.
		OffchainElectionBogusCompact,
		/// One of the submitted nominators is not an active nominator on chain.
		OffchainElectionBogusNominator,
		/// One of the submitted nominators has an edge to which they have not voted on chain.
		OffchainElectionBogusNomination,
		/// One of the submitted nominators has an edge which is submitted before the last non-zero
		/// slash of the target.
		OffchainElectionSlashedNomination,
		/// A self vote must only be originated from a validator to ONLY themselves.
		OffchainElectionBogusSelfVote,
		/// The submitted result has unknown edges that are not among the presented winners.
		OffchainElectionBogusEdge,
		/// The claimed score does not match with the one computed from the data.
		OffchainElectionBogusScore,
		/// The election size is invalid.
		OffchainElectionBogusElectionSize,
		/// The call is not allowed at the given time due to restrictions of election period.
		CallNotAllowed,
		/// Incorrect previous history depth input provided.
		IncorrectHistoryDepth,
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		/// Number of sessions per era.
		const SessionsPerEra: SessionIndex = T::SessionsPerEra::get();

		/// Number of eras that staked funds must remain bonded for.
		const BondingDuration: EraIndex = T::BondingDuration::get();

		/// Number of eras that slashes are deferred by, after computation.
		///
		/// This should be less than the bonding duration.
		/// Set to 0 if slashes should be applied immediately, without opportunity for
		/// intervention.
		const SlashDeferDuration: EraIndex = T::SlashDeferDuration::get();

		/// The number of blocks before the end of the era from which election submissions are allowed.
		///
		/// Setting this to zero will disable the offchain compute and only on-chain seq-phragmen will
		/// be used.
		///
		/// This is bounded by being within the last session. Hence, setting it to a value more than the
		/// length of a session will be pointless.
		const ElectionLookahead: T::BlockNumber = T::ElectionLookahead::get();

		/// Maximum number of balancing iterations to run in the offchain submission.
		///
		/// If set to 0, balance_solution will not be executed at all.
		const MaxIterations: u32 = T::MaxIterations::get();

		/// The threshold of improvement that should be provided for a new solution to be accepted.
		const MinSolutionScoreBump: Perbill = T::MinSolutionScoreBump::get();

		/// The maximum number of nominators rewarded for each validator.
		///
		/// For each validator only the `$MaxNominatorRewardedPerValidator` biggest stakers can claim
		/// their reward. This used to limit the i/o cost for the nominator payout.
		const MaxNominatorRewardedPerValidator: u32 = T::MaxNominatorRewardedPerValidator::get();

		type Error = Error<T>;

		fn deposit_event() = default;

		fn on_runtime_upgrade() -> Weight {
			match StorageVersion::get().into() {
				// upgraded!
				Releases::V2 => Zero::zero(),
				// needs upgrade
				Releases::V1 => {
					migration::upgrade_v1_to_v2::<T>();
					T::BlockWeights::get().max_block
				},
				// won't occur, no live networks on this version...
				Releases::V0 => Zero::zero(),
			}
		}

		fn on_initialize(now: T::BlockNumber) -> Weight {
			let mut consumed_weight = 0;
			let mut add_weight = |reads, writes, weight| {
				consumed_weight += T::DbWeight::get().reads_writes(reads, writes);
				consumed_weight += weight;
			};
			if
				// if we don't have any ongoing offchain compute.
				Self::era_election_status().is_closed() &&
				// either current session final based on the plan, or we're forcing.
				(Self::is_current_session_final() || Self::will_era_be_forced())
			{
				if let Some(next_session_change) = T::NextNewSession::estimate_next_new_session(now) {
					if let Some(remaining) = next_session_change.checked_sub(&now) {
						if remaining <= T::ElectionLookahead::get() && !remaining.is_zero() {
							// create snapshot.
							let (did_snapshot, snapshot_weight) = Self::create_stakers_snapshot();
							add_weight(0, 0, snapshot_weight);
							if did_snapshot {
								// Set the flag to make sure we don't waste any compute here in the same era
								// after we have triggered the offline compute.
								<EraElectionStatus<T>>::put(
									ElectionStatus::<T::BlockNumber>::Open(now)
								);
								add_weight(0, 1, 0);
								log!(info, "💸 Election window is Open({:?}). Snapshot created", now);
							} else {
								log!(warn, "💸 Failed to create snapshot at {:?}.", now);
							}
						}
					}
				} else {
					log!(warn, "💸 Estimating next session change failed.");
				}
				add_weight(0, 0, T::NextNewSession::weight(now))
			}
			// For `era_election_status`, `is_current_session_final`, `will_era_be_forced`
			add_weight(3, 0, 0);
			// Additional read from `on_finalize`
			add_weight(1, 0, 0);

			consumed_weight
		}

		/// Check if the current block number is the one at which the election window has been set
		/// to open. If so, it runs the offchain worker code.
		fn offchain_worker(now: T::BlockNumber) {
			use offchain_election::{set_check_offchain_execution_status, compute_offchain_election};

			if Self::era_election_status().is_open_at(now) {
				let offchain_status = set_check_offchain_execution_status::<T>(now);
				if let Err(why) = offchain_status {
					log!(warn, "💸 skipping offchain worker in open election window due to [{}]", why);
				} else {
					if let Err(e) = compute_offchain_election::<T>() {
						log!(error, "💸 Error in election offchain worker: {:?}", e);
					} else {
						log!(debug, "💸 Executed offchain worker thread without errors.");
					}
				}
			}
		}

		fn on_finalize() {
			// Set the start of the first era.
			if let Some(mut active_era) = Self::active_era() {
				if active_era.start.is_none() {
					let now_as_millis_u64 = T::UnixTime::now().as_millis().saturated_into::<u64>();
					active_era.start = Some(now_as_millis_u64);
					// This write only ever happens once, we don't include it in the weight in general
					ActiveEra::put(active_era);
				}
			}
			// `on_finalize` weight is tracked in `on_initialize`
		}

		fn integrity_test() {
			sp_io::TestExternalities::new_empty().execute_with(||
				assert!(
					T::SlashDeferDuration::get() < T::BondingDuration::get() || T::BondingDuration::get() == 0,
					"As per documentation, slash defer duration ({}) should be less than bonding duration ({}).",
					T::SlashDeferDuration::get(),
					T::BondingDuration::get(),
				)
			);

			use sp_runtime::UpperOf;
			// see the documentation of `Assignment::try_normalize`. Now we can ensure that this
			// will always return `Ok`.
			// 1. Maximum sum of Vec<ChainAccuracy> must fit into `UpperOf<ChainAccuracy>`.
			assert!(
				<usize as TryInto<UpperOf<ChainAccuracy>>>::try_into(MAX_NOMINATIONS)
				.unwrap()
				.checked_mul(<ChainAccuracy>::one().deconstruct().try_into().unwrap())
				.is_some()
			);

			// 2. Maximum sum of Vec<OffchainAccuracy> must fit into `UpperOf<OffchainAccuracy>`.
			assert!(
				<usize as TryInto<UpperOf<OffchainAccuracy>>>::try_into(MAX_NOMINATIONS)
				.unwrap()
				.checked_mul(<OffchainAccuracy>::one().deconstruct().try_into().unwrap())
				.is_some()
			);
		}

		/// Take the origin account as a stash and lock up `value` of its balance. `controller` will
		/// be the account that controls it.
		///
		/// `value` must be more than the `minimum_bond` specified in genesis config.
		///
		/// The dispatch origin for this call must be _Signed_ by the stash account.
		///
		/// Emits `Bonded`.
		///
		/// # <weight>
		/// - Independent of the arguments. Moderate complexity.
		/// - O(1).
		/// - Three extra DB entries.
		///
		/// NOTE: Two of the storage writes (`Self::bonded`, `Self::payee`) are _never_ cleaned
		/// unless the `origin` falls below _existential deposit_ and gets removed as dust.
		/// ------------------
		/// Weight: O(1)
		/// DB Weight:
		/// - Read: Bonded, Ledger, [Origin Account], Locks
		/// - Write: Bonded, Payee, [Origin Account], Locks, Ledger
		/// # </weight>
		#[weight = T::WeightInfo::bond()]
		fn bond(origin,
			controller: T::AccountId,
			#[compact] value: BalanceOf<T>,
			payee: RewardDestination<T::AccountId>
		) {
			let stash = ensure_signed(origin)?;

			if <Bonded<T>>::contains_key(&stash) {
				Err(Error::<T>::AlreadyBonded)?
			}

			if <Ledger<T>>::contains_key(&controller) {
				Err(Error::<T>::AlreadyPaired)?
			}

			// reject a bond which is considered to be _dust_.
			if value < Self::minimum_bond() {
				Err(Error::<T>::InsufficientBond)?
			}

			// You're auto-bonded forever, here. We might improve this by only bonding when
			// you actually validate/nominate and remove once you unbond __everything__.
			<Bonded<T>>::insert(&stash, &controller);

			let id = match payee {
				RewardDestination::Stash => stash.clone(),
				RewardDestination::Controller => controller.clone(),
				RewardDestination::Account(account) => account,
			};
			T::Rewarder::set_payee(&stash, &id);

			let stash_balance = T::Currency::free_balance(&stash);
			let value = value.min(stash_balance);
			Self::deposit_event(RawEvent::Bonded(stash.clone(), value));
			let item = StakingLedger { stash, total: value, active: value, unlocking: vec![] };
			Self::update_ledger(&controller, &item);
		}

		/// Add some extra amount that have appeared in the stash `free_balance` into the balance up
		/// for staking.
		///
		/// Use this if there are additional funds in your stash account that you wish to bond.
		/// Unlike [`bond`] or [`unbond`] this function does not impose any limitation on the amount
		/// that can be added.
		///
		/// The dispatch origin for this call must be _Signed_ by the stash, not the controller and
		/// it can be only called when [`EraElectionStatus`] is `Closed`.
		///
		/// Emits `Bonded`.
		///
		/// # <weight>
		/// - Independent of the arguments. Insignificant complexity.
		/// - O(1).
		/// - One DB entry.
		/// ------------
		/// DB Weight:
		/// - Read: Era Election Status, Bonded, Ledger, [Origin Account], Locks
		/// - Write: [Origin Account], Locks, Ledger
		/// # </weight>
		#[weight = T::WeightInfo::bond_extra()]
		fn bond_extra(origin, #[compact] max_additional: BalanceOf<T>) {
			ensure!(Self::era_election_status().is_closed(), Error::<T>::CallNotAllowed);
			let stash = ensure_signed(origin)?;

			let controller = Self::bonded(&stash).ok_or(Error::<T>::NotStash)?;
			let mut ledger = Self::ledger(&controller).ok_or(Error::<T>::NotController)?;

			let stash_balance = T::Currency::free_balance(&stash);

			if let Some(extra) = stash_balance.checked_sub(&ledger.total) {
				let extra = extra.min(max_additional);
				ensure!((ledger.active + extra) >= Self::minimum_bond(), Error::<T>::InsufficientBond);
				ledger.total += extra;
				ledger.active += extra;
				Self::deposit_event(RawEvent::Bonded(stash, extra));
				Self::update_ledger(&controller, &ledger);
			} else {
				// use doesn't have enough balance, better to retun some failure
				return Err(Error::<T>::InsufficientFreeBalance.into())
			}
		}

		/// Schedule a portion of the stash to be unlocked ready for transfer out after the bond
		/// period ends. If this leaves an amount actively bonded less than
		/// T::Currency::minimum_balance(), then it is increased to the full amount.
		///
		/// Once the unlock period is done, you can call `withdraw_unbonded` to actually move
		/// the funds out of management ready for transfer.
		///
		/// No more than a limited number of unlocking chunks (see `MAX_UNLOCKING_CHUNKS`)
		/// can co-exists at the same time. In that case, [`Call::withdraw_unbonded`] need
		/// to be called first to remove some of the chunks (if possible).
		///
		/// The dispatch origin for this call must be _Signed_ by the controller, not the stash.
		/// And, it can be only called when [`EraElectionStatus`] is `Closed`.
		///
		/// Emits `Unbonded`.
		///
		/// See also [`Call::withdraw_unbonded`].
		///
		/// # <weight>
		/// - Independent of the arguments. Limited but potentially exploitable complexity.
		/// - Contains a limited number of reads.
		/// - Each call (requires the remainder of the bonded balance to be above `minimum_balance`)
		///   will cause a new entry to be inserted into a vector (`Ledger.unlocking`) kept in storage.
		///   The only way to clean the aforementioned storage item is also user-controlled via
		///   `withdraw_unbonded`.
		/// - One DB entry.
		/// ----------
		/// Weight: O(1)
		/// DB Weight:
		/// - Read: EraElectionStatus, Ledger, CurrentEra, Locks, BalanceOf Stash,
		/// - Write: Locks, Ledger, BalanceOf Stash,
		/// </weight>
		#[weight = T::WeightInfo::unbond()]
		fn unbond(origin, #[compact] value: BalanceOf<T>) {
			ensure!(Self::era_election_status().is_closed(), Error::<T>::CallNotAllowed);
			let controller = ensure_signed(origin)?;
			let mut ledger = Self::ledger(&controller).ok_or(Error::<T>::NotController)?;
			ensure!(
				ledger.unlocking.len() < MAX_UNLOCKING_CHUNKS,
				Error::<T>::NoMoreChunks,
			);

			if ledger.active.is_zero() || value.is_zero() {
				return Ok(());
			}

			// If active stake drops below the minimum bond threshold then the entirety of the stash
			// should be scheduled to unlock.
			// Care must be taken to ensure that funds are still at stake until the unlocking period is over.
			let remaining_active = ledger.active.checked_sub(&value).unwrap_or(Zero::zero());
			let era = Self::current_era().unwrap_or(0) + T::BondingDuration::get();
			if remaining_active < Self::minimum_bond() {
				// Must unbond all funds
				ledger.unlocking.push(UnlockChunk { value: ledger.active, era });
				Self::deposit_event(RawEvent::Unbonded(ledger.stash.clone(), ledger.active));
				ledger.active = Zero::zero();
				// The account should no longer be considered for election as a validator nor should it have any
				// voting power via nomination.
				Self::chill_stash(&ledger.stash);
			} else {
				ledger.unlocking.push(UnlockChunk { value, era });
				Self::deposit_event(RawEvent::Unbonded(ledger.stash.clone(), value));
				ledger.active = remaining_active;
			}

			Self::update_ledger(&controller, &ledger);
		}

		/// Remove any unlocked chunks from the `unlocking` queue from our management.
		///
		/// This essentially frees up that balance to be used by the stash account to do
		/// whatever it wants.
		///
		/// The dispatch origin for this call must be _Signed_ by the controller, not the stash.
		/// And, it can be only called when [`EraElectionStatus`] is `Closed`.
		///
		/// Emits `Withdrawn`.
		///
		/// See also [`Call::unbond`].
		///
		/// # <weight>
		/// - Could be dependent on the `origin` argument and how much `unlocking` chunks exist.
		///  It implies `consolidate_unlocked` which loops over `Ledger.unlocking`, which is
		///  indirectly user-controlled. See [`unbond`] for more detail.
		/// - Contains a limited number of reads, yet the size of which could be large based on `ledger`.
		/// - Writes are limited to the `origin` account key.
		/// ---------------
		/// Complexity O(S) where S is the number of slashing spans to remove
		/// Update:
		/// - Reads: EraElectionStatus, Ledger, Current Era, Locks, [Origin Account]
		/// - Writes: [Origin Account], Locks, Ledger
		/// Kill:
		/// - Reads: EraElectionStatus, Ledger, Current Era, Bonded, Slashing Spans, [Origin
		///   Account], Locks, BalanceOf stash
		/// - Writes: Bonded, Slashing Spans (if S > 0), Ledger, Payee, Validators, Nominators,
		///   [Origin Account], Locks, BalanceOf stash.
		/// - Writes Each: SpanSlash * S
		/// NOTE: Weight annotation is the kill scenario, we refund otherwise.
		/// # </weight>
		#[weight = T::WeightInfo::withdraw_unbonded_kill(1_u32)]
		fn withdraw_unbonded(origin) {
			ensure!(Self::era_election_status().is_closed(), Error::<T>::CallNotAllowed);
			let controller = ensure_signed(origin)?;
			let mut ledger = Self::ledger(&controller).ok_or(Error::<T>::NotController)?;
			let (stash, old_total) = (ledger.stash.clone(), ledger.total);
			if let Some(current_era) = Self::current_era() {
				ledger = ledger.consolidate_unlocked(current_era)
			}

			if ledger.unlocking.is_empty() && ledger.active.is_zero() {
				// This account must have called `unbond()` with some value that caused the active
				// portion to fall below existential deposit + will have no more unlocking chunks
				// left. We can now safely remove this.
				let stash = ledger.stash;
				// remove the lock.
				T::Currency::remove_lock(STAKING_ID, &stash);
				// remove all staking-related information.
				Self::kill_stash(&stash)?;
			} else {
				// This was the consequence of a partial unbond. just update the ledger and move on.
				Self::update_ledger(&controller, &ledger);
			}

			// `old_total` should never be less than the new total because
			// `consolidate_unlocked` strictly subtracts balance.
			if ledger.total < old_total {
				// Already checked that this won't overflow by entry condition.
				let value = old_total - ledger.total;
				Self::deposit_event(RawEvent::Withdrawn(stash, value));
			}
		}

		/// Declare the desire to validate for the origin controller.
		///
		/// Effects will be felt at the beginning of the next era.
		///
		/// The dispatch origin for this call must be _Signed_ by the controller, not the stash.
		/// And, it can be only called when [`EraElectionStatus`] is `Closed`.
		///
		/// # <weight>
		/// - Independent of the arguments. Insignificant complexity.
		/// - Contains a limited number of reads.
		/// - Writes are limited to the `origin` account key.
		/// -----------
		/// Weight: O(1)
		/// DB Weight:
		/// - Read: Era Election Status, Ledger
		/// - Write: Nominators, Validators
		/// # </weight>
		#[weight = T::WeightInfo::validate()]
		fn validate(origin, prefs: ValidatorPrefs) {
			ensure!(Self::era_election_status().is_closed(), Error::<T>::CallNotAllowed);
			let controller = ensure_signed(origin)?;
			let ledger = Self::ledger(&controller).ok_or(Error::<T>::NotController)?;
			ensure!(ledger.active >= Self::minimum_bond(), Error::<T>::InsufficientBond);
			let stash = &ledger.stash;

			let prefs = ValidatorPrefs {
				commission: prefs.commission.min(Perbill::one())
			};

			<Nominators<T>>::remove(stash);
			<Validators<T>>::insert(stash, prefs);
		}

		/// Declare the desire to nominate `targets` for the origin controller.
		///
		/// Effects will be felt at the beginning of the next era. This can only be called when
		/// [`EraElectionStatus`] is `Closed`.
		///
		/// The dispatch origin for this call must be _Signed_ by the controller, not the stash.
		/// And, it can be only called when [`EraElectionStatus`] is `Closed`.
		///
		/// # <weight>
		/// - The transaction's complexity is proportional to the size of `targets` (N)
		/// which is capped at CompactAssignments::LIMIT (MAX_NOMINATIONS).
		/// - Both the reads and writes follow a similar pattern.
		/// ---------
		/// Weight: O(N)
		/// where N is the number of targets
		/// DB Weight:
		/// - Reads: Era Election Status, Ledger, Current Era
		/// - Writes: Validators, Nominators
		/// # </weight>
		#[weight = T::WeightInfo::nominate(targets.len() as u32)]
		fn nominate(origin, targets: Vec<T::AccountId>) {
			ensure!(Self::era_election_status().is_closed(), Error::<T>::CallNotAllowed);
			let controller = ensure_signed(origin)?;
			let ledger = Self::ledger(&controller).ok_or(Error::<T>::NotController)?;
			let stash = &ledger.stash;
			ensure!(ledger.active >= Self::minimum_bond(), Error::<T>::InsufficientBond);
			ensure!(!targets.is_empty(), Error::<T>::EmptyTargets);

			// nominating the same account multiple times is not allowed
			let deduped = BTreeSet::from_iter(targets.iter());
			ensure!(deduped.len() == targets.len(), Error::<T>::DuplicateNominee);

			let targets = targets.into_iter()
				.take(MAX_NOMINATIONS)
				.collect::<Vec<T::AccountId>>();

			let nominations = Nominations {
				targets,
				submitted_in: Self::current_era().unwrap_or(0),
			};

			<Validators<T>>::remove(stash);
			<Nominators<T>>::insert(stash, &nominations);
		}

		/// Declare no desire to either validate or nominate.
		///
		/// Effects will be felt at the beginning of the next era.
		///
		/// The dispatch origin for this call must be _Signed_ by the controller, not the stash.
		/// And, it can be only called when [`EraElectionStatus`] is `Closed`.
		///
		/// # <weight>
		/// - Independent of the arguments. Insignificant complexity.
		/// - Contains one read.
		/// - Writes are limited to the `origin` account key.
		/// --------
		/// Weight: O(1)
		/// DB Weight:
		/// - Read: EraElectionStatus, Ledger
		/// - Write: Validators, Nominators
		/// # </weight>
		#[weight = T::WeightInfo::chill()]
		fn chill(origin) {
			ensure!(Self::era_election_status().is_closed(), Error::<T>::CallNotAllowed);
			let controller = ensure_signed(origin)?;
			let ledger = Self::ledger(&controller).ok_or(Error::<T>::NotController)?;
			Self::chill_stash(&ledger.stash);
		}

		/// (Re-)set the payment target for a controller.
		///
		/// Effects will be felt at the beginning of the next era.
		///
		/// The dispatch origin for this call must be _Signed_ by the controller, not the stash.
		///
		/// # <weight>
		/// - Independent of the arguments. Insignificant complexity.
		/// - Contains a limited number of reads.
		/// - Writes are limited to the `origin` account key.
		/// ---------
		/// - Weight: O(1)
		/// - DB Weight:
		///     - Read: Ledger
		///     - Write: Payee
		/// # </weight>
		#[weight = T::WeightInfo::set_payee()]
		fn set_payee(origin, payee: RewardDestination<T::AccountId>) {
			let controller = ensure_signed(origin)?;
			let ledger = Self::ledger(&controller).ok_or(Error::<T>::NotController)?;
			let stash = &ledger.stash;
			let id = match payee {
				RewardDestination::Stash => stash.clone(),
				RewardDestination::Controller => controller,
				RewardDestination::Account(account) => account,
			};
			T::Rewarder::set_payee(stash, &id);
		}

		/// (Re-)set the controller of a stash.
		///
		/// Effects will be felt at the beginning of the next era.
		///
		/// The dispatch origin for this call must be _Signed_ by the stash, not the controller.
		///
		/// # <weight>
		/// - Independent of the arguments. Insignificant complexity.
		/// - Contains a limited number of reads.
		/// - Writes are limited to the `origin` account key.
		/// # </weight>
		#[weight = T::WeightInfo::set_controller()]
		fn set_controller(origin, controller: T::AccountId) {
			let stash = ensure_signed(origin)?;
			let old_controller = Self::bonded(&stash).ok_or(Error::<T>::NotStash)?;
			if <Ledger<T>>::contains_key(&controller) {
				Err(Error::<T>::AlreadyPaired)?
			}
			if controller != old_controller {
				<Bonded<T>>::insert(&stash, &controller);
				if let Some(l) = <Ledger<T>>::take(&old_controller) {
					<Ledger<T>>::insert(&controller, l);
				}
			}
		}

		/// Sets the ideal number of validators.
		///
		/// The dispatch origin must be Root.
		///
		/// # <weight>
		/// Weight: O(1)
		/// Write: Validator Count
		/// # </weight>
		#[weight = T::WeightInfo::set_validator_count()]
		fn set_validator_count(origin, #[compact] new: u32) {
			ensure_root(origin)?;
			ValidatorCount::put(new);
		}

		/// Increments the ideal number of validators.
		///
		/// The dispatch origin must be Root.
		///
		/// # <weight>
		/// Same as [`set_validator_count`].
		/// # </weight>
		#[weight = T::WeightInfo::set_validator_count()]
		fn increase_validator_count(origin, #[compact] additional: u32) {
			ensure_root(origin)?;
			ValidatorCount::mutate(|n| *n += additional);
		}

		/// Force there to be no new eras indefinitely.
		///
		/// # <weight>
		/// - No arguments.
		/// # </weight>
		#[weight = T::WeightInfo::force_no_eras()]
		fn force_no_eras(origin) {
			ensure_root(origin)?;
			ForceEra::put(Forcing::ForceNone);
		}

		/// Force there to be a new era at the end of the next session. After this, it will be
		/// reset to normal (non-forced) behaviour.
		///
		/// # <weight>
		/// - No arguments.
		/// # </weight>
		#[weight = T::WeightInfo::force_new_era()]
		fn force_new_era(origin) {
			ensure_root(origin)?;
			ForceEra::put(Forcing::ForceNew);
		}

		/// Set the minimum bond amount.
		#[weight = T::WeightInfo::force_new_era()]
		fn set_minimum_bond(origin, value: BalanceOf<T>) {
			ensure_root(origin)?;
			<MinimumBond<T>>::put(value);
			Self::deposit_event(RawEvent::SetMinimumBond(value));
		}

		/// Set the validators who cannot be slashed (if any).
		#[weight = T::WeightInfo::set_invulnerables(validators.len() as u32)]
		fn set_invulnerables(origin, validators: Vec<T::AccountId>) {
			ensure_root(origin)?;
			<Invulnerables<T>>::put(validators.clone());
			Self::deposit_event(RawEvent::SetInvulnerables(validators) );

		}

		/// Force a current staker to become completely unstaked, immediately.
		///
		/// The dispatch origin must be Root.
		///
		/// # <weight>
		/// O(S) where S is the number of slashing spans to be removed
		/// Reads: Bonded, Slashing Spans, Account, Locks
		/// Writes: Bonded, Slashing Spans (if S > 0), Ledger, Payee, Validators, Nominators, Account, Locks
		/// Writes Each: SpanSlash * S
		/// # </weight>
		#[weight = T::WeightInfo::force_unstake(1_u32)]
		fn force_unstake(origin, stash: T::AccountId) {
			ensure_root(origin)?;

			// remove all staking-related information.
			Self::kill_stash(&stash)?;

			// remove the lock.
			T::Currency::remove_lock(STAKING_ID, &stash);
		}

		/// Force there to be a new era at the end of sessions indefinitely.
		///
		/// The dispatch origin must be Root.
		///
		/// # <weight>
		/// - Weight: O(1)
		/// - Write: ForceEra
		/// # </weight>
		#[weight = T::WeightInfo::force_new_era_always()]
		fn force_new_era_always(origin) {
			ensure_root(origin)?;
			ForceEra::put(Forcing::ForceAlways);
		}

		/// Cancel enactment of a deferred slash.
		///
		/// Parameters: era and indices of the slashes for that era to kill.
		///
		/// # <weight>
		/// Complexity: O(U + S)
		/// with U unapplied slashes weighted with U=1000
		/// and S is the number of slash indices to be canceled.
		/// - Read: Unapplied Slashes
		/// - Write: Unapplied Slashes
		/// # </weight>
		#[weight = T::WeightInfo::cancel_deferred_slash(slash_indices.len() as u32)]
		fn cancel_deferred_slash(origin, era: EraIndex, slash_indices: Vec<u32>) {
			ensure_root(origin)?;

			ensure!(!slash_indices.is_empty(), Error::<T>::EmptyTargets);
			ensure!(is_sorted_and_unique(&slash_indices), Error::<T>::NotSortedAndUnique);

			let mut unapplied = <Self as Store>::UnappliedSlashes::get(&era);
			let last_item = slash_indices[slash_indices.len() - 1];
			ensure!((last_item as usize) < unapplied.len(), Error::<T>::InvalidSlashIndex);

			for (removed, index) in slash_indices.into_iter().enumerate() {
				let index = (index as usize) - removed;
				unapplied.remove(index);
			}

			<Self as Store>::UnappliedSlashes::insert(&era, &unapplied);
		}


		/// Rebond a portion of the stash scheduled to be unlocked.
		///
		/// The dispatch origin must be signed by the controller, and it can be only called when
		/// [`EraElectionStatus`] is `Closed`.
		///
		/// # <weight>
		/// - Time complexity: O(L), where L is unlocking chunks
		/// - Bounded by `MAX_UNLOCKING_CHUNKS`.
		/// - Storage changes: Can't increase storage, only decrease it.
		/// ---------------
		/// - DB Weight:
		///     - Reads: EraElectionStatus, Ledger, Locks, [Origin Account]
		///     - Writes: [Origin Account], Locks, Ledger
		/// # </weight>
		#[weight = T::WeightInfo::rebond(MAX_UNLOCKING_CHUNKS as u32)]
		fn rebond(origin, #[compact] value: BalanceOf<T>) -> DispatchResultWithPostInfo {
			ensure!(Self::era_election_status().is_closed(), Error::<T>::CallNotAllowed);
			let controller = ensure_signed(origin)?;
			let ledger = Self::ledger(&controller).ok_or(Error::<T>::NotController)?;
			ensure!(!ledger.unlocking.is_empty(), Error::<T>::NoUnlockChunk);
			ensure!(ledger.active.saturating_add(value) >= Self::minimum_bond(), Error::<T>::InsufficientBond);

			let ledger = ledger.rebond(value);
			Self::update_ledger(&controller, &ledger);
			Ok(Some(
				35 * WEIGHT_PER_MICROS
				+ 50 * WEIGHT_PER_NANOS * (ledger.unlocking.len() as Weight)
				+ T::DbWeight::get().reads_writes(3, 2)
			).into())
		}

		/// Set `HistoryDepth` value. This function will delete any history information
		/// when `HistoryDepth` is reduced.
		///
		/// Parameters:
		/// - `new_history_depth`: The new history depth you would like to set.
		/// - `era_items_deleted`: The number of items that will be deleted by this dispatch.
		///    This should report all the storage items that will be deleted by clearing old
		///    era history. Needed to report an accurate weight for the dispatch. Trusted by
		///    `Root` to report an accurate number.
		///
		/// Origin must be root.
		///
		/// # <weight>
		/// - E: Number of history depths removed, i.e. 10 -> 7 = 3
		/// - Weight: O(E)
		/// - DB Weight:
		///     - Reads: Current Era, History Depth
		///     - Writes: History Depth
		///     - Clear Prefix Each: Era Stakers, EraStakersClipped, ErasValidatorPrefs
		///     - Writes Each: ErasTotalStake, ErasStartSessionIndex
		/// # </weight>
		#[weight = T::WeightInfo::set_history_depth(*_era_items_deleted)]
		fn set_history_depth(origin,
			#[compact] new_history_depth: EraIndex,
			#[compact] _era_items_deleted: u32,
		) {
			ensure_root(origin)?;
			// exposure history must be tracked for atleast 1 era to allow retroactive payout schemes
			// to function correctly
			ensure!(new_history_depth > 1, Error::<T>::IncorrectHistoryDepth);
			if let Some(current_era) = Self::current_era() {
				HistoryDepth::mutate(|history_depth| {
					let last_kept = current_era.checked_sub(*history_depth).unwrap_or(0);
					let new_last_kept = current_era.checked_sub(new_history_depth).unwrap_or(0);
					for era_index in last_kept..new_last_kept {
						Self::clear_era_information(era_index);
					}
					*history_depth = new_history_depth
				})
			}
		}

		/// Remove all data structure concerning a staker/stash once its balance is zero.
		/// This is essentially equivalent to `withdraw_unbonded` except it can be called by anyone
		/// and the target `stash` must have no funds left.
		///
		/// This can be called from any origin.
		///
		/// - `stash`: The stash account to reap. Its balance must be zero.
		///
		/// # <weight>
		/// Complexity: O(S) where S is the number of slashing spans on the account.
		/// DB Weight:
		/// - Reads: Stash Account, Bonded, Slashing Spans, Locks
		/// - Writes: Bonded, Slashing Spans (if S > 0), Ledger, Payee, Validators, Nominators, Stash Account, Locks
		/// - Writes Each: SpanSlash * S
		/// # </weight>
		#[weight = T::WeightInfo::reap_stash(1_u32)]
		fn reap_stash(_origin, stash: T::AccountId) {
			let at_minimum_or_dust = T::Currency::total_balance(&stash) <= T::Currency::minimum_balance();
			ensure!(at_minimum_or_dust, Error::<T>::FundedTarget);
			Self::kill_stash(&stash)?;
			T::Currency::remove_lock(STAKING_ID, &stash);
		}

		/// Submit an election result to the chain. If the solution:
		///
		/// 1. is valid.
		/// 2. has a better score than a potentially existing solution on chain.
		///
		/// then, it will be _put_ on chain.
		///
		/// A solution consists of two pieces of data:
		///
		/// 1. `winners`: a flat vector of all the winners of the round.
		/// 2. `assignments`: the compact version of an assignment vector that encodes the edge
		///    weights.
		///
		/// Both of which may be computed using _phragmen_, or any other algorithm.
		///
		/// Additionally, the submitter must provide:
		///
		/// - The `score` that they claim their solution has.
		///
		/// Both validators and nominators will be represented by indices in the solution. The
		/// indices should respect the corresponding types ([`ValidatorIndex`] and
		/// [`NominatorIndex`]). Moreover, they should be valid when used to index into
		/// [`SnapshotValidators`] and [`SnapshotNominators`]. Any invalid index will cause the
		/// solution to be rejected. These two storage items are set during the election window and
		/// may be used to determine the indices.
		///
		/// A solution is valid if:
		///
		/// 0. It is submitted when [`EraElectionStatus`] is `Open`.
		/// 1. Its claimed score is equal to the score computed on-chain.
		/// 2. Presents the correct number of winners.
		/// 3. All indexes must be value according to the snapshot vectors. All edge values must
		///    also be correct and should not overflow the granularity of the ratio type (i.e. 256
		///    or billion).
		/// 4. For each edge, all targets are actually nominated by the voter.
		/// 5. Has correct self-votes.
		///
		/// A solutions score is consisted of 3 parameters:
		///
		/// 1. `min { support.total }` for each support of a winner. This value should be maximized.
		/// 2. `sum { support.total }` for each support of a winner. This value should be minimized.
		/// 3. `sum { support.total^2 }` for each support of a winner. This value should be
		///    minimized (to ensure less variance)
		///
		/// # <weight>
		/// The transaction is assumed to be the longest path, a better solution.
		///   - Initial solution is almost the same.
		///   - Worse solution is retraced in pre-dispatch-checks which sets its own weight.
		/// # </weight>
		#[weight = T::WeightInfo::submit_solution_better(
			size.validators.into(),
			size.nominators.into(),
			compact.voter_count() as u32,
			winners.len() as u32,
		)]
		pub fn submit_election_solution(
			origin,
			winners: Vec<ValidatorIndex>,
			compact: CompactAssignments,
			score: ElectionScore,
			era: EraIndex,
			size: ElectionSize,
		) -> DispatchResultWithPostInfo {
			let _who = ensure_signed(origin)?;
			Self::check_and_replace_solution(
				winners,
				compact,
				ElectionCompute::Signed,
				score,
				era,
				size,
			)
		}

		/// Unsigned version of `submit_election_solution`.
		///
		/// Note that this must pass the [`ValidateUnsigned`] check which only allows transactions
		/// from the local node to be included. In other words, only the block author can include a
		/// transaction in the block.
		///
		/// # <weight>
		/// See `crate::weight` module.
		/// # </weight>
		#[weight = T::WeightInfo::submit_solution_better(
			size.validators.into(),
			size.nominators.into(),
			compact.voter_count() as u32,
			winners.len() as u32,
		)]
		pub fn submit_election_solution_unsigned(
			origin,
			winners: Vec<ValidatorIndex>,
			compact: CompactAssignments,
			score: ElectionScore,
			era: EraIndex,
			size: ElectionSize,
		) -> DispatchResultWithPostInfo {
			ensure_none(origin)?;
			let adjustments = Self::check_and_replace_solution(
				winners,
				compact,
				ElectionCompute::Unsigned,
				score,
				era,
				size,
			).expect(
				"An unsigned solution can only be submitted by validators; A validator should \
				always produce correct solutions, else this block should not be imported, thus \
				effectively depriving the validators from their authoring reward. Hence, this panic
				is expected."
			);
			Ok(adjustments)
		}
	}
}

impl<T: Config> Module<T> {
	/// The total balance that can be slashed from a stash account as of right now.
	pub fn slashable_balance_of(stash: &T::AccountId) -> BalanceOf<T> {
		// Weight note: consider making the stake accessible through stash.
		Self::bonded(stash)
			.and_then(Self::ledger)
			.map(|l| l.active)
			.unwrap_or_default()
	}

	/// Internal impl of [`slashable_balance_of`] that returns [`VoteWeight`].
	pub fn slashable_balance_of_vote_weight(stash: &T::AccountId, issuance: BalanceOf<T>) -> VoteWeight {
		T::CurrencyToVote::to_vote(Self::slashable_balance_of(stash), issuance)
	}

	/// Returns a closure around `slashable_balance_of_vote_weight` that can be passed around.
	///
	/// This prevents call sites from repeatedly requesting `total_issuance` from backend. But it is
	/// important to be only used while the total issuance is not changing.
	pub fn slashable_balance_of_fn() -> Box<dyn Fn(&T::AccountId) -> VoteWeight> {
		// NOTE: changing this to unboxed `impl Fn(..)` return type and the module will still
		// compile, while some types in mock fail to resolve.
		let issuance = T::Currency::total_issuance();
		Box::new(move |who: &T::AccountId| -> VoteWeight { Self::slashable_balance_of_vote_weight(who, issuance) })
	}

	/// Dump the list of validators and nominators into vectors and keep them on-chain.
	///
	/// This data is used to efficiently evaluate election results. returns `true` if the operation
	/// is successful.
	pub fn create_stakers_snapshot() -> (bool, Weight) {
		let mut consumed_weight = 0;
		let mut add_db_reads_writes = |reads, writes| {
			consumed_weight += T::DbWeight::get().reads_writes(reads, writes);
		};
		let validators = <Validators<T>>::iter().map(|(v, _)| v).collect::<Vec<_>>();
		let mut nominators = <Nominators<T>>::iter().map(|(n, _)| n).collect::<Vec<_>>();

		let num_validators = validators.len();
		let num_nominators = nominators.len();
		add_db_reads_writes((num_validators + num_nominators) as Weight, 0);

		if num_validators > MAX_VALIDATORS || num_nominators.saturating_add(num_validators) > MAX_NOMINATORS {
			log!(
				warn,
				"💸 Snapshot size too big [{} <> {}][{} <> {}].",
				num_validators,
				MAX_VALIDATORS,
				num_nominators,
				MAX_NOMINATORS,
			);
			(false, consumed_weight)
		} else {
			// all validators nominate themselves;
			nominators.extend(validators.clone());

			<SnapshotValidators<T>>::put(validators);
			<SnapshotNominators<T>>::put(nominators);
			add_db_reads_writes(0, 2);
			(true, consumed_weight)
		}
	}

	/// Clears both snapshots of stakers.
	fn kill_stakers_snapshot() {
		<SnapshotValidators<T>>::kill();
		<SnapshotNominators<T>>::kill();
	}

	/// Calculate the next accrued payout for a stash id (a payee) without affecting the storage.
	pub fn accrued_payout(stash: &T::AccountId) -> BalanceOf<T> {
		let era_index = Self::active_era().map(|e| e.index).unwrap_or(0);
		let validator_commission_stake_map = ErasValidatorPrefs::<T>::iter_prefix(&era_index)
			.map(|(validator_stash, prefs)| {
				(
					validator_stash.clone(),
					prefs.commission,
					Self::eras_stakers_clipped(era_index, validator_stash),
				)
			})
			.collect::<Vec<(_, _, _)>>();

		T::Rewarder::calculate_individual_reward(stash, validator_commission_stake_map.as_slice())
	}

	/// Update the ledger for a controller.
	///
	/// This will also update the stash lock.
	fn update_ledger(controller: &T::AccountId, ledger: &StakingLedger<T::AccountId, BalanceOf<T>>) {
		T::Currency::set_lock(STAKING_ID, &ledger.stash, ledger.total, WithdrawReasons::all());
		<Ledger<T>>::insert(controller, ledger);
	}

	/// Chill a stash account.
	fn chill_stash(stash: &T::AccountId) {
		<Validators<T>>::remove(stash);
		<Nominators<T>>::remove(stash);
	}

	/// Plan a new session potentially trigger a new era.
	fn new_session(session_index: SessionIndex) -> Option<Vec<T::AccountId>> {
		if let Some(current_era) = Self::current_era() {
			// Initial era has been set.
			let current_era_start_session_index = Self::eras_start_session_index(current_era).unwrap_or_else(|| {
				frame_support::print("Error: start_session_index must be set for current_era");
				0
			});

			// `session_index` here is current + 1
			// i.e it will start next session
			let era_length = session_index.checked_sub(current_era_start_session_index).unwrap_or(0); // Must never happen.

			// forcing a new era, means forcing an era end for the next active era
			if Self::will_era_be_forced() {
				WasEndEraForced::put(true);
				IsCurrentSessionFinal::put(true);
			}

			match ForceEra::get() {
				Forcing::ForceNew => ForceEra::kill(),
				Forcing::ForceAlways => (),
				Forcing::NotForcing => {
					if era_length + 1 == T::SessionsPerEra::get() {
						IsPlannedSessionFinal::put(true);
					} else if era_length >= T::SessionsPerEra::get() {
						IsPlannedSessionFinal::kill();
						IsCurrentSessionFinal::put(true);
					} else if era_length <= 1 {
						IsCurrentSessionFinal::kill()
					}
				}
				Forcing::ForceNone => {
					// Either `ForceNone`, or `NotForcing && era_length < T::SessionsPerEra::get()`.
					if era_length + 1 == T::SessionsPerEra::get() {
						IsPlannedSessionFinal::put(true);
					// Mark the started/currently executing session as final (session_index - 1)
					} else if era_length >= T::SessionsPerEra::get() {
						IsPlannedSessionFinal::kill();
						IsCurrentSessionFinal::put(true);
						// Should only happen when we are ready to trigger an era but we have ForceNone,
						// otherwise previous arm would short circuit.
						Self::close_election_window();
					} else if era_length <= 1 {
						IsCurrentSessionFinal::kill()
					}
					return None;
				}
			}

			// new era.
			Self::new_era(session_index)
		} else {
			// Set initial era
			log!(
				debug,
				"💸 schedule genesis era, starting at session index: {:?}",
				session_index
			);
			Self::new_era(session_index)
		}
	}

	/// Select a new validator set from the assembled stakers and their role preferences. It tries
	/// first to peek into [`QueuedElected`]. Otherwise, it runs a new on-chain phragmen election.
	///
	/// If [`QueuedElected`] and [`QueuedScore`] exists, they are both removed. No further storage
	/// is updated.
	fn try_do_election() -> Option<ElectionResult<T::AccountId, BalanceOf<T>>> {
		// an election result from either a stored submission or locally executed one.
		let next_result = <QueuedElected<T>>::take().or_else(|| Self::do_on_chain_phragmen());

		// either way, kill this. We remove it here to make sure it always has the exact same
		// lifetime as `QueuedElected`.
		QueuedScore::kill();

		next_result
	}

	/// Execute election and return the new results. The edge weights are processed into support
	/// values.
	///
	/// This is basically a wrapper around [`do_phragmen`] which translates
	/// `PrimitiveElectionResult` into `ElectionResult`.
	///
	/// No storage item is updated.
	pub fn do_on_chain_phragmen() -> Option<ElectionResult<T::AccountId, BalanceOf<T>>> {
		if let Some(phragmen_result) = Self::do_phragmen::<ChainAccuracy>(0) {
			let elected_stashes = phragmen_result
				.winners
				.iter()
				.map(|(s, _)| s.clone())
				.collect::<Vec<T::AccountId>>();
			let assignments = phragmen_result.assignments;

			let staked_assignments =
				sp_npos_elections::assignment_ratio_to_staked(assignments, Self::slashable_balance_of_fn());

			let supports = to_supports(&elected_stashes, &staked_assignments)
				.map_err(|_| {
					log!(
						error,
						"💸 on-chain phragmen is failing due to a problem in the result. This must be a bug."
					)
				})
				.ok()?;

			// collect exposures
			let exposures = Self::collect_exposures(supports);

			// In order to keep the property required by `on_session_ending` that we must return the
			// new validator set even if it's the same as the old, as long as any underlying
			// economic conditions have changed, we don't attempt to do any optimization where we
			// compare against the prior set.
			Some(ElectionResult::<T::AccountId, BalanceOf<T>> {
				elected_stashes,
				exposures,
				compute: ElectionCompute::OnChain,
			})
		} else {
			// There were not enough candidates for even our minimal level of functionality. This is
			// bad. We should probably disable all functionality except for block production and let
			// the chain keep producing blocks until we can decide on a sufficiently substantial
			// set. TODO: #2494
			None
		}
	}

	/// Execute phragmen election and return the new results. No post-processing is applied and the
	/// raw edge weights are returned.
	///
	/// Self votes are added and nominations before the most recent slashing span are ignored.
	///
	/// No storage item is updated.
	pub fn do_phragmen<Accuracy: PerThing128>(
		iterations: usize,
	) -> Option<PrimitiveElectionResult<T::AccountId, Accuracy>>
	where
		ExtendedBalance: From<InnerOf<Accuracy>>,
	{
		let weight_of = Self::slashable_balance_of_fn();
		let mut all_nominators: Vec<(T::AccountId, VoteWeight, Vec<T::AccountId>)> = Vec::new();
		let mut all_validators = Vec::new();
		for (validator, _) in <Validators<T>>::iter() {
			// append self vote
			let self_vote = (validator.clone(), weight_of(&validator), vec![validator.clone()]);
			all_nominators.push(self_vote);
			all_validators.push(validator);
		}

		let nominator_votes = <Nominators<T>>::iter().map(|(nominator, nominations)| {
			let Nominations {
				submitted_in,
				mut targets,
			} = nominations;

			// Filter out nomination targets which were nominated before the most recent
			// slashing span.
			targets.retain(|stash| {
				<Self as Store>::SlashingSpans::get(&stash)
					.map_or(true, |spans| submitted_in >= spans.last_nonzero_slash())
			});

			(nominator, targets)
		});
		all_nominators.extend(nominator_votes.map(|(n, ns)| {
			let s = weight_of(&n);
			(n, s, ns)
		}));

		if all_validators.len() < Self::minimum_validator_count().max(1) as usize {
			// If we don't have enough candidates, nothing to do.
			log!(
				error,
				"💸 Chain does not have enough staking candidates to operate. Era {:?}.",
				Self::current_era()
			);
			None
		} else {
			seq_phragmen::<_, Accuracy>(
				Self::validator_count() as usize,
				all_validators,
				all_nominators,
				Some((iterations, 0)), // exactly run `iterations` rounds.
			)
			.map_err(|err| log!(error, "Call to seq-phragmen failed due to {:?}", err))
			.ok()
		}
	}

	/// Basic and cheap checks that we perform in validate unsigned, and in the execution.
	///
	/// State reads: ElectionState, CurrentEr, QueuedScore.
	///
	/// This function does weight refund in case of errors, which is based upon the fact that it is
	/// called at the very beginning of the call site's function.
	pub fn pre_dispatch_checks(score: ElectionScore, era: EraIndex) -> DispatchResultWithPostInfo {
		// discard solutions that are not in-time
		// check window open
		ensure!(
			Self::era_election_status().is_open(),
			Error::<T>::OffchainElectionEarlySubmission.with_weight(T::DbWeight::get().reads(1)),
		);

		// check current era.
		if let Some(current_era) = Self::current_era() {
			ensure!(
				current_era == era,
				Error::<T>::OffchainElectionEarlySubmission.with_weight(T::DbWeight::get().reads(2)),
			)
		}

		// assume the given score is valid. Is it better than what we have on-chain, if we have any?
		if let Some(queued_score) = Self::queued_score() {
			ensure!(
				is_score_better(score, queued_score, T::MinSolutionScoreBump::get()),
				Error::<T>::OffchainElectionWeakSubmission.with_weight(T::DbWeight::get().reads(3)),
			)
		}

		Ok(None.into())
	}

	/// Checks a given solution and if correct and improved, writes it on chain as the queued result
	/// of the next round. This may be called by both a signed and an unsigned transaction.
	pub fn check_and_replace_solution(
		winners: Vec<ValidatorIndex>,
		compact_assignments: CompactAssignments,
		compute: ElectionCompute,
		claimed_score: ElectionScore,
		era: EraIndex,
		election_size: ElectionSize,
	) -> DispatchResultWithPostInfo {
		// Do the basic checks. era, claimed score and window open.
		let _ = Self::pre_dispatch_checks(claimed_score, era)?;

		// before we read any further state, we check that the unique targets in compact is same as
		// compact. is a all in-memory check and easy to do. Moreover, it ensures that the solution
		// is not full of bogus edges that can cause lots of reads to SlashingSpans. Thus, we can
		// assume that the storage access of this function is always O(|winners|), not
		// O(|compact.edge_count()|).
		ensure!(
			compact_assignments.unique_targets().len() == winners.len(),
			Error::<T>::OffchainElectionBogusWinnerCount,
		);

		// Check that the number of presented winners is sane. Most often we have more candidates
		// than we need. Then it should be `Self::validator_count()`. Else it should be all the
		// candidates.
		let snapshot_validators_length = <SnapshotValidators<T>>::decode_len()
			.map(|l| l as u32)
			.ok_or_else(|| Error::<T>::SnapshotUnavailable)?;

		// size of the solution must be correct.
		ensure!(
			snapshot_validators_length == u32::from(election_size.validators),
			Error::<T>::OffchainElectionBogusElectionSize,
		);

		// check the winner length only here and when we know the length of the snapshot validators
		// length.
		let desired_winners = Self::validator_count().min(snapshot_validators_length);
		ensure!(
			winners.len() as u32 == desired_winners,
			Error::<T>::OffchainElectionBogusWinnerCount
		);

		let snapshot_nominators_len = <SnapshotNominators<T>>::decode_len()
			.map(|l| l as u32)
			.ok_or_else(|| Error::<T>::SnapshotUnavailable)?;

		// rest of the size of the solution must be correct.
		ensure!(
			snapshot_nominators_len == election_size.nominators,
			Error::<T>::OffchainElectionBogusElectionSize,
		);

		// decode snapshot validators.
		let snapshot_validators = Self::snapshot_validators().ok_or(Error::<T>::SnapshotUnavailable)?;

		// check if all winners were legit; this is rather cheap. Replace with accountId.
		let winners = winners
			.into_iter()
			.map(|widx| {
				// NOTE: at the moment, since staking is explicitly blocking any offence until election
				// is closed, we don't check here if the account id at `snapshot_validators[widx]` is
				// actually a validator. If this ever changes, this loop needs to also check this.
				snapshot_validators
					.get(widx as usize)
					.cloned()
					.ok_or(Error::<T>::OffchainElectionBogusWinner)
			})
			.collect::<Result<Vec<T::AccountId>, Error<T>>>()?;

		// decode the rest of the snapshot.
		let snapshot_nominators = Self::snapshot_nominators().ok_or(Error::<T>::SnapshotUnavailable)?;

		// helpers
		let nominator_at = |i: NominatorIndex| -> Option<T::AccountId> { snapshot_nominators.get(i as usize).cloned() };
		let validator_at = |i: ValidatorIndex| -> Option<T::AccountId> { snapshot_validators.get(i as usize).cloned() };

		// un-compact.
		let assignments = compact_assignments
			.into_assignment(nominator_at, validator_at)
			.map_err(|e| {
				// log the error since it is not propagated into the runtime error.
				log!(warn, "💸 un-compacting solution failed due to {:?}", e);
				Error::<T>::OffchainElectionBogusCompact
			})?;

		// check all nominators actually including the claimed vote. Also check correct self votes.
		// Note that we assume all validators and nominators in `assignments` are properly bonded,
		// because they are coming from the snapshot via a given index.
		for Assignment { who, distribution } in assignments.iter() {
			let is_validator = <Validators<T>>::contains_key(&who);
			let maybe_nomination = Self::nominators(&who);

			if !(maybe_nomination.is_some() ^ is_validator) {
				// all of the indices must map to either a validator or a nominator. If this is ever
				// not the case, then the locking system of staking is most likely faulty, or we
				// have bigger problems.
				log!(error, "💸 detected an error in the staking locking and snapshot.");
				// abort.
				return Err(Error::<T>::OffchainElectionBogusNominator.into());
			}

			if !is_validator {
				// a normal vote
				let nomination = maybe_nomination.expect(
					"exactly one of `maybe_validator` and `maybe_nomination.is_some` is true. \
					is_validator is false; maybe_nomination is some; qed",
				);

				// NOTE: we don't really have to check here if the sum of all edges are the
				// nominator correct. Un-compacting assures this by definition.

				for (t, _) in distribution {
					// each target in the provided distribution must be actually nominated by the
					// nominator after the last non-zero slash.
					if nomination.targets.iter().find(|&tt| tt == t).is_none() {
						return Err(Error::<T>::OffchainElectionBogusNomination.into());
					}

					if <Self as Store>::SlashingSpans::get(&t)
						.map_or(false, |spans| nomination.submitted_in < spans.last_nonzero_slash())
					{
						return Err(Error::<T>::OffchainElectionSlashedNomination.into());
					}
				}
			} else {
				// a self vote
				ensure!(distribution.len() == 1, Error::<T>::OffchainElectionBogusSelfVote);
				ensure!(distribution[0].0 == *who, Error::<T>::OffchainElectionBogusSelfVote);
				// defensive only. A compact assignment of length one does NOT encode the weight and
				// it is always created to be 100%.
				ensure!(
					distribution[0].1 == OffchainAccuracy::one(),
					Error::<T>::OffchainElectionBogusSelfVote,
				);
			}
		}

		// convert into staked assignments.
		let staked_assignments =
			sp_npos_elections::assignment_ratio_to_staked(assignments, Self::slashable_balance_of_fn());

		// build the support map thereof in order to evaluate.
		let supports = to_supports(&winners, &staked_assignments).map_err(|_| Error::<T>::OffchainElectionBogusEdge)?;

		// Check if the score is the same as the claimed one.
		let submitted_score = (&supports).evaluate();
		ensure!(submitted_score == claimed_score, Error::<T>::OffchainElectionBogusScore);

		// At last, alles Ok. Exposures and store the result.
		let exposures = Self::collect_exposures(supports);
		log!(
			info,
			"💸 A better solution (with compute {:?} and score {:?}) has been validated and stored on chain.",
			compute,
			submitted_score,
		);

		// write new results.
		<QueuedElected<T>>::put(ElectionResult {
			elected_stashes: winners,
			compute,
			exposures,
		});
		QueuedScore::put(submitted_score);

		// emit event.
		Self::deposit_event(RawEvent::SolutionStored(compute));

		Ok(None.into())
	}

	/// Start a session potentially starting an era.
	fn start_session(start_session: SessionIndex) {
		let next_active_era = Self::active_era().map(|e| e.index + 1).unwrap_or(0);
		// This is only `Some` when current era has already progressed to the next era, while the
		// active era is one behind (i.e. in the *last session of the active era*, or *first session
		// of the new current era*, depending on how you look at it).
		if let Some(next_active_era_start_session_index) = Self::eras_start_session_index(next_active_era) {
			if next_active_era_start_session_index == start_session {
				Self::start_era(start_session);
			} else if next_active_era_start_session_index < start_session {
				// This arm should never happen, but better handle it than to stall the
				// staking pallet.
				log!(warn, "💸 a session appears to have been skipped");
				Self::start_era(start_session);
			}
		}
	}

	/// End a session potentially ending an era.
	fn end_session(session_index: SessionIndex) {
		if let Some(active_era) = Self::active_era() {
			if let Some(next_active_era_start_session_index) = Self::eras_start_session_index(active_era.index + 1) {
				if next_active_era_start_session_index == session_index + 1 {
					Self::end_era(active_era, session_index);
				}
			}
		}
	}

	/// * Increment `active_era.index`,
	/// * reset `active_era.start`,
	/// * update `BondedEras` and apply slashes.
	fn start_era(start_session: SessionIndex) {
		let active_era = ActiveEra::mutate(|active_era| {
			let new_index = active_era.as_ref().map(|info| info.index + 1).unwrap_or(0);
			*active_era = Some(ActiveEraInfo {
				index: new_index,
				// Set new active era start in next `on_finalize`. To guarantee usage of `Time`
				start: None,
			});
			new_index
		});

		let bonding_duration = T::BondingDuration::get();

		BondedEras::mutate(|bonded| {
			bonded.push((active_era, start_session));

			if active_era > bonding_duration {
				let first_kept = active_era - bonding_duration;

				// prune out everything that's from before the first-kept index.
				let n_to_prune = bonded.iter().take_while(|&&(era_idx, _)| era_idx < first_kept).count();

				// kill slashing metadata.
				for (pruned_era, _) in bonded.drain(..n_to_prune) {
					slashing::clear_era_metadata::<T>(pruned_era);
				}

				if let Some(&(_, first_session)) = bonded.first() {
					T::SessionInterface::prune_historical_up_to(first_session);
				}
			}
		});

		Self::apply_unapplied_slashes(active_era);
	}

	/// Compute payout for era.
	fn end_era(active_era: ActiveEraInfo, _session_index: SessionIndex) {
		// Note: `active_era.start` can be None if end era is called during genesis config.
		// important if era duration calculations are required.
		let validators = ErasValidatorPrefs::<T>::iter_prefix(active_era.index)
			.map(|(stash, _prefs)| stash)
			.collect::<Vec<T::AccountId>>();

		T::Rewarder::on_end_era(validators.as_slice(), active_era.index, WasEndEraForced::take());
	}

	/// Plan a new era. Return the potential new staking set.
	fn new_era(start_session_index: SessionIndex) -> Option<Vec<T::AccountId>> {
		// Increment or set current era.
		let current_era = CurrentEra::mutate(|s| {
			*s = Some(s.map(|s| s + 1).unwrap_or(0));
			s.unwrap()
		});
		ErasStartSessionIndex::insert(&current_era, &start_session_index);

		// Clean old era information.
		if let Some(old_era) = current_era.checked_sub(Self::history_depth() + 1) {
			Self::clear_era_information(old_era);
		}

		// Set staking information for new era.
		let maybe_new_validators = Self::select_and_update_validators(current_era);

		maybe_new_validators
	}

	/// Remove all the storage items associated with the election.
	fn close_election_window() {
		// Close window.
		<EraElectionStatus<T>>::put(ElectionStatus::Closed);
		// Kill snapshots.
		Self::kill_stakers_snapshot();
	}

	/// Select the new validator set at the end of the era.
	///
	/// Runs [`try_do_phragmen`] and updates the following storage items:
	/// - [`EraElectionStatus`]: with `None`.
	/// - [`ErasStakers`]: with the new staker set.
	/// - [`ErasStakersClipped`].
	/// - [`ErasValidatorPrefs`].
	/// - [`ErasTotalStake`]: with the new total stake.
	/// - [`SnapshotValidators`] and [`SnapshotNominators`] are both removed.
	///
	/// Internally, [`QueuedElected`], snapshots and [`QueuedScore`] are also consumed.
	///
	/// If the election has been successful, It passes the new set upwards.
	///
	/// This should only be called at the end of an era.
	fn select_and_update_validators(current_era: EraIndex) -> Option<Vec<T::AccountId>> {
		if let Some(ElectionResult::<T::AccountId, BalanceOf<T>> {
			elected_stashes,
			exposures,
			compute,
		}) = Self::try_do_election()
		{
			// Totally close the election round and data.
			Self::close_election_window();

			// Populate Stakers and write slot stake.
			let mut total_stake: BalanceOf<T> = Zero::zero();
			exposures.into_iter().for_each(|(stash, exposure)| {
				total_stake = total_stake.saturating_add(exposure.total);
				<ErasStakers<T>>::insert(current_era, &stash, &exposure);

				let mut exposure_clipped = exposure;
				let clipped_max_len = T::MaxNominatorRewardedPerValidator::get() as usize;
				if exposure_clipped.others.len() > clipped_max_len {
					exposure_clipped.others.sort_by(|a, b| a.value.cmp(&b.value).reverse());
					exposure_clipped.others.truncate(clipped_max_len);
				}
				<ErasStakersClipped<T>>::insert(&current_era, &stash, exposure_clipped);
			});

			// Insert current era staking information
			<ErasTotalStake<T>>::insert(&current_era, total_stake);

			// collect the pref of all winners
			for stash in &elected_stashes {
				let pref = Self::validators(stash);
				<ErasValidatorPrefs<T>>::insert(&current_era, stash, pref);
			}

			// emit event
			Self::deposit_event(RawEvent::StakingElection(compute));

			log!(
				info,
				"💸 new validator set of size {:?} has been elected via {:?} for era {:?}",
				elected_stashes.len(),
				compute,
				current_era,
			);

			Some(elected_stashes)
		} else {
			Self::close_election_window();
			None
		}
	}

	/// Consume a set of [`Supports`] from [`sp_npos_elections`] and collect them into a [`Exposure`]
	fn collect_exposures(
		supports: Supports<T::AccountId>,
	) -> Vec<(T::AccountId, Exposure<T::AccountId, BalanceOf<T>>)> {
		let total_issuance = T::Currency::total_issuance();
		let to_currency = |e: ExtendedBalance| T::CurrencyToVote::to_currency(e, total_issuance);

		supports
			.into_iter()
			.map(|(validator, support)| {
				// build `struct exposure` from `support`
				let mut others = Vec::with_capacity(support.voters.len());
				let mut own: BalanceOf<T> = Zero::zero();
				let mut total: BalanceOf<T> = Zero::zero();
				support
					.voters
					.into_iter()
					.map(|(nominator, weight)| (nominator, to_currency(weight)))
					.for_each(|(nominator, stake)| {
						if nominator == validator {
							own = own.saturating_add(stake);
						} else {
							others.push(IndividualExposure {
								who: nominator,
								value: stake,
							});
						}
						total = total.saturating_add(stake);
					});

				let exposure = Exposure { own, others, total };

				(validator, exposure)
			})
			.collect::<Vec<(T::AccountId, Exposure<_, _>)>>()
	}

	/// Remove all associated data of a stash account from the staking system.
	///
	/// Assumes storage is upgraded before calling.
	///
	/// This is called :
	/// - Immediately when an account's balance falls below existential deposit.
	/// - after a `withdraw_unbond()` call that frees all of a stash's bonded balance.
	fn kill_stash(stash: &T::AccountId) -> DispatchResult {
		let controller = <Bonded<T>>::get(stash).ok_or(Error::<T>::NotStash)?;

		slashing::clear_stash_metadata::<T>(stash)?;

		<Bonded<T>>::remove(stash);
		<Ledger<T>>::remove(&controller);

		T::Rewarder::remove_payee(stash);
		<Validators<T>>::remove(stash);
		<Nominators<T>>::remove(stash);

		Ok(())
	}

	/// Clear all era information for given era.
	fn clear_era_information(era_index: EraIndex) {
		<ErasStakers<T>>::remove_prefix(era_index);
		<ErasStakersClipped<T>>::remove_prefix(era_index);
		<ErasValidatorPrefs<T>>::remove_prefix(era_index);
		<ErasTotalStake<T>>::remove(era_index);
		ErasStartSessionIndex::remove(era_index);
	}

	/// Apply previously-unapplied slashes on the beginning of a new era, after a delay.
	fn apply_unapplied_slashes(active_era: EraIndex) {
		let slash_defer_duration = T::SlashDeferDuration::get();
		<Self as Store>::EarliestUnappliedSlash::mutate(|earliest| {
			if let Some(ref mut earliest) = earliest {
				let keep_from = active_era.saturating_sub(slash_defer_duration);
				for era in (*earliest)..keep_from {
					let era_slashes = <Self as Store>::UnappliedSlashes::take(&era);
					for slash in era_slashes {
						slashing::apply_slash::<T>(slash);
					}
				}

				*earliest = (*earliest).max(keep_from)
			}
		})
	}

	/// Ensures that at the end of the current session there will be a new era.
	fn ensure_new_era() {
		match ForceEra::get() {
			Forcing::ForceAlways | Forcing::ForceNew => (),
			_ => ForceEra::put(Forcing::ForceNew),
		}
	}

	fn will_era_be_forced() -> bool {
		match ForceEra::get() {
			Forcing::ForceAlways | Forcing::ForceNew => true,
			Forcing::ForceNone | Forcing::NotForcing => false,
		}
	}

	#[cfg(feature = "std")]
	/// set nominations directly for tests
	pub fn set_nominations(stash: T::AccountId, nominations: Vec<T::AccountId>) {
		let nominations_ = Nominations {
			targets: nominations,
			submitted_in: Self::current_era().unwrap_or(0),
		};
		<Nominators<T>>::insert(stash, nominations_);
	}

	#[cfg(feature = "std")]
	/// set bond directly for tests
	pub fn set_bond(stash: T::AccountId, amount: BalanceOf<T>) {
		<Bonded<T>>::insert(&stash, &stash);
		Ledger::<T>::insert(
			stash.clone(),
			StakingLedger {
				stash,
				total: amount,
				active: amount,
				unlocking: vec![],
			},
		);
	}
}

/// In this implementation `new_session(session)` must be called before `end_session(session-1)`
/// i.e. the new session must be planned before the ending of the previous session.
///
/// Once the first new_session is planned, all session must start and then end in order, though
/// some session can lag in between the newest session planned and the latest session started.
impl<T: Config> pallet_session::SessionManager<T::AccountId> for Module<T> {
	fn new_session(new_index: SessionIndex) -> Option<Vec<T::AccountId>> {
		log!(
			trace,
			"[{:?}] planning new_session({})",
			<frame_system::Module<T>>::block_number(),
			new_index
		);
		Self::new_session(new_index)
	}
	fn start_session(start_index: SessionIndex) {
		log!(
			trace,
			"[{:?}] starting start_session({})",
			<frame_system::Module<T>>::block_number(),
			start_index
		);
		Self::start_session(start_index)
	}
	fn end_session(end_index: SessionIndex) {
		log!(
			trace,
			"[{:?}] ending end_session({})",
			<frame_system::Module<T>>::block_number(),
			end_index
		);
		Self::end_session(end_index)
	}
}

impl<T: Config> historical::SessionManager<T::AccountId, Exposure<T::AccountId, BalanceOf<T>>> for Module<T> {
	fn new_session(new_index: SessionIndex) -> Option<Vec<(T::AccountId, Exposure<T::AccountId, BalanceOf<T>>)>> {
		<Self as pallet_session::SessionManager<_>>::new_session(new_index).map(|validators| {
			let current_era = Self::current_era()
				// Must be some as a new era has been created.
				.unwrap_or(0);

			validators
				.into_iter()
				.map(|v| {
					let exposure = Self::eras_stakers(current_era, &v);
					(v, exposure)
				})
				.collect()
		})
	}
	fn start_session(start_index: SessionIndex) {
		<Self as pallet_session::SessionManager<_>>::start_session(start_index)
	}
	fn end_session(end_index: SessionIndex) {
		<Self as pallet_session::SessionManager<_>>::end_session(end_index)
	}
}

/// A `Convert` implementation that finds the stash of the given controller account,
/// if any.
pub struct StashOf<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> Convert<T::AccountId, Option<T::AccountId>> for StashOf<T> {
	fn convert(controller: T::AccountId) -> Option<T::AccountId> {
		<Module<T>>::ledger(&controller).map(|l| l.stash)
	}
}

/// A typed conversion from stash account ID to the active exposure of nominators
/// on that account.
///
/// Active exposure is the exposure of the validator set currently validating, i.e. in
/// `active_era`. It can differ from the latest planned exposure in `current_era`.
pub struct ExposureOf<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> Convert<T::AccountId, Option<Exposure<T::AccountId, BalanceOf<T>>>> for ExposureOf<T> {
	fn convert(validator: T::AccountId) -> Option<Exposure<T::AccountId, BalanceOf<T>>> {
		if let Some(active_era) = <Module<T>>::active_era() {
			Some(<Module<T>>::eras_stakers(active_era.index, &validator))
		} else {
			None
		}
	}
}

/// This is intended to be used with `FilterHistoricalOffences`.
impl<T: Config> OnOffenceHandler<T::AccountId, pallet_session::historical::IdentificationTuple<T>, Weight> for Module<T>
where
	T: pallet_session::Config<ValidatorId = <T as frame_system::Config>::AccountId>,
	T: pallet_session::historical::Config<
		FullIdentification = Exposure<<T as frame_system::Config>::AccountId, BalanceOf<T>>,
		FullIdentificationOf = ExposureOf<T>,
	>,
	T::SessionHandler: pallet_session::SessionHandler<<T as frame_system::Config>::AccountId>,
	T::SessionManager: pallet_session::SessionManager<<T as frame_system::Config>::AccountId>,
	T::ValidatorIdOf: Convert<<T as frame_system::Config>::AccountId, Option<<T as frame_system::Config>::AccountId>>,
{
	fn on_offence(
		offenders: &[OffenceDetails<T::AccountId, pallet_session::historical::IdentificationTuple<T>>],
		slash_fraction: &[Perbill],
		slash_session: SessionIndex,
	) -> Result<Weight, ()> {
		if !Self::can_report() {
			return Err(());
		}

		let reward_proportion = SlashRewardFraction::get();
		let mut consumed_weight: Weight = 0;
		let mut add_db_reads_writes = |reads, writes| {
			consumed_weight += T::DbWeight::get().reads_writes(reads, writes);
		};

		let active_era = {
			let active_era = Self::active_era();
			add_db_reads_writes(1, 0);
			if active_era.is_none() {
				// this offence need not be re-submitted.
				return Ok(consumed_weight);
			}
			active_era.expect("value checked not to be `None`; qed").index
		};
		let active_era_start_session_index = Self::eras_start_session_index(active_era).unwrap_or_else(|| {
			frame_support::print("Error: start_session_index must be set for current_era");
			0
		});
		add_db_reads_writes(1, 0);

		let window_start = active_era.saturating_sub(T::BondingDuration::get());

		// fast path for active-era report - most likely.
		// `slash_session` cannot be in a future active era. It must be in `active_era` or before.
		let slash_era = if slash_session >= active_era_start_session_index {
			active_era
		} else {
			let eras = BondedEras::get();
			add_db_reads_writes(1, 0);

			// reverse because it's more likely to find reports from recent eras.
			match eras
				.iter()
				.rev()
				.filter(|&&(_, ref sesh)| sesh <= &slash_session)
				.next()
			{
				Some(&(ref slash_era, _)) => *slash_era,
				// before bonding period. defensive - should be filtered out.
				None => return Ok(consumed_weight),
			}
		};

		<Self as Store>::EarliestUnappliedSlash::mutate(|earliest| {
			if earliest.is_none() {
				*earliest = Some(active_era)
			}
		});
		add_db_reads_writes(1, 1);

		let slash_defer_duration = T::SlashDeferDuration::get();

		let invulnerables = Self::invulnerables();
		add_db_reads_writes(1, 0);

		for (details, slash_fraction) in offenders.iter().zip(slash_fraction) {
			let (stash, exposure) = &details.offender;

			// Skip if the validator is invulnerable.
			if invulnerables.contains(stash) {
				continue;
			}

			let unapplied = slashing::compute_slash::<T>(slashing::SlashParams {
				stash,
				slash: *slash_fraction,
				exposure,
				slash_era,
				window_start,
				now: active_era,
				reward_proportion,
			});

			if let Some(mut unapplied) = unapplied {
				let nominators_len = unapplied.others.len() as u64;
				let reporters_len = details.reporters.len() as u64;

				{
					let upper_bound = 1 /* Validator/NominatorSlashInEra */ + 2 /* fetch_spans */;
					let rw = upper_bound + nominators_len * upper_bound;
					add_db_reads_writes(rw, rw);
				}
				unapplied.reporters = details.reporters.clone();
				if slash_defer_duration == 0 {
					// apply right away.
					slashing::apply_slash::<T>(unapplied);
					{
						let slash_cost = (6, 5);
						let reward_cost = (2, 2);
						add_db_reads_writes(
							(1 + nominators_len) * slash_cost.0 + reward_cost.0 * reporters_len,
							(1 + nominators_len) * slash_cost.1 + reward_cost.1 * reporters_len,
						);
					}
				} else {
					// defer to end of some `slash_defer_duration` from now.
					<Self as Store>::UnappliedSlashes::mutate(active_era, move |for_later| for_later.push(unapplied));
					add_db_reads_writes(1, 1);
				}
			} else {
				add_db_reads_writes(4 /* fetch_spans */, 5 /* kick_out_if_recent */)
			}
		}

		Ok(consumed_weight)
	}

	fn can_report() -> bool {
		Self::era_election_status().is_closed()
	}
}

/// Filter historical offences out and only allow those from the bonding period.
pub struct FilterHistoricalOffences<T, R> {
	_inner: sp_std::marker::PhantomData<(T, R)>,
}

impl<T, Reporter, Offender, R, O> ReportOffence<Reporter, Offender, O> for FilterHistoricalOffences<Module<T>, R>
where
	T: Config,
	R: ReportOffence<Reporter, Offender, O>,
	O: Offence<Offender>,
{
	fn report_offence(reporters: Vec<Reporter>, offence: O) -> Result<(), OffenceError> {
		// disallow any slashing from before the current bonding period.
		let offence_session = offence.session_index();
		let bonded_eras = BondedEras::get();

		if bonded_eras
			.first()
			.filter(|(_, start)| offence_session >= *start)
			.is_some()
		{
			R::report_offence(reporters, offence)
		} else {
			<Module<T>>::deposit_event(RawEvent::OldSlashingReportDiscarded(offence_session));
			Ok(())
		}
	}

	fn is_known_offence(offenders: &[Offender], time_slot: &O::TimeSlot) -> bool {
		R::is_known_offence(offenders, time_slot)
	}
}

#[allow(deprecated)]
impl<T: Config> frame_support::unsigned::ValidateUnsigned for Module<T> {
	type Call = Call<T>;
	fn validate_unsigned(source: TransactionSource, call: &Self::Call) -> TransactionValidity {
		if let Call::submit_election_solution_unsigned(_, _, score, era, _) = call {
			use offchain_election::DEFAULT_LONGEVITY;

			// discard solution not coming from the local OCW.
			match source {
				TransactionSource::Local | TransactionSource::InBlock => { /* allowed */ }
				_ => {
					log!(
						warn,
						"💸 rejecting unsigned transaction because it is not local/in-block."
					);
					return InvalidTransaction::Call.into();
				}
			}

			if let Err(error_with_post_info) = Self::pre_dispatch_checks(*score, *era) {
				let invalid = to_invalid(error_with_post_info);
				log!(
					debug,
					"💸 validate unsigned pre dispatch checks failed due to error #{:?}.",
					invalid,
				);
				return invalid.into();
			}

			log!(debug, "💸 validateUnsigned succeeded for a solution at era {}.", era);

			ValidTransaction::with_tag_prefix("StakingOffchain")
				// The higher the score[0], the better a solution is.
				.priority(T::UnsignedPriority::get().saturating_add(score[0].saturated_into()))
				// Defensive only. A single solution can exist in the pool per era. Each validator
				// will run OCW at most once per era, hence there should never exist more than one
				// transaction anyhow.
				.and_provides(era)
				// Note: this can be more accurate in the future. We do something like
				// `era_end_block - current_block` but that is not needed now as we eagerly run
				// offchain workers now and the above should be same as `T::ElectionLookahead`
				// without the need to query more storage in the validation phase. If we randomize
				// offchain worker, then we might re-consider this.
				.longevity(TryInto::<u64>::try_into(T::ElectionLookahead::get()).unwrap_or(DEFAULT_LONGEVITY))
				// We don't propagate this. This can never the validated at a remote node.
				.propagate(false)
				.build()
		} else {
			InvalidTransaction::Call.into()
		}
	}

	fn pre_dispatch(call: &Self::Call) -> Result<(), TransactionValidityError> {
		if let Call::submit_election_solution_unsigned(_, _, score, era, _) = call {
			// IMPORTANT NOTE: These checks are performed in the dispatch call itself, yet we need
			// to duplicate them here to prevent a block producer from putting a previously
			// validated, yet no longer valid solution on chain.
			// OPTIMISATION NOTE: we could skip this in the `submit_election_solution_unsigned`
			// since we already do it here. The signed version needs it though. Yer for now we keep
			// this duplicate check here so both signed and unsigned can use a singular
			// `check_and_replace_solution`.
			Self::pre_dispatch_checks(*score, *era)
				.map(|_| ())
				.map_err(to_invalid)
				.map_err(Into::into)
		} else {
			Err(InvalidTransaction::Call.into())
		}
	}
}

/// Check that list is sorted and has no duplicates.
fn is_sorted_and_unique(list: &[u32]) -> bool {
	list.windows(2).all(|w| w[0] < w[1])
}

/// convert a DispatchErrorWithPostInfo to a custom InvalidTransaction with the inner code being the
/// error number.
fn to_invalid(error_with_post_info: DispatchErrorWithPostInfo) -> InvalidTransaction {
	let error = error_with_post_info.error;
	let error_number = match error {
		DispatchError::Module { error, .. } => error,
		_ => 0,
	};
	InvalidTransaction::Custom(error_number)
}

impl<T: Config> crml_support::FinalSessionTracker for Module<T> {
	/// True if the next session is final in an era
	fn is_planned_session_final() -> bool {
		Self::is_planned_session_final()
	}
	/// True if the current session is final in an era
	/// True if the era was forced
	fn is_current_session_final() -> (bool, bool) {
		// TODO: forced hard-coded to false
		(Self::is_current_session_final(), false)
	}
}
