/* Copyright 2020-2021 Centrality Investments Limited
*
* Licensed under the LGPL, Version 3.0 (the "License");
* you may not use this file except in compliance with the License.
* Unless required by applicable law or agreed to in writing, software
* distributed under the License is distributed on an "AS IS" BASIS,
* WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
* See the License for the specific language governing permissions and
* limitations under the License.
* You may obtain a copy of the License at the root of this project source code,
* or at:
*     https://centrality.ai/licenses/gplv3.txt
*     https://centrality.ai/licenses/lgplv3.txt
*/

//! CENNZnet Staking Rewards
//! This module handles the economic model for payouts of staking rewards for validators and their nominators.
//! It also provides a simple treasury account suited for CENNZnet.
//!
//! The staking module should call into this module to trigger reward payouts at the end of an era.

use crate::{EraIndex, Exposure};
use crml_support::NotarizationRewardHandler;
use frame_support::{
	decl_event, decl_module, decl_storage,
	traits::{Currency, Get, Imbalance},
	weights::{constants::RocksDbWeight as DbWeight, DispatchClass, Weight},
	PalletId,
};
use frame_system::{self as system, ensure_root};
use sp_runtime::{
	traits::{AccountIdConversion, CheckedDiv, One, Saturating, UniqueSaturatedFrom, UniqueSaturatedInto, Zero},
	FixedPointNumber, FixedU128, Perbill,
};
use sp_std::{collections::vec_deque::VecDeque, prelude::*};

mod default_weights;
mod migration;
mod types;
pub use types::*;

/// A balance amount in the reward currency
type BalanceOf<T> = <<T as Config>::CurrencyToReward as Currency<<T as system::Config>::AccountId>>::Balance;

pub trait WeightInfo {
	fn process_reward_payouts(p: u32) -> Weight;
	fn process_zero_payouts() -> Weight;
}

pub trait Config: frame_system::Config {
	/// The system event type
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
	/// The reward currency system (total issuance, account balance, etc.) for payouts.
	type CurrencyToReward: Currency<Self::AccountId>;
	/// The treasury account for payouts
	type TreasuryPalletId: Get<PalletId>;
	/// The number of historical eras for which tx fee payout info should be retained.
	type HistoricalPayoutEras: Get<u16>;
	/// The number of staking eras in a fiscal era.
	type FiscalEraLength: Get<u32>;
	/// Handles running a scheduled payout
	type ScheduledPayoutRunner: RunScheduledPayout<AccountId = Self::AccountId, Balance = BalanceOf<Self>>;
	/// Extrinsic weight info
	type WeightInfo: WeightInfo;
}

decl_event!(
	pub enum Event<T>
	where
		Balance = BalanceOf<T>,
		AccountId = <T as frame_system::Config>::AccountId,
	{
		/// Staker payout (era, nominator/validator account, amount)
		EraStakerPayout(EraIndex, AccountId, Balance),
		/// Era reward payout the total (era, amount to treasury, amount to stakers)
		EraPayout(EraIndex, Balance, Balance),
		/// A fiscal era has begun with the parameter (target_inflation_per_staking_era)
		NewFiscalEra(Balance),
	}
);

decl_storage! {
	trait Store for Module<T: Config> as Rewards {
		/// Inflation rate % to apply on reward payouts
		pub BaseInflationRate get(fn inflation_rate) config(): FixedU128 = FixedU128::saturating_from_rational(1u64, 100u64);
		/// Development fund % take for reward payouts, parts-per-billion
		pub DevelopmentFundTake get(fn development_fund_take) config(): Perbill;
		/// Accumulated transaction fees for reward payout
		pub TransactionFeePot get(fn transaction_fee_pot): BalanceOf<T>;
		/// Historic accumulated transaction fees on reward payout
		pub TransactionFeePotHistory get(fn transaction_fee_pot_history): VecDeque<BalanceOf<T>>;
		/// Where the reward payment should be made. Keyed by stash.
		// TODO: migrate to blake2 to prevent trie unbalancing
		pub Payee: map hasher(twox_64_concat) T::AccountId => T::AccountId;
		/// Scheduled payout amounts keyed by (era, validator stash)
		pub ScheduledPayoutAmounts: double_map hasher(twox_64_concat) EraIndex, hasher(blake2_128_concat) T::AccountId => BalanceOf<T>;
		/// Scheduled payout eras and # of payouts to be made
		ScheduledPayoutErasAndCounts: Vec<(EraIndex, u32)>;
		/// The amount of new reward tokens that will be minted on every staking era in order to
		/// approximate the inflation rate. We calculate the target inflation based on
		/// T::CurrencyToReward::TotalIssuance() at the beginning of a fiscal era.
		TargetInflationPerStakingEra get(fn target_inflation_per_staking_era): BalanceOf<T>;
		/// The staking era index that specifies the start of a fiscal era based on which
		/// we can calculate the start of other fiscal eras. This is either 0 or forced by SUDO to
		/// another value. Have a look at force_new_fiscal_era for more info.
		FiscalEraEpoch get(fn fiscal_era_epoch): EraIndex;
		/// When true the next staking era will become the start of a new fiscal era.
		ForceFiscalEra get(fn force_fiscal_era): bool = false;
		/// Authorship rewards for the current active era.
		pub CurrentEraRewardPoints get(fn current_era_points): EraRewardPoints<T::AccountId>;
		/// Version of this module's storage schema
		StorageVersion build(|_: &GenesisConfig| Releases::V1 as u32): u32;
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {

		fn deposit_event() = default;

		/// Set the per payout inflation rate (`numerator` / `denominator`) (it may be negative)
		/// Please be advised that a newly set inflation rate would only affect the next fiscal year.
		#[weight = (10_000, DispatchClass::Operational)]
		pub fn set_inflation_rate(origin, numerator: u64, denominator: u64) {
			ensure_root(origin)?;
			if denominator.is_zero() {
				return Err("denominator cannot be zero".into());
			}
			BaseInflationRate::put(FixedU128::saturating_from_rational(numerator, denominator));
		}

		/// Set the development fund take %, capped at 100%.
		#[weight = (10_000, DispatchClass::Operational)]
		pub fn set_development_fund_take(origin, new_take_percent: u32) {
			ensure_root(origin)?;
			DevelopmentFundTake::put(
				Perbill::from_percent(new_take_percent) // `from_percent` will saturate at `100`
			);
		}

		/// Force a new fiscal era to start as soon as the next staking era.
		#[weight = (10_000, DispatchClass::Operational)]
		pub fn force_new_fiscal_era(origin) {
			ensure_root(origin)?;
			ForceFiscalEra::put(true);
		}

		fn on_runtime_upgrade() -> Weight {
			if StorageVersion::get() == Releases::V0 as u32 {
				migration::do_v0_to_v1::<T>()
			} else {
				Zero::zero()
			}
		}

		fn on_idle(_now: T::BlockNumber, remaining_weight: Weight) -> Weight {
			// +1 read  `SchedulePayoutErasAndCounts` and +1 write to update it at the end
			let mut consumed_weight = DbWeight::get().reads(1) + DbWeight::get().writes(1);
			if remaining_weight.checked_sub(consumed_weight).is_none() {
				return consumed_weight;
			}

			let mut payout_eras_and_counts = ScheduledPayoutErasAndCounts::get();
			if payout_eras_and_counts.is_empty() {
				return consumed_weight;
			}
			let (payout_era, mut payouts_left) = payout_eras_and_counts[0];
			let mut payouts_completed: Vec<T::AccountId> = vec![];

			// run until `remaining_weight` is consumed or the era's payouts are finished, whichever happens first
			for (validator_stash, amount) in ScheduledPayoutAmounts::<T>::iter_prefix(payout_era) {
				// +1 read on each loop and +1 write to remove it from storage later
				consumed_weight += DbWeight::get().reads(1) + DbWeight::get().writes(1);
				let estimated_weight = T::ScheduledPayoutRunner::estimate_run_payout_weight(&validator_stash, amount, payout_era);
				if let Some(_) = remaining_weight.checked_sub(consumed_weight + estimated_weight) {
					consumed_weight += T::ScheduledPayoutRunner::run_payout(&validator_stash, amount, payout_era);
					payouts_completed.push(validator_stash);
					payouts_left -= 1;
				} else {
					// exhausted idle weight
					break;
				}
			}

			// update era payout counts
			if payouts_left.is_zero() {
				// no payouts left for the era
				ScheduledPayoutErasAndCounts::put(payout_eras_and_counts[1..].to_vec());
			} else {
				payout_eras_and_counts[0].1 = payouts_left;
				ScheduledPayoutErasAndCounts::put(payout_eras_and_counts);
			}
			// remove completed payouts
			for stash in payouts_completed {
				ScheduledPayoutAmounts::<T>::remove(payout_era, stash);
			}

			return consumed_weight;
		}
	}
}

/// Reward points for bridge notaries
impl<T: Config> NotarizationRewardHandler for Module<T> {
	type AccountId = T::AccountId;
	fn reward_notary(notary: &Self::AccountId) {
		Self::reward_by_ids(vec![(notary.clone(), 8)])
	}
}

/// Add reward points to block authors:
/// * 20 points to the block producer for producing a (non-uncle) block in the relay chain,
/// * 2 points to the block producer for each reference to a previously unreferenced uncle, and
/// * 1 point to the producer of each referenced uncle block.
impl<T> pallet_authorship::EventHandler<T::AccountId, T::BlockNumber> for Module<T>
where
	T: Config + pallet_authorship::Config,
{
	fn note_author(author: T::AccountId) {
		Self::reward_by_ids(vec![(author, 20)])
	}
	fn note_uncle(author: T::AccountId, _age: T::BlockNumber) {
		Self::reward_by_ids(vec![(<pallet_authorship::Pallet<T>>::author(), 2), (author, 1)])
	}
}

impl<T: Config> OnEndEra for Module<T> {
	type AccountId = T::AccountId;
	/// A staking era has ended
	/// Check if we have a new fiscal era starting
	/// Schedule a staking reward payout
	fn on_end_era(era_validator_stashes: &[T::AccountId], era_index: EraIndex, era_duration_ms: u64, _is_forced: bool) {
		// calculate reward before changing fiscal era
		let next_reward = Self::calculate_total_reward(era_duration_ms);

		// Check if fiscal era should renew
		if ForceFiscalEra::get() {
			FiscalEraEpoch::put(era_index);
		}
		if era_index.saturating_sub(Self::fiscal_era_epoch()) % T::FiscalEraLength::get() == 0 {
			Self::new_fiscal_era();
		}

		// Setup staker payments 💪, delayed by 1 block
		let remainder = Self::schedule_reward_payouts(era_validator_stashes, next_reward.stakers_cut, era_index);

		// Deduct taxes from network spending
		let _ = T::CurrencyToReward::deposit_creating(
			&T::TreasuryPalletId::get().into_account(),
			next_reward.treasury_cut + remainder,
		);

		Self::deposit_event(RawEvent::EraPayout(
			era_index,
			next_reward.treasury_cut,
			next_reward.stakers_cut,
		));

		// Future tracking for dynamic inflation
		Self::note_fee_payout(next_reward.transaction_fees);

		// Clear storage for next eraœ
		TransactionFeePot::<T>::kill();
		CurrentEraRewardPoints::<T>::kill();
		ForceFiscalEra::kill();
	}
}

impl<T: Config> RewardCalculation for Module<T> {
	type AccountId = T::AccountId;
	type Balance = BalanceOf<T>;
	// 24 hour eras in milliseconds
	const FULL_ERA_DURATION: u64 = 1_000 * 3600 * 24;

	/// Calculate the total reward payout, considering the length of the era
	fn calculate_total_reward(era_duration_ms: u64) -> RewardParts<Self::Balance> {
		let inflation =
			Perbill::from_rational(era_duration_ms, Self::FULL_ERA_DURATION) * Self::target_inflation_per_staking_era();
		RewardParts::new(inflation, Self::transaction_fee_pot(), Self::development_fund_take())
	}

	/// Calculate the reward payout (accrued as of right now) for the given stash and era duration
	fn calculate_individual_reward(
		stash: &Self::AccountId,
		era_duration_ms: u64,
		validator_commission_stake_map: &[(Self::AccountId, Perbill, Exposure<Self::AccountId, Self::Balance>)],
	) -> Self::Balance {
		let mut payee_cut: Self::Balance = Zero::zero();

		let (_, _, payouts) = Self::calculate_payouts_filtered(
			validator_commission_stake_map,
			era_duration_ms,
			|validator, exposure| stash != validator && !exposure.others.iter().any(|x| &x.who == stash),
		);
		let payee = Self::payee(stash);
		payouts.into_iter().for_each(|(account, payout)| {
			if account == payee {
				payee_cut = payee_cut.saturating_add(payout);
			}
		});

		payee_cut
	}
}

impl<T: Config> HandlePayee for Module<T> {
	type AccountId = T::AccountId;

	/// Set the payment target for a stash account.
	fn set_payee(stash: &Self::AccountId, payee: &Self::AccountId) {
		Payee::<T>::insert(stash, payee);
	}

	/// Remove the corresponding stash-payee from the look up
	fn remove_payee(stash: &Self::AccountId) {
		Payee::<T>::remove(stash);
	}

	/// Return the payee account for the given stash account.
	fn payee(stash: &T::AccountId) -> Self::AccountId {
		let payee = Payee::<T>::get(stash);

		// a default value means it's unset, just return the stash
		// note this shouldn't occur in practice, this is useful for tests
		if payee == T::AccountId::default() {
			stash.clone()
		} else {
			payee
		}
	}
}

impl<T: Config> Module<T> {
	/// Call at the end of a staking era to schedule the calculation and distribution of rewards to stakers.
	/// Returns a remainder, the amount indivisble by the stakers
	///
	/// Payouts will be processed later consuming idle block weight
	///
	/// Requires `O(validators)` writes
	fn schedule_reward_payouts(
		validators: &[T::AccountId],
		total_staker_payout: BalanceOf<T>,
		era: EraIndex,
	) -> BalanceOf<T> {
		// Calculate the necessary total payout for each validator and it's stakers
		let (per_validator_payouts, remainder) =
			Self::calculate_per_validator_payouts(total_staker_payout, validators, Self::current_era_points());

		// Schedule the payouts for future blocks
		for (stash, amount) in per_validator_payouts.iter() {
			ScheduledPayoutAmounts::<T>::insert(era, stash, amount);
		}
		ScheduledPayoutErasAndCounts::mutate(|v| {
			v.push((era, validators.len() as u32));
			*v = v.clone();
		});

		remainder
	}

	/// Process the reward payout for the given validator stash and all its supporting nominators
	/// Requires `O(nominators)` writes
	pub fn process_reward_payout(
		validator_stash: &T::AccountId,
		validator_commission: Perbill,
		exposures: &Exposure<T::AccountId, BalanceOf<T>>,
		total_payout: BalanceOf<T>,
		era: EraIndex,
	) {
		if total_payout.is_zero() {
			return;
		}
		let mut total_payout_imbalance = T::CurrencyToReward::burn(Zero::zero());
		for (stash, amount) in
			Self::calculate_npos_payouts(validator_stash, validator_commission, exposures, total_payout)
		{
			total_payout_imbalance.subsume(T::CurrencyToReward::deposit_creating(&Self::payee(&stash), amount));
			Self::deposit_event(RawEvent::EraStakerPayout(era, stash, amount));
		}
		let remainder = total_payout.saturating_sub(total_payout_imbalance.peek());
		T::CurrencyToReward::deposit_creating(&T::TreasuryPalletId::get().into_account(), remainder);
	}

	/// Given a list of validator stashes, calculate the value of stake reward for
	/// each based on their block contribution ratio
	/// `stakers_cut` the initial reward amount to divvy up between validators
	fn calculate_per_validator_payouts(
		stakers_cut: BalanceOf<T>,
		validators: &[T::AccountId],
		era_reward_points: EraRewardPoints<T::AccountId>,
	) -> (Vec<(&T::AccountId, BalanceOf<T>)>, BalanceOf<T>) {
		let total_reward_points = era_reward_points.total;

		let mut remainder = stakers_cut;
		let payouts = validators
			.iter()
			.map(|validator| {
				let validator_reward_points = era_reward_points
					.individual
					.get(validator)
					.copied()
					.unwrap_or_else(Zero::zero);
				// This is how much every validator is entitled to get, including its nominators shares
				let payout = if total_reward_points.is_zero() {
					// When no authorship points are recorded, divide the payout equally
					stakers_cut / (validators.len() as u32).into()
				} else {
					Perbill::from_rational(validator_reward_points, total_reward_points) * stakers_cut
				};
				remainder -= payout;
				(validator, payout)
			})
			.collect::<Vec<(&T::AccountId, BalanceOf<T>)>>();

		(payouts, remainder)
	}

	/// Calculate all payouts of the current era as of right now. Then filter out those not relevant
	/// validator-exposure sets by calling the "filter" function.
	/// Return the total rewards calculated for the stakers at the time of this call paired with the
	/// the development cut and the list of calculated payouts.
	/// # Example: calculate and store payouts only for validators with less than 10% commission
	///
	/// ```ignore
	/// let filter = |_validator, exposure| exposure.commission > Perbill::from_percent(10);
	/// let (stakers_cut, development_cut, payouts) = Self::calculate_payouts_filtered(
	/// 														validator_commission_stake_map,
	///												 			filter);
	/// ```
	fn calculate_payouts_filtered<F>(
		validator_commission_stake_map: &[(T::AccountId, Perbill, Exposure<T::AccountId, BalanceOf<T>>)],
		era_duration_ms: u64,
		filter: F,
	) -> (BalanceOf<T>, BalanceOf<T>, Vec<(T::AccountId, BalanceOf<T>)>)
	where
		F: Fn(&T::AccountId, &Exposure<T::AccountId, BalanceOf<T>>) -> bool,
	{
		let payout = <Self as RewardCalculation>::calculate_total_reward(era_duration_ms);

		if payout.total.is_zero() {
			return (Zero::zero(), Zero::zero(), vec![]);
		}

		let era_reward_points = <CurrentEraRewardPoints<T>>::get();
		let total_reward_points = era_reward_points.total;

		let payouts = validator_commission_stake_map
			.iter()
			.flat_map(|(validator, validator_commission, stake_map)| {
				// Nothing to do if this entry should be filtered out
				if filter(validator, stake_map) {
					return vec![];
				}

				let validator_reward_points = era_reward_points
					.individual
					.get(validator)
					.copied()
					.unwrap_or_else(Zero::zero);

				// This is how much every validator is entitled to get, including its nominators shares
				let validator_total_payout = if total_reward_points.is_zero() {
					// When no authorship points are recorded, divide the payout equally
					payout.stakers_cut / (validator_commission_stake_map.len() as u32).into()
				} else {
					Perbill::from_rational(validator_reward_points, total_reward_points) * payout.stakers_cut
				};

				if validator_total_payout.is_zero() {
					return vec![];
				}

				Self::calculate_npos_payouts(&validator, *validator_commission, stake_map, validator_total_payout)
			})
			.collect();

		(payout.stakers_cut, payout.treasury_cut, payouts)
	}

	/// Add the given `fee` amount to the next reward payout
	pub fn note_transaction_fees(amount: BalanceOf<T>) {
		TransactionFeePot::<T>::mutate(|acc| *acc = acc.saturating_add(amount));
	}

	/// Note a fee payout for future calculations Retaining only the latest `T::HistoricalPayoutEras::get()`
	fn note_fee_payout(amount: BalanceOf<T>) {
		let mut history = TransactionFeePotHistory::<T>::get();
		history.push_front(amount);
		history.truncate(T::HistoricalPayoutEras::get() as usize); // truncate the oldest
		TransactionFeePotHistory::<T>::put(history);
	}

	/// Calculate NPoS payouts given a `reward` amount for a `validator` account and its nominators.
	/// The reward schedule is as follows:
	/// 1) The validator receives an 'off the table' portion of the `reward` given by it's `validator_commission_rate`.
	/// 2) The remaining reward is distributed to nominators based on their individual contribution to the total stake behind the `validator`.
	/// Returns the payouts to be paid as (stash, amount)
	fn calculate_npos_payouts(
		validator: &T::AccountId,
		validator_commission_rate: Perbill,
		validator_stake: &Exposure<T::AccountId, BalanceOf<T>>,
		reward: BalanceOf<T>,
	) -> Vec<(T::AccountId, BalanceOf<T>)> {
		let validator_cut = (validator_commission_rate * reward).min(reward);
		let nominators_cut = reward.saturating_sub(validator_cut);

		if nominators_cut.is_zero() {
			// There's nothing left after validator has taken it's commission
			// only the validator gets a payout.
			return vec![(validator.clone(), validator_cut)];
		}

		// There's some reward to distribute to nominators.
		// Distribute a share of the `nominators_cut` to each nominator based on it's contribution to the `validator`'s total stake.
		let mut payouts = Vec::with_capacity(validator_stake.others.len().saturating_add(One::one()));
		let aggregate_validator_stake = validator_stake.total.max(One::one());

		// Iterate all nominator staked amounts
		for nominator_stake in &validator_stake.others {
			let contribution_ratio = Perbill::from_rational(nominator_stake.value, aggregate_validator_stake);
			payouts.push((Self::payee(&nominator_stake.who), contribution_ratio * nominators_cut));
		}

		// Finally payout the validator. commission (`validator_cut`) + it's share of the `nominators_cut`
		// As a validator always self-nominates using it's own stake.
		let validator_contribution_ratio = Perbill::from_rational(validator_stake.own, aggregate_validator_stake);

		// this cannot overflow, `validator_cut` is a fraction of `reward`
		payouts.push((
			Self::payee(validator),
			(validator_contribution_ratio * nominators_cut) + validator_cut,
		));
		(*payouts).to_vec()
	}

	/// Start a new fiscal era. Calculate the new inflation target based on the latest set inflation rate.
	pub fn new_fiscal_era() {
		let total_issuance: u128 = T::CurrencyToReward::total_issuance().unique_saturated_into();
		let target_inflation =
			<BalanceOf<T>>::unique_saturated_from(Self::inflation_rate().saturating_mul_int(total_issuance));
		let target_inflation_per_staking_era = target_inflation
			.checked_div(&T::FiscalEraLength::get().into())
			.unwrap_or_else(Zero::zero);
		<TargetInflationPerStakingEra<T>>::put(target_inflation_per_staking_era);

		Self::deposit_event(RawEvent::NewFiscalEra(target_inflation_per_staking_era));
	}

	/// Add reward points to validators using their stash account ID.
	///
	/// Validators are keyed by stash account ID and must be in the current elected set.
	///
	/// For each element in the iterator the given number of points in u32 is added to the
	/// validator, thus duplicates are handled.
	///
	/// At the end of the era each the total payout will be distributed among validator
	/// relatively to their points.
	///
	/// COMPLEXITY: Complexity is `number_of_validator_to_reward x current_elected_len`.
	pub fn reward_by_ids(validators_points: impl IntoIterator<Item = (T::AccountId, u32)>) {
		<CurrentEraRewardPoints<T>>::mutate(|era_rewards| {
			for (validator, points) in validators_points.into_iter() {
				*era_rewards.individual.entry(validator).or_default() += points;
				era_rewards.total += points;
			}
		});
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{rewards, IndividualExposure};
	use crml_generic_asset::impls::TransferDustImbalance;
	use frame_support::{
		assert_err, assert_noop, assert_ok, parameter_types,
		traits::{Currency, OnIdle},
		PalletId, StorageValue,
	};
	use pallet_authorship::EventHandler;
	use sp_core::H256;
	use sp_runtime::{
		testing::Header,
		traits::{AccountIdConversion, BadOrigin, BlakeTwo256, IdentityLookup, Zero},
		FixedPointNumber, FixedU128, Perbill,
	};

	/// The account Id type in this test runtime
	type AccountId = u64;
	/// The asset Id type in this test runtime
	type AssetId = u64;
	/// The balance type in this test runtime
	type Balance = u64;

	type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
	type Block = frame_system::mocking::MockBlock<Test>;

	frame_support::construct_runtime!(
		pub enum Test where
			Block = Block,
			NodeBlock = Block,
			UncheckedExtrinsic = UncheckedExtrinsic,
		{
			System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
			GenericAsset: crml_generic_asset::{Pallet, Call, Storage, Config<T>, Event<T>},
			Authorship: pallet_authorship::{Pallet, Call, Storage},
			Rewards: rewards::{Pallet, Call, Storage, Config, Event<T>},
		}
	);

	parameter_types! {
		pub const BlockHashCount: u64 = 250;
	}
	impl frame_system::Config for Test {
		type BaseCallFilter = frame_support::traits::Everything;
		type BlockWeights = ();
		type BlockLength = ();
		type DbWeight = ();
		type Origin = Origin;
		type Index = u64;
		type Call = Call;
		type BlockNumber = u64;
		type Hash = H256;
		type Hashing = BlakeTwo256;
		type AccountId = u64;
		type Lookup = IdentityLookup<Self::AccountId>;
		type Header = Header;
		type Event = Event;
		type BlockHashCount = BlockHashCount;
		type Version = ();
		type PalletInfo = PalletInfo;
		type AccountData = ();
		type OnNewAccount = ();
		type OnKilledAccount = ();
		type SystemWeightInfo = ();
		type SS58Prefix = ();
		type OnSetCode = ();
	}

	impl crml_generic_asset::Config for Test {
		type AssetId = AssetId;
		type Balance = Balance;
		type Event = Event;
		type OnDustImbalance = TransferDustImbalance<TreasuryPalletId>;
		type WeightInfo = ();
	}

	impl pallet_authorship::Config for Test {
		type FindAuthor = crate::mock::Author11;
		type UncleGenerations = crate::mock::UncleGenerations;
		type FilterUncle = ();
		type EventHandler = Rewards;
	}

	parameter_types! {
		pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
		pub const HistoricalPayoutEras: u16 = 7;
		pub const FiscalEraLength: u32 = 5;
	}
	impl Config for Test {
		type CurrencyToReward = crml_generic_asset::SpendingAssetCurrency<Self>;
		type Event = Event;
		type FiscalEraLength = FiscalEraLength;
		type HistoricalPayoutEras = HistoricalPayoutEras;
		type ScheduledPayoutRunner = MockPayoutRunner<Self>;
		type TreasuryPalletId = TreasuryPalletId;
		type WeightInfo = ();
	}

	/// A payout runner which deposits the reward immediately
	pub struct MockPayoutRunner<T: Config>(sp_std::marker::PhantomData<T>);

	impl<T: Config> RunScheduledPayout for MockPayoutRunner<T> {
		type AccountId = T::AccountId;
		type Balance = BalanceOf<T>;

		/// Make a payout to stash for the given era
		fn run_payout(stash: &Self::AccountId, amount: Self::Balance, _era_index: EraIndex) -> Weight {
			let _ = T::CurrencyToReward::deposit_creating(stash, amount);
			return 1_000 as Weight;
		}

		fn estimate_run_payout_weight(
			_validator_stash: &Self::AccountId,
			_amount: Self::Balance,
			_era_index: EraIndex,
		) -> Weight {
			return 1_000 as Weight;
		}
	}

	// Provides configurable mock genesis storage data.
	#[derive(Default)]
	pub struct ExtBuilder {
		// The inflation rate (numerator, denominator)
		inflation_rate: (u32, u32),
	}

	impl ExtBuilder {
		/// Set the inflation rate
		pub fn set_inflation_rate(mut self, inflation_rate: (u32, u32)) -> Self {
			self.inflation_rate = inflation_rate;
			self
		}
		/// Setup mock genesis, starts a new fiscal era to set inflation rate
		pub fn build(self) -> sp_io::TestExternalities {
			let mut storage = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

			// denominator can't be zero
			let inflation_denominator = self.inflation_rate.1.max(One::one());
			let _ = rewards::GenesisConfig {
				inflation_rate: FixedU128::saturating_from_rational(self.inflation_rate.0, inflation_denominator),
				development_fund_take: Perbill::from_percent(10),
			}
			.assimilate_storage(&mut storage);

			let _ = crml_generic_asset::GenesisConfig::<Test> {
				endowed_accounts: vec![10, 11],
				initial_balance: 500,
				staking_asset_id: 16000,
				spending_asset_id: 16001,
				assets: vec![16000, 16001],
				next_asset_id: 16002,
				permissions: vec![],
				asset_meta: vec![],
			}
			.assimilate_storage(&mut storage);

			let mut ext = sp_io::TestExternalities::from(storage);
			ext.execute_with(|| {
				System::initialize(&1, &[0u8; 32].into(), &Default::default(), Default::default());
				Rewards::new_fiscal_era();
			});

			ext
		}
	}

	/// Helper for creating the info required for validator reward payout
	struct MockCommissionStakeInfo {
		validator_stash: AccountId,
		commission: Perbill,
		exposures: Exposure<AccountId, Balance>,
	}

	impl MockCommissionStakeInfo {
		/// Helper constructor
		fn new(
			validator_exposure: (AccountId, Balance),
			nominator_exposures: Vec<(AccountId, Balance)>,
			validator_commission: Perbill,
		) -> Self {
			let exposures = nominator_exposures
				.iter()
				.map(|x| IndividualExposure { who: x.0, value: x.1 })
				.collect();
			let total_nominator_exposure: Balance = nominator_exposures.iter().map(|(_, value)| value).sum();
			let exposures = Exposure {
				total: total_nominator_exposure + validator_exposure.1,
				own: validator_exposure.1,
				others: exposures,
			};

			MockCommissionStakeInfo {
				validator_stash: validator_exposure.0,
				commission: validator_commission,
				exposures,
			}
		}

		fn as_tuple(&self) -> (AccountId, Perbill, Exposure<AccountId, Balance>) {
			(self.validator_stash, self.commission, self.exposures.clone())
		}
	}

	#[test]
	fn set_and_change_payee_correctly() {
		ExtBuilder::default().build().execute_with(|| {
			// Return the same id, if a separate payee is not set
			assert_eq!(Rewards::payee(&7), 7);

			Rewards::set_payee(&10, &10);
			assert_eq!(Rewards::payee(&10), 10);

			Rewards::set_payee(&10, &11);
			assert_eq!(Rewards::payee(&10), 11);

			// Setting the payee address back to the stash
			Rewards::set_payee(&10, &10);
			assert_eq!(Rewards::payee(&10), 10);
		});
	}

	#[test]
	fn note_transaction_fees() {
		ExtBuilder::default().build().execute_with(|| {
			// successive transaction fees are added to the pot
			assert!(Rewards::transaction_fee_pot().is_zero());
			let noted = 1_234;
			Rewards::note_transaction_fees(noted);
			assert_eq!(Rewards::transaction_fee_pot(), noted);
			Rewards::note_transaction_fees(noted);
			assert_eq!(Rewards::transaction_fee_pot(), noted * 2);
		});
	}

	#[test]
	fn note_fee_payout_retains_n_latest() {
		// note multiple fee payouts, it should keep only the latest n in state.
		ExtBuilder::default().build().execute_with(|| {
			let historical_payouts = [1_000_u64; <Test as Config>::HistoricalPayoutEras::get() as usize];
			for payout in &historical_payouts {
				Rewards::note_fee_payout(*payout);
			}

			assert_eq!(Rewards::transaction_fee_pot_history(), historical_payouts);

			let new_payouts = vec![1_111_u64, 2_222, 3_333_u64];
			for latest_payout in new_payouts.iter() {
				// oldest payouts are replaced by the newest
				Rewards::note_fee_payout(*latest_payout);
				assert_eq!(Rewards::transaction_fee_pot_history().front(), Some(latest_payout));
			}

			assert_eq!(
				Rewards::transaction_fee_pot_history(),
				// new_payouts     historical_payouts[3..]
				[3333, 2222, 1111, 1000, 1000, 1000, 1000]
			);
		});
	}

	#[test]
	fn set_inflation_rate() {
		// only root
		// value is set
		ExtBuilder::default().build().execute_with(|| {
			assert_noop!(Rewards::set_inflation_rate(Origin::signed(1), 1, 1_000), BadOrigin);
			assert_ok!(Rewards::set_inflation_rate(Origin::root(), 1, 1_000));
			assert_eq!(Rewards::inflation_rate(), FixedU128::saturating_from_rational(1, 1_000))
		});
	}

	#[test]
	fn set_inflation_rate_bounds() {
		ExtBuilder::default().build().execute_with(|| {
			assert_noop!(
				Rewards::set_inflation_rate(Origin::root(), 0, 0),
				"denominator cannot be zero"
			);
			assert_ok!(Rewards::set_inflation_rate(
				Origin::root(),
				u64::max_value(),
				u64::max_value()
			));
			assert_ok!(Rewards::set_inflation_rate(
				Origin::root(),
				u64::min_value(),
				u64::max_value()
			));
			assert_ok!(Rewards::set_inflation_rate(Origin::root(), 1, u64::max_value()));
		});
	}

	#[test]
	fn emits_new_fiscal_era_event() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Rewards::set_inflation_rate(Origin::root(), 3, 10));
			Rewards::new_fiscal_era();

			let events = System::events();
			assert_eq!(events.last().unwrap().event, Event::Rewards(RawEvent::NewFiscalEra(60)));
		});
	}

	#[test]
	fn fiscal_era_should_naturally_take_fiscal_era_length_eras() {
		ExtBuilder::default().build().execute_with(|| {
			// There should be an event for a new fiscal era on era 0 (due to ext builder setup)
			assert_ok!(Rewards::set_inflation_rate(Origin::root(), 7, 100));
			let era_duration_ms = <Rewards as RewardCalculation>::FULL_ERA_DURATION;
			Rewards::on_end_era(&vec![], 0, era_duration_ms, false);

			let era_1_inflation_target = 14;
			let expected_event = Event::Rewards(RawEvent::NewFiscalEra(era_1_inflation_target));
			assert!(System::events().iter().any(|record| record.event == expected_event));
			System::reset_events();

			// No fiscal era events are expected for the following eras
			Rewards::on_end_era(&vec![], 1, era_duration_ms, false);
			Rewards::on_end_era(&vec![], 2, era_duration_ms, false);
			assert!(!System::events().iter().any(|record| match record.event {
				Event::Rewards(RawEvent::NewFiscalEra(_)) => true,
				_ => false,
			}));

			// Request inflation rate change, it shouldn't apply until the next fiscal era
			assert_ok!(Rewards::set_inflation_rate(Origin::root(), 11, 100));
			assert_eq!(Rewards::target_inflation_per_staking_era(), era_1_inflation_target);

			Rewards::on_end_era(&vec![], 3, era_duration_ms, false);
			Rewards::on_end_era(&vec![], 4, era_duration_ms, false);
			Rewards::on_end_era(&vec![], 5, era_duration_ms, false);

			let era_2_inflation_target = 23;
			// The newly set inflation rate is going to take effect with a new fiscal era
			let expected_event = Event::Rewards(RawEvent::NewFiscalEra(era_2_inflation_target));
			assert!(System::events().iter().any(|record| record.event == expected_event));
			assert_eq!(Rewards::target_inflation_per_staking_era(), era_2_inflation_target);
		});
	}

	#[test]
	fn force_new_fiscal_era() {
		ExtBuilder::default()
			.set_inflation_rate((1, 100))
			.build()
			.execute_with(|| {
				let era_duration_ms = <Rewards as RewardCalculation>::FULL_ERA_DURATION;
				// set a new annual inflation rate
				assert_ok!(Rewards::set_inflation_rate(Origin::root(), 7, 100));
				// the default fiscal rate should still be in effect 1%
				assert_eq!(Rewards::target_inflation_per_staking_era(), 2);

				// force a fiscal era change next era
				assert_ok!(Rewards::force_new_fiscal_era(Origin::root()));
				assert!(Rewards::force_fiscal_era());
				// Even after "force" the inflation rate is not going to change if a new staking era has not begun
				assert_eq!(Rewards::target_inflation_per_staking_era(), 2);

				// Trigger era end, new fiscal era should be enacted
				Rewards::on_end_era(&vec![], 0, era_duration_ms, false);

				let expected_event = Event::Rewards(RawEvent::NewFiscalEra(14));
				let events = System::events();
				assert!(events.iter().any(|record| record.event == expected_event));

				assert_eq!(Rewards::target_inflation_per_staking_era(), 14);
				assert!(!Rewards::force_fiscal_era());
			});
	}

	#[test]
	fn set_development_fund_take() {
		// only root
		// value is set
		ExtBuilder::default().build().execute_with(|| {
			assert_err!(Rewards::set_development_fund_take(Origin::signed(1), 80), BadOrigin);
			assert_ok!(Rewards::set_development_fund_take(Origin::root(), 80));
			assert_eq!(Rewards::development_fund_take(), Perbill::from_percent(80))
		});
	}

	#[test]
	fn set_development_fund_take_saturates() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Rewards::set_development_fund_take(Origin::root(), u32::max_value()));
			assert_eq!(Rewards::development_fund_take(), Perbill::from_percent(100))
		});
	}

	#[test]
	fn calculate_total_reward_cases() {
		ExtBuilder::default().build().execute_with(|| {
			let tx_fees = 10;
			let base_inflation = 20;
			let era_duration_ms = <Rewards as RewardCalculation>::FULL_ERA_DURATION;

			// the basic reward model on CENNZnet is base inflation + fees
			TransactionFeePot::<Test>::put(tx_fees);
			TargetInflationPerStakingEra::<Test>::put(base_inflation);

			let next_reward = Rewards::calculate_total_reward(era_duration_ms);
			assert_eq!(tx_fees, next_reward.transaction_fees,);
			assert_eq!(base_inflation, next_reward.inflation,);
			assert_eq!(base_inflation + tx_fees, next_reward.total);
			assert_eq!(
				DevelopmentFundTake::get() * next_reward.transaction_fees,
				next_reward.treasury_cut
			);
			assert_eq!(
				next_reward.total - (DevelopmentFundTake::get() * next_reward.transaction_fees),
				next_reward.stakers_cut
			);

			// no tx fees, still rewards based on inflation
			TransactionFeePot::<Test>::put(0);
			let next_reward = Rewards::calculate_total_reward(era_duration_ms);
			assert!(next_reward.transaction_fees.is_zero());
			assert_eq!(base_inflation, next_reward.inflation);
			assert_eq!(base_inflation, next_reward.total);

			// no inflation, still rewards tx fees
			TransactionFeePot::<Test>::put(tx_fees);
			TargetInflationPerStakingEra::<Test>::put(0);
			let next_reward = Rewards::calculate_total_reward(era_duration_ms);
			assert_eq!(tx_fees, next_reward.transaction_fees);
			assert!(next_reward.inflation.is_zero());
			assert_eq!(tx_fees, next_reward.total);

			// Era duration cases
			// longer than max era duraiton, inflation saturates at 100%
			TargetInflationPerStakingEra::<Test>::put(base_inflation);
			let next_reward = Rewards::calculate_total_reward(era_duration_ms + 36_000);
			assert_eq!(next_reward.inflation, base_inflation);
			// inflation reward is 0
			let next_reward = Rewards::calculate_total_reward(0);
			assert!(next_reward.inflation.is_zero());
			// era duraiton is halved => inflation reward is halved
			let next_reward = Rewards::calculate_total_reward(era_duration_ms / 2);
			assert_eq!(next_reward.inflation, base_inflation / 2);
		});
	}

	#[test]
	fn calculate_npos_payouts() {
		ExtBuilder::default().build().execute_with(|| {
			let mock_commission_stake_map =
				MockCommissionStakeInfo::new((1, 1_000), vec![(2, 2_000), (3, 2_000)], Perbill::from_percent(10));
			let staker_reward = 1_000_000;

			let payouts = Rewards::calculate_npos_payouts(
				&mock_commission_stake_map.validator_stash,
				mock_commission_stake_map.commission,
				&mock_commission_stake_map.exposures,
				staker_reward,
			);

			// validator takes 10% of reward + 20% of 90% remainder
			// nominators split 80% of 90% remainder at 50/50 each
			let validator_commission = Perbill::from_percent(10) * staker_reward;
			let reward_share = Perbill::from_percent(90) * staker_reward;
			let validator_share = Perbill::from_percent(20) * reward_share;
			let nominator_share = Perbill::from_percent(80) * reward_share;
			assert_eq!(
				payouts,
				vec![
					(2, nominator_share / 2),
					(3, nominator_share / 2),
					(1, validator_commission + validator_share)
				]
			);
		});
	}

	#[test]
	fn calculate_npos_payouts_zero_commission() {
		ExtBuilder::default().build().execute_with(|| {
			let mock_commission_stake_map = MockCommissionStakeInfo::new(
				(1, 1_000),
				vec![(2, 2_000), (3, 2_000)],
				Perbill::from_percent(Zero::zero()),
			);
			let staker_reward = 1_000_000;

			let payouts = Rewards::calculate_npos_payouts(
				&mock_commission_stake_map.validator_stash,
				mock_commission_stake_map.commission,
				&mock_commission_stake_map.exposures,
				staker_reward,
			);

			// splits according to stake
			assert_eq!(
				payouts,
				vec![
					(2, staker_reward * 2 / 5),
					(3, staker_reward * 2 / 5),
					(1, staker_reward * 1 / 5)
				]
			);
		})
	}

	#[test]
	fn calculate_npos_payouts_no_nominators() {
		ExtBuilder::default().build().execute_with(|| {
			let validator_id = 1;
			let mock_commission_stake_map =
				MockCommissionStakeInfo::new((validator_id, 1_000), vec![], Perbill::from_percent(10));
			let staker_reward = 1_000_000;

			let payouts = Rewards::calculate_npos_payouts(
				&mock_commission_stake_map.validator_stash,
				mock_commission_stake_map.commission,
				&mock_commission_stake_map.exposures,
				staker_reward,
			);

			// validator takes 100% of the staker_reward
			assert_eq!(payouts, vec![(validator_id, staker_reward)]);
		});
	}

	#[test]
	fn calculate_npos_payouts_zero_reward() {
		ExtBuilder::default().build().execute_with(|| {
			let mock_commission_stake_map =
				MockCommissionStakeInfo::new((1, 1_000), vec![(2, 2_000), (3, 2_000)], Perbill::from_percent(10));

			let payouts = Rewards::calculate_npos_payouts(
				&mock_commission_stake_map.validator_stash,
				mock_commission_stake_map.commission,
				&mock_commission_stake_map.exposures,
				0,
			);

			// validator takes 100% of the reward
			assert_eq!(payouts, vec![(1, 0)]);
		});
	}

	#[test]
	fn calculate_npos_payouts_saturate_validator_commission() {
		ExtBuilder::default().build().execute_with(|| {
			// validator requests 200% commission
			let mock_commission_stake_map = MockCommissionStakeInfo::new(
				(1, 1_000),
				vec![(2, 2_000), (3, 2_000)],
				Perbill::from_rational(2u32, 1u32),
			);

			let reward = 1_000;
			let payouts = Rewards::calculate_npos_payouts(
				&mock_commission_stake_map.validator_stash,
				mock_commission_stake_map.commission,
				&mock_commission_stake_map.exposures,
				reward,
			);

			// validator only takes 100% of the reward
			assert_eq!(payouts, vec![(1, reward)]);
		});
	}

	#[test]
	fn reward_from_authorship_event_handler_works() {
		ExtBuilder::default().build().execute_with(|| {
			assert_eq!(<pallet_authorship::Pallet<Test>>::author(), 11);

			Rewards::note_author(11);
			Rewards::note_uncle(21, 1);
			// An uncle author that is not currently elected doesn't get rewards,
			// but the block producer does get reward for referencing it.
			Rewards::note_uncle(31, 1);
			// Rewarding the same two times works.
			Rewards::note_uncle(11, 1);

			// 21 is rewarded as an uncle producer
			// 11 is rewarded as a block producer and uncle referencer and uncle producer
			let reward_points: Vec<RewardPoint> = <CurrentEraRewardPoints<Test>>::get()
				.individual
				.values()
				.cloned()
				.collect();
			assert_eq!(reward_points, vec![20 + 2 * 3 + 1, 1, 1]);
			assert_eq!(<CurrentEraRewardPoints<Test>>::get().total, 29);
		})
	}

	#[test]
	fn add_reward_points_fns_works() {
		ExtBuilder::default().build().execute_with(|| {
			let alice: AccountId = 11;
			let bob: AccountId = 21;
			let charlie: AccountId = 31;
			Rewards::reward_by_ids(vec![(bob, 1), (alice, 1), (charlie, 1), (alice, 1)]);

			let reward_points: Vec<RewardPoint> = <CurrentEraRewardPoints<Test>>::get()
				.individual
				.values()
				.cloned()
				.collect();
			assert_eq!(reward_points, vec![2, 1, 1]);
			assert_eq!(<CurrentEraRewardPoints<Test>>::get().total, 4);
		})
	}

	#[test]
	fn calculate_accrued_reward() {
		ExtBuilder::default().build().execute_with(|| {
			let stake_map_1 = MockCommissionStakeInfo::new((1, 1_000), vec![(4, 1_000)], Perbill::from_percent(10));
			let stake_map_2 =
				MockCommissionStakeInfo::new((2, 2_000), vec![(4, 1_000), (5, 1_000)], Perbill::from_percent(5));
			let stake_map_3 =
				MockCommissionStakeInfo::new((3, 3_000), vec![(5, 1_000), (6, 2_000)], Perbill::from_percent(2));

			Rewards::reward_by_ids(vec![(1, 30), (2, 50), (3, 20)]);

			assert_ok!(Rewards::set_inflation_rate(Origin::root(), 1, 20));
			Rewards::new_fiscal_era();

			let fee_payout = 1_000_000;
			Rewards::note_transaction_fees(fee_payout);

			let era_duration_ms = <Rewards as RewardCalculation>::FULL_ERA_DURATION;
			let total_payout = Rewards::calculate_total_reward(era_duration_ms);

			// According to the authorship reward points
			let staked_on_1_reward_share = Perbill::from_percent(30) * total_payout.stakers_cut;
			let staked_on_2_reward_share = Perbill::from_percent(50) * total_payout.stakers_cut;
			let staked_on_3_reward_share = Perbill::from_percent(20) * total_payout.stakers_cut;

			// According to the commissions
			let nominated_1_reward_share = Perbill::from_percent(90) * staked_on_1_reward_share;
			let nominated_2_reward_share = Perbill::from_percent(95) * staked_on_2_reward_share;
			let nominated_3_reward_share = Perbill::from_percent(98) * staked_on_3_reward_share;

			// According to the stakes
			let reward_to_4 = nominated_1_reward_share / 2 + nominated_2_reward_share / 4;
			let reward_to_5 = nominated_2_reward_share / 4 + nominated_3_reward_share / 6;
			let reward_to_6 = nominated_3_reward_share / 3 + 1; // + 1 is needed due to the integer calculation inaccuracy

			let era_duration_ms = <Rewards as RewardCalculation>::FULL_ERA_DURATION;
			assert_eq!(
				Rewards::calculate_individual_reward(
					&4,
					era_duration_ms,
					&[stake_map_1.as_tuple(), stake_map_2.as_tuple(), stake_map_3.as_tuple()]
				),
				reward_to_4
			);
			assert_eq!(
				Rewards::calculate_individual_reward(
					&5,
					era_duration_ms,
					&[stake_map_1.as_tuple(), stake_map_2.as_tuple(), stake_map_3.as_tuple()]
				),
				reward_to_5
			);
			assert_eq!(
				Rewards::calculate_individual_reward(
					&6,
					era_duration_ms,
					&[stake_map_1.as_tuple(), stake_map_2.as_tuple(), stake_map_3.as_tuple()]
				),
				reward_to_6
			);

			let reward_to_1 = staked_on_1_reward_share - nominated_1_reward_share / 2 - 1; // - 1 is needed due to the integer calculation inaccuracy
			assert_eq!(
				Rewards::calculate_individual_reward(
					&1,
					era_duration_ms,
					&[stake_map_1.as_tuple(), stake_map_2.as_tuple(), stake_map_3.as_tuple()]
				),
				reward_to_1
			);
		});
	}

	#[test]
	fn accrued_reward_when_payee_is_not_stash() {
		ExtBuilder::default().build().execute_with(|| {
			Rewards::set_payee(&4, &5);
			assert_eq!(Rewards::payee(&4), 5);

			Rewards::note_transaction_fees(1000);
			Rewards::reward_by_ids(vec![(1, 20)]);

			assert!(
				Rewards::calculate_individual_reward(
					&4,
					<Rewards as RewardCalculation>::FULL_ERA_DURATION,
					&[
						MockCommissionStakeInfo::new((1, 1_000), vec![(4, 1_000)], Perbill::from_percent(10))
							.as_tuple()
					]
				) > 0
			);
		});
	}

	#[test]
	fn validators_reward_split_returns_remainder() {
		// payout is indivisible by number of stakers so there is a remainder
		// nb: validators have equal 0 reward points for simplicity
		let (_per_validator_payouts, remainder) =
			Rewards::calculate_per_validator_payouts(10_000, &vec![1, 2, 3], EraRewardPoints::default());

		assert!(remainder > 0);
		assert_eq!(remainder, (10_000 - (10_000 / 3) * 3));
	}

	#[test]
	fn validator_reward_split_according_to_points() {
		ExtBuilder::default().build().execute_with(|| {
			let (validator_1, validator_2, validator_3) = (11, 21, 31);

			let fee = 628;
			Rewards::note_transaction_fees(fee);

			// 42 points to validator_1
			// 21 points to validator_2
			// 0 blocks/points to validator_3
			Rewards::note_author(validator_1);
			Rewards::note_author(validator_1);
			Rewards::note_author(validator_2);
			Rewards::note_uncle(validator_2, 1); // 2 points to the actual author here validator_1, 1 point to validator_2
			let era_duration_ms = <Rewards as RewardCalculation>::FULL_ERA_DURATION;

			let validator_total_payout = Rewards::calculate_total_reward(era_duration_ms).stakers_cut;

			let validators = vec![validator_1, validator_2, validator_3];
			let authoring_points = CurrentEraRewardPoints::<Test>::get();
			let (per_validator_payouts, _remainder) =
				Rewards::calculate_per_validator_payouts(validator_total_payout, &validators, authoring_points.clone());

			let validator_1_payout = Perbill::from_rational(
				authoring_points
					.individual
					.get(&validator_1)
					.map(|points| *points)
					.unwrap(),
				authoring_points.total,
			) * validator_total_payout;

			let validator_2_payout = Perbill::from_rational(
				authoring_points
					.individual
					.get(&validator_2)
					.map(|points| *points)
					.unwrap(),
				authoring_points.total,
			) * validator_total_payout;

			assert_eq!(
				per_validator_payouts,
				vec![
					(&validator_1, validator_1_payout),
					(&validator_2, validator_2_payout),
					(&validator_3, Zero::zero())
				]
			);
		})
	}

	#[test]
	fn validator_reward_equal_split_when_no_points() {
		// this should never happen in practice, it is here defensively
		ExtBuilder::default().build().execute_with(|| {
			let validators = [11_u64, 21, 31];
			let [validator_1, validator_2, validator_3] = validators;

			let validator_total_payout = 333;
			let authoring_points = CurrentEraRewardPoints::<Test>::get();
			let (per_validator_payouts, _remainder) =
				Rewards::calculate_per_validator_payouts(validator_total_payout, &validators, authoring_points.clone());

			assert_eq!(
				per_validator_payouts,
				vec![
					(&validator_1, validator_total_payout / 3),
					(&validator_2, validator_total_payout / 3),
					(&validator_3, validator_total_payout / 3)
				]
			);
		})
	}

	#[test]
	fn schedule_reward_payouts_per_era() {
		ExtBuilder::default().build().execute_with(|| {
			let era = 1;
			let validator_stashes = [11_u64, 21, 31];
			let [validator_1, validator_2, validator_3] = validator_stashes;

			// Setup reward points (equal split)
			for stash in &validator_stashes {
				Rewards::note_author(*stash);
			}

			let total_validator_reward = 1_000_000;
			let _remainder = Rewards::schedule_reward_payouts(&validator_stashes, total_validator_reward, era);

			// 1 payout for each validator
			assert_eq!(
				ScheduledPayoutAmounts::<Test>::iter_prefix(era).count(),
				validator_stashes.len()
			);
			assert_eq!(
				ScheduledPayoutErasAndCounts::get(),
				vec![(era, validator_stashes.len() as u32)]
			);

			// payout storage is set
			assert_eq!(
				ScheduledPayoutAmounts::<Test>::get(era, validator_1),
				total_validator_reward / 3,
			);

			assert_eq!(
				ScheduledPayoutAmounts::<Test>::get(era, validator_2),
				total_validator_reward / 3,
			);

			assert_eq!(
				ScheduledPayoutAmounts::<Test>::get(era, validator_3),
				total_validator_reward / 3,
			);
		})
	}

	#[test]
	fn on_idle_payouts() {
		ExtBuilder::default().build().execute_with(|| {
			// setup
			let (era_1, era_2) = (1, 2);
			let validator_stashes = [11_u64, 21, 31];
			let [validator_1, validator_2, validator_3] = validator_stashes;
			let [validator_1_initial_balance, validator_2_initial_balance, validator_3_initial_balance] = [
				<Test as Config>::CurrencyToReward::free_balance(&validator_1),
				<Test as Config>::CurrencyToReward::free_balance(&validator_2),
				<Test as Config>::CurrencyToReward::free_balance(&validator_3),
			];
			// Setup reward points (equal split)
			for stash in &validator_stashes {
				Rewards::note_author(*stash);
			}
			let total_validator_reward_era_1 = 1_000_000;
			let _remainder_1 =
				Rewards::schedule_reward_payouts(&validator_stashes, total_validator_reward_era_1, era_1);

			// setup era 2 w 2 validators only
			// Setup reward points (equal split)
			CurrentEraRewardPoints::<Test>::kill();
			for stash in &validator_stashes[1..] {
				Rewards::note_author(*stash);
			}
			let total_validator_reward_era_2 = 500_000;
			let _remainder_2 =
				Rewards::schedule_reward_payouts(&validator_stashes[1..], total_validator_reward_era_2, era_2);

			// test
			let single_payout_weight = DbWeight::get().writes(2)
				+ DbWeight::get().reads(2)
				+ MockPayoutRunner::<Test>::estimate_run_payout_weight(&1_u64, 1, era_1);
			// block 5 enough space to make 1 era 1 payout
			let consumed = Rewards::on_idle(5, single_payout_weight);
			println!("{:?}", single_payout_weight);
			println!("{:?}", consumed);
			assert_eq!(
				ScheduledPayoutErasAndCounts::get(),
				vec![
					(era_1, validator_stashes.len() as u32 - 1),
					(era_2, validator_stashes.len() as u32 - 1)
				]
			);
			assert_eq!(
				ScheduledPayoutAmounts::<Test>::iter_prefix(era_1)
					.collect::<Vec<(AccountId, Balance)>>()
					.len(),
				2,
			);
			assert_eq!(
				<Test as Config>::CurrencyToReward::free_balance(&validator_1),
				validator_1_initial_balance + total_validator_reward_era_1 / 3
			);

			// not enough space to do any payments
			Rewards::on_idle(6, 1_000);
			assert_eq!(
				ScheduledPayoutErasAndCounts::get(),
				vec![
					(era_1, validator_stashes.len() as u32 - 1),
					(era_2, validator_stashes.len() as u32 - 1)
				]
			);

			// finish era 1 payouts
			let consumed = Rewards::on_idle(7, single_payout_weight * 2);
			println!("{:?}", single_payout_weight * 2);
			println!("{:?}", consumed);
			assert_eq!(
				ScheduledPayoutErasAndCounts::get(),
				vec![(era_2, validator_stashes.len() as u32 - 1)]
			);
			assert_eq!(
				<Test as Config>::CurrencyToReward::free_balance(&validator_2),
				validator_2_initial_balance + total_validator_reward_era_1 / 3
			);
			assert_eq!(
				<Test as Config>::CurrencyToReward::free_balance(&validator_3),
				validator_3_initial_balance + total_validator_reward_era_1 / 3
			);

			// finish era 2 payouts
			let consumed = Rewards::on_idle(8, single_payout_weight * 2);
			println!("{:?}", single_payout_weight * 2);
			println!("{:?}", consumed);
			assert!(ScheduledPayoutAmounts::<Test>::iter().count().is_zero());
			assert!(ScheduledPayoutErasAndCounts::get().is_empty());

			// payouts made
			assert_eq!(
				<Test as Config>::CurrencyToReward::free_balance(&validator_1),
				validator_1_initial_balance + total_validator_reward_era_1 / 3
			);
			assert_eq!(
				<Test as Config>::CurrencyToReward::free_balance(&validator_2),
				validator_2_initial_balance + total_validator_reward_era_1 / 3 + total_validator_reward_era_2 / 2
			);
			assert_eq!(
				<Test as Config>::CurrencyToReward::free_balance(&validator_3),
				validator_3_initial_balance + total_validator_reward_era_1 / 3 + total_validator_reward_era_2 / 2
			);

			// no payouts scheduled
			assert_eq!(
				Rewards::on_idle(9, 1_000_000),
				DbWeight::get().reads(1) + DbWeight::get().writes(1)
			);
			assert!(ScheduledPayoutAmounts::<Test>::iter()
				.collect::<Vec<(u32, AccountId, Balance)>>()
				.is_empty());
			assert!(ScheduledPayoutErasAndCounts::get().is_empty());
		});
	}

	#[test]
	fn process_reward_payout() {
		ExtBuilder::default().build().execute_with(|| {
			let (validator_stash, validator_stake) = (13, 1_000);
			let nominator_stakes = [(1_u64, 1_000_u64), (2, 2_000), (3, 500)];
			let commission = Perbill::from_rational(5_u32, 100);

			let exposures = MockCommissionStakeInfo::new(
				(validator_stash, validator_stake),
				nominator_stakes.to_vec(),
				commission,
			)
			.exposures;
			let initial_issuance = <Test as Config>::CurrencyToReward::total_issuance();
			let era = 1;
			// check different payee account support (nominator 3)
			Rewards::set_payee(&3, &8);

			// Execute the payout for this validator and it's nominators
			let payout = 1_033_221;
			Rewards::process_reward_payout(&validator_stash, commission, &exposures, payout, era);

			// Assume these are the correct values based on other unit tests
			let expected_payouts = Rewards::calculate_npos_payouts(&validator_stash, commission, &exposures, payout);

			// check payout happened to payee account and event deposited
			let mut remainder = payout;
			for (payee, amount) in expected_payouts {
				assert_eq!(<Test as Config>::CurrencyToReward::free_balance(&payee), amount);
				assert!(System::events()
					.iter()
					.find(|e| e.event == Event::Rewards(RawEvent::EraStakerPayout(era, payee, amount)))
					.is_some());
				remainder = remainder.saturating_sub(amount);
			}

			// remainder to treasury
			assert!(remainder > 0);
			assert_eq!(
				<Test as Config>::CurrencyToReward::free_balance(&TreasuryPalletId::get().into_account()),
				remainder
			);

			// issuance total increase
			assert_eq!(
				<Test as Config>::CurrencyToReward::total_issuance(),
				initial_issuance + payout
			);
		})
	}

	#[test]
	fn on_end_era() {
		ExtBuilder::default().build().execute_with(|| {
			Rewards::note_transaction_fees(1_111);
			let era_duration_ms = <Rewards as RewardCalculation>::FULL_ERA_DURATION;

			let next_reward = Rewards::calculate_total_reward(era_duration_ms);
			let era = 2;
			let validators = [11, 22, 33];
			Rewards::on_end_era(&validators, era, era_duration_ms, false);

			// treasury is paid
			assert_eq!(
				<Test as Config>::CurrencyToReward::free_balance(&TreasuryPalletId::get().into_account()),
				// +1 is the remainder from after stakers cut distribution
				next_reward.treasury_cut + 1
			);

			// payouts scheduled for each validator
			assert_eq!(ScheduledPayoutErasAndCounts::get()[0].1, validators.len() as u32);

			assert!(System::events()
				.iter()
				.find(|e| e.event
					== Event::Rewards(RawEvent::EraPayout(
						era,
						next_reward.treasury_cut,
						next_reward.stakers_cut
					)))
				.is_some());

			// Storage reset for next era
			assert!(!TransactionFeePot::<Test>::exists());
			assert!(!CurrentEraRewardPoints::<Test>::exists());
		})
	}

	#[test]
	fn migrate_to_v1() {
		use frame_support::traits::OnRuntimeUpgrade;

		ExtBuilder::default().build().execute_with(|| {
			use migration::storage_v0;

			// migration #1 scheduled paymetns exist
			StorageVersion::put(Releases::V0 as u32);
			storage_v0::ScheduledPayoutEra::put(5);
			storage_v0::ScheduledPayouts::<Test>::insert(3, (11, 1_000));
			storage_v0::ScheduledPayouts::<Test>::insert(5, (2, 2_000));
			storage_v0::ScheduledPayouts::<Test>::insert(7, (40, 123));

			// run migration
			Rewards::on_runtime_upgrade();
			assert_eq!(StorageVersion::get(), Releases::V1 as u32);
			assert_eq!(ScheduledPayoutErasAndCounts::get(), vec![(5, 3)]);
			let payouts = ScheduledPayoutAmounts::<Test>::iter().collect::<Vec<(EraIndex, AccountId, Balance)>>();
			assert_eq!(
				payouts,
				vec![
					(5 as EraIndex, 11 as AccountId, 1_000 as Balance),
					(5, 40, 123),
					(5, 2, 2_000),
				]
			);
			assert!(storage_v0::ScheduledPayoutEra::get().is_zero());
			assert!(storage_v0::ScheduledPayouts::<Test>::iter()
				.collect::<Vec<(u64, (AccountId, Balance))>>()
				.is_empty());

			// migration #2
			// there are no scheduled payments
			ScheduledPayoutAmounts::<Test>::remove_all(None);
			ScheduledPayoutErasAndCounts::kill();
			storage_v0::ScheduledPayoutEra::put(5);
			StorageVersion::put(Releases::V0 as u32);

			// migrate
			Rewards::on_runtime_upgrade();
			assert_eq!(StorageVersion::get(), Releases::V1 as u32);
			assert!(ScheduledPayoutErasAndCounts::get().is_empty());
			assert!(storage_v0::ScheduledPayoutEra::get().is_zero());
			let payouts = ScheduledPayoutAmounts::<Test>::iter().collect::<Vec<(EraIndex, AccountId, Balance)>>();
			assert!(payouts.is_empty());
		})
	}
}
