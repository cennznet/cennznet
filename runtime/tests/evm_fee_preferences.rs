/* Copyright 2019-2022 Centrality Investments Limited
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

use cennznet_primitives::types::{AccountId, AssetId, Balance};
use cennznet_runtime::{
	constants::{asset::*, currency::*, evm::*},
	Cennzx, GenericAsset, Origin, Runtime,
};
use crml_support::{MultiCurrency, PrefixedAddressMapping, H160};
use ethabi::Token;
use frame_support::{assert_ok, assert_storage_noop};
use hex_literal::hex;
use pallet_evm::AddressMapping;
use pallet_evm_precompiles_erc20::Erc20IdConversion;
use sp_runtime::traits::Zero;
mod common;
use common::keyring::{alice, ferdie};
use common::mock::ExtBuilder;
use common::precompiles_builder::RunnerCallBuilder;

/// Type alias for the runtime FeePreferencesRunner
pub type FeePreferencesRunner = cennznet_runtime::runner::FeePreferencesRunner<Runtime, Runtime>;

fn encode_fee_preferences_input(asset_id: AssetId, max_payment: Balance, input: Vec<u8>) -> Vec<u8> {
	// Encode input arguments into an input for callWithFeePreferences
	let asset_token = Token::Address(Runtime::runtime_id_to_evm_id(asset_id).0);
	let max_payment_token = Token::Uint(max_payment.into());
	let target = H160::from_slice(&hex!("cCccccCc00003E80000000000000000000000000"));
	let target_token = Token::Address(target);
	let input_token = Token::Bytes(input.into());

	let token_stream: Vec<Token> = vec![asset_token, max_payment_token, target_token, input_token];
	let mut input_selector: Vec<u8> = FEE_FUNCTION_SELECTOR.to_vec();
	input_selector.append(&mut ethabi::encode(&token_stream));
	input_selector
}

fn encode_transfer_input(target: H160, amount: u128) -> Vec<u8> {
	// Encode input arguments into an input for transfer
	let target_token = Token::Address(target);
	let asset_token = Token::Uint(amount.into());

	let token_stream = vec![target_token, asset_token];
	let mut input_selector = vec![169_u8, 5, 156, 187];
	input_selector.append(&mut ethabi::encode(&token_stream));
	input_selector
}

/// Setup CENNZ/CPAY liquidity pool using alice address
fn setup_liquidity(initial_liquidity_lhs: u128, initial_liquidity_rhs: u128) {
	assert_ok!(Cennzx::add_liquidity(
		Origin::signed(alice()),
		CENNZ_ASSET_ID,
		initial_liquidity_rhs, // min. liquidity
		initial_liquidity_lhs, // liquidity CENNZ
		initial_liquidity_rhs, // liquidity CPAY
	));
}

#[test]
fn encode_fee_preferences_input_works() {
	ExtBuilder::default()
		.build()
		.execute_with(|| {
            let asset: AssetId = 16_000;
            let max_payment: Balance = 123_456_789;
            let transfer_input = hex!("a9059cbb0000000000000000000000007a107fc1794f505cb351148f529accae12ffbcd8000000000000000000000000000000000000000000000000000000000000007b").to_vec();
            let input = encode_fee_preferences_input(asset, max_payment, transfer_input);

            let expected = hex!("255a3432000000000000000000000000cccccccc00003e8000000000000000000000000000000000000000000000000000000000000000000000000000000000075bcd15000000000000000000000000cccccccc00003e8000000000000000000000000000000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000044a9059cbb0000000000000000000000007a107fc1794f505cb351148f529accae12ffbcd8000000000000000000000000000000000000000000000000000000000000007b00000000000000000000000000000000000000000000000000000000");
            assert_eq!(expected.to_vec(), input);
        });
}

#[test]
fn encode_transfer_input_works() {
	ExtBuilder::default()
        .build()
        .execute_with(|| {
            let target = H160::from_slice(&hex!("7a107Fc1794f505Cb351148F529AcCae12fFbcD8"));
            let amount = 123_u128;
            let input = encode_transfer_input(target, amount);
            let expected = hex!("a9059cbb0000000000000000000000007a107fc1794f505cb351148f529accae12ffbcd8000000000000000000000000000000000000000000000000000000000000007b");
            assert_eq!(expected.to_vec(), input);
        });
}

#[test]
fn evm_call_with_fee_preferences() {
	let eth_address: H160 = hex!("420aC537F1a4f78d4Dfb3A71e902be0E3d480AFB").into();
	let cennznet_address: AccountId = <Runtime as pallet_evm::Config>::AddressMapping::into_account_id(eth_address);
	let initial_balance = 1000 * 10_u128.pow(18_u32);
	let cpay_liquidity = 500 * DOLLARS;
	let cennz_liquidity = 500 * DOLLARS;

	ExtBuilder::default()
		.initial_balance(initial_balance)
		.stash(initial_balance)
		.build()
		.execute_with(|| {
			// set CENNZ exchange rate at 100000000000000.0000 : 1.0000
			// this is equivalent to 1:1 with 1 CPAY token and 1 18dp token
			setup_liquidity(cennz_liquidity, cpay_liquidity);

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
			let max_payment: Balance = 10 * 10_u128.pow(18); // at a 1:1 rate this is ~max 10 CPAY
			let transfer_amount = 123_u128;
			let transfer_input = encode_transfer_input(receiver_eth, transfer_amount);
			let input = encode_fee_preferences_input(CENNZ_ASSET_ID, max_payment, transfer_input);

			assert_ok!(RunnerCallBuilder::new(eth_address, input, H160::from_low_u64_be(FEE_PROXY)).run());

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

			// Check CENNZ balance has changed within `max_payment` amount, this should have been used to pay fees
			let cennz_balance_after = <GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CENNZ_ASSET_ID);
			assert!(
				cennz_balance_after > cennz_balance_before - max_payment && cennz_balance_after < cennz_balance_before
			);
		});
}

#[test]
fn evm_call_with_cpay_as_fee_preference_should_fail() {
	let eth_address: H160 = hex!("420aC537F1a4f78d4Dfb3A71e902be0E3d480AFB").into();
	let cennznet_address: AccountId = <Runtime as pallet_evm::Config>::AddressMapping::into_account_id(eth_address);
	let initial_balance = 1000 * DOLLARS;

	ExtBuilder::default()
		.initial_balance(initial_balance)
		.initialise_eth_accounts(vec![cennznet_address.clone()])
		.stash(initial_balance)
		.build()
		.execute_with(|| {
			// Create input parameters for call
			let input = encode_fee_preferences_input(CPAY_ASSET_ID, 50, vec![]);

			// TODO: the proper error types should be asserted post subsrtate polkadot-v0.9.23 update
			assert_storage_noop!(assert!(RunnerCallBuilder::new(
				eth_address,
				input,
				H160::from_low_u64_be(FEE_PROXY)
			)
			.run()
			.is_err()));
		});
}

#[test]
fn evm_call_with_fee_preferences_low_max_payment_fails() {
	let eth_address: H160 = hex!("420aC537F1a4f78d4Dfb3A71e902be0E3d480AFB").into();
	let initial_balance = 10000 * DOLLARS;
	let initial_liquidity = 500 * DOLLARS;

	ExtBuilder::default()
		.initial_balance(initial_balance)
		.stash(initial_balance)
		.build()
		.execute_with(|| {
			setup_liquidity(initial_liquidity, initial_liquidity);

			// max_payment is 0
			let input = encode_fee_preferences_input(CENNZ_ASSET_ID, 0, vec![]);
			assert_storage_noop!(assert!(RunnerCallBuilder::new(
				eth_address,
				input,
				H160::from_low_u64_be(FEE_PROXY)
			)
			.run()
			.is_err()));

			// max payemnt is 1 CENNZ
			let input = encode_fee_preferences_input(CENNZ_ASSET_ID, 10_000, vec![]);
			assert_storage_noop!(assert!(RunnerCallBuilder::new(
				eth_address,
				input,
				H160::from_low_u64_be(FEE_PROXY)
			)
			.run()
			.is_err()));
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
			setup_liquidity(initial_liquidity, initial_liquidity);

			// Create input parameters
			let input = encode_fee_preferences_input(10, 50, vec![]);

			// Test
			assert_storage_noop!(assert!(RunnerCallBuilder::new(
				eth_address,
				input,
				H160::from_low_u64_be(FEE_PROXY)
			)
			.run()
			.is_err()));
		});
}

#[test]
fn evm_call_with_fee_preferences_no_liquidity_should_fail() {
	let eth_address: H160 = hex!("420aC537F1a4f78d4Dfb3A71e902be0E3d480AFB").into();
	let initial_balance = 1000 * DOLLARS;

	ExtBuilder::default()
		.initial_balance(initial_balance)
		.stash(initial_balance)
		.build()
		.execute_with(|| {
			// Create input parameters for call
			let input = encode_fee_preferences_input(10, 50, vec![]);

			// Test
			assert_storage_noop!(assert!(RunnerCallBuilder::new(
				eth_address,
				input,
				H160::from_low_u64_be(FEE_PROXY)
			)
			.run()
			.is_err()));
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
			setup_liquidity(initial_liquidity, initial_liquidity);
			// Create input parameters for call
			let input = encode_fee_preferences_input(10, 50, vec![]);

			// Test
			assert_storage_noop!(assert!(RunnerCallBuilder::new(
				eth_address,
				input,
				H160::from_low_u64_be(FEE_PROXY)
			)
			.run()
			.is_err()));
		});
}
