// Copyright 2018-2021 Parity Techn ologies(UK) Ltd. and Centrality Investments Ltd.
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

//! Some configurable implementations as associated type for the substrate runtime.

use crate::{Babe, Identity, Rewards, Runtime, Session, Staking, System, Treasury};
use cennznet_primitives::types::{AccountId, Balance};
use crml_generic_asset::{NegativeImbalance, StakingAssetCurrency};
use crml_staking::{
	rewards::{RunScheduledPayout, WeightInfo},
	EraIndex, HandlePayee,
};
use crml_support::{H160, U256};
use frame_support::{
	pallet_prelude::*,
	traits::{
		tokens::{fungible::Inspect, DepositConsequence, WithdrawConsequence},
		Contains, ContainsLengthBound, Currency, ExistenceRequirement, FindAuthor, Get, Imbalance, OnUnbalanced,
		SignedImbalance, WithdrawReasons,
	},
	weights::{
		constants::RocksDbWeight as DbWeight, Weight, WeightToFeeCoefficient, WeightToFeeCoefficients,
		WeightToFeePolynomial,
	},
};
use pallet_evm::OnChargeEVMTransaction;
use smallvec::smallvec;
use sp_runtime::traits::UniqueSaturatedInto;
use sp_runtime::{
	traits::{SaturatedConversion, Zero},
	ConsensusEngineId, Perbill,
};
use sp_std::{marker::PhantomData, prelude::*};

/// Runs scheduled payouts for the rewards module.
pub struct ScheduledPayoutRunner<T: crml_staking::rewards::Config>(PhantomData<T>);

#[allow(dead_code)]
#[cfg(not(feature = "integration_config"))]
const MAX_VALIDATORS: u32 = 5_000;
#[allow(dead_code)]
#[cfg(feature = "integration_config")]
const MAX_VALIDATORS: u32 = 7; // low value for integration tests

/// Constant factor for scaling CPAY to its smallest indivisible unit
const CPAY_UNIT_VALUE: Balance = 10_u128.pow(14);

/// Convert 18dp wei values to 4dp equivalents (CPAY)
/// fractional amounts < `CPAY_UNIT_VALUE` are rounded up by adding 1 / 0.0001 cpay
pub fn scale_to_4dp(value: Balance) -> Balance {
	let (quotient, remainder) = (value / CPAY_UNIT_VALUE, value % CPAY_UNIT_VALUE);
	if remainder.is_zero() {
		quotient
	} else {
		// if value has a fractional part < CPAY unit value
		// it is lost in this divide operation
		quotient + 1
	}
}

/// Wraps spending currency (CPAY) for use by the EVM
/// Scales balances into 18dp equivalents which ethereum tooling and contracts expect
pub struct EvmCurrencyScaler<I: Inspect<AccountId>>(PhantomData<I>);
impl<I: Inspect<AccountId, Balance = Balance> + Currency<AccountId>> Inspect<AccountId> for EvmCurrencyScaler<I> {
	type Balance = Balance;

	/// The total amount of issuance in the system.
	fn total_issuance() -> Self::Balance {
		<I as Inspect<AccountId>>::total_issuance()
	}

	/// The minimum balance any single account may have.
	fn minimum_balance() -> Self::Balance {
		<I as Inspect<AccountId>>::minimum_balance()
	}

	/// Get the balance of `who`.
	/// Scaled up so values match expectations of an 18dp asset
	fn balance(who: &AccountId) -> Self::Balance {
		Self::reducible_balance(who, true)
	}

	/// Get the maximum amount that `who` can withdraw/transfer successfully.
	/// Scaled up so values match expectations of an 18dp asset
	fn reducible_balance(who: &AccountId, keep_alive: bool) -> Self::Balance {
		// Careful for overflow!
		let raw = I::reducible_balance(who, keep_alive);
		U256::from(raw)
			.saturating_mul(U256::from(CPAY_UNIT_VALUE))
			.saturated_into()
	}

	/// Returns `true` if the balance of `who` may be increased by `amount`.
	fn can_deposit(_who: &AccountId, _amount: Self::Balance) -> DepositConsequence {
		unimplemented!();
	}

	/// Returns `Failed` if the balance of `who` may not be decreased by `amount`, otherwise
	/// the consequence.
	fn can_withdraw(who: &AccountId, amount: Self::Balance) -> WithdrawConsequence<Self::Balance> {
		I::can_withdraw(who, amount)
	}
}

/// Currency impl for EVM usage
/// It proxies to the inner curreny impl while leaving some unused methods
/// unimplemented
impl<I> Currency<AccountId> for EvmCurrencyScaler<I>
where
	I: Inspect<AccountId, Balance = Balance>,
	I: Currency<
		AccountId,
		Balance = Balance,
		PositiveImbalance = crml_generic_asset::PositiveImbalance<Runtime>,
		NegativeImbalance = crml_generic_asset::NegativeImbalance<Runtime>,
	>,
{
	type Balance = <I as Currency<AccountId>>::Balance;
	type PositiveImbalance = <I as Currency<AccountId>>::PositiveImbalance;
	type NegativeImbalance = <I as Currency<AccountId>>::NegativeImbalance;

	fn free_balance(who: &AccountId) -> Self::Balance {
		Self::balance(who)
	}
	fn total_issuance() -> Self::Balance {
		<I as Currency<AccountId>>::total_issuance()
	}
	fn minimum_balance() -> Self::Balance {
		<I as Currency<AccountId>>::minimum_balance()
	}
	fn total_balance(who: &AccountId) -> Self::Balance {
		Self::balance(who)
	}
	fn transfer(from: &AccountId, to: &AccountId, value: Self::Balance, req: ExistenceRequirement) -> DispatchResult {
		I::transfer(from, to, scale_to_4dp(value), req)
	}
	fn ensure_can_withdraw(
		_who: &AccountId,
		_amount: Self::Balance,
		_reasons: WithdrawReasons,
		_new_balance: Self::Balance,
	) -> DispatchResult {
		unimplemented!();
	}
	fn withdraw(
		who: &AccountId,
		value: Self::Balance,
		reasons: WithdrawReasons,
		req: ExistenceRequirement,
	) -> Result<Self::NegativeImbalance, DispatchError> {
		I::withdraw(who, scale_to_4dp(value), reasons, req)
	}
	fn deposit_into_existing(who: &AccountId, value: Self::Balance) -> Result<Self::PositiveImbalance, DispatchError> {
		I::deposit_into_existing(who, scale_to_4dp(value))
	}
	fn deposit_creating(who: &AccountId, value: Self::Balance) -> Self::PositiveImbalance {
		I::deposit_creating(who, scale_to_4dp(value))
	}
	fn make_free_balance_be(
		who: &AccountId,
		balance: Self::Balance,
	) -> SignedImbalance<Self::Balance, Self::PositiveImbalance> {
		I::make_free_balance_be(who, scale_to_4dp(balance))
	}
	fn can_slash(_who: &AccountId, _value: Self::Balance) -> bool {
		false
	}
	fn slash(_who: &AccountId, _value: Self::Balance) -> (Self::NegativeImbalance, Self::Balance) {
		unimplemented!();
	}
	fn burn(mut _amount: Self::Balance) -> Self::PositiveImbalance {
		unimplemented!();
	}
	fn issue(mut _amount: Self::Balance) -> Self::NegativeImbalance {
		unimplemented!();
	}
}

/// Find block author formatted for ethereum compat
pub struct EthereumFindAuthor<F>(PhantomData<F>);
impl<F: FindAuthor<u32>> FindAuthor<H160> for EthereumFindAuthor<F> {
	fn find_author<'a, I>(digests: I) -> Option<H160>
	where
		I: 'a + IntoIterator<Item = (ConsensusEngineId, &'a [u8])>,
	{
		F::find_author(digests).map(|author_index| {
			if let Some(stash) = Session::validators().get(author_index as usize) {
				// Take first 20 bytes of the validator AccountId
				// NB: this matches the behaviour of `EnsureAddressTruncated`
				H160::from_slice(&AsRef::<[u8; 32]>::as_ref(stash)[..20])
			} else {
				H160::default()
			}
		})
	}
}

/// Implements transaction payments which handles withdrawing,
/// refunding and depositing of transaction fees.
/// Similar to `CurrencyAdapter` of `pallet_transaction_payment`
pub struct CENNZnetOnChargeEVMTransaction<T>(PhantomData<T>);

impl<T> OnChargeEVMTransaction<T> for CENNZnetOnChargeEVMTransaction<T>
where
	T: pallet_evm::Config
		+ frame_system::Config<AccountId = AccountId>
		+ pallet_session::Config<ValidatorId = AccountId>,
{
	type LiquidityInfo = <() as OnChargeEVMTransaction<T>>::LiquidityInfo;

	fn withdraw_fee(who: &H160, fee: U256) -> Result<Self::LiquidityInfo, pallet_evm::Error<T>> {
		<() as OnChargeEVMTransaction<T>>::withdraw_fee(who, fee)
	}

	fn correct_and_deposit_fee(who: &H160, corrected_fee: U256, already_withdrawn: Self::LiquidityInfo) {
		<() as OnChargeEVMTransaction<T>>::correct_and_deposit_fee(who, corrected_fee, already_withdrawn)
	}

	/// Pay the validator's priority fees to its reward account
	fn pay_priority_fee(tip: U256) {
		let digest = System::digest();
		let pre_runtime_digests = digest.logs.iter().filter_map(|d| d.as_pre_runtime());
		if let Some(author_index) = Babe::find_author(pre_runtime_digests) {
			if let Some(stash) = <pallet_session::Pallet<T>>::validators().get(author_index as usize) {
				let pay_to = Rewards::payee(stash);
				let _ = <T as pallet_evm::Config>::Currency::deposit_into_existing(
					&pay_to,
					tip.low_u128().unique_saturated_into(),
				);
			} else {
				log::error!("Error processing priority fee, validator not found");
			}
		} else {
			log::error!("Error processing priority fee, block author not found");
		}
	}
}

/// Handles block transaction fees tracking them using the Rewards module
pub struct DealWithFees;
impl OnUnbalanced<NegativeImbalance<Runtime>> for DealWithFees {
	fn on_unbalanceds<B>(mut fees_then_tips: impl Iterator<Item = NegativeImbalance<Runtime>>) {
		if let Some(_fees) = fees_then_tips.next() {
			if let Some(tips) = fees_then_tips.next() {
				Rewards::note_transaction_fees(tips.peek());
			}
		}
	}
}

// Move to Substrate identity module eventually
pub struct RegistrationImplementation<T: crml_governance::Config>(sp_std::marker::PhantomData<T>);
impl<T: crml_governance::Config> crml_support::RegistrationInfo for RegistrationImplementation<T> {
	// `T::AccountId` is missing `EncodeLike<AccountId32`
	type AccountId = cennznet_primitives::types::AccountId;

	fn registered_identity_count(who: &Self::AccountId) -> u32 {
		let registration = Identity::identity(who.clone());
		match registration {
			Some(registration) => registration
				.judgements
				.iter()
				.filter(|j| j.1 == pallet_identity::Judgement::KnownGood)
				.count() as u32,
			None => 0,
		}
	}
}

impl<T: crml_staking::rewards::Config> RunScheduledPayout for ScheduledPayoutRunner<T> {
	type AccountId = AccountId;
	type Balance = Balance;

	/// Feed exposure info from staking to run reward calculations
	/// This is triggered by Rewards `on_idle`
	fn run_payout(validator_stash: &Self::AccountId, amount: Self::Balance, payout_era: EraIndex) -> Weight {
		// payouts for previous era
		let consumed_weight = DbWeight::get().reads(2);
		let exposures = Staking::eras_stakers_clipped(payout_era, validator_stash);
		let commission = Staking::eras_validator_prefs(payout_era, validator_stash).commission;

		log::debug!(
			target: "runtime::rewards",
			"🏃‍♂️💰 reward payout for: ({:?}) worth: ({:?} CPAY) earned in: ({:?})",
			validator_stash,
			amount,
			payout_era,
		);

		Rewards::process_reward_payout(&validator_stash, commission, &exposures, amount, payout_era);

		return consumed_weight.saturating_add(T::WeightInfo::process_reward_payouts(exposures.others.len() as u32));
	}

	// Return weight estimate of given payout
	fn estimate_run_payout_weight(
		validator_stash: &Self::AccountId,
		_amount: Self::Balance,
		payout_era: EraIndex,
	) -> Weight {
		let exposures = Staking::eras_stakers_clipped(payout_era, validator_stash);
		// + 1 read for the estimate
		// + 2 reads for exposure and commission on the real execution
		return T::WeightInfo::process_reward_payouts(exposures.others.len() as u32)
			.saturating_add(DbWeight::get().reads(2 as Weight));
	}
}

/// Provides a simple weight to fee conversion function for
/// use with the CENNZnet 4dp spending asset, CPAY.
pub struct WeightToCpayFee<G: Get<Perbill>>(sp_std::marker::PhantomData<G>);

impl<G: Get<Perbill>> WeightToFeePolynomial for WeightToCpayFee<G> {
	/// The runtime Balance type
	type Balance = Balance;
	/// Scale weights to fees by a factor of 1/`G`
	fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
		smallvec!(WeightToFeeCoefficient {
			coeff_integer: 0,
			coeff_frac: G::get(),
			negative: false,
			degree: 1,
		})
	}
}

/// Provides a membership set with only the configured sudo user
pub struct RootMemberOnly<T: pallet_sudo::Config>(PhantomData<T>);
impl<T: pallet_sudo::Config> Contains<T::AccountId> for RootMemberOnly<T> {
	fn contains(t: &T::AccountId) -> bool {
		t == (&pallet_sudo::Pallet::<T>::key())
	}
}
impl<T: pallet_sudo::Config> ContainsLengthBound for RootMemberOnly<T> {
	fn min_len() -> usize {
		1
	}
	fn max_len() -> usize {
		1
	}
}

/// An on unbalanced handler which takes a slash amount in the staked currency
/// and moves it to the system `Treasury` account.
pub struct SlashFundsToTreasury;
impl OnUnbalanced<NegativeImbalance<Runtime>> for SlashFundsToTreasury {
	fn on_nonzero_unbalanced(slash_amount: NegativeImbalance<Runtime>) {
		StakingAssetCurrency::resolve_creating(&Treasury::account_id(), slash_amount);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{
		constants::{
			currency::{DOLLARS, MICROS, WEI},
			time::DAYS,
		},
		AdjustmentVariable, MinimumMultiplier, Multiplier, Runtime, RuntimeBlockWeights as BlockWeights, System,
		TargetBlockFullness, TargetedFeeAdjustment, TransactionPayment, WeightToCpayFactor,
	};
	use frame_support::weights::{DispatchClass, Weight, WeightToFeePolynomial};
	use sp_runtime::{assert_eq_error_rate, traits::Convert, FixedPointNumber};

	fn max_normal() -> Weight {
		BlockWeights::get()
			.get(DispatchClass::Normal)
			.max_total
			.unwrap_or_else(|| BlockWeights::get().max_block)
	}

	fn min_multiplier() -> Multiplier {
		MinimumMultiplier::get()
	}

	fn target() -> Weight {
		TargetBlockFullness::get() * max_normal()
	}

	// update based on runtime impl.
	fn runtime_multiplier_update(fm: Multiplier) -> Multiplier {
		TargetedFeeAdjustment::<Runtime, TargetBlockFullness, AdjustmentVariable, MinimumMultiplier>::convert(fm)
	}

	// update based on reference impl.
	fn truth_value_update(block_weight: Weight, previous: Multiplier) -> Multiplier {
		let accuracy = Multiplier::accuracy() as f64;
		let previous_float = previous.into_inner() as f64 / accuracy;
		// bump if it is zero.
		let previous_float = previous_float.max(min_multiplier().into_inner() as f64 / accuracy);

		// maximum tx weight
		let m = max_normal() as f64;
		// block weight always truncated to max weight
		let block_weight = (block_weight as f64).min(m);
		let v: f64 = AdjustmentVariable::get().to_float();

		// Ideal saturation in terms of weight
		let ss = target() as f64;
		// Current saturation in terms of weight
		let s = block_weight;

		let t1 = v * (s / m - ss / m);
		let t2 = v.powi(2) * (s / m - ss / m).powi(2) / 2.0;
		let next_float = previous_float * (1.0 + t1 + t2);
		Multiplier::from_float(next_float)
	}

	fn run_with_system_weight<F>(w: Weight, assertions: F)
	where
		F: Fn() -> (),
	{
		let mut t: sp_io::TestExternalities = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap()
			.into();
		t.execute_with(|| {
			System::set_block_consumed_resources(w, 0);
			assertions()
		});
	}

	#[test]
	fn wei_to_cpay_units_scaling() {
		let amounts_18 = vec![
			1000050000000000000u128, // fractional bits <  0.0001
			1000000000000000001u128, // fractional bits <  0.0001
			1000100000000000000u128, // fractional bits at 0.0001
			1000000000000000000u128, // no fractional bits < 0.0001
			999u128,                 // entirely < 0.0001
			1u128,
			0u128,
		];
		let amounts_4 = vec![10001_u128, 10001, 10001, 10000, 1, 1, 0];
		for (amount_18, amount_4) in amounts_18.into_iter().zip(amounts_4.into_iter()) {
			println!("{:?}/{:?}", amount_18, amount_4);
			assert_eq!(scale_to_4dp(amount_18), amount_4);
		}
	}

	#[test]
	fn truth_value_update_poc_works() {
		let fm = Multiplier::saturating_from_rational(1, 2);
		let test_set = vec![
			(0, fm.clone()),
			(100, fm.clone()),
			(1000, fm.clone()),
			(target(), fm.clone()),
			(max_normal() / 2, fm.clone()),
			(max_normal(), fm.clone()),
		];
		test_set.into_iter().for_each(|(w, fm)| {
			run_with_system_weight(w, || {
				assert_eq_error_rate!(
					truth_value_update(w, fm),
					runtime_multiplier_update(fm),
					// Error is only 1 in 100^18
					Multiplier::from_inner(100),
				);
			})
		})
	}

	#[test]
	fn multiplier_can_grow_from_zero() {
		// if the min is too small, then this will not change, and we are doomed forever.
		// the weight is 1/100th bigger than target.
		run_with_system_weight(target() * 101 / 100, || {
			let next = runtime_multiplier_update(min_multiplier());
			assert!(next > min_multiplier(), "{:?} !>= {:?}", next, min_multiplier());
		})
	}

	#[test]
	fn multiplier_cannot_go_below_limit() {
		// will not go any further below even if block is empty.
		run_with_system_weight(0, || {
			let next = runtime_multiplier_update(min_multiplier());
			assert_eq!(next, min_multiplier());
		})
	}

	#[test]
	fn time_to_reach_zero() {
		// blocks per 24h in substrate-node: 28,800 (k)
		// s* = 0.1875
		// The bound from the research in an empty chain is:
		// v <~ (p / k(0 - s*))
		// p > v * k * -0.1875
		// to get p == -1 we'd need
		// -1 > 0.00001 * k * -0.1875
		// 1 < 0.00001 * k * 0.1875
		// 10^9 / 1875 < k
		// k > 533_333 ~ 18,5 days.
		run_with_system_weight(0, || {
			// start from 1, the default.
			let mut fm = Multiplier::from(1);
			let mut iterations: u64 = 0;
			loop {
				let next = runtime_multiplier_update(fm);
				fm = next;
				if fm == min_multiplier() {
					break;
				}
				iterations += 1;
			}
			assert!(iterations > 533_333);
		})
	}

	#[test]
	#[ignore]
	fn min_change_per_day() {
		// Start with an adjustment multiplier of 1.
		// if every block in 24 hour period has a maximum weight then the multiplier should have increased
		// to > ~23% by the end of the period.
		run_with_system_weight(max_normal(), || {
			let mut fm = Multiplier::from(1);
			// `DAYS` is a function of `SECS_PER_BLOCK`
			// this function will be invoked `DAYS / SECS_PER_BLOCK` times, the original test from substrate assumes a
			// 3 second block time
			for _ in 0..DAYS {
				let next = runtime_multiplier_update(fm);
				fm = next;
			}
			assert!(fm > Multiplier::saturating_from_rational(1234, 1000));
		})
	}

	#[test]
	#[ignore]
	fn congested_chain_simulation() {
		// `cargo test congested_chain_simulation -- --nocapture` to get some insight.

		// almost full. The entire quota of normal transactions is taken.
		let block_weight = BlockWeights::get().get(DispatchClass::Normal).max_total.unwrap() - 100;

		// Default substrate weight.
		let tx_weight = frame_support::weights::constants::ExtrinsicBaseWeight::get();

		run_with_system_weight(block_weight, || {
			// initial value configured on module
			let mut fm = Multiplier::from(1);
			assert_eq!(fm, TransactionPayment::next_fee_multiplier());

			let mut iterations: u64 = 0;
			loop {
				let next = runtime_multiplier_update(fm);
				// if no change, panic. This should never happen in this case.
				if fm == next {
					panic!("The fee should ever increase");
				}
				fm = next;
				iterations += 1;
				let fee = <Runtime as crml_transaction_payment::Config>::WeightToFee::calc(&tx_weight);
				let adjusted_fee = fm.saturating_mul_acc_int(fee);
				println!(
					"iteration {}, new fm = {:?}. Fee at this point is: {} units / {} weis, \
					{} micros, {} dollars",
					iterations,
					fm,
					adjusted_fee,
					adjusted_fee / WEI,
					adjusted_fee / MICROS,
					adjusted_fee / DOLLARS,
				);
			}
		});
	}

	#[test]
	fn stateless_weight_mul() {
		let fm = Multiplier::saturating_from_rational(1, 2);
		run_with_system_weight(target() / 4, || {
			let next = runtime_multiplier_update(fm);
			assert_eq_error_rate!(next, truth_value_update(target() / 4, fm), Multiplier::from_inner(100),);

			// Light block. Multiplier is reduced a little.
			assert!(next < fm);
		});

		run_with_system_weight(target() / 2, || {
			let next = runtime_multiplier_update(fm);
			assert_eq_error_rate!(next, truth_value_update(target() / 2, fm), Multiplier::from_inner(100),);
			// Light block. Multiplier is reduced a little.
			assert!(next < fm);
		});
		run_with_system_weight(target(), || {
			let next = runtime_multiplier_update(fm);
			assert_eq_error_rate!(next, truth_value_update(target(), fm), Multiplier::from_inner(100),);
			// ideal. No changes.
			assert_eq!(next, fm)
		});
		run_with_system_weight(target() * 2, || {
			// More than ideal. Fee is increased.
			let next = runtime_multiplier_update(fm);
			assert_eq_error_rate!(next, truth_value_update(target() * 2, fm), Multiplier::from_inner(100),);

			// Heavy block. Fee is increased a little.
			assert!(next > fm);
		});
	}

	#[test]
	#[ignore]
	fn weight_mul_grow_on_big_block() {
		run_with_system_weight(target() * 2, || {
			let mut original = Multiplier::from(0);
			let mut next = Multiplier::default();

			(0..1_000).for_each(|_| {
				next = runtime_multiplier_update(original);
				assert_eq_error_rate!(
					next,
					truth_value_update(target() * 2, original),
					Multiplier::from_inner(100),
				);
				// must always increase
				assert!(next > original, "{:?} !>= {:?}", next, original);
				original = next;
			});
		});
	}

	#[test]
	fn weight_mul_decrease_on_small_block() {
		run_with_system_weight(target() / 2, || {
			let mut original = Multiplier::saturating_from_rational(1, 2);
			let mut next;

			for _ in 0..100 {
				// decreases
				next = runtime_multiplier_update(original);
				assert!(next < original, "{:?} !<= {:?}", next, original);
				original = next;
			}
		})
	}

	#[test]
	fn weight_to_fee_should_not_overflow_on_large_weights() {
		let kb = 1024 as Weight;
		let mb = kb * kb;
		let max_fm = Multiplier::saturating_from_integer(i128::max_value());

		// check that for all values it can compute, correctly.
		vec![
			0,
			1,
			10,
			1000,
			kb,
			10 * kb,
			100 * kb,
			mb,
			10 * mb,
			2147483647,
			4294967295,
			BlockWeights::get().max_block / 2,
			BlockWeights::get().max_block,
			Weight::max_value() / 2,
			Weight::max_value(),
		]
		.into_iter()
		.for_each(|i| {
			run_with_system_weight(i, || {
				let next = runtime_multiplier_update(Multiplier::from(1));
				let truth = truth_value_update(i, Multiplier::from(1));
				assert_eq_error_rate!(truth, next, Multiplier::from_inner(50_000_000));
			});
		});

		// Some values that are all above the target and will cause an increase.
		let t = target();
		vec![t + 100, t * 2, t * 4].into_iter().for_each(|i| {
			run_with_system_weight(i, || {
				let fm = runtime_multiplier_update(max_fm);
				// won't grow. The convert saturates everything.
				assert_eq!(fm, max_fm);
			})
		});
	}

	#[test]
	fn weight_to_cpay_fee_scaling() {
		// ~1,000,000:1, configured in runtime/src/lib.rs `WeightToCpayFactor`
		assert_eq!(WeightToCpayFee::<WeightToCpayFactor>::calc(&1_000_000), 1 * MICROS);
		assert_eq!(WeightToCpayFee::<WeightToCpayFactor>::calc(&0), 0);
		// check no issues at max. value
		let _ = WeightToCpayFee::<WeightToCpayFactor>::calc(&u64::max_value());
	}
}
