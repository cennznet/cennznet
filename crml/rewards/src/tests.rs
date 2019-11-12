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
//! Tests for the module.

#![cfg(test)]

use super::*;
use runtime_io::with_externalities;

use crate::mock::{ChargeFeeMock, ExtBuilder, Rewards, SessionChangeMock, Staking, Test};
use runtime_primitives::traits::OnFinalize;
use support::{assert_noop, assert_ok, StorageValue};

#[test]
fn set_reward_parameters_works() {
	with_externalities(
		&mut ExtBuilder::default()
			.block_reward(1000)
			.fee_reward_multiplier(Permill::from_percent(100))
			.build(),
		|| {
			assert_eq!(Rewards::block_reward(), 1000);
			assert_eq!(Rewards::fee_reward_multiplier(), Permill::from_percent(100));

			// typical ranges: s in 2~4, k in 80~150, m in 150~135.
			let (s, k, m, cost) = (4, 139, 347, 7);
			assert_ok!(Rewards::set_parameters(s, k, m, cost));

			let s_plus_one = s + 1;
			assert_eq!(Rewards::block_reward(), (s_plus_one + k) * m / (s_plus_one * m + k) * 7);
			assert_eq!(
				Rewards::fee_reward_multiplier(),
				Permill::from_parts((s_plus_one * m * 1_000_000 / (s_plus_one * m + k)) as u32,)
			);

			// large range (unlikely to happen, but it should work)
			assert_ok!(Rewards::set_parameters(10_000, 10_000, 10_000, 100u128.pow(18)));
		},
	);
}

#[test]
fn set_reward_parameters_should_fail_if_overflow() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		let block_reward_overflow = "block reward calculation overflow";
		// (s_plus_one + k) overflows
		assert_noop!(
			Rewards::set_parameters(1, u128::max_value(), 2, 1),
			block_reward_overflow
		);
		// (s_plus_one + k) doesn't overflow, but (s_plus_one + k) * m does.
		assert_noop!(
			Rewards::set_parameters(1, 1, u128::max_value(), 1),
			block_reward_overflow
		);
		// Overflow after multiplies `average_cost_per_tx`
		assert_noop!(
			Rewards::set_parameters(1, 11, 13, u128::max_value()),
			block_reward_overflow
		);

		// (s_plus_one * qmax * 1_000_000) overflows
		assert_noop!(
			Rewards::set_parameters(2, 1, u128::max_value() / 10_000, 1),
			"fee reward multiplier calculation overflow"
		);
	});
}

#[test]
fn mint_block_reward_on_finalize_works() {
	with_externalities(&mut ExtBuilder::default().block_reward(3).build(), || {
		assert_eq!(Staking::current_era_reward(), 0);
		Rewards::on_finalize(0);
		assert_eq!(Staking::current_era_reward(), 3);
		Rewards::on_finalize(1);
		assert_eq!(Staking::current_era_reward(), 6);
	});
}

#[test]
fn on_fee_charged_works() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		assert_eq!(Rewards::session_transaction_fee(), 0);

		ChargeFeeMock::trigger_rewards_on_fee_charged(3);
		assert_eq!(Rewards::session_transaction_fee(), 3);

		ChargeFeeMock::trigger_rewards_on_fee_charged(5);
		assert_eq!(Rewards::session_transaction_fee(), 3 + 5);
	});
}

#[test]
fn on_session_change_works() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		assert_eq!(Staking::current_era_reward(), 0);
		<SessionTransactionFee<Test>>::put(3);
		assert_eq!(Rewards::session_transaction_fee(), 3);

		SessionChangeMock::trigger_rewards_on_session_change();
		assert_eq!(Staking::current_era_reward(), 3);
		assert_eq!(Rewards::session_transaction_fee(), 0);
	});
}
