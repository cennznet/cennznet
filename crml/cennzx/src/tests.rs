/* Copyright 2019-2020 Centrality Investments Limited
*
* Licensed under the LGPL, Version 3.0 (the "License");
* you may not use this file except in compliance with the License.
* Unless required by applicable law or agreed to in writing, software
* distributed under the License is distributed on an "AS IS" BASIS,
* WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
* See the License for the specific language governing permissions and
* limitations under the License.
* You may obtain a copy of the License at the root of this project source code,
* or at:
*     https://centrality.ai/licenses/gplv3.txt
*     https://centrality.ai/licenses/lgplv3.txt
*/
//!
//! CENNZX Tests
//!
#![cfg(test)]

use crate::{
	assert_balance_eq, assert_exchange_balance_eq,
	mock::{
		last_event, AccountId, Cennzx, Event, ExtBuilder, Origin, Test, CORE_ASSET_ID, TRADE_ASSET_A_ID,
		TRADE_ASSET_B_ID,
	},
	types::{FeeRate, LowPrecisionUnsigned, PerMillion, PerThousand},
	with_account, with_exchange, Error, ExchangeAddressFor, RawEvent,
};
use core::convert::TryFrom;
use frame_support::{assert_err, assert_ok};
use prml_support::MultiCurrencyAccounting;

#[test]
fn investor_can_add_liquidity() {
	ExtBuilder::default().build().execute_with(|| {
		let investor: AccountId = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A_ID => 100);

		// First investment
		assert_ok!(Cennzx::add_liquidity(
			Origin::signed(investor.clone()),
			TRADE_ASSET_A_ID,
			2,  // min_liquidity: T::Balance,
			15, // max_asset_amount: T::Balance,
			10, // core_amount: T::Balance,
		));
		assert_eq!(
			last_event(),
			Event::cennzx(RawEvent::AddLiquidity(investor.clone(), 10, TRADE_ASSET_A_ID, 15)),
		);

		// Second investment
		// because a round up, second time asset amount become 15 + 1
		assert_ok!(Cennzx::add_liquidity(
			Origin::signed(investor.clone()),
			TRADE_ASSET_A_ID,
			2,  // min_liquidity: T::Balance,
			16, // max_asset_amount: T::Balance,
			10, // core_amount: T::Balance,
		));
		assert_eq!(
			last_event(),
			Event::cennzx(RawEvent::AddLiquidity(investor.clone(), 10, TRADE_ASSET_A_ID, 16)),
		);

		assert_exchange_balance_eq!(CORE_ASSET_ID => 20, TRADE_ASSET_A_ID => 31);
		assert_eq!(
			Cennzx::liquidity_balance((CORE_ASSET_ID, TRADE_ASSET_A_ID), &investor),
			20
		);
	});
}

#[test]
fn add_liquidity_fails_with_insufficient_trade_balance() {
	ExtBuilder::default().build().execute_with(|| {
		let core_balance = 1000;
		let trade_balance = 100;
		let investor: AccountId = with_account!(CORE_ASSET_ID => core_balance, TRADE_ASSET_A_ID => trade_balance);

		let min_liquidity = 1;
		let max_trade_amount = 101;
		let core_amount = 100;

		let origin = Origin::signed(investor.clone());
		let asset_id = TRADE_ASSET_A_ID;

		assert_err!(
			Cennzx::add_liquidity(origin, asset_id, min_liquidity, max_trade_amount, core_amount),
			Error::<Test>::InsufficientTradeAssetBalance
		);
	});
}

#[test]
fn add_liquidity_fails_with_insufficient_core_balance() {
	ExtBuilder::default().build().execute_with(|| {
		let core_balance = 1000;
		let trade_balance = 100;
		let investor: AccountId = with_account!(CORE_ASSET_ID => core_balance, TRADE_ASSET_A_ID => trade_balance);

		let min_liquidity = 1;
		let max_trade_amount = 100;
		let core_amount = 1001;

		let origin = Origin::signed(investor.clone());
		let asset_id = TRADE_ASSET_A_ID;

		assert_err!(
			Cennzx::add_liquidity(origin, asset_id, min_liquidity, max_trade_amount, core_amount),
			Error::<Test>::InsufficientCoreAssetBalance
		);
	});
}

#[test]
fn add_liquidity_fails_with_zero_add() {
	ExtBuilder::default().build().execute_with(|| {
		let core_balance = 1000;
		let trade_balance = 100;
		let investor: AccountId = with_account!(CORE_ASSET_ID => core_balance, TRADE_ASSET_A_ID => trade_balance);

		let min_liquidity = 1;
		let max_trade_amount = 10;
		let core_amount = 100;

		let origin = Origin::signed(investor.clone());
		let asset_id = TRADE_ASSET_A_ID;

		assert_err!(
			Cennzx::add_liquidity(origin.clone(), asset_id.clone(), min_liquidity, 0, core_amount),
			Error::<Test>::CannotAddLiquidityWithZero
		);
		assert_err!(
			Cennzx::add_liquidity(origin, asset_id, min_liquidity, max_trade_amount, 0),
			Error::<Test>::CannotAddLiquidityWithZero
		);
	});
}

#[test]
fn remove_liquidity_fails_with_empty_pool() {
	ExtBuilder::default().build().execute_with(|| {
		let core_balance = 1000;
		let trade_balance = 100;
		let investor: AccountId = with_account!(CORE_ASSET_ID => core_balance, TRADE_ASSET_A_ID => trade_balance);

		let liquidity = 0;
		let min_asset = 0;
		let min_core = 0;

		let origin = Origin::signed(investor.clone());
		let asset_id = TRADE_ASSET_A_ID;

		assert_err!(
			Cennzx::remove_liquidity(origin, asset_id, liquidity, min_asset, min_core),
			Error::<Test>::EmptyExchangePool
		);
	});
}

#[test]
fn calculate_buy_price_zero_cases() {
	ExtBuilder::default().build().execute_with(|| {
		assert_err!(
			Cennzx::calculate_buy_price(100, 0, 10),
			Error::<Test>::EmptyExchangePool
		);

		assert_err!(
			Cennzx::calculate_buy_price(100, 10, 0),
			Error::<Test>::EmptyExchangePool
		);
	});
}

/// Formula Price = ((input reserve * output amount) / (output reserve - output amount)) +  1 (round up)
/// and apply fee rate to the price
#[test]
fn calculate_buy_price_for_valid_data() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Cennzx::calculate_buy_price(123, 1000, 1000), 141);

		assert_ok!(
			Cennzx::calculate_buy_price(100_000_000_000_000, 120_627_710_511_649_660, 20_627_710_511_649_660,),
			589396433540516
		);
	});
}

/// Formula Price = ((input reserve * output amount) / (output reserve - output amount)) +  1 (round up)
/// and apply fee rate to the price
#[test]
fn calculate_buy_price_for_max_reserve_balance() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(
			Cennzx::calculate_buy_price(
				LowPrecisionUnsigned::max_value() / 2,
				LowPrecisionUnsigned::max_value() / 2,
				LowPrecisionUnsigned::max_value(),
			),
			170651607010850639426882365627031758044
		);
	});
}

/// Formula Price = ((input reserve * output amount) / (output reserve - output amount)) +  1 (round up)
/// and apply fee rate to the price
// Overflows as the both input and output reserve is at max capacity and output amount is little less than max of Balance
#[test]
fn calculate_buy_price_should_fail_with_max_reserve_and_max_amount() {
	ExtBuilder::default().build().execute_with(|| {
		assert_err!(
			Cennzx::calculate_buy_price(
				LowPrecisionUnsigned::max_value() - 100,
				LowPrecisionUnsigned::max_value(),
				LowPrecisionUnsigned::max_value(),
			),
			Error::<Test>::Overflow
		);
	});
}

/// Formula Price = ((input reserve * output amount) / (output reserve - output amount)) +  1 (round up)
/// and apply fee rate to the price
#[test]
fn calculate_buy_price_max_withdrawal() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);

		assert_err!(
			Cennzx::calculate_buy_price(1000, 1000, 1000),
			Error::<Test>::InsufficientExchangePoolReserve
		);

		assert_err!(
			Cennzx::calculate_buy_price(1_000_000, 1000, 1000),
			Error::<Test>::InsufficientExchangePoolReserve
		);
	});
}

#[test]
fn asset_buy_price() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);

		assert_ok!(Cennzx::get_asset_to_core_buy_price(TRADE_ASSET_A_ID, 123), 141);

		assert_ok!(Cennzx::get_core_to_asset_buy_price(TRADE_ASSET_A_ID, 123), 141);
	});
}

#[test]
fn asset_buy_zero_error() {
	ExtBuilder::default().build().execute_with(|| {
		assert_err!(
			Cennzx::get_core_to_asset_buy_price(TRADE_ASSET_A_ID, 0),
			Error::<Test>::CannotTradeZero
		);
		assert_err!(
			Cennzx::get_asset_to_core_buy_price(TRADE_ASSET_A_ID, 0),
			Error::<Test>::CannotTradeZero
		);
	});
}

#[test]
fn asset_buy_insufficient_reserve_error() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);

		assert_err!(
			Cennzx::get_asset_to_core_buy_price(
				TRADE_ASSET_A_ID,
				1001, // amount_bought
			),
			Error::<Test>::InsufficientExchangePoolReserve
		);

		assert_err!(
			Cennzx::get_core_to_asset_buy_price(
				TRADE_ASSET_A_ID,
				1001, // amount_bought
			),
			Error::<Test>::InsufficientExchangePoolReserve
		);
	});
}

#[test]
fn asset_to_core_buy_with_none_for_recipient() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 10, TRADE_ASSET_A_ID => 1000);
		let trader: AccountId = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A_ID => 2200);

		// asset to core swap output
		assert_ok!(Cennzx::buy_asset(
			Origin::signed(trader.clone()),
			None,
			TRADE_ASSET_A_ID,
			CORE_ASSET_ID,
			5,    // buy_amount: T::Balance,
			1400, // max_sale: T::Balance,
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 5, TRADE_ASSET_A_ID => 2004);
		assert_balance_eq!(trader, TRADE_ASSET_A_ID => 1196);
	});
}

#[test]
fn asset_to_core_execute_buy() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 10, TRADE_ASSET_A_ID => 1000);
		let trader = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A_ID => 2200);

		assert_ok!(
			Cennzx::execute_buy(
				&trader, // buyer
				&trader, // recipient
				TRADE_ASSET_A_ID,
				CORE_ASSET_ID,
				5,    // buy_amount: T::Balance,
				1400, // max_sale: T::Balance,
			),
			1004
		);

		assert_exchange_balance_eq!(CORE_ASSET_ID => 5, TRADE_ASSET_A_ID => 2004);
		assert_balance_eq!(trader, TRADE_ASSET_A_ID => 1196);

		assert_eq!(
			last_event(),
			Event::cennzx(RawEvent::AssetBought(TRADE_ASSET_A_ID, CORE_ASSET_ID, trader, 1004, 5)),
		);
	});
}

#[test]
fn asset_buy_error_zero_asset_sold() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);
		let trader: AccountId = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A_ID => 100);

		// asset to core swap output
		assert_err!(
			Cennzx::buy_asset(
				Origin::signed(trader.clone()),
				None,
				TRADE_ASSET_A_ID,
				CORE_ASSET_ID,
				0,   // buy_amount
				100, // max_sale,
			),
			Error::<Test>::CannotTradeZero
		);
		// core to asset swap output
		assert_err!(
			Cennzx::buy_asset(
				Origin::signed(trader),
				None,
				CORE_ASSET_ID,
				TRADE_ASSET_A_ID,
				0,   // buy_amount
				100, // max_sale,
			),
			Error::<Test>::CannotTradeZero
		);
	});
}

#[test]
fn asset_buy_error_insufficient_balance() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 500, TRADE_ASSET_A_ID => 500);
		let trader: AccountId = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A_ID => 50);

		// asset to core swap output
		assert_err!(
			Cennzx::buy_asset(
				Origin::signed(trader.clone()),
				None,
				TRADE_ASSET_A_ID,
				CORE_ASSET_ID,
				51,  // buy_amount
				500, // max_sale,
			),
			Error::<Test>::InsufficientBalance
		);
		// core to asset swap output
		assert_err!(
			Cennzx::buy_asset(
				Origin::signed(trader),
				None,
				CORE_ASSET_ID,
				TRADE_ASSET_A_ID,
				101, // buy_amount
				500, // max_sale,
			),
			Error::<Test>::InsufficientBalance
		);
	});
}

#[test]
fn asset_buy_error_exceed_max_sale() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);
		let trader: AccountId = with_account!(CORE_ASSET_ID => 50, TRADE_ASSET_A_ID => 50);

		// asset to core swap output
		assert_err!(
			Cennzx::buy_asset(
				Origin::signed(trader.clone()),
				None,
				TRADE_ASSET_A_ID,
				CORE_ASSET_ID,
				50, // buy_amount
				0,  // max_sale,
			),
			Error::<Test>::MaximumSellRequirementNotMet
		);

		// core to asset swap output
		assert_err!(
			Cennzx::buy_asset(
				Origin::signed(trader),
				None,
				CORE_ASSET_ID,
				TRADE_ASSET_A_ID,
				50, // buy_amount
				0,  // max_sale,
			),
			Error::<Test>::MaximumSellRequirementNotMet
		);
	});
}

#[test]
fn core_to_asset_buy() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 10);
		let trader: AccountId = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A_ID => 2200);

		// core to asset swap output
		assert_ok!(Cennzx::buy_asset(
			Origin::signed(trader.clone()),
			None,
			CORE_ASSET_ID,
			TRADE_ASSET_A_ID,
			5,    // buy_amount: T::Balance,
			1400, // max_sale: T::Balance,
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 2004, TRADE_ASSET_A_ID => 5);
		assert_balance_eq!(trader, CORE_ASSET_ID => 1196);
		assert_balance_eq!(trader, TRADE_ASSET_A_ID => 2205);
	});
}

#[test]
fn core_to_asset_transfer_buy() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 10);
		let buyer = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A_ID => 2200);
		let recipient = with_account!("bob", CORE_ASSET_ID => 0, TRADE_ASSET_A_ID => 0);

		assert_ok!(
			Cennzx::execute_buy(
				&buyer,
				&recipient,
				CORE_ASSET_ID,
				TRADE_ASSET_A_ID,
				5,    // buy_amount: T::Balance,
				1400, // max_sale: T::Balance,
			),
			1004
		);

		assert_exchange_balance_eq!(CORE_ASSET_ID => 2004, TRADE_ASSET_A_ID => 5);
		assert_balance_eq!(buyer, CORE_ASSET_ID => 1196);
		assert_balance_eq!(recipient, TRADE_ASSET_A_ID => 5);
	});
}

#[test]
fn add_liquidity_exact_min_liquidity() {
	ExtBuilder::default().build().execute_with(|| {
		let investor: AccountId = with_account!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);

		assert_ok!(Cennzx::add_liquidity(
			Origin::signed(investor.clone()),
			TRADE_ASSET_A_ID,
			500, // min_liquidity: T::Balance,
			500, // max_asset_amount: T::Balance,
			500, // core_amount: T::Balance,
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 500, TRADE_ASSET_A_ID => 500);
		assert_balance_eq!(investor, TRADE_ASSET_A_ID => 500);
		assert_balance_eq!(investor, CORE_ASSET_ID => 500);
	});
}

#[test]
fn add_liquidity_fails_with_too_high_min_liquidity_on_first_add() {
	ExtBuilder::default().build().execute_with(|| {
		let investor: AccountId = with_account!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);

		assert_err!(
			Cennzx::add_liquidity(
				Origin::signed(investor.clone()),
				TRADE_ASSET_A_ID,
				501, // min_liquidity: T::Balance,
				500, // max_asset_amount: T::Balance,
				500, // core_amount: T::Balance,
			),
			Error::<Test>::MinimumLiquidityRequirementNotMet
		);

		assert_exchange_balance_eq!(CORE_ASSET_ID => 0, TRADE_ASSET_A_ID => 0);
		assert_balance_eq!(investor, TRADE_ASSET_A_ID => 1000);
		assert_balance_eq!(investor, CORE_ASSET_ID => 1000);
	});
}

#[test]
fn add_liquidity_fails_with_too_high_min_liquidity_on_consecutive_add() {
	ExtBuilder::default().build().execute_with(|| {
		let investor: AccountId = with_account!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);

		assert_ok!(Cennzx::add_liquidity(
			Origin::signed(investor.clone()),
			TRADE_ASSET_A_ID,
			100, // min_liquidity: T::Balance,
			100, // max_asset_amount: T::Balance,
			100, // core_amount: T::Balance,
		));

		assert_err!(
			Cennzx::add_liquidity(
				Origin::signed(investor.clone()),
				TRADE_ASSET_A_ID,
				501, // min_liquidity: T::Balance,
				500, // max_asset_amount: T::Balance,
				500, // core_amount: T::Balance,
			),
			Error::<Test>::MinimumLiquidityRequirementNotMet
		);

		assert_exchange_balance_eq!(CORE_ASSET_ID => 100, TRADE_ASSET_A_ID => 100);
		assert_balance_eq!(investor, TRADE_ASSET_A_ID => 900);
		assert_balance_eq!(investor, CORE_ASSET_ID => 900);
	});
}

#[test]
fn add_liquidity_fails_with_too_low_trade_asset() {
	ExtBuilder::default().build().execute_with(|| {
		let investor: AccountId = with_account!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);

		assert_ok!(Cennzx::add_liquidity(
			Origin::signed(investor.clone()),
			TRADE_ASSET_A_ID,
			500, // min_liquidity: T::Balance,
			500, // max_asset_amount: T::Balance,
			500, // core_amount: T::Balance,
		));

		assert_err!(
			Cennzx::add_liquidity(
				Origin::signed(investor.clone()),
				TRADE_ASSET_A_ID,
				100, // min_liquidity: T::Balance,
				99,  // max_asset_amount: T::Balance,
				100, // core_amount: T::Balance,
			),
			Error::<Test>::MaximumTradeAssetRequirementNotMet
		);

		assert_exchange_balance_eq!(CORE_ASSET_ID => 500, TRADE_ASSET_A_ID => 500);
		assert_balance_eq!(investor, TRADE_ASSET_A_ID => 500);
		assert_balance_eq!(investor, CORE_ASSET_ID => 500);
	});
}

#[test]
fn liquidity_price_new_exchange() {
	ExtBuilder::default().build().execute_with(|| {
		let price = Cennzx::liquidity_price(TRADE_ASSET_A_ID, 1_000_000);

		assert_eq!(price.core, 1_000_000);
		assert_eq!(price.asset, 1);
	});
}

#[test]
fn liquidity_price_exisiting_exchange_one_to_one() {
	ExtBuilder::default().build().execute_with(|| {
		let investor: AccountId = with_account!(CORE_ASSET_ID => 10_000, TRADE_ASSET_A_ID => 10_000);

		assert_ok!(Cennzx::add_liquidity(
			Origin::signed(investor.clone()),
			TRADE_ASSET_A_ID,
			1,     // min_liquidity: T::Balance,
			1_000, // max_asset_amount: T::Balance,
			1_000, // core_amount: T::Balance,
		));

		let price = Cennzx::liquidity_price(TRADE_ASSET_A_ID, 1_000_000);

		assert_eq!(price.asset, 1_000_000 + 1);
		assert_eq!(price.core, 1_000_000);
	});
}

#[test]
fn liquidity_price_exisiting_exchange_one_to_three() {
	ExtBuilder::default().build().execute_with(|| {
		let investor: AccountId = with_account!(CORE_ASSET_ID => 10_000, TRADE_ASSET_A_ID => 10_000);

		assert_ok!(Cennzx::add_liquidity(
			Origin::signed(investor.clone()),
			TRADE_ASSET_A_ID,
			1,     // min_liquidity: T::Balance,
			3_000, // max_asset_amount: T::Balance,
			1_000, // core_amount: T::Balance,
		));

		let price = Cennzx::liquidity_price(TRADE_ASSET_A_ID, 1_000_000);

		assert_eq!(price.asset, 3_000_000 + 1);
		assert_eq!(price.core, 1_000_000);
	});
}

#[test]
fn liquidity_price_existing_exchange_accrued() {
	ExtBuilder::default().build().execute_with(|| {
		let investor: AccountId = with_account!(CORE_ASSET_ID => 10_000, TRADE_ASSET_A_ID => 10_000);

		assert_ok!(Cennzx::add_liquidity(
			Origin::signed(investor.clone()),
			TRADE_ASSET_A_ID,
			1,     // min_liquidity: T::Balance,
			1_000, // max_asset_amount: T::Balance,
			1_000, // core_amount: T::Balance,
		));

		// Over time, the exchange grows due to exorbitant fees
		with_exchange!(CORE_ASSET_ID => 999_000, TRADE_ASSET_A_ID => 499_000);
		assert_exchange_balance_eq!(CORE_ASSET_ID => 1_000_000, TRADE_ASSET_A_ID => 500_000);

		let price = Cennzx::liquidity_price(TRADE_ASSET_A_ID, 1_000);

		assert_eq!(price.asset, 500_000 + 1);
		assert_eq!(price.core, 1_000_000);
	});
}

#[test]
fn calculate_liquidity_value_simple() {
	ExtBuilder::default().build().execute_with(|| {
		let value = Cennzx::calculate_liquidity_value(
			1000, // asset_reserve
			1000, // core_reserve
			100,  // liquidity_to_withdraw
			1000, // total_liquidity
		);

		assert_eq!(value.liquidity, 100);
		assert_eq!(value.core, 100);
		assert_eq!(value.asset, 100);
	});
}

#[test]
fn calculate_liquidity_value_zero_total() {
	ExtBuilder::default().build().execute_with(|| {
		let value = Cennzx::calculate_liquidity_value(
			1000, // asset_reserve
			1000, // core_reserve
			100,  // liquidity_to_withdraw
			0,    // total_liquidity
		);

		assert_eq!(value.liquidity, 0);
		assert_eq!(value.core, 0);
		assert_eq!(value.asset, 0);
	});
}

#[test]
fn calculate_liquidity_value_zero_withdraw() {
	ExtBuilder::default().build().execute_with(|| {
		let value = Cennzx::calculate_liquidity_value(
			1000, // asset_reserve
			1000, // core_reserve
			0,    // liquidity_to_withdraw
			1000, // total_liquidity
		);

		assert_eq!(value.liquidity, 0);
		assert_eq!(value.core, 0);
		assert_eq!(value.asset, 0);
	});
}

#[test]
fn calculate_liquidity_value_withdraw_greater_than_total() {
	ExtBuilder::default().build().execute_with(|| {
		let value = Cennzx::calculate_liquidity_value(
			1000, // asset_reserve
			1000, // core_reserve
			2000, // liquidity_to_withdraw
			1000, // total_liquidity
		);

		assert_eq!(value.liquidity, 1000);
		assert_eq!(value.core, 1000);
		assert_eq!(value.asset, 1000);
	});
}

#[test]
fn calculate_liquidity_value_ratio() {
	ExtBuilder::default().build().execute_with(|| {
		let value = Cennzx::calculate_liquidity_value(
			1000, // asset_reserve
			300,  // core_reserve
			50,   // liquidity_to_withdraw
			100,  // total_liquidity
		);

		assert_eq!(value.liquidity, 50);
		assert_eq!(value.core, 150);
		assert_eq!(value.asset, 500);
	});
}

#[test]
fn account_liquidity_value_simple() {
	ExtBuilder::default().build().execute_with(|| {
		let investor: AccountId = with_account!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);

		assert_ok!(Cennzx::add_liquidity(
			Origin::signed(investor.clone()),
			TRADE_ASSET_A_ID,
			1,   // min_liquidity: T::Balance,
			350, // max_asset_amount: T::Balance,
			250, // core_amount: T::Balance,
		));

		let value = Cennzx::account_liquidity_value(&investor, TRADE_ASSET_A_ID);

		assert_eq!(value.liquidity, 250);
		assert_eq!(value.core, 250);
		assert_eq!(value.asset, 350);
	});
}

#[test]
fn account_liquidity_value_accrued() {
	ExtBuilder::default().build().execute_with(|| {
		let investor: AccountId = with_account!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);

		assert_ok!(Cennzx::add_liquidity(
			Origin::signed(investor.clone()),
			TRADE_ASSET_A_ID,
			1,   // min_liquidity: T::Balance,
			350, // max_asset_amount: T::Balance,
			250, // core_amount: T::Balance,
		));

		// Over time, the exchange grows due to exorbitant fees
		with_exchange!(CORE_ASSET_ID => 750, TRADE_ASSET_A_ID => 850);
		assert_exchange_balance_eq!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1200);

		let value = Cennzx::account_liquidity_value(&investor, TRADE_ASSET_A_ID);

		assert_eq!(value.liquidity, 250);
		assert_eq!(value.core, 1000);
		assert_eq!(value.asset, 1200);
	});
}

#[test]
fn account_liquidity_value_multi_investor_accrued() {
	ExtBuilder::default().build().execute_with(|| {
		let investor_1: AccountId = with_account!("andrea", CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);
		let investor_2: AccountId = with_account!("bob", CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);

		// Investor 1 owns 3/4 of the liquidity, investor 2 owns 1/4
		let _ = Cennzx::add_liquidity(
			Origin::signed(investor_1.clone()),
			TRADE_ASSET_A_ID,
			1,   // min_liquidity: T::Balance,
			300, // max_asset_amount: T::Balance,
			150, // core_amount: T::Balance,
		);
		let _ = Cennzx::add_liquidity(
			Origin::signed(investor_2.clone()),
			TRADE_ASSET_A_ID,
			1,   // min_liquidity: T::Balance,
			101, // max_asset_amount: T::Balance,
			50,  // core_amount: T::Balance,
		);

		// Over time, the exchange grows due to exorbitant fees
		with_exchange!(CORE_ASSET_ID => 300, TRADE_ASSET_A_ID => 599);
		assert_exchange_balance_eq!(CORE_ASSET_ID => 500, TRADE_ASSET_A_ID => 1000);

		let value_1 = Cennzx::account_liquidity_value(&investor_1, TRADE_ASSET_A_ID);
		assert_eq!(value_1.liquidity, 150);
		assert_eq!(value_1.core, 375);
		assert_eq!(value_1.asset, 750);

		let value_2 = Cennzx::account_liquidity_value(&investor_2, TRADE_ASSET_A_ID);
		assert_eq!(value_2.liquidity, 50);
		assert_eq!(value_2.core, 125);
		assert_eq!(value_2.asset, 250);
	});
}

#[test]
fn remove_liquidity() {
	ExtBuilder::default().build().execute_with(|| {
		let investor: AccountId = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A_ID => 100);

		// investment
		let _ = Cennzx::add_liquidity(
			Origin::signed(investor.clone()),
			TRADE_ASSET_A_ID,
			2,  // min_liquidity: T::Balance,
			15, // max_asset_amount: T::Balance,
			10, // core_amount: T::Balance,
		);

		assert_ok!(Cennzx::remove_liquidity(
			Origin::signed(investor.clone()),
			TRADE_ASSET_A_ID,
			10, //`asset_amount` - Amount of exchange asset to burn
			4,  //`min_asset_withdraw` - The minimum trade asset withdrawn
			4   //`min_core_withdraw` -  The minimum core asset withdrawn
		));
		assert_exchange_balance_eq!(CORE_ASSET_ID => 0, TRADE_ASSET_A_ID => 0);
		assert_balance_eq!(investor, TRADE_ASSET_A_ID => 100);
		assert_balance_eq!(investor, CORE_ASSET_ID => 100);
	});
}

#[test]
fn remove_liquidity_fails_min_core_asset_limit() {
	ExtBuilder::default().build().execute_with(|| {
		let investor: AccountId = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A_ID => 100);

		// investment
		let _ = Cennzx::add_liquidity(
			Origin::signed(investor.clone()),
			TRADE_ASSET_A_ID,
			2,  // min_liquidity: T::Balance,
			15, // max_asset_amount: T::Balance,
			10, // core_amount: T::Balance,
		);

		assert_err!(
			Cennzx::remove_liquidity(
				Origin::signed(investor.clone()),
				TRADE_ASSET_A_ID,
				10, //`asset_amount` - Amount of exchange asset to burn
				4,  //`min_asset_withdraw` - The minimum trade asset withdrawn
				14  //`min_core_withdraw` -  The minimum core asset withdrawn
			),
			Error::<Test>::MinimumCoreAssetRequirementNotMet
		);
		assert_exchange_balance_eq!(CORE_ASSET_ID => 10, TRADE_ASSET_A_ID => 15);
		assert_balance_eq!(investor, TRADE_ASSET_A_ID => 85);
		assert_balance_eq!(investor, CORE_ASSET_ID => 90);
	});
}

#[test]
fn remove_liquidity_fails_min_trade_asset_limit() {
	ExtBuilder::default().build().execute_with(|| {
		let investor: AccountId = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A_ID => 100);

		// investment
		let _ = Cennzx::add_liquidity(
			Origin::signed(investor.clone()),
			TRADE_ASSET_A_ID,
			2,  // min_liquidity: T::Balance,
			15, // max_asset_amount: T::Balance,
			10, // core_amount: T::Balance,
		);

		assert_err!(
			Cennzx::remove_liquidity(
				Origin::signed(investor.clone()),
				TRADE_ASSET_A_ID,
				10, //`asset_amount` - Amount of exchange asset to burn
				18, //`min_asset_withdraw` - The minimum trade asset withdrawn
				4   //`min_core_withdraw` -  The minimum core asset withdrawn
			),
			Error::<Test>::MinimumTradeAssetRequirementNotMet
		);
		assert_exchange_balance_eq!(CORE_ASSET_ID => 10, TRADE_ASSET_A_ID => 15);
		assert_balance_eq!(investor, TRADE_ASSET_A_ID => 85);
		assert_balance_eq!(investor, CORE_ASSET_ID => 90);
	});
}

#[test]
fn remove_liquidity_fails_on_overdraw_liquidity() {
	ExtBuilder::default().build().execute_with(|| {
		let investor: AccountId = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A_ID => 100);

		// investment
		let _ = Cennzx::add_liquidity(
			Origin::signed(investor.clone()),
			TRADE_ASSET_A_ID,
			2,  // min_liquidity: T::Balance,
			15, // max_asset_amount: T::Balance,
			10, // core_amount: T::Balance,
		);

		assert_err!(
			Cennzx::remove_liquidity(
				Origin::signed(investor.clone()),
				TRADE_ASSET_A_ID,
				20, //`asset_amount` - Amount of exchange asset to burn
				18, //`min_asset_withdraw` - The minimum trade asset withdrawn
				4   //`min_core_withdraw` -  The minimum core asset withdrawn
			),
			Error::<Test>::InsufficientLiquidity
		);
		assert_exchange_balance_eq!(CORE_ASSET_ID => 10, TRADE_ASSET_A_ID => 15);
		assert_balance_eq!(investor, TRADE_ASSET_A_ID => 85);
		assert_balance_eq!(investor, CORE_ASSET_ID => 90);
	});
}

#[test]
fn asset_to_core_buy() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 10, TRADE_ASSET_A_ID => 1000);
		let buyer: AccountId = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A_ID => 2200);
		let recipient = with_account!("bob", CORE_ASSET_ID => 100, TRADE_ASSET_A_ID => 100);

		// asset to core swap output
		assert_ok!(Cennzx::buy_asset(
			Origin::signed(buyer.clone()),
			Some(recipient.clone()),
			TRADE_ASSET_A_ID,
			CORE_ASSET_ID,
			5,    // buy_amount: T::Balance,
			1400, // max_sale: T::Balance,
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 5, TRADE_ASSET_A_ID => 2004);
		assert_balance_eq!(buyer, TRADE_ASSET_A_ID => 1196);
		assert_balance_eq!(recipient, CORE_ASSET_ID => 105);
	});
}

#[test]
fn core_to_asset_transfer_buy_10_to_1000() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 10, TRADE_ASSET_A_ID => 1000);
		let buyer: AccountId = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A_ID => 2200);
		let recipient: AccountId = with_account!("bob", CORE_ASSET_ID => 100, TRADE_ASSET_A_ID => 100);

		// core to asset swap output
		assert_ok!(Cennzx::buy_asset(
			Origin::signed(buyer.clone()),
			Some(recipient.clone()),
			CORE_ASSET_ID,
			TRADE_ASSET_A_ID,
			5,    // buy_amount: T::Balance,
			1400, // max_sale: T::Balance,
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 11, TRADE_ASSET_A_ID => 995);
		assert_balance_eq!(buyer, CORE_ASSET_ID => 2199);
		assert_balance_eq!(recipient, TRADE_ASSET_A_ID => 105);
	});
}

/// Calculate input_amount_without_fee using fee rate and input amount and then calculate price
/// Price = (input_amount_without_fee * output reserve) / (input reserve + input_amount_without_fee)
#[test]
fn calculate_sell_price_for_valid_data() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Cennzx::calculate_sell_price(123, 1000, 1000), 108);

		// No f32/f64 types, so we use large values to test precision
		assert_ok!(
			Cennzx::calculate_sell_price(123_000_000, 1_000_000_000, 1_000_000_000),
			109236233
		);

		assert_ok!(
			Cennzx::calculate_sell_price(100_000_000_000_000, 120_627_710_511_649_660, 4_999_727_416_279_531_363),
			4128948876492407
		);

		assert_ok!(
			Cennzx::calculate_sell_price(
				100_000_000_000_000,
				120_627_710_511_649_660,
				LowPrecisionUnsigned::max_value()
			),
			281017019450612581324176880746747822
		);
	});
}

/// Calculate input_amount_without_fee using fee rate and input amount and then calculate price
/// Price = (input_amount_without_fee * output reserve) / (input reserve + input_amount_without_fee)
// Input amount is half of max(Balance) and output reserve is max(Balance) and input reserve is half of max(Balance)
#[test]
fn calculate_sell_price_for_max_reserve_balance() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(
			Cennzx::calculate_sell_price(
				LowPrecisionUnsigned::max_value() / 2,
				LowPrecisionUnsigned::max_value() / 2,
				LowPrecisionUnsigned::max_value()
			),
			169886353929574869427545984738775941814
		);
	});
}

#[test]
fn asset_swap_sell_price() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);

		assert_ok!(Cennzx::get_asset_to_core_sell_price(TRADE_ASSET_A_ID, 123), 108);

		assert_ok!(Cennzx::get_core_to_asset_sell_price(TRADE_ASSET_A_ID, 123), 108);
	});
}

#[test]
fn asset_sell_error_zero_sell_amount() {
	ExtBuilder::default().build().execute_with(|| {
		assert_err!(
			Cennzx::get_asset_to_core_sell_price(TRADE_ASSET_A_ID, 0),
			Error::<Test>::CannotTradeZero
		);
		assert_err!(
			Cennzx::get_core_to_asset_sell_price(TRADE_ASSET_A_ID, 0),
			Error::<Test>::CannotTradeZero
		);
	});
}

#[test]
fn asset_sell_error_insufficient_balance() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);

		let trader = with_account!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);
		assert_err!(
			Cennzx::execute_sell(
				&trader, // seller
				&trader, // recipient
				TRADE_ASSET_A_ID,
				CORE_ASSET_ID,
				10001, // sell_amount
				100    // min buy limit
			),
			Error::<Test>::InsufficientBalance
		);

		assert_err!(
			Cennzx::execute_sell(
				&trader, // seller
				&trader, // recipient
				CORE_ASSET_ID,
				TRADE_ASSET_A_ID,
				10001, // sell_amount
				100    // min buy limit
			),
			Error::<Test>::InsufficientBalance
		);
	});
}

#[test]
fn asset_to_core_sell() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);
		let trader: AccountId = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A_ID => 2200);

		// asset to core swap input
		assert_ok!(Cennzx::sell_asset(
			Origin::signed(trader.clone()),
			None,
			TRADE_ASSET_A_ID,
			CORE_ASSET_ID,
			100, // sell_amount: T::Balance,
			50,  // min buy limit: T::Balance,
		));
		assert_exchange_balance_eq!(CORE_ASSET_ID => 910, TRADE_ASSET_A_ID => 1100);
		assert_balance_eq!(trader, TRADE_ASSET_A_ID => 2100);
		assert_balance_eq!(trader, CORE_ASSET_ID => 2290);
	});
}

#[test]
fn core_to_asset_sell() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);
		let trader: AccountId = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A_ID => 2200);

		// core to asset swap input
		assert_ok!(Cennzx::sell_asset(
			Origin::signed(trader.clone()),
			None,
			CORE_ASSET_ID,
			TRADE_ASSET_A_ID,
			100, // sell_amount: T::Balance,
			50,  // min buy limit: T::Balance,
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 1100, TRADE_ASSET_A_ID => 910);
		assert_balance_eq!(trader, CORE_ASSET_ID => 2100);
		assert_balance_eq!(trader, TRADE_ASSET_A_ID => 2290);
	});
}

#[test]
fn asset_to_core_execute_sell() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);
		let trader = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A_ID => 2200);

		assert_ok!(
			Cennzx::execute_sell(
				&trader, // buyer
				&trader, // recipient
				TRADE_ASSET_A_ID,
				CORE_ASSET_ID,
				90, // sell_amount: T::Balance,
				50, // min buy: T::Balance,
			),
			81
		);

		assert_exchange_balance_eq!(CORE_ASSET_ID => 919, TRADE_ASSET_A_ID => 1090);
		assert_balance_eq!(trader, TRADE_ASSET_A_ID => 2110);
		assert_balance_eq!(trader, CORE_ASSET_ID => 2281);

		assert_eq!(
			last_event(),
			Event::cennzx(RawEvent::AssetSold(TRADE_ASSET_A_ID, CORE_ASSET_ID, trader, 90, 81)),
		);
	});
}

#[test]
fn core_to_asset_execute_sell() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);
		let trader = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A_ID => 2200);

		assert_ok!(
			Cennzx::execute_sell(
				&trader, // buyer
				&trader, // recipient
				CORE_ASSET_ID,
				TRADE_ASSET_A_ID,
				90, // sell_amount: T::Balance,
				50, // min buy: T::Balance,
			),
			81
		);

		assert_exchange_balance_eq!(CORE_ASSET_ID => 1090, TRADE_ASSET_A_ID => 919);
		assert_balance_eq!(trader, CORE_ASSET_ID => 2110);
		assert_balance_eq!(trader, TRADE_ASSET_A_ID => 2281);
	});
}

#[test]
fn asset_sell_error_zero_asset_sold() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);
		let trader: AccountId = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A_ID => 100);
		// asset to core swap input
		assert_err!(
			Cennzx::sell_asset(
				Origin::signed(trader.clone()),
				None,
				TRADE_ASSET_A_ID,
				CORE_ASSET_ID,
				0,   // sell amount
				100, // min buy,
			),
			Error::<Test>::CannotTradeZero
		);
		// core to asset swap input
		assert_err!(
			Cennzx::sell_asset(
				Origin::signed(trader),
				None,
				CORE_ASSET_ID,
				TRADE_ASSET_A_ID,
				0,   // sell amount
				100, // min buy,
			),
			Error::<Test>::CannotTradeZero
		);
	});
}

#[test]
fn asset_sell_error_less_than_min_sale() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);
		let trader: AccountId = with_account!(CORE_ASSET_ID => 50, TRADE_ASSET_A_ID => 50);

		// asset to core swap input
		assert_err!(
			Cennzx::sell_asset(
				Origin::signed(trader.clone()),
				None,
				TRADE_ASSET_A_ID,
				CORE_ASSET_ID,
				50,  // sell_amount
				100, // min buy,
			),
			Error::<Test>::MinimumBuyRequirementNotMet
		);
		// core to asset swap input
		assert_err!(
			Cennzx::sell_asset(
				Origin::signed(trader),
				None,
				CORE_ASSET_ID,
				TRADE_ASSET_A_ID,
				50,  // sell_amount
				100, // min buy,
			),
			Error::<Test>::MinimumBuyRequirementNotMet
		);
	});
}

#[test]
fn asset_to_core_transfer_sell() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);
		let trader: AccountId = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A_ID => 2200);
		let recipient: AccountId = with_account!("bob", CORE_ASSET_ID => 100, TRADE_ASSET_A_ID => 100);

		// asset to core swap input
		assert_ok!(Cennzx::sell_asset(
			Origin::signed(trader.clone()),
			Some(recipient.clone()),
			TRADE_ASSET_A_ID,
			CORE_ASSET_ID,
			50, // sell_amount: T::Balance,
			40, // min_sale: T::Balance,
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 954, TRADE_ASSET_A_ID => 1050);
		assert_balance_eq!(trader, TRADE_ASSET_A_ID => 2150);
		assert_balance_eq!(recipient, CORE_ASSET_ID => 146);
	});
}

#[test]
fn core_to_asset_transfer_sell() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);
		let trader: AccountId = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A_ID => 2200);
		let recipient: AccountId = with_account!("bob", CORE_ASSET_ID => 100, TRADE_ASSET_A_ID => 100);

		// core to asset swap input
		assert_ok!(Cennzx::sell_asset(
			Origin::signed(trader.clone()),
			Some(recipient.clone()),
			CORE_ASSET_ID,
			TRADE_ASSET_A_ID,
			50, // sell_amount: T::Balance,
			40, // min_sale: T::Balance,
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 1050, TRADE_ASSET_A_ID => 954);
		assert_balance_eq!(trader, CORE_ASSET_ID => 2150);
		assert_balance_eq!(recipient, TRADE_ASSET_A_ID => 146);
	});
}

#[test]
fn asset_to_asset_buy() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B_ID => 1000);
		let trader: AccountId = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A_ID => 2200);

		assert_ok!(Cennzx::buy_asset(
			Origin::signed(trader.clone()),
			None,             // Account to receive asset_bought, defaults to origin if None
			TRADE_ASSET_A_ID, // asset_sold
			TRADE_ASSET_B_ID, // asset_bought
			150,              // buy_amount: T::Balance,
			300,              // maximum asset A to sell
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 824, TRADE_ASSET_A_ID => 1216);
		assert_exchange_balance_eq!(CORE_ASSET_ID => 1176, TRADE_ASSET_B_ID => 850);
		assert_balance_eq!(trader, TRADE_ASSET_A_ID => 1984);
		assert_balance_eq!(trader, TRADE_ASSET_B_ID => 150);
		assert_balance_eq!(trader, CORE_ASSET_ID => 2200);
	});
}

#[test]
fn asset_to_asset_buy_error_zero_asset_sold() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B_ID => 1000);
		let trader = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A_ID => 100);

		assert_err!(
			Cennzx::buy_asset(
				Origin::signed(trader),
				None,             // Account to receive asset_bought, defaults to origin if None
				TRADE_ASSET_A_ID, // asset_sold
				TRADE_ASSET_B_ID, // asset_bought
				0,                // buy_amount
				300,              // maximum asset A to sell
			),
			Error::<Test>::CannotTradeZero
		);
	});
}

#[test]
fn asset_to_asset_buy_error_insufficient_balance() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 100, TRADE_ASSET_A_ID => 100);
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B_ID => 1000);
		let trader = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A_ID => 50);

		assert_err!(
			Cennzx::buy_asset(
				Origin::signed(trader),
				None,             // Account to receive asset_bought, defaults to origin if None
				TRADE_ASSET_A_ID, // asset_sold
				TRADE_ASSET_B_ID, // asset_bought
				51,               // buy_amount
				400,              // maximum asset A to sell
			),
			Error::<Test>::InsufficientBalance
		);
	});
}

#[test]
fn asset_to_asset_buy_error_exceed_max_sale() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B_ID => 1000);
		let trader = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A_ID => 100);

		assert_err!(
			Cennzx::buy_asset(
				Origin::signed(trader),
				None,             // Account to receive asset_bought, defaults to origin if None
				TRADE_ASSET_A_ID, // asset_sold
				TRADE_ASSET_B_ID, // asset_bought
				156,              // buy_amount
				100,              // maximum asset A to sell
			),
			Error::<Test>::MaximumSellRequirementNotMet
		);
	});
}

#[test]
fn asset_to_asset_transfer_buy() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B_ID => 1000);
		let trader: AccountId = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A_ID => 2200);
		let recipient: AccountId = with_account!("bob", CORE_ASSET_ID => 100, TRADE_ASSET_B_ID => 100);

		assert_ok!(Cennzx::buy_asset(
			Origin::signed(trader.clone()),
			Some(recipient.clone()), // Account to receive asset_bought, defaults to origin if None
			TRADE_ASSET_A_ID,        // asset_sold
			TRADE_ASSET_B_ID,        // asset_bought
			150,                     // buy_amount: T::Balance,
			300,                     // maximum asset A to sell
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 824, TRADE_ASSET_A_ID => 1216);
		assert_exchange_balance_eq!(CORE_ASSET_ID => 1176, TRADE_ASSET_B_ID => 850);
		assert_balance_eq!(trader, TRADE_ASSET_A_ID => 1984);
		assert_balance_eq!(recipient, TRADE_ASSET_B_ID => 250);
		assert_balance_eq!(trader, CORE_ASSET_ID => 2200);
	});
}

#[test]
fn asset_to_asset_sell() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B_ID => 1000);
		let trader: AccountId = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A_ID => 2200);

		assert_ok!(Cennzx::sell_asset(
			Origin::signed(trader.clone()),
			None,             // Trader is also recipient so passing None in this case
			TRADE_ASSET_A_ID, // asset_sold
			TRADE_ASSET_B_ID, // asset_bought
			150,              // sell_amount
			100,              // min buy limit for asset B
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 871, TRADE_ASSET_A_ID => 1150);
		assert_exchange_balance_eq!(CORE_ASSET_ID => 1129, TRADE_ASSET_B_ID => 887);
		assert_balance_eq!(trader, TRADE_ASSET_A_ID => 2050);
		assert_balance_eq!(trader, TRADE_ASSET_B_ID => 113);
		assert_balance_eq!(trader, CORE_ASSET_ID => 2200);
	});
}

#[test]
fn asset_to_asset_swap_sell_error_zero_asset_sold() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B_ID => 1000);
		let trader = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A_ID => 100);

		assert_err!(
			Cennzx::sell_asset(
				Origin::signed(trader),
				None,             // Trader is also recipient so passing None in this case
				TRADE_ASSET_A_ID, // asset_sold
				TRADE_ASSET_B_ID, // asset_bought
				0,                // sell_amount
				100,              // min buy limit for asset B
			),
			Error::<Test>::CannotTradeZero
		);
	});
}

#[test]
fn asset_to_asset_sell_error_insufficient_balance() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 100, TRADE_ASSET_A_ID => 100);
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B_ID => 1000);
		let trader = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A_ID => 50);

		assert_err!(
			Cennzx::sell_asset(
				Origin::signed(trader),
				None,             // Account to receive asset_bought, defaults to origin if None
				TRADE_ASSET_A_ID, // asset_sold
				TRADE_ASSET_B_ID, // asset_bought
				51,               // sell_amount
				100,              // min buy limit for asset B
			),
			Error::<Test>::InsufficientBalance
		);
	});
}

#[test]
fn asset_to_asset_sell_error_less_than_min_sale() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B_ID => 1000);
		let trader = with_account!(CORE_ASSET_ID => 100, TRADE_ASSET_A_ID => 200);

		assert_err!(
			Cennzx::sell_asset(
				Origin::signed(trader),
				None,             // Account to receive asset_bought, defaults to origin if None
				TRADE_ASSET_A_ID, // asset_sold
				TRADE_ASSET_B_ID, // asset_bought
				156,              // sell_amount
				200,              // min buy limit for asset B
			),
			Error::<Test>::MinimumBuyRequirementNotMet
		);
	});
}

#[test]
fn asset_to_asset_transfer_sell() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B_ID => 1000);
		let trader: AccountId = with_account!(CORE_ASSET_ID => 2200, TRADE_ASSET_A_ID => 2200);
		let recipient: AccountId = with_account!("bob", CORE_ASSET_ID => 100, TRADE_ASSET_B_ID => 100);

		assert_ok!(Cennzx::sell_asset(
			Origin::signed(trader.clone()),
			Some(recipient.clone()), // Account to receive asset_bought, defaults to origin if None
			TRADE_ASSET_A_ID,        // asset_sold
			TRADE_ASSET_B_ID,        // asset_bought
			150,                     // sell_amount
			100,                     // min buy limit for asset B
		));

		assert_exchange_balance_eq!(CORE_ASSET_ID => 871, TRADE_ASSET_A_ID => 1150);
		assert_exchange_balance_eq!(CORE_ASSET_ID => 1129, TRADE_ASSET_B_ID => 887);
		assert_balance_eq!(trader, TRADE_ASSET_A_ID => 2050);
		assert_balance_eq!(recipient, TRADE_ASSET_B_ID => 213);
		assert_balance_eq!(trader, CORE_ASSET_ID => 2200);
	});
}

#[test]
fn set_fee_rate() {
	ExtBuilder::default().build().execute_with(|| {
		let new_fee_rate = FeeRate::<PerMillion>::try_from(FeeRate::<PerThousand>::from(5u128)).unwrap();
		assert_ok!(Cennzx::set_fee_rate(Origin::root(), new_fee_rate), ());
		assert_eq!(Cennzx::fee_rate(), new_fee_rate);
	});
}

#[test]
fn get_buy_price_simple() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B_ID => 1000);
		let _ = Cennzx::set_fee_rate(Origin::root(), 0.into());

		assert_eq!(Cennzx::get_buy_price(TRADE_ASSET_B_ID, 100, TRADE_ASSET_A_ID,), Ok(127));
	});
}

#[test]
fn get_buy_price_with_fee_rate() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B_ID => 1000);
		let _ = Cennzx::set_fee_rate(Origin::root(), 100_000.into());

		assert_eq!(Cennzx::get_buy_price(TRADE_ASSET_B_ID, 100, TRADE_ASSET_A_ID,), Ok(155));
	});
}

#[test]
fn get_buy_price_when_buying_core() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);
		let _ = Cennzx::set_fee_rate(Origin::root(), 0.into());

		assert_eq!(Cennzx::get_buy_price(CORE_ASSET_ID, 100, TRADE_ASSET_A_ID,), Ok(112));
	});
}

#[test]
fn get_buy_price_when_selling_core() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);
		let _ = Cennzx::set_fee_rate(Origin::root(), 0.into());

		assert_eq!(Cennzx::get_buy_price(TRADE_ASSET_A_ID, 100, CORE_ASSET_ID,), Ok(112));
	});
}

#[test]
fn get_buy_price_same_asset_id_ignored() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);

		assert_err!(
			Cennzx::get_buy_price(TRADE_ASSET_A_ID, 100, TRADE_ASSET_A_ID,),
			Error::<Test>::AssetCannotSwapForItself
		);
	});
}

#[test]
fn get_buy_price_low_buy_asset_liquidity_error() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 10, TRADE_ASSET_A_ID => 10);
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B_ID => 1000);

		assert_err!(
			Cennzx::get_buy_price(TRADE_ASSET_A_ID, 100, TRADE_ASSET_B_ID,),
			Error::<Test>::InsufficientExchangePoolReserve
		);
	});
}

#[test]
fn get_buy_price_low_buy_core_liquidity_error() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);
		with_exchange!(CORE_ASSET_ID => 10, TRADE_ASSET_B_ID => 10);

		assert_err!(
			Cennzx::get_buy_price(TRADE_ASSET_A_ID, 100, TRADE_ASSET_B_ID,),
			Error::<Test>::InsufficientExchangePoolReserve
		);
	});
}

#[test]
fn get_buy_price_no_exchange() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);

		assert_err!(
			Cennzx::get_buy_price(TRADE_ASSET_A_ID, 100, TRADE_ASSET_B_ID,),
			Error::<Test>::EmptyExchangePool
		);
	});
}

#[test]
fn get_sell_price_simple() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B_ID => 1000);
		let _ = Cennzx::set_fee_rate(Origin::root(), 0.into());

		assert_eq!(Cennzx::get_sell_price(TRADE_ASSET_B_ID, 100, TRADE_ASSET_A_ID,), Ok(82));
	});
}

#[test]
fn get_sell_price_with_fee_rate() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_B_ID => 1000);
		let _ = Cennzx::set_fee_rate(Origin::root(), 100_000.into());

		assert_eq!(Cennzx::get_sell_price(TRADE_ASSET_B_ID, 100, TRADE_ASSET_A_ID,), Ok(68));
	});
}

#[test]
fn get_sell_price_when_selling_core() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);
		let _ = Cennzx::set_fee_rate(Origin::root(), 0.into());

		assert_eq!(Cennzx::get_sell_price(CORE_ASSET_ID, 100, TRADE_ASSET_A_ID,), Ok(90));
	});
}

#[test]
fn get_sell_price_when_buying_core() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);
		let _ = Cennzx::set_fee_rate(Origin::root(), 0.into());

		assert_eq!(Cennzx::get_sell_price(TRADE_ASSET_A_ID, 100, CORE_ASSET_ID,), Ok(90));
	});
}

#[test]
fn get_sell_price_same_asset_id_ignored() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);

		assert_err!(
			Cennzx::get_sell_price(TRADE_ASSET_A_ID, 100, TRADE_ASSET_A_ID,),
			Error::<Test>::AssetCannotSwapForItself
		);
	});
}

#[test]
fn get_sell_price_low_sell_asset_liquidity() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);
		with_exchange!(CORE_ASSET_ID => 10, TRADE_ASSET_B_ID => 10);

		assert_eq!(
			Cennzx::get_sell_price(TRADE_ASSET_A_ID, 100, TRADE_ASSET_B_ID,),
			Ok(8) // unlike buying, we can sell as long as exchange exists
		);
	});
}

#[test]
fn get_sell_price_low_sell_core_liquidity() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);
		with_exchange!(CORE_ASSET_ID => 10, TRADE_ASSET_B_ID => 10);

		assert_eq!(
			Cennzx::get_sell_price(TRADE_ASSET_A_ID, 100, TRADE_ASSET_B_ID,),
			Ok(8) // unlike buying, we can sell as long as exchange exists
		);
	});
}

#[test]
fn get_sell_price_no_exchange() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);

		assert_err!(
			Cennzx::get_sell_price(TRADE_ASSET_A_ID, 100, TRADE_ASSET_B_ID,),
			Error::<Test>::EmptyExchangePool
		);
	});
}

#[test]
fn execute_trade_rollback_on_intermediate_failure_core_to_asset() {
	// scenario:
	// Given: Trader funds are transferred to the exchange
	// When: Exchange fails to transfer funds back
	// Then: Trader fund transfer is rolled back
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 10, TRADE_ASSET_A_ID => 10);
		let trader = with_account!(CORE_ASSET_ID => 1000, TRADE_ASSET_A_ID => 1000);

		// Execute trade will transfer 2 times to complete:
		// 1) trader funds goto exchange A
		// 2) exchange A funds goto trader B << forcing failure here (min buy: 11 > liquidity: 10)
		assert!(Cennzx::execute_trade(
			&trader, // seller
			&trader, // recipient
			TRADE_ASSET_A_ID,
			CORE_ASSET_ID,
			1000, // sell_amount
			11    // min buy limit
		)
		.is_err());

		// trader funds untouched
		assert_balance_eq!(trader, CORE_ASSET_ID => 1000);
		assert_balance_eq!(trader, TRADE_ASSET_A_ID => 1000);
		// exchange funds untouched
		assert_exchange_balance_eq!(CORE_ASSET_ID => 10, TRADE_ASSET_A_ID => 10);
	});
}

#[test]
fn execute_trade_rollback_on_intermediate_failure_asset_to_asset() {
	// scenario:
	// Given: Trader funds are transferred to the exchange
	// When: Exchange fails to transfer funds back
	// Then: Trader fund transfer is rolled back
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 5, TRADE_ASSET_A_ID => 5);
		with_exchange!(CORE_ASSET_ID => 5, TRADE_ASSET_B_ID => 5);
		let trader = with_account!(TRADE_ASSET_A_ID => 5, TRADE_ASSET_B_ID => u128::max_value());

		// Execute trade will transfer 3 times to complete:
		// 1) trader funds goto exchange A
		// 2) exchange A funds goto exchange B
		// 3) exchange B funds goto trader  << fail here
		// forcing failure as trader balance will overflow
		assert!(Cennzx::execute_trade(
			&trader, // seller
			&trader, // recipient
			TRADE_ASSET_A_ID,
			TRADE_ASSET_B_ID,
			5, // sell_amount
			1  // min buy limit
		)
		.is_err());

		// trader funds unchanged
		assert_balance_eq!(trader, TRADE_ASSET_A_ID => 5);
		assert_balance_eq!(trader, TRADE_ASSET_B_ID => u128::max_value());
		// exchange funds unchanged
		assert_exchange_balance_eq!(CORE_ASSET_ID => 5, TRADE_ASSET_A_ID => 5);
		assert_exchange_balance_eq!(CORE_ASSET_ID => 5, TRADE_ASSET_B_ID => 5);
	});
}

#[test]
fn execute_trade_calculate_sale_price_fails() {
	ExtBuilder::default().build().execute_with(|| {
		with_exchange!(CORE_ASSET_ID => 0, TRADE_ASSET_A_ID => 0);
		let trader = with_account!(CORE_ASSET_ID => 5, TRADE_ASSET_A_ID => 5);

		// empty exchange pool will cause a div by 0 and fail price calculation
		assert_err!(
			Cennzx::execute_trade(
				&trader, // seller
				&trader, // recipient
				TRADE_ASSET_A_ID,
				TRADE_ASSET_B_ID,
				100, // sell_amount
				10   // min buy limit
			),
			Error::<Test>::EmptyExchangePool
		);

		// trader funds unchanged
		assert_balance_eq!(trader, CORE_ASSET_ID => 5);
		assert_balance_eq!(trader, TRADE_ASSET_A_ID => 5);
		// exchange funds unchanged
		assert_exchange_balance_eq!(CORE_ASSET_ID => 0, TRADE_ASSET_A_ID => 0);
	});
}
