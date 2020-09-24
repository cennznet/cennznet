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
	sylo_payment, Call, MaximumBlockWeight, Runtime, System,
};
use cennznet_primitives::{
	traits::{BuyFeeAsset, IsGasMeteredCall},
	types::{Balance, FeeExchange},
};
use cennznut::{CENNZnut, RuntimeDomain, ValidationErr};
use codec::Decode;
use crml_transaction_payment::GAS_FEE_EXCHANGE_KEY;
use frame_support::{
	additional_traits, storage,
	traits::{Currency, ExistenceRequirement, Get, OnUnbalanced, WithdrawReason},
	weights::Weight,
};
use pallet_contracts::{Gas, GasMeter};
use pallet_generic_asset::StakingAssetCurrency;
use sp_runtime::{
	traits::{
		CheckedMul, CheckedSub, Convert, PlugDoughnutApi, SaturatedConversion, Saturating, UniqueSaturatedFrom, Zero,
	},
	DispatchError, Fixed64, Perbill,
};
use sp_std::{any::Any, prelude::Vec};

type CennzxSpot<T> = crml_cennzx_spot::Module<T>;
type Contracts<T> = pallet_contracts::Module<T>;
type GenericAsset<T> = pallet_generic_asset::Module<T>;

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

/// Update the given multiplier based on the following formula
///
///   diff = (target_weight - previous_block_weight)
///   v = 0.00004
///   next_weight = weight * (1 + (v . diff) + (v . diff) ^ 2 / 2)
///
/// Where `target_weight` must be given as the `Get` implementation of the `T` generic type.
/// https://research.web3.foundation/en/latest/polkadot/Token%20Economics/#relay-chain-transaction-fees
pub struct TargetedFeeAdjustment<T>(sp_std::marker::PhantomData<T>);

impl<T: Get<Perbill>> Convert<Fixed64, Fixed64> for TargetedFeeAdjustment<T> {
	fn convert(multiplier: Fixed64) -> Fixed64 {
		let block_weight = System::all_extrinsics_weight();
		let max_weight = MaximumBlockWeight::get();
		let target_weight = (T::get() * max_weight) as u128;
		let block_weight = block_weight as u128;

		// determines if the first_term is positive
		let positive = block_weight >= target_weight;
		let diff_abs = block_weight.max(target_weight) - block_weight.min(target_weight);
		// diff is within u32, safe.
		let diff = Fixed64::from_rational(diff_abs as i64, max_weight as u64);
		let diff_squared = diff.saturating_mul(diff);

		// 0.00004 = 4/100_000 = 40_000/10^9
		let v = Fixed64::from_rational(4, 100_000);
		// 0.00004^2 = 16/10^10 ~= 2/10^9. Taking the future /2 into account, then it is just 1
		// parts from a billionth.
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

impl<T> pallet_contracts::GasHandler<T> for GasHandler
where
	T: pallet_contracts::Trait + pallet_generic_asset::Trait + crml_cennzx_spot::Trait,
{
	/// Fill the gas meter
	///
	/// The process is as follows:
	/// 1) Calculate the cost to fill the gas meter (gas price * gas limit)
	/// 2a) Default case:
	///    - User is paying in the native fee currency
	///    - Deduct the 'fill meter cost' from the users balance and fill the gas meter
	/// 2b) User has nominated to pay fees in another currency
	///    - Calculate the 'fill gas cost' in terms of their nominated payment currency-
	///      using the CENNZX spot exchange rate
	///....- Check the user has liquid balance to pay the converted 'fill gas cost' and fill the gas meter
	fn fill_gas(transactor: &T::AccountId, gas_limit: Gas) -> Result<GasMeter<T>, DispatchError> {
		// Calculate the cost to fill the meter in the CENNZnet fee currency
		let gas_price = Contracts::<T>::gas_price();
		let fill_meter_cost = if gas_price.is_zero() {
			// Gas is free in this configuration, fill the meter
			return Ok(GasMeter::with_limit(gas_limit, gas_price));
		} else {
			gas_price
				.checked_mul(&gas_limit.saturated_into())
				.ok_or("Overflow during gas cost calculation")?
		};

		// Check if a fee exchange has been specified by the user
		let fee_exchange: Option<FeeExchange<T::AssetId, T::Balance>> = storage::unhashed::get(&GAS_FEE_EXCHANGE_KEY);

		if fee_exchange.is_none() {
			// User will pay for gas in CENNZnet's native fee currency
			let imbalance = T::Currency::withdraw(
				transactor,
				fill_meter_cost,
				WithdrawReason::Fee.into(),
				ExistenceRequirement::KeepAlive,
			)?;
			T::GasPayment::on_unbalanced(imbalance);
			return Ok(GasMeter::with_limit(gas_limit, gas_price));
		}

		// User wants to pay fee in a nominated currency
		let exchange_op = fee_exchange.unwrap();
		let payment_asset = exchange_op.asset_id();

		// Calculate the `fill_meter_cost` in terms of the user's nominated payment asset
		let converted_fill_meter_cost = CennzxSpot::<T>::get_asset_to_core_buy_price(
			&payment_asset,
			T::Balance::unique_saturated_from(fill_meter_cost.saturated_into()),
		)?;

		// Respect the user's max. fee preference
		if converted_fill_meter_cost > exchange_op.max_payment() {
			return Err("Fee cost exceeds max. payment limit".into());
		}

		// Calculate the expected user balance after paying the `converted_fill_meter_cost`
		// This value is required to ensure liquidity restrictions are upheld
		let balance_after_fill_meter = GenericAsset::<T>::free_balance(&payment_asset, transactor)
			.checked_sub(&converted_fill_meter_cost)
			.ok_or("Insufficient liquidity to fill gas meter")?;

		// Does the user have enough funds to pay the `converted_fill_meter_cost` with `payment_asset`
		// also taking into consideration any liquidity restrictions
		GenericAsset::<T>::ensure_can_withdraw(
			&payment_asset,
			transactor,
			converted_fill_meter_cost,
			WithdrawReason::Fee.into(),
			balance_after_fill_meter,
		)?;

		// User has the requisite amount of `payment_asset` to fund the meter
		// Actual payment will be handled in `empty_unused_gas` as the user may not spend the entire limit
		// Performing payment on the known gas spent will avoid a refund situation
		return Ok(GasMeter::with_limit(gas_limit, gas_price));
	}

	/// Handle settlement of unused gas after contract execution
	///
	/// The process is as follows:
	/// - Default case: refund unused gas tokens to the user (`transactor`) in CENNZnet's native fee currency as the current gas price
	/// - FeeExchange case: Gas spent will be charged to the user in their nominated fee currency at the current gas price
	fn empty_unused_gas(transactor: &T::AccountId, gas_meter: GasMeter<T>) {
		// TODO: Update `GasSpent` for the block
		let gas_left = gas_meter.gas_left();
		let gas_price = Contracts::<T>::gas_price();
		let gas_spent = gas_meter.spent();

		// The `take()` function ensures the entry is killed after access
		if let Some(exchange_op) = storage::unhashed::take::<FeeExchange<T::AssetId, T::Balance>>(&GAS_FEE_EXCHANGE_KEY)
		{
			// Pay for `gas_spent` in a user nominated currency using the CENNZX spot exchange
			// Payment can never fail as liquidity is verified before filling the meter
			if let Some(used_gas_cost) = gas_price.checked_mul(&gas_spent.saturated_into()) {
				let _ = CennzxSpot::<T>::buy_fee_asset(
					transactor,
					T::Balance::unique_saturated_from(used_gas_cost.saturated_into()),
					&exchange_op,
				);
				let imbalance = T::Currency::withdraw(
					transactor,
					used_gas_cost,
					WithdrawReason::Fee.into(),
					ExistenceRequirement::KeepAlive,
				)
				.expect("Used gas cost could not be withdrawn");
				T::GasPayment::on_unbalanced(imbalance);
			}
		} else {
			// Refund remaining gas by minting it as CENNZnet fee currency
			if let Some(refund) = gas_price.checked_mul(&gas_left.saturated_into()) {
				let _imbalance = T::Currency::deposit_creating(transactor, refund);
			}
		}
	}
}

// It implements `IsGasMeteredCall`
pub struct GasMeteredCallResolver;

impl IsGasMeteredCall for GasMeteredCallResolver {
	/// The runtime extrinsic `Call` type
	type Call = Call;
	/// Return whether the given `call` is gas metered
	fn is_gas_metered(call: &Self::Call) -> bool {
		match call {
			Call::Contracts(pallet_contracts::Call::call(_, _, _, _)) => true,
			Call::Contracts(pallet_contracts::Call::instantiate(_, _, _, _)) => true,
			Call::Contracts(pallet_contracts::Call::put_code(_, _)) => true,
			_ => false,
		}
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
			sylo_payment::Module::<Runtime>::payment_account()
		} else {
			None
		}
	}
}

/// Provides a cennznet version of doughnut dispatch verification
pub struct CENNZnetDispatchVerifier;

/// Helpers which are used by the DelegatedDispatchVerifier
impl CENNZnetDispatchVerifier {
	/// Checks if the doughnut holds a `cennznut` and if so, it returns the decoded `cennznut`
	fn get_cennznut(doughnut: &<Runtime as frame_system::Trait>::Doughnut) -> Result<CENNZnut, &'static str> {
		let mut domain = doughnut
			.get_domain(<CENNZnetDispatchVerifier as additional_traits::DelegatedDispatchVerifier>::DOMAIN)
			.ok_or("CENNZnut does not grant permission for cennznet domain")?;
		let cennznut: CENNZnut = Decode::decode(&mut domain).map_err(|_| "Bad CENNZnut encoding")?;
		Ok(cennznut)
	}

	/// Verify that the smart contract being called is permissioned in the cennznut
	fn check_contract(
		contract_address: &<Runtime as frame_system::Trait>::AccountId,
		cennznut: CENNZnut,
	) -> Result<(), &'static str> {
		let address: [u8; 32] = contract_address.clone().into();
		match cennznut.validate_contract_call(&address) {
			Ok(r) => Ok(r),
			_ => Err("CENNZnut does not grant permission for contract"),
		}
	}
}

impl additional_traits::DelegatedDispatchVerifier for CENNZnetDispatchVerifier {
	type Doughnut = <Runtime as frame_system::Trait>::Doughnut;
	type AccountId = <Runtime as frame_system::Trait>::AccountId;

	const DOMAIN: &'static str = "cennznet";

	/// Verify that a call to a runtime module is permissioned by the doughnut
	fn verify_dispatch(
		doughnut: &Self::Doughnut,
		module: &str,
		method: &str,
		_args: Vec<(&str, &dyn Any)>,
	) -> Result<(), &'static str> {
		let cennznut: CENNZnut = Self::get_cennznut(doughnut)?;

		// Extract Module name from <prefix>-<Module_name>
		let module_offset = module
			.find('-')
			.ok_or("CENNZnut does not grant permission for module")?
			+ 1;
		if module_offset <= 1 || module_offset >= module.chars().count() {
			return Err("error during module name segmentation");
		}
		match cennznut.validate_runtime_call(&module[module_offset..], method, &[]) {
			Ok(r) => Ok(r),
			Err(ValidationErr::ConstraintsInterpretation) => Err("error while interpreting constraints"),
			Err(ValidationErr::NoPermission(RuntimeDomain::Method)) => {
				Err("CENNZnut does not grant permission for method")
			}
			Err(ValidationErr::NoPermission(RuntimeDomain::Module)) => {
				Err("CENNZnut does not grant permission for module")
			}
			Err(ValidationErr::NoPermission(RuntimeDomain::MethodArguments)) => {
				Err("CENNZnut does not grant permission for method arguments")
			}
		}
	}

	/// Verify that the contract being called is permissioned on the doughnut
	fn verify_runtime_to_contract_call(
		caller: &Self::AccountId,
		doughnut: &Self::Doughnut,
		contract_addr: &Self::AccountId,
	) -> Result<(), &'static str> {
		debug_assert!(caller.clone() == doughnut.issuer(), "Invalid doughnut caller");
		let cennznut: CENNZnut = Self::get_cennznut(doughnut)?;
		return Self::check_contract(contract_addr, cennznut);
	}

	/// This is used when an issuer delegates permissions to a smart contract
	/// It should verify that a contract being called by the smart contract is
	/// permissioned. Not implemented yet.
	fn verify_contract_to_contract_call(
		_caller: &Self::AccountId,
		_doughnut: &Self::Doughnut,
		_contract_addr: &Self::AccountId,
	) -> Result<(), &'static str> {
		Ok(()) // Just return OK for now
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
