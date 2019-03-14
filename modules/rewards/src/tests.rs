//! Tests for the module.

#![cfg(test)]

use runtime_io::with_externalities;

use crate::mock::{ExtBuilder, Rewards, ChargeFeeMock};
use support::{assert_err, assert_ok};

#[test]
fn set_block_reward_works() {
	with_externalities(&mut ExtBuilder::default().block_reward(3).build(), || {
		assert_eq!(Rewards::block_reward(), 3);
		Rewards::set_block_reward(5);
		assert_eq!(Rewards::block_reward(), 5);
	})
}

#[test]
fn session_transaction_fee_accumulates_charged_fees_amount() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		assert_eq!(Rewards::session_transaction_fee(), 0);
		ChargeFeeMock::trigger_rewards_on_fee_charged(3);
		assert_eq!(Rewards::session_transaction_fee(), 3);
		ChargeFeeMock::trigger_rewards_on_fee_charged(5);
		assert_eq!(Rewards::session_transaction_fee(), 3 + 5);
	})
}
