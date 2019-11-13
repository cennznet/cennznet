// Copyright (C) 2019 Centrality Investments Limited
// This file is part of CENNZnet.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.
// TODO: add legal and license info.

//! Reward module.
//!
//! This module provides reward accumulation feature, includes gathering transaction fees
//! block rewards etc.

#![cfg_attr(not(feature = "std"), no_std)]

// FIXME:
// use fees::OnFeeCharged;\
use runtime_primitives::{
	traits::{As, CheckedAdd, CheckedMul, One, OpaqueKeys},
	Permill,
};
use session::SessionHandler;
use staking::CurrentEraReward;
use support::{decl_module, decl_storage, dispatch::Result, traits::Currency, StorageValue};

mod mock;
mod tests;

type AmountOf<T> = <<T as staking::Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;

pub trait Trait: staking::Trait {}

decl_storage! {
	trait Store for Module<T: Trait> as Rewards {
		/// Accumulated transaction fees in the current session.
		SessionTransactionFee get(session_transaction_fee): AmountOf<T>;
		/// A fixed amount of currency minted and issued every block.
		BlockReward get(block_reward) config(): AmountOf<T>;
		/// A multiplier applied on transaction fees to calculate total validator rewards.
		FeeRewardMultiplier get(fee_reward_multiplier) config(): Permill;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		/// Calculate and then set `BlockReward` and `FeeRewardMultiplier`.
		///
		/// `s` is storage / CPU ratio; k is empty_block / CCC ratio; `qmax` is target transaction count in a block;
		/// `cost` is the estimated average spending tokens cost per transaction.
		pub fn set_parameters(origin, #[compact] s: AmountOf<T>, #[compact] k: AmountOf<T>, #[compact] qmax: AmountOf<T>, #[compact] cost: AmountOf<T>) -> Result {
			let s_plus_one = s + One::one();

			// block_reward = (s_plus_one + k) * qmax / (s_plus_one * qmax + k)
			let block_reward_divident = s_plus_one
				.checked_add(&k)
				.and_then(|x| x.checked_mul(&qmax))
				.ok_or_else(|| "block reward calculation overflow")?;
			// Given s/k/qmax are all integers, if (s_plus_one + k) * qmax doesn't overflow,
			// (s_plus_one * qmax + k) cannot overflow, as the former one is always larger.
			let reward_divisor = s_plus_one * qmax + k;
			let block_reward = (block_reward_divident / reward_divisor)
				.checked_mul(&cost)
				.ok_or_else(|| "block reward calculation overflow")?;

			// fee_reward_multiplier = s_plus_one * qmax * 1_000_000 / (s_plus_one * qmax + k)
			let fee_reward_multiplier_divident = s_plus_one
				.checked_mul(&qmax)
				.and_then(|x| x.checked_mul(&<AmountOf<T>>::sa(1_000_000)))
				.ok_or_else(|| "fee reward multiplier calculation overflow")?;
			let fee_reward_multiplier_mill = fee_reward_multiplier_divident / reward_divisor;

			<BlockReward<T>>::put(block_reward);
			<FeeRewardMultiplier<T>>::put(
				// `fee_reward_multiplier_bill` cannot overflow u32, since (s_plus_one * qmax)/(s_plus_one * qmax + k)
				// always smaller than 1.
				Permill::from_parts(fee_reward_multiplier_mill.as_() as u32),
			);

			Ok(())
		}

		// Block finalization
		fn on_finalize() {
			// FIXME: Mint and issue block reward.
			<CurrentEraReward<T>>::mutate(|reward| *reward += Self::block_reward());
		}
	}
}

// FIXME:
// impl<T: Trait> OnFeeCharged<AmountOf<T>> for Module<T> {
// 	fn on_fee_charged(fee: &AmountOf<T>) {
// 		<SessionTransactionFee<T>>::mutate(|current| *current += *fee);
// 	}
// }

// impl<T: Trait, U> SessionHandler<U> for Module<T> {
// 	fn on_new_session(: U, _: bool) {
// 		let session_transaction_fee = <SessionTransactionFee<T>>::take();
// 		let multiplier = <FeeRewardMultiplier<T>>::get();
// 		<CurrentEraReward<T>>::mutate(|reward| *reward += multiplier * session_transaction_fee);
// 	}
// }

impl<T: Trait> SessionHandler<T::AccountId> for Module<T> {
	fn on_new_session<Ks: OpaqueKeys>(
		changed: bool,
		validators: &[(ValidatorId, Ks)],
		_queued_validators: &[(ValidatorId, Ks)])
	{
		if changed {
			let session_transaction_fee = <SessionTransactionFee<T>>::take();
			let multiplier = <FeeRewardMultiplier<T>>::get();
			// FIXME: `CurrentEraReward` was removed in the current staking module
			// There is no item using for `accumulated reward for the current era` 
			<CurrentEraReward<T>>::mutate(|reward| *reward += multiplier * session_transaction_fee);
		}
	}
}