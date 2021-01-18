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

use crate::{EraIndex, Exposure};
use frame_support::traits::OnUnbalanced;
use frame_support::{
	decl_event, decl_module, decl_storage,
	traits::{Currency, Get, Imbalance},
	weights::{DispatchClass, Weight},
};
use frame_system::{self as system, ensure_root};
use sp_runtime::{
	traits::{AccountIdConversion, CheckedDiv, One, Saturating, UniqueSaturatedFrom, UniqueSaturatedInto, Zero},
	FixedPointNumber, FixedPointOperand, FixedU128, ModuleId, Perbill,
};
use sp_std::{collections::vec_deque::VecDeque, prelude::*};

mod benchmarking;
mod default_weights;
mod types;
pub use types::*;

/// A balance amount in the reward currency
type BalanceOf<T> = <<T as Trait>::CurrencyToReward as Currency<<T as system::Trait>::AccountId>>::Balance;
/// A pending increase to total issuance of the reward currency
type PositiveImbalanceOf<T> =
	<<T as Trait>::CurrencyToReward as Currency<<T as frame_system::Trait>::AccountId>>::PositiveImbalance;
/// A pending decrease to total issuance of the reward currency
type NegativeImbalanceOf<T> =
	<<T as Trait>::CurrencyToReward as Currency<<T as frame_system::Trait>::AccountId>>::NegativeImbalance;

pub trait WeightInfo {
	fn process_reward_payouts(p: u32) -> Weight;
	fn process_zero_payouts() -> Weight;
}

pub trait Trait: frame_system::Trait {
	/// The system event type
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
	/// The reward currency system (total issuance, account balance, etc.) for payouts.
	type CurrencyToReward: Currency<Self::AccountId>;
	/// The treasury account for payouts
	type TreasuryModuleId: Get<ModuleId>;
	/// The number of historical eras for which tx fee payout info should be retained.
	type HistoricalPayoutEras: Get<u16>;
	/// The reward payouts would be split among several blocks when their number exceeds this threshold.
	type PayoutSplitThreshold: Get<u32>;
	/// The number of staking eras in a fiscal era.
	type FiscalDuration: Get<u32>;
	/// Extrinsic weight info
	type WeightInfo: WeightInfo;
}

decl_event!(
	pub enum Event<T>
	where
		Balance = BalanceOf<T>,
		AccountId = <T as frame_system::Trait>::AccountId,
	{
		/// A reward payout happened (nominator/validator account id, amount, era in which the reward was accrued)
		RewardPayout(AccountId, Balance, EraIndex),
		/// All the rewards of the specified era is now processed with an unallocated `remainder` that went to treasury
		AllRewardsPaidOut(EraIndex, Balance),
	}
);

decl_storage! {
	trait Store for Module<T: Trait> as Rewards {
		/// Inflation rate % to apply on reward payouts
		pub BaseInflationRate get(fn inflation_rate): FixedU128 = FixedU128::saturating_from_rational(1u64, 100u64);
		/// Development fund % take for reward payouts, parts-per-billion
		pub DevelopmentFundTake get(fn development_fund_take) config(): Perbill;
		/// Accumulated transaction fees for reward payout
		pub TransactionFeePot get(fn transaction_fee_pot): BalanceOf<T>;
		/// Historic accumulated transaction fees on reward payout
		pub TransactionFeePotHistory get(fn transaction_fee_pot_history): VecDeque<BalanceOf<T>>;
		/// Remaining reward amount of the eras which are not fully processed yet
		pub QueuedEraRewards get(fn queued_era_rewards): VecDeque<BalanceOf<T>>;
		/// Hold the latest not processed payouts and the era when each is accrued
		pub Payouts get(fn payouts): VecDeque<(T::AccountId, BalanceOf<T>, EraIndex)>;
		/// Targeted inflation is the amount of new reward tokens that will be minted over a fiscal
		/// era in order to achieve the inflation rate. We calculate the targeted inflation based on
		/// T::CurrencyToReward::totalIssuance() at the beginning of a fiscal era.
		TargetedInflation get(fn targeted_inflation): BalanceOf<T>;
		/// The staking era index that specifies the start of the current fiscal era.
		FiscalEraStart get(fn fiscal_era_start): EraIndex;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {

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
	}
}

impl<T: Trait> StakerRewardPayment for Module<T>
where
	BalanceOf<T>: FixedPointOperand,
{
	type AccountId = T::AccountId;
	type Balance = BalanceOf<T>;
	type BlockNumber = T::BlockNumber;
	/// Perform a reward payout given a mapping of validators and their nominators stake.
	/// Accounts IDs are the ones which should receive payment.
	fn enqueue_reward_payouts(
		validator_commission_stake_map: &[(Self::AccountId, Perbill, Exposure<Self::AccountId, Self::Balance>)],
		era: EraIndex,
	) {
		// track historic era fee amounts
		Self::note_fee_payout(TransactionFeePot::<T>::get());

		if era.saturating_sub(Self::fiscal_era_start()) >= T::FiscalDuration::get() {
			Self::new_fiscal_era(era);
		}

		let total_payout = Self::calculate_next_reward_payout();

		// The observed fee will be processed, so we can here take it off the pot
		let _ = TransactionFeePot::<T>::take();

		if total_payout.is_zero() || validator_commission_stake_map.len().is_zero() {
			return;
		}

		// Deduct development fund take %
		let development_fund_payout = Self::development_fund_take() * total_payout;

		// implementation note: imbalances have the side affect of updating storage when `drop`ped.
		// we use `subsume` to absorb all small imbalances (from individual payouts) into one big imbalance (from all payouts).
		// This ensures only one storage update to total issuance will happen when dropped.
		let _ = T::CurrencyToReward::deposit_into_existing(
			&T::TreasuryModuleId::get().into_account(),
			development_fund_payout,
		);

		let validator_payout = total_payout.saturating_sub(development_fund_payout);

		// Payout reward to validators and their nominators
		let total_payout_share = validator_payout / BalanceOf::<T>::from(validator_commission_stake_map.len() as u32);

		validator_commission_stake_map
			.iter()
			.flat_map(|(validator, validator_commission, stake_map)| {
				Self::calculate_npos_payouts(&validator, *validator_commission, stake_map, total_payout_share)
			})
			.for_each(|(account, payout)| {
				Payouts::<T>::mutate(|p| p.push_back((account, payout, era)));
			});

		QueuedEraRewards::<T>::mutate(|q| q.push_back(validator_payout));
	}

	/// Process the reward payouts considering the given quota which is the number of payouts to be processed now.
	/// Return the benchmarked weight of the call.
	fn process_reward_payouts(remaining_blocks: Self::BlockNumber) -> Weight {
		let remaining_payouts = Payouts::<T>::get().len() as u32;
		let quota = Self::calculate_payout_quota(remaining_payouts, remaining_blocks);
		if quota.is_zero() {
			return T::WeightInfo::process_zero_payouts();
		}

		let weight = T::WeightInfo::process_reward_payouts(quota as u32);

		let mut payouts = Payouts::<T>::get();

		// First payout in the current series, gives the right context for processing the rest.
		let (first_payee, first_amount, first_era) = payouts.pop_front().unwrap_or_default();
		let mut total_payout_imbalance = T::CurrencyToReward::deposit_into_existing(&first_payee, first_amount)
			.ok()
			.unwrap_or_else(PositiveImbalanceOf::<T>::zero);
		Self::deposit_event(RawEvent::RewardPayout(first_payee, first_amount, first_era));
		let mut era_under_process = first_era;

		let handle_remainder = |imbalance: &mut PositiveImbalanceOf<T>| -> BalanceOf<T> {
			let mut remainder = Zero::zero();
			QueuedEraRewards::<T>::mutate(|rra| {
				remainder = rra.pop_front().unwrap_or_default().saturating_sub(imbalance.peek());
				imbalance.maybe_subsume(
					T::CurrencyToReward::deposit_into_existing(&T::TreasuryModuleId::get().into_account(), remainder)
						.ok(),
				);
			});
			remainder
		};

		for _ in 1..quota {
			if let Some((payee, amount, era)) = payouts.pop_front() {
				if era > era_under_process {
					let remainder = handle_remainder(&mut total_payout_imbalance);
					Self::deposit_event(RawEvent::AllRewardsPaidOut(era_under_process, remainder));
					era_under_process = era;
				}
				total_payout_imbalance.maybe_subsume(T::CurrencyToReward::deposit_into_existing(&payee, amount).ok());
				Self::deposit_event(RawEvent::RewardPayout(payee, amount, era));
			}
		}

		if payouts.is_empty() {
			let remainder = handle_remainder(&mut total_payout_imbalance);
			Self::deposit_event(RawEvent::AllRewardsPaidOut(era_under_process, remainder));
		} else {
			QueuedEraRewards::<T>::mutate(|rra| {
				if let Some(remainder) = rra.front_mut() {
					*remainder = remainder.saturating_sub(total_payout_imbalance.peek());
				}
			});
		}

		Payouts::<T>::put(payouts);

		weight
	}

	/// Calculate the total reward payout as of right now
	fn calculate_next_reward_payout() -> Self::Balance {
		let fee_payout = TransactionFeePot::<T>::get();
		let era_mined_tokens = Self::targeted_inflation()
			.checked_div(&T::FiscalDuration::get().into())
			.unwrap_or_else(Zero::zero);
		fee_payout.saturating_add(era_mined_tokens)
	}
}

impl<T: Trait> Module<T> {
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

	/// Return the number of reward payouts that need to be processed in the current block.
	/// The result is dependent on the number of the current era's remaining payouts and the number
	/// of remaining blocks before a new era.
	fn calculate_payout_quota(remaining_payouts: u32, remaining_blocks: T::BlockNumber) -> u32 {
		if remaining_blocks.is_zero() {
			return remaining_payouts;
		}

		let payout_split_threshold = T::PayoutSplitThreshold::get();

		if remaining_payouts <= payout_split_threshold {
			return remaining_payouts;
		}

		let remaining_payouts = <T::BlockNumber as UniqueSaturatedFrom<u32>>::unique_saturated_from(remaining_payouts);
		let min_payouts = remaining_payouts / (remaining_blocks + One::one());
		let min_payouts = <T::BlockNumber as UniqueSaturatedInto<u32>>::unique_saturated_into(min_payouts);
		min_payouts.max(payout_split_threshold)
	}

	/// Start a new fiscal era. Calculate the new inflation target based on the latest set inflation rate.
	fn new_fiscal_era(era: EraIndex) {
		FiscalEraStart::put(era);
		let total_issuance: u128 = T::CurrencyToReward::total_issuance().unique_saturated_into();
		let targeted_inflation =
			<BalanceOf<T>>::unique_saturated_from(Self::inflation_rate().saturating_mul_int(total_issuance));
		<TargetedInflation<T>>::put(targeted_inflation);
	}
}

/// This handles the `NegativeImbalance` from burning transaction fees.
/// The amount is noted by the rewards module for later distribution.
impl<T: Trait> OnUnbalanced<NegativeImbalanceOf<T>> for Module<T> {
	fn on_unbalanced(imbalance: NegativeImbalanceOf<T>) {
		Self::note_transaction_fees(imbalance.peek());
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{rewards, IndividualExposure};
	use frame_support::{assert_err, assert_noop, assert_ok, impl_outer_event, impl_outer_origin, parameter_types};
	use frame_system::{InitKind, Module as System};
	use sp_core::H256;
	use sp_runtime::{
		testing::Header,
		traits::{BadOrigin, BlakeTwo256, IdentityLookup},
		ModuleId,
	};

	/// The account Id type in this test runtime
	type AccountId = u64;
	/// The asset Id type in this test runtime
	type AssetId = u64;
	/// The balance type in this test runtime
	type Balance = u64;

	/// The test runtime struct
	#[derive(Clone, Eq, PartialEq)]
	pub struct TestRuntime;

	impl_outer_origin! {
		pub enum Origin for TestRuntime {}
	}

	use prml_generic_asset as generic;
	impl_outer_event! {
		pub enum TestEvent for TestRuntime {
			system<T>,
			generic<T>,
			rewards<T>,
		}
	}

	parameter_types! {
		pub const BlockHashCount: u64 = 250;
		pub const MaximumBlockWeight: u32 = 1024;
		pub const MaximumBlockLength: u32 = 2 * 1024;
		pub const AvailableBlockRatio: Perbill = Perbill::one();
	}
	impl frame_system::Trait for TestRuntime {
		type BaseCallFilter = ();
		type Origin = Origin;
		type Index = u64;
		type Call = ();
		type BlockNumber = u64;
		type Hash = H256;
		type Hashing = BlakeTwo256;
		type AccountId = AccountId;
		type Lookup = IdentityLookup<Self::AccountId>;
		type Header = Header;
		type Event = TestEvent;
		type BlockHashCount = BlockHashCount;
		type MaximumBlockWeight = MaximumBlockWeight;
		type DbWeight = ();
		type BlockExecutionWeight = ();
		type ExtrinsicBaseWeight = ();
		type MaximumExtrinsicWeight = MaximumBlockWeight;
		type AvailableBlockRatio = AvailableBlockRatio;
		type MaximumBlockLength = MaximumBlockLength;
		type Version = ();
		type PalletInfo = ();
		type AccountData = ();
		type OnNewAccount = ();
		type OnKilledAccount = ();
		type SystemWeightInfo = ();
	}

	impl prml_generic_asset::Trait for TestRuntime {
		type AssetId = AssetId;
		type Balance = Balance;
		type Event = TestEvent;
		type WeightInfo = ();
	}

	parameter_types! {
		pub const TreasuryModuleId: ModuleId = ModuleId(*b"py/trsry");
		pub const HistoricalPayoutEras: u16 = 7;
		pub const PayoutSplitThreshold: u32 = 10;
		pub const FiscalDuration: u32 = 5;
	}
	impl Trait for TestRuntime {
		type Event = TestEvent;
		type CurrencyToReward = prml_generic_asset::SpendingAssetCurrency<Self>;
		type TreasuryModuleId = TreasuryModuleId;
		type HistoricalPayoutEras = HistoricalPayoutEras;
		type PayoutSplitThreshold = PayoutSplitThreshold;
		type FiscalDuration = FiscalDuration;
		type WeightInfo = ();
	}

	// Provides configurable mock genesis storage data.
	#[derive(Default)]
	pub struct ExtBuilder {}

	impl ExtBuilder {
		pub fn build(self) -> sp_io::TestExternalities {
			let mut storage = frame_system::GenesisConfig::default()
				.build_storage::<TestRuntime>()
				.unwrap();

			let _ = GenesisConfig {
				development_fund_take: Perbill::from_percent(10),
			}
			.assimilate_storage(&mut storage);

			let _ = prml_generic_asset::GenesisConfig::<TestRuntime> {
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

			storage.into()
		}
	}

	/// Alias for the mocked module under test
	type Rewards = Module<TestRuntime>;
	/// Alias for the reward currency in the module under test
	type RewardCurrency = <TestRuntime as Trait>::CurrencyToReward;
	/// Alias for the mocked system module
	type TestSystem = System<TestRuntime>;
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
			let historical_payouts = [1_000_u64; <TestRuntime as Trait>::HistoricalPayoutEras::get() as usize];
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
	fn on_unbalanced_handler_notes_fees() {
		ExtBuilder::default().build().execute_with(|| {
			let issued = 1_000;
			let imbalance = RewardCurrency::issue(issued);
			Rewards::on_unbalanced(imbalance);
			assert_eq!(Rewards::transaction_fee_pot(), issued);
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
	fn enqueue_reward_payouts_development_fund_take() {
		ExtBuilder::default().build().execute_with(|| {
			let mock_commission_stake_map =
				MockCommissionStakeInfo::new((1, 1_000), vec![(2, 2_000), (3, 3_000)], Perbill::from_percent(10));
			let tx_fee_reward = 1_000_000;
			Rewards::note_transaction_fees(tx_fee_reward);
			let total_payout = Rewards::calculate_next_reward_payout();
			Rewards::enqueue_reward_payouts(&[mock_commission_stake_map.as_tuple()], 0);

			let development_fund = RewardCurrency::free_balance(&TreasuryModuleId::get().into_account());
			let take = Rewards::development_fund_take();
			assert_eq!(development_fund, take * total_payout,);
			assert_eq!(
				Rewards::queued_era_rewards()[0],
				total_payout.saturating_sub(development_fund)
			);
		});
	}

	#[test]
	fn simple_reward_payout_inflation() {
		// the basic reward model on CENNZnet is fees * inflation
		ExtBuilder::default().build().execute_with(|| {
			let tx_fee_reward = 10;
			Rewards::note_transaction_fees(tx_fee_reward);
			assert_ok!(Rewards::set_inflation_rate(Origin::root(), 1, 100));
			Rewards::new_fiscal_era(0);
			let total_payout = Rewards::calculate_next_reward_payout();
			assert_eq!(total_payout, 12);
		});
	}

	#[test]
	fn large_payouts_split() {
		ExtBuilder::default().build().execute_with(|| {
			let _ = RewardCurrency::deposit_creating(&1, 1_234);
			assert_ok!(Rewards::set_development_fund_take(Origin::root(), 10));

			let tx_fee_reward = 1_000_000;
			Rewards::note_transaction_fees(tx_fee_reward);
			let total_payout = Rewards::calculate_next_reward_payout();
			let pre_reward_issuance = RewardCurrency::total_issuance();

			let validator_stake_map1 =
				MockCommissionStakeInfo::new((1, 1_000), vec![(2, 2_000), (3, 3_000)], Perbill::from_percent(10));
			let validator_stake_map2 =
				MockCommissionStakeInfo::new((10, 1_000), vec![(2, 2_000), (3, 3_000)], Perbill::from_percent(10));
			let validator_stake_map3 =
				MockCommissionStakeInfo::new((20, 1_000), vec![(2, 2_000), (3, 3_000)], Perbill::from_percent(10));
			let validator_stake_map4 =
				MockCommissionStakeInfo::new((30, 1_000), vec![(2, 2_000), (3, 3_000)], Perbill::from_percent(10));
			Rewards::enqueue_reward_payouts(
				&[
					validator_stake_map1.as_tuple(),
					validator_stake_map2.as_tuple(),
					validator_stake_map3.as_tuple(),
					validator_stake_map4.as_tuple(),
				],
				0,
			);
			Rewards::process_reward_payouts(3);
			assert_eq!(Payouts::<TestRuntime>::get().len(), 2);
			Rewards::process_reward_payouts(2);
			assert_eq!(Payouts::<TestRuntime>::get().len(), 0);
			assert_eq!(RewardCurrency::total_issuance(), pre_reward_issuance + total_payout);
		});
	}

	#[test]
	fn emit_all_rewards_paid_out_event() {
		ExtBuilder::default().build().execute_with(|| {
			let payout_split_threshold = <TestRuntime as Trait>::PayoutSplitThreshold::get();
			TestSystem::initialize(
				&1,
				&[0u8; 32].into(),
				&[0u8; 32].into(),
				&Default::default(),
				InitKind::Full,
			);

			assert_ok!(Rewards::set_development_fund_take(Origin::root(), 10));

			let tx_fee_reward = 1_000_000;

			let validators_number = 4;
			let validators_stake_info: Vec<(AccountId, Perbill, Exposure<AccountId, Balance>)> = (0..validators_number)
				.map(|i| {
					MockCommissionStakeInfo::new(
						((i + 1) * 10, 1_000),
						vec![(2, 2_000), (3, 3_000)],
						Perbill::from_percent(10),
					)
					.as_tuple()
				})
				.collect();

			Rewards::note_transaction_fees(tx_fee_reward);
			Rewards::enqueue_reward_payouts(&validators_stake_info, 1);

			Rewards::process_reward_payouts(3);
			assert_eq!(Payouts::<TestRuntime>::get().len(), 2);

			Rewards::note_transaction_fees(tx_fee_reward);
			Rewards::enqueue_reward_payouts(&validators_stake_info, 2);

			Rewards::process_reward_payouts(3);
			assert_eq!(Payouts::<TestRuntime>::get().len(), 4);

			let events = TestSystem::events();
			let expected_event = TestEvent::rewards(RawEvent::AllRewardsPaidOut(1, 0));
			assert_eq!(events.len() as u32, 2 * payout_split_threshold + 1);
			assert!(events.iter().any(|record| record.event == expected_event));

			Rewards::process_reward_payouts(2);
			assert_eq!(Payouts::<TestRuntime>::get().len(), 0);

			let events = TestSystem::events();
			assert_eq!(events.len() as u64, validators_number * 6 + 2);
			assert_eq!(
				events.last().unwrap().event,
				TestEvent::rewards(RawEvent::AllRewardsPaidOut(2, 0))
			)
		});
	}

	#[test]
	fn make_reward_payouts_handles_total_issuance() {
		ExtBuilder::default().build().execute_with(|| {
			let _ = RewardCurrency::deposit_creating(&1, 1_234);
			assert_ok!(Rewards::set_development_fund_take(Origin::root(), 10));

			let tx_fee_reward = 1_000_000;
			Rewards::note_transaction_fees(tx_fee_reward);
			let total_payout = Rewards::calculate_next_reward_payout();
			let pre_reward_issuance = RewardCurrency::total_issuance();

			let validator_stake_map1 =
				MockCommissionStakeInfo::new((1, 1_000), vec![(2, 2_000), (3, 3_000)], Perbill::from_percent(10));
			let validator_stake_map2 =
				MockCommissionStakeInfo::new((10, 1_000), vec![(2, 2_000), (3, 3_000)], Perbill::from_percent(10));
			let validator_stake_map3 =
				MockCommissionStakeInfo::new((20, 1_000), vec![(2, 2_000), (3, 3_000)], Perbill::from_percent(10));
			Rewards::enqueue_reward_payouts(
				&[
					validator_stake_map1.as_tuple(),
					validator_stake_map2.as_tuple(),
					validator_stake_map3.as_tuple(),
				],
				0,
			);
			Rewards::process_reward_payouts(3);
			assert_eq!(RewardCurrency::total_issuance(), pre_reward_issuance + total_payout);
		});
	}

	#[test]
	fn successive_reward_payouts() {
		ExtBuilder::default().build().execute_with(|| {
			let initial_total_issuance = RewardCurrency::total_issuance();
			let round1_reward = 1_000_000;
			Rewards::note_transaction_fees(round1_reward);

			let mock_commission_stake_map =
				MockCommissionStakeInfo::new((1, 1_000), vec![(2, 2_000), (3, 3_000)], Perbill::from_percent(10));
			let total_payout1 = Rewards::calculate_next_reward_payout();
			Rewards::enqueue_reward_payouts(&[mock_commission_stake_map.as_tuple()], 0);
			Rewards::process_reward_payouts(3);
			assert_eq!(RewardCurrency::total_issuance(), total_payout1 + initial_total_issuance,);

			// after reward payout, the next payout should be `0`
			assert!(Rewards::transaction_fee_pot().is_zero());
			assert!(Rewards::calculate_next_reward_payout().is_zero());
			assert_eq!(Rewards::transaction_fee_pot_history().front(), Some(&round1_reward));

			// Next payout
			let round2_reward = 10_000;
			Rewards::note_transaction_fees(round2_reward);

			let total_payout2 = Rewards::calculate_next_reward_payout();
			Rewards::enqueue_reward_payouts(&[mock_commission_stake_map.as_tuple()], 0);
			Rewards::process_reward_payouts(3);
			assert_eq!(
				RewardCurrency::total_issuance(),
				total_payout1 + total_payout2 + initial_total_issuance,
			);

			// after reward payout, the next payout should be `0`
			assert!(Rewards::transaction_fee_pot().is_zero());
			assert!(Rewards::calculate_next_reward_payout().is_zero());
			assert_eq!(Rewards::transaction_fee_pot_history().front(), Some(&round2_reward));
		});
	}

	#[test]
	fn reward_payout_calculations() {
		ExtBuilder::default().build().execute_with(|| {
			let mock_commission_stake_map =
				MockCommissionStakeInfo::new((1, 1_000), vec![(2, 2_000), (3, 2_000)], Perbill::from_percent(10));

			let _ = Rewards::set_inflation_rate(Origin::signed(1), 1, 10);
			Rewards::new_fiscal_era(0);
			Rewards::note_transaction_fees(1_000_000);
			let total_payout = Rewards::calculate_next_reward_payout();
			let development_fund_payout = Rewards::development_fund_take() * total_payout;
			let reward = total_payout.saturating_sub(development_fund_payout);

			let payouts = Rewards::calculate_npos_payouts(
				&mock_commission_stake_map.validator_stash,
				mock_commission_stake_map.commission,
				&mock_commission_stake_map.exposures,
				reward,
			);

			// validator takes 10% of reward + 20% of 90% remainder
			// nominators split 80% of 90% remainder at 50/50 each
			let validator_commission = Perbill::from_percent(10) * reward;
			let reward_share = Perbill::from_percent(90) * reward;
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

			// Run the payout for real
			Rewards::enqueue_reward_payouts(&vec![mock_commission_stake_map.as_tuple()], 0);
			Rewards::process_reward_payouts(0);
			for (staker, r) in payouts {
				assert_eq!(RewardCurrency::free_balance(&staker), r);
			}
		});
	}

	#[test]
	fn calculate_npos_payouts_no_nominators() {
		ExtBuilder::default().build().execute_with(|| {
			let mock_commission_stake_map = MockCommissionStakeInfo::new((1, 1_000), vec![], Perbill::from_percent(10));

			let reward = 1_000_000;

			let payouts = Rewards::calculate_npos_payouts(
				&mock_commission_stake_map.validator_stash,
				mock_commission_stake_map.commission,
				&mock_commission_stake_map.exposures,
				reward,
			);

			// validator takes 100% of the reward
			assert_eq!(payouts, vec![(1, reward)]);
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
				Perbill::from_rational_approximation(2u32, 1u32),
			);

			let reward = 1_000;
			let payouts = Rewards::calculate_npos_payouts(
				&mock_commission_stake_map.validator_stash,
				mock_commission_stake_map.commission,
				&mock_commission_stake_map.exposures,
				reward,
			);

			// validator takes 100% of the reward
			assert_eq!(payouts, vec![(1, reward)]);
		});
	}

	#[test]
	fn calculate_npos_payouts_empty_stake_map() {
		ExtBuilder::default().build().execute_with(|| {
			let reward = 1_000;
			Rewards::note_transaction_fees(reward);
			let total_issuance = RewardCurrency::total_issuance();
			Rewards::enqueue_reward_payouts(Default::default(), 0);

			assert!(Rewards::transaction_fee_pot().is_zero());
			assert!(Rewards::calculate_next_reward_payout().is_zero());
			assert_eq!(Rewards::transaction_fee_pot_history().front(), Some(&reward));
			// no payout, expect the total issuance to be as before
			assert_eq!(RewardCurrency::total_issuance(), total_issuance);
		});
	}

	#[test]
	fn small_reward_payouts() {
		ExtBuilder::default().build().execute_with(|| {
			let payout_split_threshold = <TestRuntime as Trait>::PayoutSplitThreshold::get();
			assert_eq!(
				Rewards::calculate_payout_quota(payout_split_threshold - 1, 5),
				payout_split_threshold - 1
			);
		});
	}

	#[test]
	fn large_reward_payouts_enough_time() {
		ExtBuilder::default().build().execute_with(|| {
			let payout_split_threshold = <TestRuntime as Trait>::PayoutSplitThreshold::get();
			assert_eq!(
				Rewards::calculate_payout_quota(payout_split_threshold, 100),
				payout_split_threshold
			);
			assert_eq!(
				Rewards::calculate_payout_quota(payout_split_threshold + 1, 100),
				payout_split_threshold
			);
			assert_eq!(
				Rewards::calculate_payout_quota(2 * payout_split_threshold, 100),
				payout_split_threshold
			);
		});
	}

	#[test]
	fn large_reward_payouts_not_enough_time() {
		ExtBuilder::default().build().execute_with(|| {
			let payout_split_threshold = <TestRuntime as Trait>::PayoutSplitThreshold::get();
			assert_eq!(
				Rewards::calculate_payout_quota(4 * payout_split_threshold, 1),
				2 * payout_split_threshold
			);
		});
	}

	#[test]
	fn large_reward_payouts_no_time() {
		ExtBuilder::default().build().execute_with(|| {
			let payout_split_threshold = <TestRuntime as Trait>::PayoutSplitThreshold::get();
			assert_eq!(
				Rewards::calculate_payout_quota(2 * payout_split_threshold, 0),
				2 * payout_split_threshold
			);
		});
	}
}
