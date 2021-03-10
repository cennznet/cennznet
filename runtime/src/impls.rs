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

//! Some configurable implementations as associated type for the substrate runtime.

use crate::{Rewards, Runtime, Staking, Treasury};
use cennznet_primitives::{
	traits::SimpleAssetSystem,
	types::{AccountId, Balance},
};
use crml_staking::rewards::RunScheduledPayout;
use frame_support::{
	dispatch::DispatchResult,
	traits::{Contains, ContainsLengthBound, Currency, Get, OnUnbalanced},
	weights::{Weight, WeightToFeeCoefficient, WeightToFeeCoefficients, WeightToFeePolynomial},
};
use prml_generic_asset::{NegativeImbalance, StakingAssetCurrency};
use prml_support::{AssetIdAuthority, MultiCurrencyAccounting};
use smallvec::smallvec;
use sp_runtime::Perbill;
use sp_std::{marker::PhantomData, prelude::*};

/// Runs scheduled payouts for the rewards module.
// It feeds staking module exposure data into rewards calculation/execution function.
pub struct ScheduledPayoutRunner<T: crml_staking::rewards::Trait>(PhantomData<T>);

impl<T: crml_staking::rewards::Trait> RunScheduledPayout for ScheduledPayoutRunner<T> {
	type AccountId = AccountId;
	type Balance = Balance;

	/// Feed exposure info from staking to run reward calculations
	fn run_payout(validator_stash: &Self::AccountId, total_payout: Self::Balance) -> Weight {
		use crml_staking::rewards::WeightInfo;

		let era = Staking::current_era().unwrap_or(0);
		let exposures = Staking::eras_stakers_clipped(era.saturating_sub(1), validator_stash);
		let commission = Staking::eras_validator_prefs(era.saturating_sub(1), validator_stash).commission;

		Rewards::process_reward_payout(&validator_stash, commission, &exposures, total_payout);

		return T::WeightInfo::process_reward_payouts(exposures.others.len() as u32);
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
pub struct RootMemberOnly<T: pallet_sudo::Trait>(PhantomData<T>);
impl<T: pallet_sudo::Trait> Contains<T::AccountId> for RootMemberOnly<T> {
	fn contains(t: &T::AccountId) -> bool {
		t == (&pallet_sudo::Module::<T>::key())
	}
	fn sorted_members() -> Vec<T::AccountId> {
		vec![(pallet_sudo::Module::<T>::key())]
	}
	fn count() -> usize {
		1
	}
}
impl<T: pallet_sudo::Trait> ContainsLengthBound for RootMemberOnly<T> {
	fn min_len() -> usize {
		1
	}
	fn max_len() -> usize {
		1
	}
}

/// Provides an impl for the `SimpleAssetSystem` trait
/// Used to integrate GA with CENNZX
pub struct SimpleAssetShim<T: prml_generic_asset::Trait>(PhantomData<T>);
impl<T: prml_generic_asset::Trait> SimpleAssetSystem for SimpleAssetShim<T> {
	type AccountId = T::AccountId;
	type AssetId = T::AssetId;
	type Balance = T::Balance;
	/// Transfer some `amount` of assets `from` one account `to` another
	fn transfer(
		asset_id: Self::AssetId,
		from: &Self::AccountId,
		to: &Self::AccountId,
		amount: Self::Balance,
	) -> DispatchResult {
		// note: we don't emit a 'transferred' event with this method
		prml_generic_asset::Module::<T>::make_transfer(asset_id, from, to, amount)
	}
	/// Get the liquid asset balance of `account`
	fn free_balance(asset_id: Self::AssetId, account: &Self::AccountId) -> Self::Balance {
		prml_generic_asset::Module::<T>::free_balance(asset_id, account)
	}
	/// Get the default asset/currency ID in the system
	fn default_asset_id() -> Self::AssetId {
		<prml_generic_asset::Module<T> as MultiCurrencyAccounting>::DefaultCurrencyId::asset_id()
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
		AdjustmentVariable, AvailableBlockRatio, MaximumBlockWeight, MinimumMultiplier, Multiplier, Runtime, System,
		TargetBlockFullness, TargetedFeeAdjustment, TransactionPayment, WeightToCpayFactor,
	};
	use frame_support::weights::{Weight, WeightToFeePolynomial};
	use sp_runtime::{
		assert_eq_error_rate,
		traits::Convert,
		FixedPointNumber,
	};

	fn max() -> Weight {
		AvailableBlockRatio::get() * MaximumBlockWeight::get()
	}

	fn min_multiplier() -> Multiplier {
		MinimumMultiplier::get()
	}

	fn target() -> Weight {
		TargetBlockFullness::get() * max()
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
		let m = max() as f64;
		// block weight always truncated to max weight
		let block_weight = (block_weight as f64).min(m);
		let v: f64 = AdjustmentVariable::get().to_fraction();

		// Ideal saturation in terms of weight
		let ss = target() as f64;
		// Current saturation in terms of weight
		let s = block_weight;

		let t1 = v * (s / m - ss / m);
		let t2 = v.powi(2) * (s / m - ss / m).powi(2) / 2.0;
		let next_float = previous_float * (1.0 + t1 + t2);
		Multiplier::from_fraction(next_float)
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
			System::set_block_limits(w, 0);
			assertions()
		});
	}

	#[test]
	fn truth_value_update_poc_works() {
		let fm = Multiplier::saturating_from_rational(1, 2);
		let test_set = vec![
			(0, fm.clone()),
			(100, fm.clone()),
			(1000, fm.clone()),
			(target(), fm.clone()),
			(max() / 2, fm.clone()),
			(max(), fm.clone()),
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
			let mut fm = Multiplier::one();
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
	fn min_change_per_day() {
		// Start with an adjustment multiplier of 1.
		// if every block in 24 hour period has a maximum weight then the multiplier should have increased
		// to > ~23% by the end of the period.
		run_with_system_weight(max(), || {
			let mut fm = Multiplier::one();
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
		let block_weight = AvailableBlockRatio::get() * max() - 100;

		// Default substrate weight.
		let tx_weight = frame_support::weights::constants::ExtrinsicBaseWeight::get();

		run_with_system_weight(block_weight, || {
			// initial value configured on module
			let mut fm = Multiplier::one();
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
				let fee = <Runtime as crml_transaction_payment::Trait>::WeightToFee::calc(&tx_weight);
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
	fn weight_mul_grow_on_big_block() {
		run_with_system_weight(target() * 2, || {
			let mut original = Multiplier::zero();
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
			MaximumBlockWeight::get() / 2,
			MaximumBlockWeight::get(),
			Weight::max_value() / 2,
			Weight::max_value(),
		]
		.into_iter()
		.for_each(|i| {
			run_with_system_weight(i, || {
				let next = runtime_multiplier_update(Multiplier::one());
				let truth = truth_value_update(i, Multiplier::one());
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
