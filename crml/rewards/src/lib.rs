// TODO: add legal and license info.

//! Reward module.
//!
//! This module provides reward accumulation feature, includes gathering transaction fees
//! block rewards etc.

#![cfg_attr(not(feature = "std"), no_std)]

use fees::OnFeeCharged;
use session::OnSessionChange;
use staking::CurrentEraReward;
use support::{decl_module, decl_storage, traits::Currency, StorageValue};

mod mock;
mod tests;

type AmountOf<T> = <<T as staking::Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;

pub trait Trait: staking::Trait {}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		pub fn set_block_reward(#[compact] reward: AmountOf<T>) {
			<BlockReward<T>>::put(reward);
		}

		fn on_finalise() {
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
		<CurrentEraReward<T>>::mutate(|reward| *reward += session_transaction_fee);
	}
}
