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

//! Extrinsic extension integration tests (fee exchange)

use cennznet_primitives::types::{FeeExchange, FeeExchangeV1};
use cennznet_runtime::{
	constants::{asset::*, currency::*},
	Call, Cennzx, CheckedExtrinsic, Executive, GenericAsset, Origin,
};
use prml_generic_asset::MultiCurrencyAccounting as MultiCurrency;

mod common;
use common::helpers::{extrinsic_fee_for, header, sign};
use common::keyring::{alice, bob, signed_extra};
use common::mock::ExtBuilder;

#[test]
fn generic_asset_transfer_works_without_fee_exchange() {
	let initial_balance = 5_000_000_000 * DOLLARS;
	let transfer_amount = 7_777 * MICROS;
	let runtime_call = Call::GenericAsset(prml_generic_asset::Call::transfer(
		CENTRAPAY_ASSET_ID,
		bob(),
		transfer_amount,
	));

	ExtBuilder::default()
		.initial_balance(initial_balance)
		.build()
		.execute_with(|| {
			let xt = sign(CheckedExtrinsic {
				signed: Some((alice(), signed_extra(0, 0, None))),
				function: runtime_call,
			});

			Executive::initialize_block(&header());
			let r = Executive::apply_extrinsic(xt.clone());
			assert!(r.is_ok());

			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&alice(), Some(CENTRAPAY_ASSET_ID)),
				initial_balance - transfer_amount - extrinsic_fee_for(&xt)
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&bob(), Some(CENTRAPAY_ASSET_ID)),
				initial_balance + transfer_amount
			);
		});
}

#[test]
fn generic_asset_transfer_works_with_fee_exchange() {
	let initial_balance = 100 * DOLLARS;
	let initial_liquidity = 5_000_000_000 * DOLLARS;
	let transfer_amount = 25 * MICROS;

	let runtime_call = Call::GenericAsset(prml_generic_asset::Call::transfer(
		CENTRAPAY_ASSET_ID,
		bob(),
		transfer_amount,
	));

	ExtBuilder::default()
		.initial_balance(initial_balance)
		.build()
		.execute_with(|| {
			// Alice sets up CENNZ <> CPAY liquidity
			let r = Cennzx::add_liquidity(
				Origin::signed(alice()),
				CENNZ_ASSET_ID,
				0,                 // min liquidity
				initial_liquidity, // liquidity CENNZ
				initial_liquidity, // liquidity CPAY
			);

			println!("{:?}", r);

			// Exchange CENNZ (sell) for CPAY (buy) to pay for transaction fee
			let fee_exchange = FeeExchange::V1(FeeExchangeV1 {
				asset_id: CENNZ_ASSET_ID,
				max_payment: 5 * DOLLARS,
			});
			// Create an extrinsic where the transaction fee is to be paid in CENNZ
			let xt = sign(CheckedExtrinsic {
				signed: Some((alice(), signed_extra(0, 0, Some(fee_exchange)))),
				function: runtime_call,
			});

			// Calculate how much CENNZ should be sold to make the above extrinsic
			let cennz_sold_amount =
				Cennzx::get_asset_to_core_buy_price(CENNZ_ASSET_ID, extrinsic_fee_for(&xt)).unwrap();
			assert_eq!(cennz_sold_amount, 11_807 * MICROS); // 1.1807 CPAY

			// Initialise block and apply the extrinsic
			Executive::initialize_block(&header());
			let r = Executive::apply_extrinsic(xt);
			println!("{:?}", r);
			assert!(r.is_ok());

			// Check remaining balances
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&alice(), Some(CENNZ_ASSET_ID)),
				initial_balance - initial_liquidity - cennz_sold_amount, // transfer fee is charged in CENNZ
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&alice(), Some(CENTRAPAY_ASSET_ID)),
				initial_balance - initial_liquidity - transfer_amount // transfer fee is not charged in CPAY
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&bob(), Some(CENTRAPAY_ASSET_ID)),
				initial_balance + transfer_amount
			);
		});
}
