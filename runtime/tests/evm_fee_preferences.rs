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

use cennznet_primitives::types::AccountId;
use cennznet_runtime::{
	constants::{asset::*, currency::*, evm::*},
	impls::scale_to_4dp,
	runner::FeePreferencesRunner,
	Cennzx, GenericAsset, Origin, Runtime, CENNZNET_EVM_CONFIG,
};
use crml_support::{MultiCurrency, PrefixedAddressMapping, H160, H256, U256};
use frame_support::assert_ok;
use hex_literal::hex;
use pallet_evm::{AddressMapping, EvmConfig, Runner as RunnerT};
use sp_runtime::{traits::Zero, Permill};

mod common;
use common::keyring::{alice, ferdie};
use common::mock::ExtBuilder;

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
            // Alice sets up CENNZ <> CPAY liquidity
            assert_ok!(Cennzx::add_liquidity(
				Origin::signed(alice()),
				CENNZ_ASSET_ID,
				initial_liquidity, // min. liquidity
				initial_liquidity, // liquidity CENNZ
				initial_liquidity, // liquidity CPAY
			));

            // The account that will receive CENNZ as a result of the call being successful
            let receiver_eth: H160 = hex!("7a107Fc1794f505Cb351148F529AcCae12fFbcD8").into();
            let receiver: AccountId = <Runtime as pallet_evm::Config>::AddressMapping::into_account_id(receiver_eth.clone());
            let receiver_cennz_balance_before = <GenericAsset as MultiCurrency>::free_balance(&receiver, CENNZ_ASSET_ID);
            assert!(receiver_cennz_balance_before.is_zero());
            let transfer_amount: u128 = 123;

            assert_ok!(GenericAsset::transfer(Origin::signed(ferdie()), CENNZ_ASSET_ID, cennznet_address.clone(), initial_balance));
            let cennz_balance_before = <GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CENNZ_ASSET_ID);
            let cpay_balance_before = <GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CPAY_ASSET_ID);
            assert!(cpay_balance_before.is_zero());
            assert_eq!(cennz_balance_before, initial_balance);

            // Create input parameters for call
            // Below is the input abi used for transferring 123 CENNZ to receiver
            // a9059cbb0000000000000000000000007a107fc1794f505cb351148f529accae12ffbcd8000000000000000000000000000000000000000000000000000000000000007b
            let abi = hex!("ccf39ea90000000000000000000000000000000000000000000000000000000000003e800000000000000000000000000000000000000000000000000000000000000032000000000000000000000000cccccccc00003e8000000000000000000000000000000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000044a9059cbb0000000000000000000000007a107fc1794f505cb351148f529accae12ffbcd8000000000000000000000000000000000000000000000000000000000000007b00000000000000000000000000000000000000000000000000000000");
            let slippage: u32 = 50;
            let input = abi.to_vec();
            let gas_limit: u64 = 100000;
            let max_fee_per_gas = U256::from(20000000000000u64);
            let max_priority_fee_per_gas = U256::from(1000000u64);
            let access_list: Vec<(H160, Vec<H256>)> = vec![];
            let config: EvmConfig = CENNZNET_EVM_CONFIG.clone();
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
				&config
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
            // Alice sets up CENNZ <> CPAY liquidity
            assert_ok!(Cennzx::add_liquidity(
				Origin::signed(alice()),
				CENNZ_ASSET_ID,
				initial_liquidity, // min. liquidity
				initial_liquidity, // liquidity CENNZ
				initial_liquidity, // liquidity CPAY
			));

            let receiver_eth: H160 = hex!("7a107Fc1794f505Cb351148F529AcCae12fFbcD8").into();
            let receiver: AccountId = <Runtime as pallet_evm::Config>::AddressMapping::into_account_id(receiver_eth.clone());
            let receiver_cennz_balance_before = <GenericAsset as MultiCurrency>::free_balance(&receiver, CENNZ_ASSET_ID);
            assert!(receiver_cennz_balance_before.is_zero());

            let cennz_balance_before = <GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CENNZ_ASSET_ID);
            let cpay_balance_before = <GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CPAY_ASSET_ID);

            // Create input parameters for call
            // Below is the input abi used for transferring 123 CENNZ to receiver
            // a9059cbb0000000000000000000000007a107fc1794f505cb351148f529accae12ffbcd8000000000000000000000000000000000000000000000000000000000000007b
            let abi = hex!("ccf39ea90000000000000000000000000000000000000000000000000000000000003e810000000000000000000000000000000000000000000000000000000000000032000000000000000000000000cccccccc00003e8000000000000000000000000000000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000044a9059cbb0000000000000000000000007a107fc1794f505cb351148f529accae12ffbcd8000000000000000000000000000000000000000000000000000000000000007b00000000000000000000000000000000000000000000000000000000");
            let input = abi.to_vec();
            let gas_limit: u64 = 100000;
            let max_fee_per_gas = U256::from(20000000000000u64);
            let max_priority_fee_per_gas = U256::from(1000000u64);
            let access_list: Vec<(H160, Vec<H256>)> = vec![];
            let config: EvmConfig = CENNZNET_EVM_CONFIG.clone();
            assert!(<Runtime as pallet_evm::Config>::Runner::call(
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
			).is_err());

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
            // Alice sets up CENNZ <> CPAY liquidity
            assert_ok!(Cennzx::add_liquidity(
				Origin::signed(alice()),
				CENNZ_ASSET_ID,
				initial_liquidity, // min. liquidity
				initial_liquidity, // liquidity CENNZ
				initial_liquidity, // liquidity CPAY
			));

            let receiver_eth: H160 = hex!("7a107Fc1794f505Cb351148F529AcCae12fFbcD8").into();
            let receiver: AccountId = <Runtime as pallet_evm::Config>::AddressMapping::into_account_id(receiver_eth.clone());
            let receiver_cennz_balance_before = <GenericAsset as MultiCurrency>::free_balance(&receiver, CENNZ_ASSET_ID);
            assert!(receiver_cennz_balance_before.is_zero());

            assert_ok!(GenericAsset::transfer(Origin::signed(ferdie()), CENNZ_ASSET_ID, cennznet_address.clone(), initial_balance));
            let cennz_balance_before = <GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CENNZ_ASSET_ID);
            let cpay_balance_before = <GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CPAY_ASSET_ID);
            assert!(cpay_balance_before.is_zero());
            assert_eq!(cennz_balance_before, initial_balance);

            // Create input parameters for call
            // Below is the input abi used for transferring 123 CENNZ to receiver
            // a9059cbb0000000000000000000000007a107fc1794f505cb351148f529accae12ffbcd8000000000000000000000000000000000000000000000000000000000000007b
            let abi = hex!("ccf39ea90000000000000000000000000000000000000000000000000000000000003e800000000000000000000000000000000000000000000000000000000000000000000000000000000000000000cccccccc00003e8000000000000000000000000000000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000044a9059cbb0000000000000000000000007a107fc1794f505cb351148f529accae12ffbcd8000000000000000000000000000000000000000000000000000000000000007b00000000000000000000000000000000000000000000000000000000");
            let input = abi.to_vec();
            let gas_limit: u64 = 100000;
            let max_fee_per_gas = U256::from(20000000000000u64);
            let max_priority_fee_per_gas = U256::from(1000000u64);
            let access_list: Vec<(H160, Vec<H256>)> = vec![];
            let config: EvmConfig = CENNZNET_EVM_CONFIG.clone();
            // Call should fail as slippage is 0
            assert!(<Runtime as pallet_evm::Config>::Runner::call(
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
			).is_err());

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
            // Alice sets up CENNZ <> CPAY liquidity
            assert_ok!(Cennzx::add_liquidity(
				Origin::signed(alice()),
				CENNZ_ASSET_ID,
				initial_liquidity, // min. liquidity
				initial_liquidity, // liquidity CENNZ
				initial_liquidity, // liquidity CPAY
			));

            let receiver_eth: H160 = hex!("7a107Fc1794f505Cb351148F529AcCae12fFbcD8").into();
            let receiver: AccountId = <Runtime as pallet_evm::Config>::AddressMapping::into_account_id(receiver_eth.clone());
            let receiver_cennz_balance_before = <GenericAsset as MultiCurrency>::free_balance(&receiver, CENNZ_ASSET_ID);
            assert!(receiver_cennz_balance_before.is_zero());

            assert_ok!(GenericAsset::transfer(Origin::signed(ferdie()), CENNZ_ASSET_ID, cennznet_address.clone(), initial_balance));
            let cennz_balance_before = <GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CENNZ_ASSET_ID);
            let cpay_balance_before = <GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CPAY_ASSET_ID);
            assert!(cpay_balance_before.is_zero());
            assert_eq!(cennz_balance_before, initial_balance);

            // Create input parameters for call with slippage of 0.1%
            // Below is the input abi used for transferring 123 CENNZ to receiver
            // a9059cbb0000000000000000000000007a107fc1794f505cb351148f529accae12ffbcd8000000000000000000000000000000000000000000000000000000000000007b
            let abi = hex!("ccf39ea90000000000000000000000000000000000000000000000000000000000003e800000000000000000000000000000000000000000000000000000000000000001000000000000000000000000cccccccc00003e8000000000000000000000000000000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000044a9059cbb0000000000000000000000007a107fc1794f505cb351148f529accae12ffbcd8000000000000000000000000000000000000000000000000000000000000007b00000000000000000000000000000000000000000000000000000000");
            let input = abi.to_vec();
            let gas_limit: u64 = 100000;
            let max_fee_per_gas = U256::from(20000000000000u64);
            let max_priority_fee_per_gas = U256::from(1000000u64);
            let access_list: Vec<(H160, Vec<H256>)> = vec![];
            let config: EvmConfig = CENNZNET_EVM_CONFIG.clone();
            // Call should fail as slippage is 0
            assert!(<Runtime as pallet_evm::Config>::Runner::call(
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
			).is_err());

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

            // Create input parameters for call
            // This abi has an asset_id of 10 which doesn't exist
            let abi = hex!("ccf39ea9000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000320000000000000000000000001122334455667788991122334455667788990000000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000051234567890000000000000000000000000000000000000000000000000000000");
            let input = abi.to_vec();
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
			).is_err());
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
            // Create input parameters for call
            let abi = hex!("ccf39ea90000000000000000000000000000000000000000000000000000000000003e8000000000000000000000000000000000000000000000000000000000000000320000000000000000000000001122334455667788991122334455667788990000000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000051234567890000000000000000000000000000000000000000000000000000000");
            let input = abi.to_vec();
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
            ).is_err());

            // CPAY and CENNZ balance should be unchanged as the transaction never went through
            assert!(
                <GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CPAY_ASSET_ID).is_zero()
            );
            assert!(
                <GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CENNZ_ASSET_ID).is_zero()
            );
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
            // Alice sets up CENNZ <> CPAY liquidity
            assert_ok!(Cennzx::add_liquidity(
				Origin::signed(alice()),
				CENNZ_ASSET_ID,
				initial_liquidity, // min. liquidity
				initial_liquidity, // liquidity CENNZ
				initial_liquidity, // liquidity CPAY
			));

            // Create input parameters for call
            let abi = hex!("ccf39ea90000000000000000000000000000000000000000000000000000000000003e8000000000000000000000000000000000000000000000000000000000000000320000000000000000000000001122334455667788991122334455667788990000000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000051234567890000000000000000000000000000000000000000000000000000000");
            let input = abi.to_vec();
            let max_fee_per_gas = U256::from(20000000000000u64);
            let max_priority_fee_per_gas = U256::from(1000000u64);
            let access_list: Vec<(H160, Vec<H256>)> = vec![];
            let config: EvmConfig = CENNZNET_EVM_CONFIG.clone();
            assert!(
                <Runtime as pallet_evm::Config>::Runner::call(
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
                ).is_err()
            );

            // CPAY and CENNZ balance should be unchanged as the transaction never went through
            assert!(
                <GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CPAY_ASSET_ID).is_zero()
            );
            assert!(
                <GenericAsset as MultiCurrency>::free_balance(&cennznet_address, CENNZ_ASSET_ID).is_zero()
            );
        });
}
