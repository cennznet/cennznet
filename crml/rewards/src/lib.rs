// TODO: add legal and license info.

//! Reward module.
//!
//! This module provides reward accumulation feature, includes gathering transaction fees
//! block rewards etc.

#![cfg_attr(not(feature = "std"), no_std)]

use fees::OnFeeCharged;
use runtime_primitives::{
	traits::{As, CheckedAdd, CheckedMul, One},
	Perbill,
};
use session::OnSessionChange;
use staking::CurrentEraReward;
use support::{decl_module, decl_storage, dispatch::Result, traits::Currency, StorageValue};

mod mock;
mod tests;

type AmountOf<T> = <<T as staking::Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;

pub trait Trait: staking::Trait {}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		/// Calculate and then set `BlockReward` and `FeeRewardMultiplier`.
		pub fn set_parameters(#[compact] s: AmountOf<T>, #[compact] k: AmountOf<T>, #[compact] qmax: AmountOf<T>) -> Result {
			let s_plus_one = s + One::one();

			// block_reward = (s_plus_one + k) * qmax / (s_plus_one * qmax + k)
			let block_reward_divident = s_plus_one
				.checked_add(&k)
				.and_then(|x| x.checked_mul(&qmax))
				.ok_or_else(|| "block reward calculation overflow")?;
			// Given s/k/qmax are all integers, if (s_plus_one + k) * qmax doesn't overflow,
			// (s_plus_one * qmax + k) cannot overflow, as the former one is always bigger.
			let reward_divisor = s_plus_one * qmax + k;
			let block_reward = block_reward_divident / reward_divisor;

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
				Perbill::from_millionths(fee_reward_multiplier_bill.as_() as u32),
			);

			Ok(())
		}

		fn on_finalize() {
			// Mint and issue block reward.
			<CurrentEraReward<T>>::mutate(|reward| *reward += Self::block_reward());
		}
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as Rewards {
		/// Accumulated transaction fees in the current session.
		SessionTransactionFee get(session_transaction_fee): AmountOf<T>;
		/// A fixed amount of currency minted and issued every block.
		BlockReward get(block_reward) config(): AmountOf<T>;
		/// A multiplier applied on transaction fees to calculate total validator rewards.
		FeeRewardMultiplier get(fee_reward_multiplier) config(): Perbill;
	}
}

impl<T: Trait> OnFeeCharged<AmountOf<T>> for Module<T> {
	fn on_fee_charged(fee: &AmountOf<T>) {
		<SessionTransactionFee<T>>::mutate(|current| *current += *fee);
	}
}

impl<T: Trait, U> OnSessionChange<U> for Module<T> {
	fn on_session_change(_: U, _: bool) {
		let session_transaction_fee = <SessionTransactionFee<T>>::take();
		let multiplier = <FeeRewardMultiplier<T>>::get();
		<CurrentEraReward<T>>::mutate(|reward| *reward += multiplier * session_transaction_fee);
	}
}
