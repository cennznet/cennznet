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
	traits::{Currency, Get, Imbalance, OnUnbalanced},
	weights::Weight,
};
use prml_generic_asset::StakingAssetCurrency;
use sp_runtime::traits::Convert;

// type Cennzx<T> = crml_cennzx::Module<T>;
type GenericAsset<T> = prml_generic_asset::Module<T>;

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
