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

use crate::{
	constants::fee::{MAX_WEIGHT, MIN_WEIGHT},
	Call, Runtime,
};
use cennznet_primitives::types::Balance;
use frame_support::{
	traits::{Contains, ContainsLengthBound, Currency, Get},
	weights::Weight,
};
use prml_generic_asset::StakingAssetCurrency;
use sp_runtime::traits::Convert;
use sp_std::{marker::PhantomData, prelude::*};

// TODO uncomment the following code after enable cennznet staking module
// use crate::NegativeImbalance
// pub struct SplitToAllValidators;
//
// /// This handles the ```NegativeImbalance``` created for transaction fee.
// /// The reward is split evenly and distributed to all of the current elected validators.
// /// The remainder from the division are burned.
// impl OnUnbalanced<NegativeImbalance> for SplitToAllValidators {
// 	fn on_nonzero_unbalanced(imbalance: NegativeImbalance) {
// 		let amount = imbalance.peek();
//
// 		if !amount.is_zero() {
// 			crml_staking::Module::<Runtime>::add_to_current_era_transaction_fee_reward(amount);
// 		}
// 	}
// }

/// Struct that handles the conversion of Balance -> `u64`. This is used for staking's election
/// calculation.
pub struct CurrencyToVoteHandler;

impl CurrencyToVoteHandler {
	fn factor() -> Balance {
		(<StakingAssetCurrency<Runtime>>::total_issuance() / u64::max_value() as Balance).max(1)
	}
}

impl Convert<Balance, u64> for CurrencyToVoteHandler {
	fn convert(x: Balance) -> u64 {
		(x / Self::factor()) as u64
	}
}

impl Convert<u128, Balance> for CurrencyToVoteHandler {
	fn convert(x: u128) -> Balance {
		x * Self::factor()
	}
}

/// Convert from weight to fee balance by scaling it into the desired fee range.
/// i.e. transpose weight values so that: `min_fee` < weight < `max_fee`
pub struct ScaledWeightToFee<MinFee, MaxFee>(sp_std::marker::PhantomData<(MinFee, MaxFee)>);

impl<MinFee: Get<Balance>, MaxFee: Get<Balance>> Convert<Weight, Balance> for ScaledWeightToFee<MinFee, MaxFee> {
	/// Transpose weight values to desired fee range i.e. `min_fee` < x < `max_fee`
	fn convert(w: Weight) -> Balance {
		let weight = Balance::from(w);

		// Runtime constants
		let min_fee = MinFee::get();
		let max_fee = MaxFee::get();
		debug_assert!(max_fee > min_fee);
		debug_assert!(MAX_WEIGHT > MIN_WEIGHT);

		//      (weight - MIN_WEIGHT) * [min_fee, max_fee]
		//  y = ------------------------------------------ + min_fee
		//              [MIN_WEIGHT, MAX_WEIGHT]

		// ensure `weight` is in range: [MIN_WEIGHT, MAX_WEIGHT] for correct scaling.
		let capped_weight = weight.min(MAX_WEIGHT).max(MIN_WEIGHT);
		((capped_weight.saturating_sub(MIN_WEIGHT)).saturating_mul(max_fee.saturating_sub(min_fee))
			/ (MAX_WEIGHT.saturating_sub(MIN_WEIGHT)))
		.saturating_add(min_fee)
	}
}

/// The type that implements FeePayer for the cennznet-runtime Call(s)
pub struct FeePayerResolver;
impl crml_transaction_payment::FeePayer for FeePayerResolver {
	type Call = Call;
	type AccountId = <Runtime as frame_system::Trait>::AccountId;
	fn fee_payer(call: &Self::Call) -> Option<<Runtime as frame_system::Trait>::AccountId> {
		let is_sylo = match call {
			Call::SyloGroups(_) => true,
			Call::SyloE2EE(_) => true,
			Call::SyloDevice(_) => true,
			Call::SyloInbox(_) => true,
			Call::SyloResponse(_) => true,
			Call::SyloVault(_) => true,
			_ => false,
		};
		if is_sylo {
			crml_sylo::payment::Module::<Runtime>::payment_account()
		} else {
			None
		}
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

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{
		constants::fee::{MAX_WEIGHT, MIN_WEIGHT},
		MaximumBlockWeight, Runtime, TargetBlockFullness, TransactionMaxWeightFee, TransactionMinWeightFee,
	};
	use frame_support::weights::Weight;
	use sp_runtime::assert_eq_error_rate;

	fn max() -> Weight {
		MaximumBlockWeight::get()
	}

	fn target() -> Weight {
		TargetBlockFullness::get() * max()
	}

	// poc reference implementation.
	fn fee_multiplier_update(block_weight: Weight, previous: Fixed64) -> Fixed64 {
		let block_weight = block_weight as f32;
		let v: f32 = 0.00004;

		// maximum tx weight
		let m = max() as f32;
		// Ideal saturation in terms of weight
		let ss = target() as f32;
		// Current saturation in terms of weight
		let s = block_weight;

		let fm = (v * (s / m - ss / m)) + (v.powi(2) * (s / m - ss / m).powi(2)) / 2.0;
		let addition_fm = Fixed64::from_parts((fm * 1_000_000_000_f32) as i64);
		previous.saturating_add(addition_fm)
	}

	fn feemul(parts: i64) -> Fixed64 {
		Fixed64::from_parts(parts)
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
	fn fee_multiplier_update_poc_works() {
		let fm = Fixed64::from_rational(0, 1);
		let test_set = vec![
			(0, fm.clone()),
			(100, fm.clone()),
			(target(), fm.clone()),
			(max() / 2, fm.clone()),
			(max(), fm.clone()),
		];
		test_set.into_iter().for_each(|(w, fm)| {
			run_with_system_weight(w, || {
				assert_eq_error_rate!(
					fee_multiplier_update(w, fm).into_inner(),
					TargetedFeeAdjustment::<TargetBlockFullness>::convert(fm).into_inner(),
					5,
				);
			})
		})
	}

	#[test]
	fn empty_chain_simulation() {
		// just a few txs per_block.
		let block_weight = 0;
		run_with_system_weight(block_weight, || {
			let mut fm = Fixed64::default();
			let mut iterations: u64 = 0;
			loop {
				let next = TargetedFeeAdjustment::<TargetBlockFullness>::convert(fm);
				fm = next;
				if fm == Fixed64::from_rational(-1, 1) {
					break;
				}
				iterations += 1;
			}
			println!("iteration {}, new fm = {:?}. Weight fee is now zero", iterations, fm);
			assert!(
				iterations > 50_000,
				"This assertion is just a warning; Don't panic. \
				Current substrate/polkadot node are configured with a _slow adjusting fee_ \
				mechanism. Hence, it is really unlikely that fees collapse to zero even on an \
				empty chain in less than at least of couple of thousands of empty blocks. But this \
				simulation indicates that fees collapsed to zero after {} almost-empty blocks. \
				Check it",
				iterations,
			);
		})
	}

	#[test]
	fn weight_to_fee_scaling_theoretical_max_weight() {
		let weight = u32::max_value();
		let weight_fee = <Runtime as crml_transaction_payment::Trait>::WeightToFee::convert(weight);
		assert_eq!(TransactionMaxWeightFee::get(), weight_fee);
	}

	#[test]
	fn weight_to_fee_scaling_practical_max_weight() {
		let weight = MAX_WEIGHT as u32;
		let weight_fee = <Runtime as crml_transaction_payment::Trait>::WeightToFee::convert(weight);
		assert_eq!(TransactionMaxWeightFee::get(), weight_fee);
	}

	#[test]
	fn weight_to_fee_scaling_min_weight() {
		let weight = MIN_WEIGHT as u32;
		let weight_fee = <Runtime as crml_transaction_payment::Trait>::WeightToFee::convert(weight);
		assert_eq!(TransactionMinWeightFee::get(), weight_fee);
	}

	#[test]
	fn weight_to_fee_scaling_returns_transposed_weight() {
		let weight = 200_000_u32;
		let weight_fee = <Runtime as crml_transaction_payment::Trait>::WeightToFee::convert(weight);
		assert_eq!(2_000, weight_fee);
	}

	#[test]
	fn weight_to_fee_scaling_for_zero_weight() {
		let weight = 0;
		let weight_fee = <Runtime as crml_transaction_payment::Trait>::WeightToFee::convert(weight);
		assert_eq!(TransactionMinWeightFee::get(), weight_fee);
	}

	#[test]
	fn stateless_weight_mul() {
		run_with_system_weight(target() / 4, || {
			// Light block. Fee is reduced a little.
			assert_eq!(
				TargetedFeeAdjustment::<TargetBlockFullness>::convert(Fixed64::default()),
				feemul(-7500),
			);
		});
		run_with_system_weight(target() / 2, || {
			// a bit more. Fee is decreased less, meaning that the fee increases as the block grows.
			assert_eq!(
				TargetedFeeAdjustment::<TargetBlockFullness>::convert(Fixed64::default()),
				feemul(-5000),
			);
		});
		run_with_system_weight(target(), || {
			// ideal. Original fee. No changes.
			assert_eq!(
				TargetedFeeAdjustment::<TargetBlockFullness>::convert(Fixed64::default()),
				feemul(0),
			);
		});
		run_with_system_weight(target() * 2, || {
			// // More than ideal. Fee is increased.
			assert_eq!(
				TargetedFeeAdjustment::<TargetBlockFullness>::convert(Fixed64::default()),
				feemul(10000),
			);
		});
	}

	#[test]
	fn stateful_weight_mul_grow_to_infinity() {
		run_with_system_weight(target() * 2, || {
			assert_eq!(
				TargetedFeeAdjustment::<TargetBlockFullness>::convert(Fixed64::default()),
				feemul(10000)
			);
			assert_eq!(
				TargetedFeeAdjustment::<TargetBlockFullness>::convert(feemul(10000)),
				feemul(20000)
			);
			assert_eq!(
				TargetedFeeAdjustment::<TargetBlockFullness>::convert(feemul(20000)),
				feemul(30000)
			);
			// ...
			assert_eq!(
				TargetedFeeAdjustment::<TargetBlockFullness>::convert(feemul(1_000_000_000)),
				feemul(1_000_000_000 + 10000)
			);
		});
	}

	#[test]
	fn stateful_weight_mil_collapse_to_minus_one() {
		run_with_system_weight(0, || {
			assert_eq!(
				TargetedFeeAdjustment::<TargetBlockFullness>::convert(Fixed64::default()),
				feemul(-10000)
			);
			assert_eq!(
				TargetedFeeAdjustment::<TargetBlockFullness>::convert(feemul(-10000)),
				feemul(-20000)
			);
			assert_eq!(
				TargetedFeeAdjustment::<TargetBlockFullness>::convert(feemul(-20000)),
				feemul(-30000)
			);
			// ...
			assert_eq!(
				TargetedFeeAdjustment::<TargetBlockFullness>::convert(feemul(1_000_000_000 * -1)),
				feemul(-1_000_000_000)
			);
		})
	}

	#[test]
	fn weight_to_fee_should_not_overflow_on_large_weights() {
		let kb = 1024 as Weight;
		let mb = kb * kb;
		let max_fm = Fixed64::from_natural(i64::max_value());

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
			Weight::max_value() / 2,
			Weight::max_value(),
		]
		.into_iter()
		.for_each(|i| {
			run_with_system_weight(i, || {
				let next = TargetedFeeAdjustment::<TargetBlockFullness>::convert(Fixed64::default());
				let truth = fee_multiplier_update(i, Fixed64::default());
				assert_eq_error_rate!(truth.into_inner(), next.into_inner(), 5);
			});
		});

		// Some values that are all above the target and will cause an increase.
		let t = target();
		vec![t + 100, t * 2, t * 4].into_iter().for_each(|i| {
			run_with_system_weight(i, || {
				let fm = TargetedFeeAdjustment::<TargetBlockFullness>::convert(max_fm);
				// won't grow. The convert saturates everything.
				assert_eq!(fm, max_fm);
			})
		});
	}
}
