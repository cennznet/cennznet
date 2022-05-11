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

use cennznet_primitives::types::{AccountId, FeeExchange, FeeExchangeV1};
use cennznet_runtime::{
	constants::{asset::*, currency::*},
	impls::{scale_to_4dp, FeePreferencesError, FeePreferencesRunner},
	Call, Cennzx, CheckedExtrinsic, Executive, GenericAsset, Origin, Runtime,
};
use crml_support::{MultiCurrency, PrefixedAddressMapping, H160, H256, U256};
use frame_support::{assert_noop, assert_ok};
use hex_literal::hex;
mod common;
use common::helpers::{extrinsic_fee_for, header, sign};
use common::keyring::{alice, bob, signed_extra};
use common::mock::ExtBuilder;
use pallet_evm::{AddressMapping, EvmConfig, Runner as RunnerT};
use pallet_evm_precompiles_fee_payment::FEE_PROXY;
use rlp::RlpStream;
use sp_runtime::Permill;

#[test]
fn generic_asset_transfer_works_without_fee_exchange() {
	let initial_balance = 10 * DOLLARS;
	let transfer_amount = 7_777 * MICROS;
	let runtime_call = Call::GenericAsset(crml_generic_asset::Call::transfer {
		asset_id: CPAY_ASSET_ID,
		to: bob(),
		amount: transfer_amount,
	});

	ExtBuilder::default()
		.initial_balance(initial_balance)
		.stash(initial_balance)
		.build()
		.execute_with(|| {
			let xt = sign(CheckedExtrinsic {
				signed: fp_self_contained::CheckedSignature::Signed(alice(), signed_extra(0, 0, None)),
				function: runtime_call,
			});

			Executive::initialize_block(&header());
			let r = Executive::apply_extrinsic(xt.clone());
			println!("{:?}", r);
			assert!(r.is_ok());

			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&alice(), CPAY_ASSET_ID),
				initial_balance - transfer_amount - extrinsic_fee_for(&xt)
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&bob(), CPAY_ASSET_ID),
				initial_balance + transfer_amount
			);
		});
}

#[test]
fn generic_asset_transfer_works_with_fee_exchange() {
	let initial_balance = 100 * DOLLARS;
	let initial_liquidity = 50 * DOLLARS;
	let transfer_amount = 25 * MICROS;

	let runtime_call = Call::GenericAsset(crml_generic_asset::Call::transfer {
		asset_id: CPAY_ASSET_ID,
		to: bob(),
		amount: transfer_amount,
	});

	ExtBuilder::default()
		.initial_balance(initial_balance)
		.stash(initial_balance)
		.build()
		.execute_with(|| {
			// Alice sets up CENNZ <> CPAY liquidity
			assert_ok!(Cennzx::add_liquidity(
				Origin::signed(alice()),
				CENNZ_ASSET_ID,
				initial_liquidity, // min. liquidity
				initial_liquidity, // liquidity CENNZ
				initial_liquidity, // liquidity CPAY
			));

			// Exchange CENNZ (sell) for CPAY (buy) to pay for transaction fee
			let fee_exchange = FeeExchange::V1(FeeExchangeV1 {
				asset_id: CENNZ_ASSET_ID,
				max_payment: 10 * DOLLARS,
			});
			// Create an extrinsic where the transaction fee is to be paid in CENNZ
			let xt = sign(CheckedExtrinsic {
				signed: fp_self_contained::CheckedSignature::Signed(alice(), signed_extra(0, 0, Some(fee_exchange))),
				function: runtime_call,
			});

			// Calculate how much CENNZ should be sold to make the above extrinsic
			let cennz_sold_amount =
				Cennzx::get_asset_to_core_buy_price(CENNZ_ASSET_ID, extrinsic_fee_for(&xt)).unwrap();

			// Initialise block and apply the extrinsic
			Executive::initialize_block(&header());
			let r = Executive::apply_extrinsic(xt);
			println!("{:?}", r);
			assert!(r.is_ok());

			// Check remaining balances
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&alice(), CENNZ_ASSET_ID),
				initial_balance - initial_liquidity - cennz_sold_amount, // transfer fee is charged in CENNZ
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&alice(), CPAY_ASSET_ID),
				initial_balance - initial_liquidity - transfer_amount // transfer fee is not charged in CPAY
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&bob(), CPAY_ASSET_ID),
				initial_balance + transfer_amount
			);
		});
}

#[test]
fn evm_call_with_fee_preferences() {
	let eth_address: H160 = hex!("420aC537F1a4f78d4Dfb3A71e902be0E3d480AFB").into();
	let cennznet_address: AccountId = PrefixedAddressMapping::into_account_id(eth_address);
	let initial_balance = 1000 * DOLLARS;
	let initial_liquidity = 500 * DOLLARS;

	ExtBuilder::default()
		.initial_balance(initial_balance)
		.initialise_eth_accounts(vec![cennznet_address.clone()])
		.stash(initial_balance)
		.build()
		.execute_with(|| {
			// Alice sets up CENNZ <> CPAY liquidity
			assert_ok!(Cennzx::add_liquidity(
				Origin::signed(alice()),
				CENNZ_ASSET_ID,
				initial_liquidity, // min. liquidity
				initial_liquidity, // liquidity CENNZ
				initial_liquidity, // liquidity CPAY
			));

			let cpay_balance_before = <GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CPAY_ASSET_ID);
			let cennz_balance_before = <GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CENNZ_ASSET_ID);

			// Create input
			let prefix = hex!("15946350").to_vec();
			let slippage: u32 = 50; // Per thousand (5%)
			let new_target = H160::from_low_u64_be(100);
			let new_input: Vec<u8> = vec![0];
			let mut rlp_stream: RlpStream = RlpStream::new_list(5);
			rlp_stream
				.append(&prefix)
				.append(&CENNZ_ASSET_ID)
				.append(&slippage)
				.append(&new_target)
				.append(&new_input);
			let input = rlp_stream.out().to_vec();

			let gas_limit: u64 = 100000;
			let max_fee_per_gas = U256::from(20000000000000u64);
			let max_priority_fee_per_gas = U256::from(1000000u64);
			let access_list: Vec<(H160, Vec<H256>)> = vec![];
			let config: EvmConfig = EvmConfig::frontier();
			assert_ok!(<FeePreferencesRunner<Runtime> as RunnerT<Runtime>>::call(
				eth_address,
				H160::from_low_u64_be(FEE_PROXY),
				input,
				U256::from(0u64),
				gas_limit,
				Some(max_fee_per_gas),
				Some(max_priority_fee_per_gas),
				None,
				access_list,
				&config
			));

			// CPAY balance should be unchanged, all CPAY swapped should be used to pay gas
			assert_eq!(
				cpay_balance_before,
				<GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CPAY_ASSET_ID)
			);

			// Calculate expected fee for transaction
			let expected_fee = scale_to_4dp(
				<FeePreferencesRunner<Runtime>>::calculate_total_gas(
					gas_limit,
					Some(max_fee_per_gas),
					Some(max_priority_fee_per_gas),
				)
				.unwrap(),
			);

			// Check CENNZ balance has changed within slippage amount, this should have been used to pay fees
			let max_payment = expected_fee.saturating_add(Permill::from_rational(slippage, 1000) * expected_fee);
			let min_payment = expected_fee.saturating_sub(Permill::from_rational(slippage, 1000) * expected_fee);

			let cennz_balance_after = <GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CENNZ_ASSET_ID);
			assert_eq!(cennz_balance_after >= cennz_balance_before - max_payment, true);
			assert_eq!(cennz_balance_after <= cennz_balance_before - min_payment, true);
		});
}

#[test]
fn evm_call_with_fee_preferences_no_balance_should_fail() {
	let eth_address: H160 = hex!("420aC537F1a4f78d4Dfb3A71e902be0E3d480AFB").into();
	let initial_balance = 1000 * DOLLARS;
	let initial_liquidity = 500 * DOLLARS;

	ExtBuilder::default()
		.initial_balance(initial_balance)
		.stash(initial_balance)
		.build()
		.execute_with(|| {
			// Alice sets up CENNZ <> CPAY liquidity
			assert_ok!(Cennzx::add_liquidity(
				Origin::signed(alice()),
				CENNZ_ASSET_ID,
				initial_liquidity, // min. liquidity
				initial_liquidity, // liquidity CENNZ
				initial_liquidity, // liquidity CPAY
			));

			// Create input parameters for call
			let abi = hex!("ccf39ea9000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000320000000000000000000000001122334455667788991122334455667788990000000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000051234567890000000000000000000000000000000000000000000000000000000");
			let input = abi.to_vec();
			let gas_limit: u64 = 100000;
			let max_fee_per_gas = U256::from(20000000000000u64);
			let max_priority_fee_per_gas = U256::from(1000000u64);
			let access_list: Vec<(H160, Vec<H256>)> = vec![];
			let config: EvmConfig = EvmConfig::frontier();
			assert_eq!(
				<FeePreferencesRunner<Runtime> as RunnerT<Runtime>>::call(
					eth_address,
					H160::from_low_u64_be(FEE_PROXY),
					input,
					U256::from(0u64),
					gas_limit,
					Some(max_fee_per_gas),
					Some(max_priority_fee_per_gas),
					None,
					access_list,
					&config
				)
				.is_err(),
				true
			);
		});
}

#[test]
fn decode_input() {
	ExtBuilder::default().build().execute_with(|| {
		// Abi generated from below parameters using the following function name:
		// callWithFeePreferences
		// abi can be easily generated here https://abi.hashex.org/
		let abi = hex!("ccf39ea9000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000320000000000000000000000001122334455667788991122334455667788990000000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000051234567890000000000000000000000000000000000000000000000000000000");
		let exp_payment_asset: u32 = 0;
		let exp_slippage: u32 = 50;
		let exp_target = H160::from_slice(&hex!("1122334455667788991122334455667788990000"));
		let exp_input: Vec<u8> = hex!("1234567890").to_vec();
		let (payment_asset, slippage, new_target, new_input) = <FeePreferencesRunner<Runtime>>::decode_input(abi.to_vec()).unwrap();

		// Ensure the values decode correctly
		assert_eq!(payment_asset, exp_payment_asset);
		assert_eq!(slippage, exp_slippage);
		assert_eq!(new_target, exp_target);
		assert_eq!(new_input, exp_input);
	});
}

#[test]
fn decode_input_invalid_function_selector_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let abi = hex!("11111111000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000320000000000000000000000001122334455667788991122334455667788990000000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000051234567890000000000000000000000000000000000000000000000000000000");
		assert_noop!(<FeePreferencesRunner<Runtime>>::decode_input(abi.to_vec()), FeePreferencesError::InvalidFunctionSelector);
	});
}

#[test]
fn decode_input_invalid_input_args_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let abi = hex!("ccf39ea9000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000320000000000000000000000001122334455667788991122334455667788990000");
		assert_noop!(<FeePreferencesRunner<Runtime>>::decode_input(abi.to_vec()), FeePreferencesError::FailedToDecodeInput);
	});
}

#[test]
fn calculate_total_gas() {
	ExtBuilder::default().build().execute_with(|| {
		let gas_limit: u64 = 100000;
		let max_fee_per_gas = U256::from(20000000000000u64);
		let max_priority_fee_per_gas = U256::from(1000000u64);

		assert_ok!(<FeePreferencesRunner<Runtime>>::calculate_total_gas(
			gas_limit,
			Some(max_fee_per_gas),
			Some(max_priority_fee_per_gas),
		));
	});
}

#[test]
fn calculate_total_gas_low_max_fee_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let gas_limit: u64 = 100000;
		let max_fee_per_gas = U256::from(200000u64);
		let max_priority_fee_per_gas = U256::from(1000000u64);

		assert_noop!(
			<FeePreferencesRunner<Runtime>>::calculate_total_gas(
				gas_limit,
				Some(max_fee_per_gas),
				Some(max_priority_fee_per_gas),
			),
			FeePreferencesError::GasPriceTooLow
		);
	});
}

#[test]
fn calculate_total_gas_no_max_fee_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let gas_limit: u64 = 100000;
		let max_fee_per_gas = None;
		let max_priority_fee_per_gas = U256::from(1000000u64);

		assert_noop!(
			<FeePreferencesRunner<Runtime>>::calculate_total_gas(
				gas_limit,
				max_fee_per_gas,
				Some(max_priority_fee_per_gas),
			),
			FeePreferencesError::GasPriceTooLow
		);
	});
}

#[test]
fn calculate_total_gas_max_fee_too_large_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let gas_limit: u64 = 100000;
		let max_fee_per_gas = U256::MAX;
		let max_priority_fee_per_gas = U256::from(1000000u64);

		assert_noop!(
			<FeePreferencesRunner<Runtime>>::calculate_total_gas(
				gas_limit,
				Some(max_fee_per_gas),
				Some(max_priority_fee_per_gas),
			),
			FeePreferencesError::FeeOverflow
		);
	});
}

#[test]
fn calculate_total_gas_max_priority_fee_too_large_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let gas_limit: u64 = 100000;
		let max_fee_per_gas = U256::from(20000000000000u64);
		let max_priority_fee_per_gas = U256::MAX;

		assert_noop!(
			<FeePreferencesRunner<Runtime>>::calculate_total_gas(
				gas_limit,
				Some(max_fee_per_gas),
				Some(max_priority_fee_per_gas),
			),
			FeePreferencesError::FeeOverflow
		);
	});
}
