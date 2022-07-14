/* Copyright 2019-2021 Centrality Investments Limited
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

//! EVM ERC20-Peg integration tests

use cennznet_primitives::types::{AssetId, Balance};
use cennznet_runtime::{
	constants::{asset::*, currency::*, evm::*},
	AddressMappingOf, Erc20Peg, GenericAsset, Origin, Runtime,
};
use crml_erc20_peg::AssetIdToErc20;
use crml_support::{MultiCurrency, H160, U256};
use ethabi::Token;
use frame_support::{assert_ok, StorageMap};
use hex_literal::hex;
use pallet_evm::AddressMapping;
use pallet_evm::{ExitReason, ExitRevert};
use pallet_evm_precompiles_erc20::Erc20IdConversion;

mod common;
use common::keyring::alice;
use common::mock::ExtBuilder;
use common::precompiles_builder::RunnerCallBuilder;

fn encode_withdraw_input(asset_id: AssetId, amount: Balance, beneficiary: H160) -> Vec<u8> {
	// keccak('withdraw(address,uint256,address)')[..4]
	let swap_selector = [0x69, 0x32, 0x8d, 0xec];
	let cennz_token_address = Runtime::runtime_id_to_evm_id(asset_id).0;
	let parameters = ethabi::encode(&[
		Token::Address(cennz_token_address), // withdraw token
		Token::Uint(U256::from(amount)),     // exact withdraw amount
		Token::Address(beneficiary),         // beneficiary
	]);
	let mut input = vec![0_u8; 4_usize + 3 * 32];
	input[..4].copy_from_slice(&swap_selector);
	input[4..].copy_from_slice(parameters.as_slice());
	input.clone()
}

#[test]
fn erc20_peg_withdraw() {
	ExtBuilder::default()
		.initial_balance(1_000_000 * DOLLARS)
		.build()
		.execute_with(|| {
			// Activate withdrawals
			assert_ok!(Erc20Peg::activate_withdrawals(Origin::root(), true));

			// setup call to the erc20-peg precompile
			let asset_id: AssetId = CENNZ_ASSET_ID;
			let amount: Balance = 100_000;
			let beneficiary: H160 = H160::from_low_u64_be(123);
			let input = encode_withdraw_input(asset_id, amount, beneficiary);

			// Setup asset to meta mapping in ERC20 Peg
			let cennz_token_address = Runtime::runtime_id_to_evm_id(asset_id).0;
			AssetIdToErc20::insert(asset_id, cennz_token_address);

			// give caller some CENNZ to fund the swap
			let caller: H160 = hex!("420aC537F1a4f78d4Dfb3A71e902be0E3d480AFB").into();
			let caller_ss58 = AddressMappingOf::<Runtime>::into_account_id(caller);
			let initial_cennz_balance = 105 * DOLLARS;
			let initial_cpay_balance = 50 * DOLLARS;
			let _ = GenericAsset::deposit_creating(&caller_ss58, CENNZ_ASSET_ID, initial_cennz_balance);
			let _ = GenericAsset::deposit_creating(&caller_ss58, CPAY_ASSET_ID, initial_cpay_balance);

			// Test
			assert_ok!(RunnerCallBuilder::new(caller, input, H160::from_low_u64_be(PEG_PRECOMPILE)).run());

			let after_cpay_balance = GenericAsset::free_balance(CPAY_ASSET_ID, &caller_ss58);
			let after_cennz_balance = GenericAsset::free_balance(CENNZ_ASSET_ID, &caller_ss58);
			// cennz has been withdrawn
			assert_eq!(after_cennz_balance, initial_cennz_balance - amount);
			// cpay should be lower as it was used to pay gas fees
			assert!(after_cpay_balance < initial_cpay_balance);
		});
}

#[test]
fn erc20_peg_withdraw_reverts() {
	ExtBuilder::default()
		.initial_balance(1_000_000 * DOLLARS)
		.build()
		.execute_with(|| {
			// setup call to the erc20-peg precompile
			let asset_id: AssetId = CENNZ_ASSET_ID;
			let amount: Balance = 100_000;
			let beneficiary: H160 = H160::from_low_u64_be(123);
			let input = encode_withdraw_input(asset_id, amount, beneficiary);

			// give caller some CENNZ to fund the swap
			let caller: H160 = hex!("420aC537F1a4f78d4Dfb3A71e902be0E3d480AFB").into();
			let caller_ss58 = AddressMappingOf::<Runtime>::into_account_id(caller);
			let initial_cennz_balance = 105 * DOLLARS;
			let initial_cpay_balance = 50 * DOLLARS;
			let _ = GenericAsset::deposit_creating(&caller_ss58, CENNZ_ASSET_ID, initial_cennz_balance);
			let _ = GenericAsset::deposit_creating(&caller_ss58, CPAY_ASSET_ID, initial_cpay_balance);

			// Test
			assert_eq!(
				RunnerCallBuilder::new(caller, input, H160::from_low_u64_be(PEG_PRECOMPILE))
					.run()
					.unwrap()
					.exit_reason,
				ExitReason::Revert(ExitRevert::Reverted),
			);

			let after_cpay_balance = GenericAsset::free_balance(CPAY_ASSET_ID, &caller_ss58);
			let after_cennz_balance = GenericAsset::free_balance(CENNZ_ASSET_ID, &caller_ss58);
			// cennz has not been withdrawn
			assert_eq!(after_cennz_balance, initial_cennz_balance);
			// cpay should be lower as it was used to pay gas fees
			assert!(after_cpay_balance < initial_cpay_balance);
		});
}

#[test]
fn erc20_peg_withdraw_with_delay_should_fail() {
	ExtBuilder::default()
		.initial_balance(1_000_000 * DOLLARS)
		.build()
		.execute_with(|| {
			// Setup new Generic Asset
			let decimal_place = 4;
			let initial_supply = 1_000_000;
			let asset_id = GenericAsset::create(&alice(), initial_supply, decimal_place, 10, b"TST1".to_vec()).unwrap();

			// Activate withdrawals
			assert_ok!(Erc20Peg::activate_withdrawals(Origin::root(), true));

			// setup call to the erc20-peg precompile
			let amount: Balance = 100_000;
			let beneficiary: H160 = H160::from_low_u64_be(123);
			let input = encode_withdraw_input(asset_id, amount, beneficiary);

			// Setup asset to meta mapping in ERC20 Peg
			let cennz_token_address = Runtime::runtime_id_to_evm_id(asset_id).0;
			AssetIdToErc20::insert(asset_id, cennz_token_address);

			// Setup claim delay for asset which should force the withdrawal to fail
			assert_ok!(Erc20Peg::set_claim_delay(Origin::root(), asset_id, 1, 100));

			// give caller some CENNZ to fund the swap
			let caller: H160 = hex!("420aC537F1a4f78d4Dfb3A71e902be0E3d480AFB").into();
			let caller_ss58 = AddressMappingOf::<Runtime>::into_account_id(caller);
			let initial_asset_balance = 105 * DOLLARS;
			let initial_cpay_balance = 50 * DOLLARS;
			let _ = GenericAsset::deposit_creating(&caller_ss58, asset_id, initial_asset_balance);
			let _ = GenericAsset::deposit_creating(&caller_ss58, CPAY_ASSET_ID, initial_cpay_balance);

			// Test should be reverted
			assert_eq!(
				RunnerCallBuilder::new(caller, input, H160::from_low_u64_be(PEG_PRECOMPILE))
					.run()
					.unwrap()
					.exit_reason,
				ExitReason::Revert(ExitRevert::Reverted),
			);

			let after_cpay_balance = GenericAsset::free_balance(CPAY_ASSET_ID, &caller_ss58);
			let after_asset_balance = GenericAsset::free_balance(asset_id, &caller_ss58);
			// cennz has not been withdrawn
			assert_eq!(after_asset_balance, initial_asset_balance);
			// cpay should be lower as it was used to pay gas fees
			assert!(after_cpay_balance < initial_cpay_balance);
		});
}
