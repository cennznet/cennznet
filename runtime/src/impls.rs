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

use crate::constants::fee::TARGET_BLOCK_FULLNESS;
use crate::{BuyFeeAsset, MaximumBlockWeight, Runtime};
use cennznet_primitives::types::{Balance, FeeExchange};
use crml_cennzx_spot::types::LowPrecisionUnsigned;
use crml_transaction_payment::GAS_FEE_EXCHANGE_KEY;
use frame_support::traits::OnUnbalanced;
use frame_support::{
	storage::unhashed,
	traits::{Currency, ExistenceRequirement, Get, WithdrawReason},
	weights::Weight,
};
use pallet_contracts::{Gas, GasMeter, NegativeImbalanceOf};
use pallet_generic_asset::StakingAssetCurrency;
use sp_runtime::{
	traits::{CheckedSub, Convert, SaturatedConversion, Saturating},
	DispatchError, Fixed64,
};

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

/// Convert from weight to balance via a simple coefficient multiplication
/// The associated type C encapsulates a constant in units of balance per weight
pub struct LinearWeightToFee<C>(sp_std::marker::PhantomData<C>);

impl<C: Get<Balance>> Convert<Weight, Balance> for LinearWeightToFee<C> {
	fn convert(w: Weight) -> Balance {
		// cennznet-node a weight of 10_000 (smallest non-zero weight) to be mapped to 10^7 units of
		// fees, hence:
		let coefficient = C::get();
		Balance::from(w).saturating_mul(coefficient)
	}
}

/// A struct that updates the weight multiplier based on the saturation level of the previous block.
/// This should typically be called once per-block.
///
/// This assumes that weight is a numeric value in the u32 range.
///
/// Given `TARGET_BLOCK_FULLNESS = 1/2`, a block saturation greater than 1/2 will cause the system
/// fees to slightly grow and the opposite for block saturations less than 1/2.
///
/// Formula:
///   diff = (target_weight - current_block_weight)
///   v = 0.00004
///   next_weight = weight * (1 + (v . diff) + (v . diff)^2 / 2)
///
/// https://research.web3.foundation/en/latest/polkadot/Token%20Economics/#relay-chain-transaction-fees
pub struct FeeMultiplierUpdateHandler;

impl Convert<(Weight, Fixed64), Fixed64> for FeeMultiplierUpdateHandler {
	fn convert(previous_state: (Weight, Fixed64)) -> Fixed64 {
		let (block_weight, multiplier) = previous_state;
		let max_weight = MaximumBlockWeight::get();
		let target_weight = (TARGET_BLOCK_FULLNESS * max_weight) as u128;
		let block_weight = block_weight as u128;

		// determines if the first_term is positive
		let positive = block_weight >= target_weight;
		let diff_abs = block_weight.max(target_weight) - block_weight.min(target_weight);
		// diff is within u32, safe.
		let diff = Fixed64::from_rational(diff_abs as i64, max_weight as u64);
		let diff_squared = diff.saturating_mul(diff);

		// 0.00004 = 4/100_000 = 40_000/10^9
		let v = Fixed64::from_rational(4, 100_000);
		// 0.00004^2 = 16/10^10 ~= 2/10^9. Taking the future /2 into account, then it is just 1 parts
		// from a billionth.
		let v_squared_2 = Fixed64::from_rational(1, 1_000_000_000);

		let first_term = v.saturating_mul(diff);
		// It is very unlikely that this will exist (in our poor perbill estimate) but we are giving
		// it a shot.
		let second_term = v_squared_2.saturating_mul(diff_squared);

		if positive {
			// Note: this is merely bounded by how big the multiplier and the inner value can go,
			// not by any economical reasoning.
			let excess = first_term.saturating_add(second_term);
			multiplier.saturating_add(excess)
		} else {
			// Proof: first_term > second_term. Safe subtraction.
			let negative = first_term - second_term;
			multiplier
				.saturating_sub(negative)
				// despite the fact that apply_to saturates weight (final fee cannot go below 0)
				// it is crucially important to stop here and don't further reduce the weight fee
				// multiplier. While at -1, it means that the network is so un-congested that all
				// transactions have no weight fee. We stop here and only increase if the network
				// became more busy.
				.max(Fixed64::from_rational(-1, 1))
		}
	}
}

/// Handles gas payment post contract execution (before deferring runtime calls) via CENNZX-Spot exchange.
pub struct GasHandler;

type CennzxModule<T> = crml_cennzx_spot::Module<T>;
type GenericAssetModule<T> = pallet_generic_asset::Module<T>;
type BalanceToUnsigned<T> = <T as crml_cennzx_spot::Trait>::BalanceToUnsignedInt;
type UnsignedToBalance<T> = <T as crml_cennzx_spot::Trait>::UnsignedIntToBalance;

impl<T> pallet_contracts::GasHandler<T> for GasHandler
where
	T: pallet_contracts::Trait + pallet_generic_asset::Trait + crml_cennzx_spot::Trait,
{
	fn fill_gas(transactor: &T::AccountId, gas_limit: Gas) -> Result<GasMeter<T>, DispatchError> {
		let mut buyable_gas = gas_limit.clone();

		let gas_price = <pallet_contracts::Module<T>>::gas_price();
		let gas_price_lpu: LowPrecisionUnsigned = gas_price.saturated_into();
		let cost_lpu = gas_price_lpu
			.checked_mul(gas_limit.into())
			.ok_or("Overflow multiplying gas limit by price")?;
		let cost = UnsignedToBalance::<T>::from(cost_lpu.clone()).into();

		if let Some(exchange_op) = unhashed::get::<FeeExchange<T::AssetId, T::Balance>>(&GAS_FEE_EXCHANGE_KEY) {
			let asset_id = exchange_op.get_asset_id();

			let exchanged_cost = CennzxModule::<T>::get_asset_to_core_output_price(
				&asset_id,
				cost.clone(),
				CennzxModule::<T>::fee_rate(),
			)?;
			let amount: T::Balance;
			if exchanged_cost > exchange_op.get_balance() {
				amount = exchange_op.get_balance();
				let buyable_core_asset = CennzxModule::<T>::get_asset_to_core_input_price(
					&asset_id,
					exchange_op.get_balance(),
					CennzxModule::<T>::fee_rate(),
				)?;
				let buyable_core_asset_lpu: LowPrecisionUnsigned =
					BalanceToUnsigned::<T>::from(buyable_core_asset).into();

				let gas_limit_lpu = buyable_core_asset_lpu
					.checked_div(gas_price_lpu)
					.ok_or("Gas price is zero")?;
				buyable_gas = Gas::saturated_from(gas_limit_lpu);
			} else {
				amount = exchanged_cost;
			}

			let balance = GenericAssetModule::<T>::free_balance(&asset_id, &transactor);
			let new_balance = balance
				.checked_sub(&amount)
				.ok_or("Balance is not covering the max payment of the fee exchange")?;

			GenericAssetModule::<T>::ensure_can_withdraw(
				&asset_id,
				transactor,
				amount,
				WithdrawReason::Fee.into(),
				new_balance,
			)?;
		} else {
			if let Ok(imbalance) = T::Currency::withdraw(
				transactor,
				cost_lpu.saturated_into(),
				WithdrawReason::Fee.into(),
				ExistenceRequirement::KeepAlive,
			) {
				<<T as pallet_contracts::Trait>::GasPayment as OnUnbalanced<NegativeImbalanceOf<T>>>::on_unbalanced(
					imbalance,
				);
			}
		}

		Ok(GasMeter::with_limit(buyable_gas, gas_price))
	}

	fn empty_unused_gas(transactor: &T::AccountId, gas_meter: GasMeter<T>) {
		let gas_spent = gas_meter.spent();

		// TODO mutate GasSpent for the block
		// Increase total spent gas.
		// This cannot overflow, since `gas_spent` is never greater than `block_gas_limit`, which
		// also has Gas type.
		// GasSpent::mutate(|block_gas_spent| *block_gas_spent += gas_spent);

		// The take() function ensures the entry is `killed` after access.
		if let Some(exchange_op) = unhashed::take::<FeeExchange<T::AssetId, T::Balance>>(&GAS_FEE_EXCHANGE_KEY) {
			// Fee exchange can never fail as conditions such as having enough liquidity
			// are checked early (before FeeExchange is put into storage)
			let _ = <crml_cennzx_spot::Module<T> as BuyFeeAsset>::buy_fee_asset(
				transactor,
				gas_spent.saturated_into(),
				&exchange_op,
			)
			.unwrap();
		} else {
			let gas_price = <pallet_contracts::Module<T>>::gas_price();
			let gas_price_lpu: LowPrecisionUnsigned = gas_price.saturated_into();
			if let Some(refund) = gas_price_lpu.checked_mul(gas_meter.gas_left().into()) {
				let _imbalance = T::Currency::deposit_creating(transactor, refund.saturated_into());
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::constants::currency::*;
	use crate::{AvailableBlockRatio, MaximumBlockWeight, Runtime};
	use frame_support::weights::Weight;

	fn max() -> Weight {
		MaximumBlockWeight::get()
	}

	fn target() -> Weight {
		TARGET_BLOCK_FULLNESS * max()
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

	fn fm(parts: i64) -> Fixed64 {
		Fixed64::from_parts(parts)
	}

	#[test]
	fn fee_multiplier_update_poc_works() {
		let fm = Fixed64::from_rational(0, 1);
		let test_set = vec![
			// TODO: this has a rounding error and fails.
			// (0, fm.clone()),
			(100, fm.clone()),
			(target(), fm.clone()),
			(max() / 2, fm.clone()),
			(max(), fm.clone()),
		];
		test_set.into_iter().for_each(|(w, fm)| {
			assert_eq!(
				fee_multiplier_update(w, fm),
				FeeMultiplierUpdateHandler::convert((w, fm)),
				"failed for weight {} and prev fm {:?}",
				w,
				fm,
			);
		})
	}

	#[test]
	fn empty_chain_simulation() {
		// just a few txs per_block.
		let block_weight = 1000;
		let mut fm = Fixed64::default();
		let mut iterations: u64 = 0;
		loop {
			let next = FeeMultiplierUpdateHandler::convert((block_weight, fm));
			fm = next;
			if fm == Fixed64::from_rational(-1, 1) {
				break;
			}
			iterations += 1;
		}
		println!("iteration {}, new fm = {:?}. Weight fee is now zero", iterations, fm);
	}

	#[test]
	#[ignore]
	fn congested_chain_simulation() {
		// `cargo test congested_chain_simulation -- --nocapture` to get some insight.

		// almost full. The entire quota of normal transactions is taken.
		let block_weight = AvailableBlockRatio::get() * max();

		// default minimum substrate weight
		let tx_weight = 10_000u32;

		// initial value of system
		let mut fm = Fixed64::default();
		assert_eq!(fm, Fixed64::from_parts(0));

		let mut iterations: u64 = 0;
		loop {
			let next = FeeMultiplierUpdateHandler::convert((block_weight, fm));
			if fm == next {
				break;
			}
			fm = next;
			iterations += 1;
			let fee = <Runtime as crml_transaction_payment::Trait>::WeightToFee::convert(tx_weight);
			let adjusted_fee = fm.saturated_multiply_accumulate(fee);
			println!(
				"iteration {}, new fm = {:?}. Fee at this point is: \
				 {} units, {} millicents, {} cents, {} dollars",
				iterations,
				fm,
				adjusted_fee,
				adjusted_fee / MILLICENTS,
				adjusted_fee / CENTS,
				adjusted_fee / DOLLARS
			);
		}
	}

	#[test]
	fn stateless_weight_mul() {
		// Light block. Fee is reduced a little.
		assert_eq!(
			FeeMultiplierUpdateHandler::convert((target() / 4, Fixed64::default())),
			fm(-7500)
		);
		// a bit more. Fee is decreased less, meaning that the fee increases as the block grows.
		assert_eq!(
			FeeMultiplierUpdateHandler::convert((target() / 2, Fixed64::default())),
			fm(-5000)
		);
		// ideal. Original fee. No changes.
		assert_eq!(
			FeeMultiplierUpdateHandler::convert((target(), Fixed64::default())),
			fm(0)
		);
		// // More than ideal. Fee is increased.
		assert_eq!(
			FeeMultiplierUpdateHandler::convert(((target() * 2), Fixed64::default())),
			fm(10000)
		);
	}

	#[test]
	fn stateful_weight_mul_grow_to_infinity() {
		assert_eq!(
			FeeMultiplierUpdateHandler::convert((target() * 2, Fixed64::default())),
			fm(10000)
		);
		assert_eq!(
			FeeMultiplierUpdateHandler::convert((target() * 2, fm(10000))),
			fm(20000)
		);
		assert_eq!(
			FeeMultiplierUpdateHandler::convert((target() * 2, fm(20000))),
			fm(30000)
		);
		// ...
		assert_eq!(
			FeeMultiplierUpdateHandler::convert((target() * 2, fm(1_000_000_000))),
			fm(1_000_000_000 + 10000)
		);
	}

	#[test]
	fn stateful_weight_mil_collapse_to_minus_one() {
		assert_eq!(FeeMultiplierUpdateHandler::convert((0, Fixed64::default())), fm(-10000));
		assert_eq!(FeeMultiplierUpdateHandler::convert((0, fm(-10000))), fm(-20000));
		assert_eq!(FeeMultiplierUpdateHandler::convert((0, fm(-20000))), fm(-30000));
		// ...
		assert_eq!(
			FeeMultiplierUpdateHandler::convert((0, fm(1_000_000_000 * -1))),
			fm(-1_000_000_000)
		);
	}

	#[test]
	fn weight_to_fee_should_not_overflow_on_large_weights() {
		let kb = 1024 as Weight;
		let mb = kb * kb;
		let max_fm = Fixed64::from_natural(i64::max_value());

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
			FeeMultiplierUpdateHandler::convert((i, Fixed64::default()));
		});

		// Some values that are all above the target and will cause an increase.
		let t = target();
		vec![t + 100, t * 2, t * 4].into_iter().for_each(|i| {
			let fm = FeeMultiplierUpdateHandler::convert((i, max_fm));
			// won't grow. The convert saturates everything.
			assert_eq!(fm, max_fm);
		});
	}
}
