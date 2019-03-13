// TODO: add legal and license info.

//! Reward module.
//!
//! This module provides reward accumulation feature, includes gathering transaction fees
//! block rewards etc.

#![cfg_attr(not(feature = "std"), no_std)]

use parity_codec::{Encode, Decode};

use rstd::prelude::*;
use support::{
	StorageValue, decl_storage, decl_module,
	traits::{ArithmeticType,Currency},
};

type AmountOf<T> = <<T as Trait>::Currency as ArithmeticType>::Type;

pub trait Trait: system::Trait {
	// The currency for rewards
	type Currency: ArithmeticType + Currency<Self::AccountId, Balance=AmountOf<Self>>;
}

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
