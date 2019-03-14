// TODO: add legal and license info.

//! Reward module.
//!
//! This module provides reward accumulation feature, includes gathering transaction fees
//! block rewards etc.

#![cfg_attr(not(feature = "std"), no_std)]

use parity_codec::{Encode, Decode};

use rstd::prelude::*;
use support::{
	StorageValue, decl_storage, decl_module, traits::ArithmeticType,
};
use fees::OnFeeCharged;
use session::OnSessionChange;
use staking::CurrentEraReward;

type AmountOf<T> = <<T as staking::Trait>::Currency as ArithmeticType>::Type;

pub trait Trait: staking::Trait {}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		pub fn set_block_reward(#[compact] reward: AmountOf<T>) {
			<BlockReward<T>>::put(reward);
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
