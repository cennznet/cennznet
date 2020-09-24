// Copyright 2017-2020 Parity Technologies (UK) Ltd. and Centrality Investments Ltd.
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
//! - [`staking::Trait`](./trait.Trait.html)
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
//! ### Example: Rewarding a validator by id.
//!
//! ```
//! use frame_support::{decl_module, dispatch};
//! use frame_system::{self as system, ensure_signed};
//! use crml_staking::{self as staking};
//!
//! pub trait Trait: staking::Trait {}
//!
//! decl_module! {
//! 	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
//!			/// Reward a validator.
//! 		pub fn reward_myself(origin) -> dispatch::DispatchResult {
//! 			let reported = ensure_signed(origin)?;
//! 			<staking::Module<T>>::reward_by_ids(vec![(reported, 10)]);
//! 			Ok(())
//! 		}
//! 	}
//! }
//! # fn main() { }
//! ```
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
//! See the [`Rewards`](../crml-rewards/index.html) module for details.
//!
//! All entities who receive a reward have the option to choose their reward destination
//! through the [`Payee`](./struct.Payee.html) storage item (see
//! [`set_payee`](enum.Call.html#variant.set_payee)), to be one of the following:
//!
//! - Stash account, not increasing the staked value.
//! - Controller account, (obviously) not increasing the staked value.
//! - Any account they choose (encompasses the previous options but added later)
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
//! - GenericAsset used to manage values at stake.
//! - [Rewards](../crml-rewards/index.html): Used to calculate and payout rewards.
//! - [Session](../pallet_session/index.html): Used to manage sessions. Also, a list of new validators
//! is stored in the Session module's `Validators` at the end of each era.

#![recursion_limit = "128"]
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(array_into_iter)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod multi_token_economy_tests;
#[cfg(test)]
mod tests;

mod slashing;

use cennznet_primitives::{
	traits::ValidatorRewardPayment,
	types::{Exposure, IndividualExposure},
};
use codec::{Decode, Encode, HasCompact};
use frame_support::{
	debug, decl_error, decl_event, decl_module, decl_storage, ensure,
	traits::{Currency, Get, LockIdentifier, LockableCurrency, OnReapAccount, OnUnbalanced, Time, WithdrawReasons},
	weights::SimpleDispatchInfo,
	IterableStorageMap,
};
use frame_system::{self as system, ensure_root, ensure_signed};
use pallet_session::historical::SessionManager;
use sp_phragmen::ExtendedBalance;
use sp_runtime::{
	traits::{AtLeast32Bit, Bounded, CheckedSub, Convert, Saturating, Zero},
	Perbill, RuntimeDebug,
};
#[cfg(feature = "std")]
use sp_runtime::{Deserialize, Serialize};
use sp_staking::{
	offence::{Offence, OffenceDetails, OffenceError, OnOffenceHandler, ReportOffence},
	SessionIndex,
};
use sp_std::{
	collections::{btree_map::BTreeMap, btree_set::BTreeSet},
	iter::FromIterator,
	prelude::*,
};

const DEFAULT_MINIMUM_VALIDATOR_COUNT: u32 = 4;
const MAX_NOMINATIONS: usize = 16;
const MAX_UNLOCKING_CHUNKS: usize = 32;
const STAKING_ID: LockIdentifier = *b"staking ";

/// Counter for the number of eras that have passed.
pub type EraIndex = u32;

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

pub type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;
type NegativeImbalanceOf<T> =
	<<T as Trait>::Currency as Currency<<T as frame_system::Trait>::AccountId>>::NegativeImbalance;
type MomentOf<T> = <<T as Trait>::Time as Time>::Moment;

/// Means for interacting with a specialized version of the `session` trait.
///
/// This is needed because `Staking` sets the `ValidatorIdOf` of the `pallet_session::Trait`
pub trait SessionInterface<AccountId>: frame_system::Trait {
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

impl<T: Trait> SessionInterface<<T as frame_system::Trait>::AccountId> for T
where
	T: pallet_session::Trait<ValidatorId = <T as frame_system::Trait>::AccountId>,
	T: pallet_session::historical::Trait<
		FullIdentification = Exposure<<T as frame_system::Trait>::AccountId, BalanceOf<T>>,
		FullIdentificationOf = ExposureOf<T>,
	>,
	T::SessionHandler: pallet_session::SessionHandler<<T as frame_system::Trait>::AccountId>,
	T::SessionManager: pallet_session::SessionManager<<T as frame_system::Trait>::AccountId>,
	T::ValidatorIdOf: Convert<<T as frame_system::Trait>::AccountId, Option<<T as frame_system::Trait>::AccountId>>,
{
	fn disable_validator(validator: &<T as frame_system::Trait>::AccountId) -> Result<bool, ()> {
		<pallet_session::Module<T>>::disable(validator)
	}

	fn validators() -> Vec<<T as frame_system::Trait>::AccountId> {
		<pallet_session::Module<T>>::validators()
	}

	fn prune_historical_up_to(up_to: SessionIndex) {
		<pallet_session::historical::Module<T>>::prune_up_to(up_to);
	}
}

pub trait Trait: frame_system::Trait {
	/// The staking balance.
	type Currency: LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;

	/// Time used for computing era duration.
	type Time: Time;

	/// Convert a balance into a number used for election calculation.
	/// This must fit into a `u64` but is allowed to be sensibly lossy.
	/// TODO: #1377
	/// The backward convert should be removed as the new Phragmen API returns ratio.
	/// The post-processing needs it but will be moved to off-chain. TODO: #2908
	type CurrencyToVote: Convert<BalanceOf<Self>, u64> + Convert<u128, BalanceOf<Self>>;

	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;

	/// Handler for the unbalanced reduction when slashing a staker.
	type Slash: OnUnbalanced<NegativeImbalanceOf<Self>>;

	/// Number of sessions per era.
	type SessionsPerEra: Get<SessionIndex>;

	/// Number of eras that staked funds must remain bonded for.
	type BondingDuration: Get<EraIndex>;

	/// Number of eras that slashes are deferred by, after computation. This
	/// should be less than the bonding duration. Set to 0 if slashes should be
	/// applied immediately, without opportunity for intervention.
	type SlashDeferDuration: Get<EraIndex>;

	/// Interface for interacting with a session module.
	type SessionInterface: self::SessionInterface<Self::AccountId>;

	/// Handles payout for validator rewards
	type Rewarder: ValidatorRewardPayment<AccountId = Self::AccountId, Balance = BalanceOf<Self>>;
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

decl_storage! {
	trait Store for Module<T: Trait> as Staking {
		/// Minimum amount to bond.
		MinimumBond get(fn minimum_bond) config(): BalanceOf<T>;

		/// The ideal number of staking participants.
		pub ValidatorCount get(fn validator_count) config(): u32;

		/// Minimum number of staking participants before emergency conditions are imposed.
		pub MinimumValidatorCount get(fn minimum_validator_count) config():
			u32 = DEFAULT_MINIMUM_VALIDATOR_COUNT;

		/// Any validators that may never be slashed or forcibly kicked. It's a Vec since they're
		/// easy to initialize and the performance hit is minimal (we expect no more than four
		/// invulnerables) and restricted to testnets.
		pub Invulnerables get(fn invulnerables) config(): Vec<T::AccountId>;

		/// Map from all locked "stash" accounts to the controller account.
		pub Bonded get(fn bonded): map hasher(twox_64_concat) T::AccountId => Option<T::AccountId>;

		/// Map from all (unlocked) "controller" accounts to the info regarding the staking.
		pub Ledger get(fn ledger):
			map hasher(twox_64_concat) T::AccountId
			=> Option<StakingLedger<T::AccountId, BalanceOf<T>>>;

		/// Where the reward payment should be made. Keyed by stash.
		pub Payee get(fn payee): map hasher(twox_64_concat) T::AccountId => RewardDestination<T::AccountId>;

		/// The map from validator candidate stash keys to their payment preferences.
		pub Validators get(fn validators):
			map hasher(twox_64_concat) T::AccountId => ValidatorPrefs;

		/// The map from nominator stash key to the set of stash keys of all validators to nominate.
		///
		/// NOTE: is private so that we can ensure upgraded before all typical accesses.
		/// Direct storage APIs can still bypass this protection.
		Nominators get(fn nominators):
			map hasher(twox_64_concat) T::AccountId => Option<Nominations<T::AccountId>>;

		/// Nominators for a particular account that is in action right now. You can't iterate
		/// through validators here, but you can find them in the Session module.
		///
		/// This is keyed by the stash account.
		pub Stakers get(fn stakers):
			map hasher(twox_64_concat) T::AccountId => Exposure<T::AccountId, BalanceOf<T>>;

		/// The currently elected validator set keyed by stash account ID.
		pub CurrentElected get(fn current_elected): Vec<T::AccountId>;

		/// The current era index.
		pub CurrentEra get(fn current_era) config(): EraIndex;

		/// The start of the current era.
		pub CurrentEraStart get(fn current_era_start): MomentOf<T>;

		/// The session index at which the current era started.
		pub CurrentEraStartSessionIndex get(fn current_era_start_session_index): SessionIndex;

		/// The amount of balance actively at stake for each validator slot, currently.
		///
		/// This is used to derive rewards and punishments.
		pub SlotStake get(fn slot_stake) build(|config: &GenesisConfig<T>| {
			config.stakers.iter().map(|&(_, _, value, _)| value).min().unwrap_or_default()
		}): BalanceOf<T>;

		/// True if the next session change will be a new era regardless of index.
		pub ForceEra get(fn force_era) config(): Forcing;

		/// The percentage of the slash that is distributed to reporters.
		///
		/// The rest of the slashed value is handled by the `Slash`.
		pub SlashRewardFraction get(fn slash_reward_fraction) config(): Perbill;

		/// The amount of currency given to reporters of a slash event which was
		/// canceled by extraordinary circumstances (e.g. governance).
		pub CanceledSlashPayout get(fn canceled_payout) config(): BalanceOf<T>;

		/// All unapplied slashes that are queued for later.
		pub UnappliedSlashes:
			map hasher(twox_64_concat) EraIndex => Vec<UnappliedSlash<T::AccountId, BalanceOf<T>>>;

		/// A mapping from still-bonded eras to the first session index of that era.
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

		/// The version of storage for upgrade.
		StorageVersion: u32;
	}
	add_extra_genesis {
		config(stakers):
			Vec<(T::AccountId, T::AccountId, BalanceOf<T>, StakerStatus<T::AccountId>)>;
		build(|config: &GenesisConfig<T>| {
			assert!(config.minimum_bond > Zero::zero(), "Minimum bond must be greater than zero.");
			for &(ref stash, ref controller, balance, ref status) in &config.stakers {
				assert!(
					T::Currency::free_balance(&stash) >= balance,
					"Stash does not have enough balance to bond."
				);
				let _ = <Module<T>>::bond(
					T::Origin::from((Some(stash.clone()), None).into()),
					controller.clone(),
					balance,
					RewardDestination::Stash,
				);
				let _ = match status {
					StakerStatus::Validator => {
						<Module<T>>::validate(
							T::Origin::from((Some(controller.clone()), None).into()),
							Default::default(),
						)
					},
					StakerStatus::Nominator(votes) => {
						<Module<T>>::nominate(
							T::Origin::from((Some(controller.clone()), None).into()),
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
	pub enum Event<T> where Balance = BalanceOf<T>, <T as frame_system::Trait>::AccountId {
		/// One validator (and its nominators) has been slashed by the given amount.
		Slash(AccountId, Balance),
		/// The validator is invulnerable, so it has NOT been slashed.
		InvulnerableNotSlashed(AccountId, Perbill),
		/// An old slashing report from a prior era was discarded because it could
		/// not be processed.
		OldSlashingReportDiscarded(SessionIndex),
		/// A new set of validators are marked to be invulnerable
		SetInvulnerables(Vec<AccountId>),
		/// Minimum bond amount is changed.
		SetMinimumBond(Balance),
	}
);

decl_error! {
	/// Error for the staking module.
	pub enum Error for Module<T: Trait> {
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
		/// Can not schedule more unlock chunks.
		NoMoreChunks,
		/// Can not rebond without unlocking chunks.
		NoUnlockChunk,
		/// Items are not sorted and unique.
		NotSortedAndUnique,
		/// Cannot nominate the same account multiple times
		DuplicateNominee,
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		/// Number of sessions per era.
		const SessionsPerEra: SessionIndex = T::SessionsPerEra::get();

		/// Number of eras that staked funds must remain bonded for.
		const BondingDuration: EraIndex = T::BondingDuration::get();

		type Error = Error<T>;

		fn deposit_event() = default;

		fn on_finalize() {
			// Set the start of the first era.
			if !<CurrentEraStart<T>>::exists() {
				<CurrentEraStart<T>>::put(T::Time::now());
			}
		}

		/// Take the origin account as a stash and lock up `value` of its balance. `controller` will
		/// be the account that controls it.
		///
		/// `value` must be more than the `minimum_bond` specified in genesis config.
		///
		/// The dispatch origin for this call must be _Signed_ by the stash account.
		///
		/// # <weight>
		/// - Independent of the arguments. Moderate complexity.
		/// - O(1).
		/// - Three extra DB entries.
		///
		/// NOTE: Two of the storage writes (`Self::bonded`, `Self::payee`) are _never_ cleaned unless
		/// the `origin` falls below minimum bond and is removed lazily in `withdraw_unbonded`.
		/// # </weight>
		#[weight = SimpleDispatchInfo::FixedNormal(500_000)]
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
			<Payee<T>>::insert(&stash, payee);

			let stash_balance = T::Currency::free_balance(&stash);
			let value = value.min(stash_balance);
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
		/// The dispatch origin for this call must be _Signed_ by the stash, not the controller.
		///
		/// # <weight>
		/// - Independent of the arguments. Insignificant complexity.
		/// - O(1).
		/// - One DB entry.
		/// # </weight>
		#[weight = SimpleDispatchInfo::FixedNormal(500_000)]
		fn bond_extra(origin, #[compact] max_additional: BalanceOf<T>) {
			let stash = ensure_signed(origin)?;

			let controller = Self::bonded(&stash).ok_or(Error::<T>::NotStash)?;
			let mut ledger = Self::ledger(&controller).ok_or(Error::<T>::NotController)?;

			let stash_balance = T::Currency::free_balance(&stash);

			if let Some(extra) = stash_balance.checked_sub(&ledger.total) {
				let extra = extra.min(max_additional);
				ledger.total += extra;
				ledger.active += extra;
				Self::update_ledger(&controller, &ledger);
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
		///
		/// See also [`Call::withdraw_unbonded`].
		///
		/// # <weight>
		/// - Independent of the arguments. Limited but potentially exploitable complexity.
		/// - Contains a limited number of reads.
		/// - Each call (requires the remainder of the bonded balance to be above `minimum_balance`)
		///   will cause a new entry to be inserted into a vector (`Ledger.unlocking`) kept in storage.
		///   The only way to clean the aforementioned storage item is also user-controlled via `withdraw_unbonded`.
		/// - One DB entry.
		/// </weight>
		#[weight = SimpleDispatchInfo::FixedNormal(400_000)]
		fn unbond(origin, #[compact] value: BalanceOf<T>) {
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
			let era = Self::current_era() + T::BondingDuration::get();
			if remaining_active < Self::minimum_bond() {
				// Must unbond all funds
				ledger.unlocking.push(UnlockChunk { value: ledger.active, era });
				ledger.active = Zero::zero();
				// The account should no longer be considered for election as a validator nor should it have any
				// voting power via nomination.
				Self::chill_stash(&ledger.stash);
			} else {
				ledger.unlocking.push(UnlockChunk { value, era });
				ledger.active = remaining_active;
			}

			Self::update_ledger(&controller, &ledger);
		}

		/// Rebond a portion of the stash scheduled to be unlocked.
		///
		/// # <weight>
		/// - Time complexity: O(1). Bounded by `MAX_UNLOCKING_CHUNKS`.
		/// - Storage changes: Can't increase storage, only decrease it.
		/// # </weight>
		#[weight = SimpleDispatchInfo::FixedNormal(500_000)]
		fn rebond(origin, #[compact] value: BalanceOf<T>) {
			let controller = ensure_signed(origin)?;
			let ledger = Self::ledger(&controller).ok_or(Error::<T>::NotController)?;
			ensure!(
				ledger.unlocking.len() > 0,
				Error::<T>::NoUnlockChunk,
			);

			let ledger = ledger.rebond(value);

			Self::update_ledger(&controller, &ledger);
		}

		/// Remove any unlocked chunks from the `unlocking` queue from our management.
		///
		/// This essentially frees up that balance to be used by the stash account to do
		/// whatever it wants.
		///
		/// The dispatch origin for this call must be _Signed_ by the controller, not the stash.
		///
		/// See also [`Call::unbond`].
		///
		/// # <weight>
		/// - Could be dependent on the `origin` argument and how much `unlocking` chunks exist.
		///  It implies `consolidate_unlocked` which loops over `Ledger.unlocking`, which is
		///  indirectly user-controlled. See [`unbond`] for more detail.
		/// - Contains a limited number of reads, yet the size of which could be large based on `ledger`.
		/// - Writes are limited to the `origin` account key.
		/// # </weight>
		#[weight = SimpleDispatchInfo::FixedNormal(400_000)]
		fn withdraw_unbonded(origin) {
			let controller = ensure_signed(origin)?;
			let ledger = Self::ledger(&controller).ok_or(Error::<T>::NotController)?;
			let ledger = ledger.consolidate_unlocked(Self::current_era());

			if ledger.unlocking.is_empty() && ledger.active.is_zero() {
				// This account must have called `unbond()` with some value that caused the active
				// portion to fall below existential deposit + will have no more unlocking chunks
				// left. We can now safely remove this.
				let stash = ledger.stash;
				// remove the lock.
				T::Currency::remove_lock(STAKING_ID, &stash);
				// remove all staking-related information.
				Self::kill_stash(&stash);
			} else {
				// This was the consequence of a partial unbond. just update the ledger and move on.
				Self::update_ledger(&controller, &ledger);
			}
		}

		/// Declare the desire to validate for the origin controller.
		///
		/// Effects will be felt at the beginning of the next era.
		///
		/// The dispatch origin for this call must be _Signed_ by the controller, not the stash.
		///
		/// # <weight>
		/// - Independent of the arguments. Insignificant complexity.
		/// - Contains a limited number of reads.
		/// - Writes are limited to the `origin` account key.
		/// # </weight>
		#[weight = SimpleDispatchInfo::FixedNormal(750_000)]
		fn validate(origin, prefs: ValidatorPrefs) {

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
		/// Effects will be felt at the beginning of the next era.
		///
		/// The dispatch origin for this call must be _Signed_ by the controller, not the stash.
		///
		/// # <weight>
		/// - The transaction's complexity is proportional to the size of `targets`,
		/// which is capped at `MAX_NOMINATIONS`.
		/// - Both the reads and writes follow a similar pattern.
		/// # </weight>
		#[weight = SimpleDispatchInfo::FixedNormal(750_000)]
		fn nominate(origin, targets: Vec<T::AccountId>) {
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
				submitted_in: Self::current_era(),
			};

			<Validators<T>>::remove(stash);
			<Nominators<T>>::insert(stash, &nominations);
		}

		/// Declare no desire to either validate or nominate.
		///
		/// Effects will be felt at the beginning of the next era.
		///
		/// The dispatch origin for this call must be _Signed_ by the controller, not the stash.
		///
		/// # <weight>
		/// - Independent of the arguments. Insignificant complexity.
		/// - Contains one read.
		/// - Writes are limited to the `origin` account key.
		/// # </weight>
		#[weight = SimpleDispatchInfo::FixedNormal(500_000)]
		fn chill(origin) {
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
		/// # </weight>
		#[weight = SimpleDispatchInfo::FixedNormal(500_000)]
		fn set_payee(origin, payee: RewardDestination<T::AccountId>) {
			let controller = ensure_signed(origin)?;
			let ledger = Self::ledger(&controller).ok_or(Error::<T>::NotController)?;
			let stash = &ledger.stash;
			<Payee<T>>::insert(stash, payee);
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
		#[weight = SimpleDispatchInfo::FixedNormal(750_000)]
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

		/// The ideal number of validators.
		#[weight = SimpleDispatchInfo::FixedNormal(5_000)]
		fn set_validator_count(origin, #[compact] new: u32) {
			ensure_root(origin)?;
			ValidatorCount::put(new);
		}

		// ----- Root calls.

		/// Force there to be no new eras indefinitely.
		///
		/// # <weight>
		/// - No arguments.
		/// # </weight>
		#[weight = SimpleDispatchInfo::FixedOperational(5_000)]
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
		#[weight = SimpleDispatchInfo::FixedOperational(5_000)]
		fn force_new_era(origin) {
			ensure_root(origin)?;
			ForceEra::put(Forcing::ForceNew);
		}

		/// Set the minimum bond amount.
		#[weight = SimpleDispatchInfo::FixedOperational(5_000)]
		fn set_minimum_bond(origin, value: BalanceOf<T>) {
			ensure_root(origin)?;
			<MinimumBond<T>>::put(value);
			Self::deposit_event(RawEvent::SetMinimumBond(value));
		}

		/// Set the validators who cannot be slashed (if any).
		#[weight = SimpleDispatchInfo::FixedOperational(5_000)]
		fn set_invulnerables(origin, validators: Vec<T::AccountId>) {
			ensure_root(origin)?;
			<Invulnerables<T>>::put(validators.clone());
			debug::print!("Set invulnerable:{:?}", validators );
			Self::deposit_event(RawEvent::SetInvulnerables(validators) );

		}

		/// Force a current staker to become completely unstaked, immediately.
		#[weight = SimpleDispatchInfo::FixedOperational(5_000)]
		fn force_unstake(origin, stash: T::AccountId) {
			ensure_root(origin)?;
			// remove the lock.
			T::Currency::remove_lock(STAKING_ID, &stash);
			// remove all staking-related information.
			Self::kill_stash(&stash);
		}

		/// Force there to be a new era at the end of sessions indefinitely.
		///
		/// # <weight>
		/// - One storage write
		/// # </weight>
		#[weight = SimpleDispatchInfo::FixedOperational(5_000)]
		fn force_new_era_always(origin) {
			ensure_root(origin)?;
			ForceEra::put(Forcing::ForceAlways);
		}

		/// Cancel enactment of a deferred slash. Can be called by root origin
		/// passing the era and indices of the slashes for that era to kill.
		///
		/// # <weight>
		/// - One storage write.
		/// # </weight>
		#[weight = SimpleDispatchInfo::FixedOperational(1_000_000)]
		fn cancel_deferred_slash(origin, era: EraIndex, slash_indices: Vec<u32>) {
			ensure_root(origin)?;

			ensure!(!slash_indices.is_empty(), Error::<T>::EmptyTargets);
			ensure!(Self::is_sorted_and_unique(&slash_indices), Error::<T>::NotSortedAndUnique);

			let mut unapplied = <Self as Store>::UnappliedSlashes::get(&era);
			let last_item = slash_indices[slash_indices.len() - 1];
			ensure!((last_item as usize) < unapplied.len(), Error::<T>::InvalidSlashIndex);

			for (removed, index) in slash_indices.into_iter().enumerate() {
				let index = (index as usize) - removed;
				unapplied.remove(index);
			}

			<Self as Store>::UnappliedSlashes::insert(&era, &unapplied);
		}
	}
}

impl<T: Trait> Module<T> {
	// PUBLIC IMMUTABLES

	/// The total balance that is at stake as of right now.
	/// It will remain slashable at least until bonding duration has exceeded.
	pub fn active_balance_of(stash: &T::AccountId) -> BalanceOf<T> {
		Self::bonded(stash)
			.and_then(Self::ledger)
			.map(|l| l.active)
			.unwrap_or_default()
	}

	/// The slashable balance of a stash account as of right now.
	/// It could lessen in the near future as funds become unlocked and are withdrawn.
	pub fn slashable_balance_of(stash: &T::AccountId) -> BalanceOf<T> {
		Self::bonded(stash)
			.and_then(Self::ledger)
			.map(|l| l.slashable_balance())
			.unwrap_or_default()
	}

	// MUTABLES (DANGEROUS)

	/// Update the ledger for a controller. This will also update the stash lock. The lock will
	/// will lock the entire funds except paying for further transactions.
	fn update_ledger(controller: &T::AccountId, ledger: &StakingLedger<T::AccountId, BalanceOf<T>>) {
		T::Currency::set_lock(STAKING_ID, &ledger.stash, ledger.total, WithdrawReasons::all());
		<Ledger<T>>::insert(controller, ledger);
	}

	/// Chill a stash account.
	fn chill_stash(stash: &T::AccountId) {
		<Validators<T>>::remove(stash);
		<Nominators<T>>::remove(stash);
	}

	/// Session has just ended. Provide the validator set for the next session if it's an era-end.
	/// This can also trigger a new era for these conditions:
	/// 1) naturally, if it is the last session of an era
	/// 2) forced, if indicated by governance (see `Forcing`)
	fn new_session(session_index: SessionIndex) -> Option<Vec<T::AccountId>> {
		let era_length = session_index
			.checked_sub(Self::current_era_start_session_index())
			.unwrap_or(0);

		match ForceEra::get() {
			Forcing::ForceNew => ForceEra::kill(),
			Forcing::ForceAlways => (),
			Forcing::NotForcing if era_length >= T::SessionsPerEra::get() => (),
			_ => return None,
		}

		Self::new_era(session_index)
	}

	/// Initialise the first session (and consequently the first era)
	fn initial_session() -> Option<Vec<T::AccountId>> {
		// note: `CurrentEraStart` is set in `on_finalize` of the first block because now is not
		// available yet.
		CurrentEraStartSessionIndex::put(0);
		BondedEras::mutate(|bonded| bonded.push((0, 0)));
		Self::select_validators().1
	}

	/// The era has changed - enact new staking set and trigger the era reward payout.
	///
	/// NOTE: This always happens immediately before a session change to ensure that new validators
	/// get a chance to set their session keys.
	fn new_era(start_session_index: SessionIndex) -> Option<Vec<T::AccountId>> {
		let now = T::Time::now();
		let previous_era_start = <CurrentEraStart<T>>::get();
		let era_duration = now - previous_era_start;

		// Trigger era reward payout, only if some work was done this era (i.e era duration > 0)
		if !era_duration.is_zero() {
			let validator_commission_stake_map = Self::current_elected()
				.iter()
				.map(|validator_stash| {
					// Get a version of `Exposure` which maps to preferred payment account _instead of_ stash
					let mut aggregate_stake = Self::stakers(validator_stash);
					for nominator_exposure in &mut aggregate_stake.others {
						// TODO: this path requires two storage reads :/
						if let RewardDestination::Controller = Self::payee(&nominator_exposure.who) {
							if let Some(controller) = Self::bonded(&nominator_exposure.who) {
								nominator_exposure.who = controller;
							}
						}
						// else: reward destination is the stash already
					}
					let validator_payee = if let RewardDestination::Controller = Self::payee(validator_stash) {
						Self::bonded(validator_stash).unwrap_or_else(|| validator_stash.clone())
					} else {
						validator_stash.clone()
					};
					// (validator payment account, validator commission %, and aggregate stake info by payment account)
					(
						validator_payee,
						Self::validators(validator_stash).commission,
						aggregate_stake,
					)
				})
				.collect::<Vec<(_, _, _)>>();

			T::Rewarder::make_reward_payout(validator_commission_stake_map.as_slice());
		}

		<CurrentEraStart<T>>::mutate(|v| *v = now);
		// Increment current era.
		let current_era = CurrentEra::mutate(|s| {
			*s += 1;
			*s
		});
		CurrentEraStartSessionIndex::mutate(|v| {
			*v = start_session_index;
		});
		let bonding_duration = T::BondingDuration::get();

		BondedEras::mutate(|bonded| {
			bonded.push((current_era, start_session_index));

			if current_era > bonding_duration {
				let first_kept = current_era - bonding_duration;

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

		// Reassign all Stakers.
		let (_slot_stake, maybe_new_validators) = Self::select_validators();
		Self::apply_unapplied_slashes(current_era);

		maybe_new_validators
	}

	/// Apply previously-unapplied slashes on the beginning of a new era, after a delay.
	fn apply_unapplied_slashes(current_era: EraIndex) {
		let slash_defer_duration = T::SlashDeferDuration::get();
		<Self as Store>::EarliestUnappliedSlash::mutate(|earliest| {
			if let Some(ref mut earliest) = earliest {
				let keep_from = current_era.saturating_sub(slash_defer_duration);
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

	/// Select a new validator set from the assembled stakers and their role preferences.
	///
	/// Returns the new `SlotStake` value and a set of newly selected _stash_ IDs.
	///
	/// Assumes storage is coherent with the declaration.
	fn select_validators() -> (BalanceOf<T>, Option<Vec<T::AccountId>>) {
		let mut all_nominators: Vec<(T::AccountId, BalanceOf<T>, Vec<T::AccountId>)> = Vec::new();
		let mut all_validators_and_prefs = BTreeMap::new();
		let mut all_validators = Vec::new();
		for (validator, preference) in <Validators<T>>::iter() {
			let active_bond = Self::active_balance_of(&validator);
			let self_vote = (validator.clone(), active_bond, vec![validator.clone()]);
			all_nominators.push(self_vote);
			all_validators_and_prefs.insert(validator.clone(), preference);
			all_validators.push(validator);
		}

		let nominator_votes = <Nominators<T>>::iter().map(|(nominator, nominations)| {
			let Nominations {
				submitted_in,
				mut targets,
			} = nominations;

			// Filter out nomination targets which were nominated before the most recent
			// non-zero slash.
			targets.retain(|stash| {
				<Self as Store>::SlashingSpans::get(&stash)
					.map_or(true, |spans| submitted_in >= spans.last_nonzero_slash())
			});

			(nominator, targets)
		});
		all_nominators.extend(nominator_votes.map(|(n, ns)| {
			let s = Self::active_balance_of(&n);
			(n, s, ns)
		}));

		let maybe_phragmen_result = sp_phragmen::elect::<_, _, T::CurrencyToVote, Perbill>(
			Self::validator_count() as usize,
			Self::minimum_validator_count().max(1) as usize,
			all_validators,
			all_nominators,
		);

		if let Some(phragmen_result) = maybe_phragmen_result {
			let elected_stashes = phragmen_result
				.winners
				.into_iter()
				.map(|(s, _)| s)
				.collect::<Vec<T::AccountId>>();
			let assignments = phragmen_result.assignments;

			let to_balance =
				|e: ExtendedBalance| <T::CurrencyToVote as Convert<ExtendedBalance, BalanceOf<T>>>::convert(e);

			let supports = sp_phragmen::build_support_map::<_, _, _, T::CurrencyToVote, Perbill>(
				&elected_stashes,
				&assignments,
				Self::active_balance_of,
			);

			// Clear Stakers.
			for v in Self::current_elected().iter() {
				<Stakers<T>>::remove(v);
			}

			// Populate Stakers and figure out the minimum stake behind a slot.
			let mut slot_stake = BalanceOf::<T>::max_value();
			for (c, s) in supports.into_iter() {
				// build `struct exposure` from `support`
				let mut others = Vec::new();
				let mut own: BalanceOf<T> = Zero::zero();
				let mut total: BalanceOf<T> = Zero::zero();
				s.voters
					.into_iter()
					.map(|(who, value)| (who, to_balance(value)))
					.for_each(|(who, value)| {
						if who == c {
							own = own.saturating_add(value);
						} else {
							others.push(IndividualExposure { who, value });
						}
						total = total.saturating_add(value);
					});
				let exposure = Exposure {
					own,
					others,
					// This might reasonably saturate and we cannot do much about it. The sum of
					// someone's stake might exceed the balance type if they have the maximum amount
					// of balance and receive some support. This is super unlikely to happen, yet
					// we simulate it in some tests.
					total,
				};

				if exposure.total < slot_stake {
					slot_stake = exposure.total;
				}
				<Stakers<T>>::insert(&c, exposure.clone());
			}

			// Update slot stake.
			<SlotStake<T>>::put(&slot_stake);

			// Set the new validator set in sessions.
			<CurrentElected<T>>::put(&elected_stashes);

			// In order to keep the property required by `n_session_ending`
			// that we must return the new validator set even if it's the same as the old,
			// as long as any underlying economic conditions have changed, we don't attempt
			// to do any optimization where we compare against the prior set.
			(slot_stake, Some(elected_stashes))
		} else {
			// There were not enough candidates for even our minimal level of functionality.
			// This is bad.
			// We should probably disable all functionality except for block production
			// and let the chain keep producing blocks until we can decide on a sufficiently
			// substantial set.
			// TODO: #2494
			(Self::slot_stake(), None)
		}
	}

	/// Check that list is sorted and has no duplicates.
	fn is_sorted_and_unique(list: &Vec<u32>) -> bool {
		list.windows(2).all(|w| w[0] < w[1])
	}

	/// Remove all associated data of a stash account from the staking system.
	///
	/// Assumes storage is upgraded before calling.
	///
	/// This is called :
	/// - Immediately when an account's balance falls below existential deposit.
	/// - after a `withdraw_unbond()` call that frees all of a stash's bonded balance.
	fn kill_stash(stash: &T::AccountId) {
		if let Some(controller) = <Bonded<T>>::take(stash) {
			<Ledger<T>>::remove(&controller);
		}
		<Payee<T>>::remove(stash);
		<Validators<T>>::remove(stash);
		<Nominators<T>>::remove(stash);

		slashing::clear_stash_metadata::<T>(stash);
	}

	/// Ensures that at the end of the current session there will be a new era.
	fn ensure_new_era() {
		match ForceEra::get() {
			Forcing::ForceAlways | Forcing::ForceNew => (),
			_ => ForceEra::put(Forcing::ForceNew),
		}
	}
}

impl<T: Trait> pallet_session::SessionManager<T::AccountId> for Module<T> {
	fn new_session(new_index: SessionIndex) -> Option<Vec<T::AccountId>> {
		if new_index == 0 {
			return Self::initial_session();
		}
		Self::new_session(new_index - 1)
	}
	fn start_session(_start_index: SessionIndex) {}
	fn end_session(_end_index: SessionIndex) {}
}

impl<T: Trait> SessionManager<T::AccountId, Exposure<T::AccountId, BalanceOf<T>>> for Module<T> {
	fn new_session(new_index: SessionIndex) -> Option<Vec<(T::AccountId, Exposure<T::AccountId, BalanceOf<T>>)>> {
		<Self as pallet_session::SessionManager<_>>::new_session(new_index).map(|validators| {
			validators
				.into_iter()
				.map(|v| {
					let exposure = <Stakers<T>>::get(&v);
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

impl<T: Trait> OnReapAccount<T::AccountId> for Module<T> {
	fn on_reap_account(stash: &T::AccountId) {
		Self::kill_stash(stash);
	}
}

/// A `Convert` implementation that finds the stash of the given controller account,
/// if any.
pub struct StashOf<T>(sp_std::marker::PhantomData<T>);

impl<T: Trait> Convert<T::AccountId, Option<T::AccountId>> for StashOf<T> {
	fn convert(controller: T::AccountId) -> Option<T::AccountId> {
		<Module<T>>::ledger(&controller).map(|l| l.stash)
	}
}

/// A typed conversion from stash account ID to the current exposure of nominators
/// on that account.
pub struct ExposureOf<T>(sp_std::marker::PhantomData<T>);

impl<T: Trait> Convert<T::AccountId, Option<Exposure<T::AccountId, BalanceOf<T>>>> for ExposureOf<T> {
	fn convert(validator: T::AccountId) -> Option<Exposure<T::AccountId, BalanceOf<T>>> {
		Some(<Module<T>>::stakers(&validator))
	}
}

/// This is intended to be used with `FilterHistoricalOffences`.
impl<T: Trait> OnOffenceHandler<T::AccountId, pallet_session::historical::IdentificationTuple<T>> for Module<T>
where
	T: pallet_session::Trait<ValidatorId = <T as frame_system::Trait>::AccountId>,
	T: pallet_session::historical::Trait<
		FullIdentification = Exposure<<T as frame_system::Trait>::AccountId, BalanceOf<T>>,
		FullIdentificationOf = ExposureOf<T>,
	>,
	T::SessionHandler: pallet_session::SessionHandler<<T as frame_system::Trait>::AccountId>,
	T::SessionManager: pallet_session::SessionManager<<T as frame_system::Trait>::AccountId>,
	T::ValidatorIdOf: Convert<<T as frame_system::Trait>::AccountId, Option<<T as frame_system::Trait>::AccountId>>,
{
	fn on_offence(
		offenders: &[OffenceDetails<T::AccountId, pallet_session::historical::IdentificationTuple<T>>],
		slash_fraction: &[Perbill],
		slash_session: SessionIndex,
	) {
		let reward_proportion = SlashRewardFraction::get();

		let era_now = Self::current_era();
		let window_start = era_now.saturating_sub(T::BondingDuration::get());
		let current_era_start_session = CurrentEraStartSessionIndex::get();

		// fast path for current-era report - most likely.
		let slash_era = if slash_session >= current_era_start_session {
			era_now
		} else {
			let eras = BondedEras::get();

			// reverse because it's more likely to find reports from recent eras.
			match eras
				.iter()
				.rev()
				.filter(|&&(_, ref sesh)| sesh <= &slash_session)
				.next()
			{
				None => return, // before bonding period. defensive - should be filtered out.
				Some(&(ref slash_era, _)) => *slash_era,
			}
		};

		let slash_defer_duration = T::SlashDeferDuration::get();

		for (details, slash_fraction) in offenders.iter().zip(slash_fraction) {
			let stash = &details.offender.0;
			let exposure = &details.offender.1;

			// Skip if the validator is invulnerable.
			if Self::invulnerables().contains(stash) {
				// Invulnerable validators do not get slashed
				debug::print!(
					"Invulnerable validator not slashed:{:?}, %:{:?}, session:{:?}",
					stash,
					slash_fraction,
					slash_session
				);
				Self::deposit_event(RawEvent::InvulnerableNotSlashed(stash.clone(), slash_fraction.clone()));
				continue;
			}

			let unapplied = slashing::compute_slash::<T>(slashing::SlashParams {
				stash,
				slash: *slash_fraction,
				exposure,
				slash_era,
				window_start,
				now: era_now,
				reward_proportion,
			});

			if let Some(mut unapplied) = unapplied {
				unapplied.reporters = details.reporters.clone();
				if slash_defer_duration == 0 {
					// apply right away.
					slashing::apply_slash::<T>(unapplied);
				} else {
					// defer to end of some `slash_defer_duration` from now.
					<Self as Store>::UnappliedSlashes::mutate(era_now, move |for_later| for_later.push(unapplied));

					<Self as Store>::EarliestUnappliedSlash::mutate(|earliest| {
						if earliest.is_none() {
							*earliest = Some(era_now)
						}
					});
				}
			}
		}
	}
}

/// Filter historical offences out and only allow those from the bonding period.
pub struct FilterHistoricalOffences<T, R> {
	_inner: sp_std::marker::PhantomData<(T, R)>,
}

impl<T, Reporter, Offender, R, O> ReportOffence<Reporter, Offender, O> for FilterHistoricalOffences<Module<T>, R>
where
	T: Trait,
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
}
