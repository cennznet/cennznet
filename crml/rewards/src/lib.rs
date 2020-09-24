/* Copyright 2020 Centrality Investments Limited
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
#![cfg_attr(not(feature = "std"), no_std)]

use cennznet_primitives::{traits::ValidatorRewardPayment, types::Exposure};
use frame_support::traits::OnUnbalanced;
use frame_support::{
	decl_event, decl_module, decl_storage,
	traits::{Currency, Imbalance},
	weights::SimpleDispatchInfo,
};
use frame_system::{self as system, ensure_root};
use sp_arithmetic::{FixedI128, FixedPointNumber, FixedPointOperand};
use sp_runtime::{
	traits::{AccountIdConversion, One, Saturating, Zero},
	ModuleId, Perbill,
};
use sp_std::{collections::vec_deque::VecDeque, prelude::*};

/// A balance amount in the reward currency
type BalanceOf<T> = <<T as Trait>::CurrencyToReward as Currency<<T as system::Trait>::AccountId>>::Balance;
/// A pending increase to total issuance of the reward currency
type PositiveImbalanceOf<T> =
	<<T as Trait>::CurrencyToReward as Currency<<T as frame_system::Trait>::AccountId>>::PositiveImbalance;
/// A pending decrease to total issuance of the reward currency
type NegativeImbalanceOf<T> =
	<<T as Trait>::CurrencyToReward as Currency<<T as frame_system::Trait>::AccountId>>::NegativeImbalance;

pub trait Trait: frame_system::Trait {
	/// The system event type
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
	/// The reward currency system (total issuance, account balance, etc.) for payouts.
	type CurrencyToReward: Currency<Self::AccountId>;
}

/// The development fund ID used for deriving its sovereign account ID.
const DEVELOPMENT_FUND_ID: ModuleId = ModuleId(*b"DevFund0");

decl_event!(
	pub enum Event<T>
	where
		Balance = BalanceOf<T>,
	{
		/// A reward payout happened (payout, remainder)
		RewardPayout(Balance, Balance),
	}
);

decl_storage! {
	trait Store for Module<T: Trait> as Rewards {
		/// Inflation rate % to apply on reward payouts, it may be negative
		pub InflationRate get(fn inflation_rate): FixedI128 = FixedI128::saturating_from_integer(1);
		/// Development fund % take for reward payouts, parts-per-billion
		pub DevelopmentFundTake get(fn development_fund_take) config(): Perbill;
		/// Accumulated transaction fees for reward payout
		pub TransactionFeePot get(fn transaction_fee_pot): BalanceOf<T>;
		/// Historic accumulated transaction fees on reward payout
		pub TransactionFeePotHistory get(fn transaction_fee_pot_history): VecDeque<BalanceOf<T>>;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {

		fn deposit_event() = default;

		/// Set the per payout inflation rate (`numerator` / `denominator`) (it may be negative)
		#[weight = SimpleDispatchInfo::FixedOperational(0)]
		pub fn set_inflation_rate(origin, numerator: i64, denominator: i64) {
			ensure_root(origin)?;
			InflationRate::put(FixedI128::saturating_from_rational(numerator, denominator));
		}

		/// Set the development fund take %, capped at 100%.
		#[weight = SimpleDispatchInfo::FixedOperational(0)]
		pub fn set_development_fund_take(origin, new_take_percent: u32) {
			ensure_root(origin)?;
			DevelopmentFundTake::put(
				Perbill::from_percent(new_take_percent.min(100))
			);
		}
	}
}

impl<T: Trait> ValidatorRewardPayment for Module<T>
where
	BalanceOf<T>: FixedPointOperand,
{
	type AccountId = T::AccountId;
	type Balance = BalanceOf<T>;
	/// Perform a reward payout given a mapping of validators and their nominators stake at some era
	/// Accounts IDs are the ones which should receive payment.
	fn make_reward_payout(
		validator_commission_stake_map: &[(Self::AccountId, Perbill, Exposure<Self::AccountId, Self::Balance>)],
	) {
		// Calculate the accumulated tx fee reward split
		let fee_payout = TransactionFeePot::<T>::take();
		// track historic era fee amounts
		Self::note_fee_payout(fee_payout);

		if fee_payout.is_zero() {
			return;
		}

		let mut total_payout = Self::inflation_rate().saturating_mul_int(fee_payout);

		// Deduct development fund take %
		let development_fund_payout = Self::development_fund_take() * total_payout;
		let _ = T::CurrencyToReward::deposit_into_existing(&Self::development_fund(), development_fund_payout);
		total_payout = total_payout.saturating_sub(development_fund_payout);

		// Payout reward to validators and their nominators
		let total_payout_share = total_payout / BalanceOf::<T>::from(validator_commission_stake_map.len() as u32);

		// implementation note: imbalances have the side affect of updating storage when `drop`ped.
		// we use `subsume` to absorb all small imbalances (from individual payouts) into one big imbalance (from all payouts).
		// This ensures only one storage update to total issuance will happen when dropped.
		let mut total_payout_imbalance = <PositiveImbalanceOf<T>>::zero();

		validator_commission_stake_map
			.iter()
			.flat_map(|(validator, validator_commission, stake_map)| {
				Self::calculate_npos_payouts(&validator, *validator_commission, stake_map, total_payout_share)
			})
			.for_each(|(account, payout)| {
				total_payout_imbalance.maybe_subsume(T::CurrencyToReward::deposit_into_existing(&account, payout).ok());
			});

		// Any unallocated reward amount can go to the development fund
		let remainder = total_payout.saturating_sub(total_payout_imbalance.peek());
		T::CurrencyToReward::deposit_creating(&Self::development_fund(), remainder);

		Self::deposit_event(RawEvent::RewardPayout(total_payout_imbalance.peek(), remainder));
	}
}

impl<T: Trait> Module<T> {
	/// The development fund address
	pub fn development_fund() -> T::AccountId {
		DEVELOPMENT_FUND_ID.into_account()
	}

	/// Add the given `fee` amount to the next reward payout
	pub fn note_transaction_fees(amount: BalanceOf<T>) {
		TransactionFeePot::<T>::mutate(|acc| acc.saturating_add(amount));
	}

	/// Note a fee payout for future calculations
	fn note_fee_payout(amount: BalanceOf<T>) {
		const ERA_WINDOW: usize = 7;
		let mut history = TransactionFeePotHistory::<T>::get();
		history.push_back(amount);
		history.truncate(ERA_WINDOW);
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
			let contribution_ratio =
				Perbill::from_rational_approximation(nominator_stake.value, aggregate_validator_stake);
			payouts.push((nominator_stake.who.clone(), contribution_ratio * nominators_cut));
		}

		// Finally payout the validator. commission (`validator_cut`) + it's share of the `nominators_cut`
		// As a validator always self-nominates using it's own stake.
		let validator_contribution_ratio =
			Perbill::from_rational_approximation(validator_stake.own, aggregate_validator_stake);

		// this cannot overflow, `validator_cut` is a fraction of `reward`
		payouts.push((
			validator.clone(),
			(validator_contribution_ratio * nominators_cut) + validator_cut,
		));
		(*payouts).to_vec()
	}
}

/// This handles the `NegativeImbalance` from burning transaction fees.
/// The amount is noted by the rewards module for later distribution.
impl<T: Trait> OnUnbalanced<NegativeImbalanceOf<T>> for Module<T> {
	fn on_nonzero_unbalanced(imbalance: NegativeImbalanceOf<T>) {
		Self::note_transaction_fees(imbalance.peek());
	}
}

// TODO: Slashed CENNZ should come to the development fund
// impl<T: Trait> OnUnbalanced<NegativeImbalanceOf<T>> for Module<T> {
// 	fn on_nonzero_unbalanced(amount: NegativeImbalanceOf<T>) {
// 		let numeric_amount = amount.peek();

// 		// Must resolve into existing but better to be safe.
// 		let _ = T::Currency::resolve_creating(&Self::account_id(), amount);

// 		Self::deposit_event(RawEvent::Deposit(numeric_amount));
// 	}
// }
