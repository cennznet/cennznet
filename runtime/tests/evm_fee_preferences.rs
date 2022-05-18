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

//! EVM Fee Preferences integration tests

use cennznet_primitives::types::{AccountId, AssetId};
use cennznet_runtime::{
	constants::{asset::*, currency::*, evm::*},
	runner::FeePreferencesRunner,
	Cennzx, GenericAsset, Origin, Runtime, CENNZNET_EVM_CONFIG,
};
use crml_support::{scale_to_4dp, MultiCurrency, PrefixedAddressMapping, H160, H256, U256};
use ethabi::Token;
use frame_support::assert_ok;
use hex_literal::hex;
use pallet_evm::{AddressMapping, EvmConfig, Runner as RunnerT};
use sp_runtime::{traits::Zero, Permill};

mod common;
use common::keyring::{alice, ferdie};
use common::mock::ExtBuilder;

fn encode_fee_preferences_input(asset_id: AssetId, slippage: u32, input: Vec<u8>) -> Vec<u8> {
	// Encode input arguments into an input for callWithFeePreferences
	let asset_token: Token = Token::Uint(asset_id.into());
	let slippage_token: Token = Token::Uint(slippage.into());
	let target: H160 = H160::from_slice(&hex!("cCccccCc00003E80000000000000000000000000"));
	let target_token: Token = Token::Address(ethabi::ethereum_types::H160::from(target.to_fixed_bytes()));
	let input_token: Token = Token::Bytes(input.into());

	let token_stream: Vec<Token> = vec![asset_token, slippage_token, target_token, input_token];
	let mut input_selector: Vec<u8> = FEE_FUNCTION_SELECTOR.to_vec();
	input_selector.append(&mut ethabi::encode(&token_stream));
	input_selector
}

fn encode_transfer_input(target: H160, amount: u128) -> Vec<u8> {
	// Encode input arguments into an input for transfer
	let target_token: Token = Token::Address(ethabi::ethereum_types::H160::from(target.to_fixed_bytes()));
	let asset_token: Token = Token::Uint(amount.into());

	let token_stream: Vec<Token> = vec![target_token, asset_token];
	let mut input_selector: Vec<u8> = vec![169, 5, 156, 187];
	input_selector.append(&mut ethabi::encode(&token_stream));
	input_selector
}

fn setup_liquidity(initial_liquidity: u128) {
	// Alice sets up CENNZ <> CPAY liquidity
	assert_ok!(Cennzx::add_liquidity(
		Origin::signed(alice()),
		CENNZ_ASSET_ID,
		initial_liquidity, // min. liquidity
		initial_liquidity, // liquidity CENNZ
		initial_liquidity, // liquidity CPAY
	));
}

#[test]
fn encode_fee_preferences_input_works() {
	ExtBuilder::default()
		.build()
		.execute_with(|| {
            let asset: AssetId = 16000;
            let slippage: u32 = 50;
            let transfer_input: Vec<u8> = hex!("a9059cbb0000000000000000000000007a107fc1794f505cb351148f529accae12ffbcd8000000000000000000000000000000000000000000000000000000000000007b").to_vec();
            let input = encode_fee_preferences_input(asset, slippage, transfer_input);

            let expected = hex!("ccf39ea90000000000000000000000000000000000000000000000000000000000003e800000000000000000000000000000000000000000000000000000000000000032000000000000000000000000cccccccc00003e8000000000000000000000000000000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000044a9059cbb0000000000000000000000007a107fc1794f505cb351148f529accae12ffbcd8000000000000000000000000000000000000000000000000000000000000007b00000000000000000000000000000000000000000000000000000000");
            assert_eq!(expected.to_vec(), input);
        });
}

#[test]
fn encode_transfer_input_works() {
	ExtBuilder::default()
        .build()
        .execute_with(|| {
            let target: H160 = H160::from_slice(&hex!("7a107Fc1794f505Cb351148F529AcCae12fFbcD8"));
            let amount: u128 = 123;
            let input = encode_transfer_input(target, amount);
            let expected = hex!("a9059cbb0000000000000000000000007a107fc1794f505cb351148f529accae12ffbcd8000000000000000000000000000000000000000000000000000000000000007b");
            assert_eq!(expected.to_vec(), input);
        });
}

#[test]
fn evm_call_with_fee_preferences() {
	let eth_address: H160 = hex!("420aC537F1a4f78d4Dfb3A71e902be0E3d480AFB").into();
	let cennznet_address: AccountId = <Runtime as pallet_evm::Config>::AddressMapping::into_account_id(eth_address);
	let initial_balance = 1000 * DOLLARS;
	let initial_liquidity = 500 * DOLLARS;

	ExtBuilder::default()
		.initial_balance(initial_balance)
		.stash(initial_balance)
		.build()
		.execute_with(|| {
			setup_liquidity(initial_liquidity);

			// The account that will receive CENNZ as a result of the call being successful
			let receiver_eth: H160 = hex!("7a107Fc1794f505Cb351148F529AcCae12fFbcD8").into();
			let receiver: AccountId =
				<Runtime as pallet_evm::Config>::AddressMapping::into_account_id(receiver_eth.clone());
			let receiver_cennz_balance_before =
				<GenericAsset as MultiCurrency>::free_balance(&receiver, CENNZ_ASSET_ID);

			assert_ok!(GenericAsset::transfer(
				Origin::signed(ferdie()),
				CENNZ_ASSET_ID,
				cennznet_address.clone(),
				initial_balance
			));
			let cennz_balance_before = <GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CENNZ_ASSET_ID);
			let cpay_balance_before = <GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CPAY_ASSET_ID);
			assert!(cpay_balance_before.is_zero());
			assert_eq!(cennz_balance_before, initial_balance);

			// Create input parameters for call
			let slippage: u32 = 50;
			let transfer_amount: u128 = 123;
			let transfer_input = encode_transfer_input(receiver_eth, transfer_amount);
			let input = encode_fee_preferences_input(CENNZ_ASSET_ID, slippage, transfer_input);

			let gas_limit: u64 = 100000;
			let max_fee_per_gas = U256::from(20000000000000u64);
			let max_priority_fee_per_gas = U256::from(1000000u64);
			let access_list: Vec<(H160, Vec<H256>)> = vec![];
			assert_ok!(<Runtime as pallet_evm::Config>::Runner::call(
				eth_address,
				H160::from_low_u64_be(FEE_PROXY),
				input,
				U256::from(0u64),
				gas_limit,
				Some(max_fee_per_gas),
				Some(max_priority_fee_per_gas),
				None,
				access_list,
				&CENNZNET_EVM_CONFIG
			));

			// Calculate expected fee for transaction
			let expected_fee = scale_to_4dp(
				<FeePreferencesRunner<Runtime>>::calculate_total_gas(
					gas_limit,
					Some(max_fee_per_gas),
					Some(max_priority_fee_per_gas),
				)
				.unwrap(),
			);

			// Check receiver has received the CENNZ
			assert_eq!(
				receiver_cennz_balance_before + transfer_amount,
				<GenericAsset as MultiCurrency>::free_balance(&receiver, CENNZ_ASSET_ID)
			);
			// CPAY balance should be unchanged, all CPAY swapped should be used to pay gas
			assert_eq!(
				cpay_balance_before,
				<GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CPAY_ASSET_ID)
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
fn evm_call_with_cpay_as_fee_preference_should_fail() {
	let eth_address: H160 = hex!("420aC537F1a4f78d4Dfb3A71e902be0E3d480AFB").into();
	let cennznet_address: AccountId = <Runtime as pallet_evm::Config>::AddressMapping::into_account_id(eth_address);
	let initial_balance = 1000 * DOLLARS;
	let initial_liquidity = 500 * DOLLARS;

	ExtBuilder::default()
		.initial_balance(initial_balance)
		.initialise_eth_accounts(vec![cennznet_address.clone()])
		.stash(initial_balance)
		.build()
		.execute_with(|| {
			setup_liquidity(initial_liquidity);

			let receiver_eth: H160 = hex!("7a107Fc1794f505Cb351148F529AcCae12fFbcD8").into();
			let receiver: AccountId =
				<Runtime as pallet_evm::Config>::AddressMapping::into_account_id(receiver_eth.clone());
			let receiver_cennz_balance_before =
				<GenericAsset as MultiCurrency>::free_balance(&receiver, CENNZ_ASSET_ID);

			let cennz_balance_before = <GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CENNZ_ASSET_ID);
			let cpay_balance_before = <GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CPAY_ASSET_ID);

			// Create input parameters for call
			let transfer_amount: u128 = 123;
			let transfer_input: Vec<u8> = encode_transfer_input(receiver_eth, transfer_amount);
			let input = encode_fee_preferences_input(CPAY_ASSET_ID, 50, transfer_input);
			let access_list: Vec<(H160, Vec<H256>)> = vec![];
			assert!(<Runtime as pallet_evm::Config>::Runner::call(
				eth_address,
				H160::from_low_u64_be(FEE_PROXY),
				input,
				U256::from(0u64),
				100000,
				Some(U256::from(20000000000000u64)),
				Some(U256::from(1000000u64)),
				None,
				access_list,
				&CENNZNET_EVM_CONFIG
			)
			.is_err());

			// All balances should be unchanged
			assert_eq!(
				receiver_cennz_balance_before,
				<GenericAsset as MultiCurrency>::free_balance(&receiver, CENNZ_ASSET_ID)
			);
			assert_eq!(
				cpay_balance_before,
				<GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CPAY_ASSET_ID)
			);
			assert_eq!(
				cennz_balance_before,
				<GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CENNZ_ASSET_ID)
			);
		});
}

#[test]
fn evm_call_with_fee_preferences_and_zero_slippage_should_fail() {
	let eth_address: H160 = hex!("420aC537F1a4f78d4Dfb3A71e902be0E3d480AFB").into();
	let cennznet_address: AccountId = <Runtime as pallet_evm::Config>::AddressMapping::into_account_id(eth_address);
	let initial_balance = 10000 * DOLLARS;
	let initial_liquidity = 500 * DOLLARS;

	ExtBuilder::default()
		.initial_balance(initial_balance)
		.stash(initial_balance)
		.build()
		.execute_with(|| {
			setup_liquidity(initial_liquidity);

			let receiver_eth: H160 = hex!("7a107Fc1794f505Cb351148F529AcCae12fFbcD8").into();
			let receiver: AccountId =
				<Runtime as pallet_evm::Config>::AddressMapping::into_account_id(receiver_eth.clone());
			let receiver_cennz_balance_before =
				<GenericAsset as MultiCurrency>::free_balance(&receiver, CENNZ_ASSET_ID);

			assert_ok!(GenericAsset::transfer(
				Origin::signed(ferdie()),
				CENNZ_ASSET_ID,
				cennznet_address.clone(),
				initial_balance
			));
			let cennz_balance_before = <GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CENNZ_ASSET_ID);
			let cpay_balance_before = <GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CPAY_ASSET_ID);
			assert!(cpay_balance_before.is_zero());
			assert_eq!(cennz_balance_before, initial_balance);

			// Create input parameters for call
			let transfer_amount: u128 = 123;
			let transfer_input: Vec<u8> = encode_transfer_input(receiver_eth, transfer_amount);
			let input = encode_fee_preferences_input(CENNZ_ASSET_ID, 0, transfer_input);
			let access_list: Vec<(H160, Vec<H256>)> = vec![];

			// Call should fail as slippage is 0
			assert!(<Runtime as pallet_evm::Config>::Runner::call(
				eth_address,
				H160::from_low_u64_be(FEE_PROXY),
				input,
				U256::from(0u64),
				100000,
				Some(U256::from(20000000000000u64)),
				Some(U256::from(1000000u64)),
				None,
				access_list,
				&CENNZNET_EVM_CONFIG
			)
			.is_err());

			// All balances should be unchanged
			assert_eq!(
				receiver_cennz_balance_before,
				<GenericAsset as MultiCurrency>::free_balance(&receiver, CENNZ_ASSET_ID)
			);
			assert_eq!(
				cpay_balance_before,
				<GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CPAY_ASSET_ID)
			);
			assert_eq!(
				cennz_balance_before,
				<GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CENNZ_ASSET_ID)
			);
		});
}

#[test]
fn evm_call_with_fee_preferences_and_low_slippage_should_fail() {
	let eth_address: H160 = hex!("420aC537F1a4f78d4Dfb3A71e902be0E3d480AFB").into();
	let cennznet_address: AccountId = <Runtime as pallet_evm::Config>::AddressMapping::into_account_id(eth_address);
	let initial_balance = 10000 * DOLLARS;
	let initial_liquidity = 500 * DOLLARS;

	ExtBuilder::default()
		.initial_balance(initial_balance)
		.stash(initial_balance)
		.build()
		.execute_with(|| {
			setup_liquidity(initial_liquidity);

			let receiver_eth: H160 = hex!("7a107Fc1794f505Cb351148F529AcCae12fFbcD8").into();
			let receiver: AccountId =
				<Runtime as pallet_evm::Config>::AddressMapping::into_account_id(receiver_eth.clone());
			let receiver_cennz_balance_before =
				<GenericAsset as MultiCurrency>::free_balance(&receiver, CENNZ_ASSET_ID);

			assert_ok!(GenericAsset::transfer(
				Origin::signed(ferdie()),
				CENNZ_ASSET_ID,
				cennznet_address.clone(),
				initial_balance
			));
			let cennz_balance_before = <GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CENNZ_ASSET_ID);
			let cpay_balance_before = <GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CPAY_ASSET_ID);
			assert!(cpay_balance_before.is_zero());
			assert_eq!(cennz_balance_before, initial_balance);

			// Create input parameters for call with slippage of 0.1%
			let transfer_amount: u128 = 123;
			let transfer_input: Vec<u8> = encode_transfer_input(receiver_eth, transfer_amount);
			let input = encode_fee_preferences_input(CENNZ_ASSET_ID, 1, transfer_input);
			let access_list: Vec<(H160, Vec<H256>)> = vec![];
			// Call should fail as slippage is 0
			assert!(<Runtime as pallet_evm::Config>::Runner::call(
				eth_address,
				H160::from_low_u64_be(FEE_PROXY),
				input,
				U256::from(0u64),
				100000,
				Some(U256::from(20000000000000u64)),
				Some(U256::from(1000000u64)),
				None,
				access_list,
				&CENNZNET_EVM_CONFIG
			)
			.is_err());

			// All balances should be unchanged
			assert_eq!(
				receiver_cennz_balance_before,
				<GenericAsset as MultiCurrency>::free_balance(&receiver, CENNZ_ASSET_ID)
			);
			assert_eq!(
				cpay_balance_before,
				<GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CPAY_ASSET_ID)
			);
			assert_eq!(
				cennz_balance_before,
				<GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CENNZ_ASSET_ID)
			);
		});
}

#[test]
fn evm_call_with_fee_preferences_no_asset_should_fail() {
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
			setup_liquidity(initial_liquidity);
			let receiver_eth: H160 = hex!("7a107Fc1794f505Cb351148F529AcCae12fFbcD8").into();

			let cpay_balance_before = <GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CPAY_ASSET_ID);
			let cennz_balance_before = <GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CENNZ_ASSET_ID);

			// Create input parameters for call with slippage of 0.1%
			let transfer_amount: u128 = 123;
			let transfer_input: Vec<u8> = encode_transfer_input(receiver_eth, transfer_amount);
			let input = encode_fee_preferences_input(10, 50, transfer_input);
			let access_list: Vec<(H160, Vec<H256>)> = vec![];
			assert!(<Runtime as pallet_evm::Config>::Runner::call(
				eth_address,
				H160::from_low_u64_be(FEE_PROXY),
				input,
				U256::from(0u64),
				100000u64,
				Some(U256::from(20000000000000u64)),
				Some(U256::from(1000000u64)),
				None,
				access_list,
				&CENNZNET_EVM_CONFIG
			)
			.is_err());

			// CPAY and CENNZ balance should be unchanged as the transaction never went through
			assert_eq!(
				cpay_balance_before,
				<GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CPAY_ASSET_ID)
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CENNZ_ASSET_ID),
				cennz_balance_before
			);
		});
}

#[test]
fn evm_call_with_fee_preferences_no_liquidity_should_fail() {
	let eth_address: H160 = hex!("420aC537F1a4f78d4Dfb3A71e902be0E3d480AFB").into();
	let cennznet_address: AccountId = PrefixedAddressMapping::into_account_id(eth_address);
	let initial_balance = 1000 * DOLLARS;

	ExtBuilder::default()
		.initial_balance(initial_balance)
		.stash(initial_balance)
		.build()
		.execute_with(|| {
			let receiver_eth: H160 = hex!("7a107Fc1794f505Cb351148F529AcCae12fFbcD8").into();

			// Create input parameters for call
			let transfer_amount: u128 = 123;
			let transfer_input: Vec<u8> = encode_transfer_input(receiver_eth, transfer_amount);
			let input = encode_fee_preferences_input(10, 50, transfer_input);
			let max_fee_per_gas = U256::from(20000000000000u64);
			let max_priority_fee_per_gas = U256::from(1000000u64);
			let access_list: Vec<(H160, Vec<H256>)> = vec![];
			let config: EvmConfig = CENNZNET_EVM_CONFIG.clone();
			assert!(<Runtime as pallet_evm::Config>::Runner::call(
				eth_address,
				H160::from_low_u64_be(FEE_PROXY),
				input,
				U256::from(0u64),
				100000u64,
				Some(max_fee_per_gas),
				Some(max_priority_fee_per_gas),
				None,
				access_list,
				&config
			)
			.is_err());

			// CPAY and CENNZ balance should be unchanged as the transaction never went through
			assert!(<GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CPAY_ASSET_ID).is_zero());
			assert!(<GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CENNZ_ASSET_ID).is_zero());
		});
}

#[test]
fn evm_call_with_fee_preferences_no_balance_should_fail() {
	let eth_address: H160 = hex!("420aC537F1a4f78d4Dfb3A71e902be0E3d480AFB").into();
	let cennznet_address: AccountId = PrefixedAddressMapping::into_account_id(eth_address);
	let initial_balance = 1000 * DOLLARS;
	let initial_liquidity = 500 * DOLLARS;

	ExtBuilder::default()
		.initial_balance(initial_balance)
		.stash(initial_balance)
		.build()
		.execute_with(|| {
			setup_liquidity(initial_liquidity);
			let receiver_eth: H160 = hex!("7a107Fc1794f505Cb351148F529AcCae12fFbcD8").into();

			// Create input parameters for call
			let transfer_amount: u128 = 123;
			let transfer_input: Vec<u8> = encode_transfer_input(receiver_eth, transfer_amount);
			let input = encode_fee_preferences_input(10, 50, transfer_input);
			let max_fee_per_gas = U256::from(20000000000000u64);
			let max_priority_fee_per_gas = U256::from(1000000u64);
			let access_list: Vec<(H160, Vec<H256>)> = vec![];
			let config: EvmConfig = CENNZNET_EVM_CONFIG.clone();
			assert!(<Runtime as pallet_evm::Config>::Runner::call(
				eth_address,
				H160::from_low_u64_be(FEE_PROXY),
				input,
				U256::from(0u64),
				100000u64,
				Some(max_fee_per_gas),
				Some(max_priority_fee_per_gas),
				None,
				access_list,
				&config
			)
			.is_err());

			// CPAY and CENNZ balance should be unchanged as the transaction never went through
			assert!(<GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CPAY_ASSET_ID).is_zero());
			assert!(<GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CENNZ_ASSET_ID).is_zero());
		});
}
