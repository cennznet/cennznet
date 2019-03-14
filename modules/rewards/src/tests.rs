//! Tests for the module.

#![cfg(test)]

use runtime_io::with_externalities;

use crate::mock::{ExtBuilder, Rewards};
use support::{assert_err, assert_ok};

#[test]
fn set_block_reward_works() {
	with_externalities(&mut ExtBuilder::default().block_reward(3).build(), || {
		assert_eq!(Rewards::block_reward(), 3);
		Rewards::set_block_reward(5);
		assert_eq!(Rewards::block_reward(), 5);
	})
}
