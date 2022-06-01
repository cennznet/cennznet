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

//! EVM Cennzx integration tests

use cennznet_primitives::types::Balance;
use cennznet_runtime::{
	constants::{asset::*, currency::*, evm::*},
	AddressMappingOf, Cennzx, GenericAsset, Origin, Runtime, CENNZNET_EVM_CONFIG,
};
use crml_support::{MultiCurrency, H160, U256};
use ethabi::Token;
use fp_rpc::runtime_decl_for_EthereumRuntimeRPCApi::EthereumRuntimeRPCApi;
use frame_support::assert_ok;
use hex_literal::hex;
use pallet_evm::{AddressMapping, ExitReason, ExitRevert, Runner as RunnerT};
use pallet_evm_precompiles_erc20::Erc20IdConversion;

mod common;
use common::{keyring::alice, mock::ExtBuilder};

fn encode_swap_input(amount: Balance, max_input: Balance) -> Vec<u8> {
	// keccak('swapForExactCPAY(address,uint128,uint256)')[..4]
	let swap_selector = [0x39, 0x32, 0xb8, 0x8a];
	let cennz_token_address = Runtime::runtime_id_to_evm_id(CENNZ_ASSET_ID).0;
	let parameters = ethabi::encode(&[
		Token::Address(cennz_token_address), // swap cennz in
		Token::Uint(U256::from(amount)),     // exact cpay out
		Token::Uint(U256::from(max_input)),  // max. cennz in
	]);
	let mut input = vec![0_u8; 4_usize + 3 * 32];
	input[..4].copy_from_slice(&swap_selector);
	input[4..].copy_from_slice(parameters.as_slice());
	input.clone()
}

#[test]
fn buy_cpay_with_cennz() {
	ExtBuilder::default()
		.initial_balance(1_000_000 * DOLLARS)
		.build()
		.execute_with(|| {
			// setup liquidity in CENNZX
			let initial_liquidity = 500_000 * DOLLARS;
			assert_ok!(Cennzx::add_liquidity(
				Origin::signed(alice()),
				CENNZ_ASSET_ID,
				initial_liquidity, // min. liquidity
				initial_liquidity, // liquidity CENNZ
				initial_liquidity, // liquidity CPAY
			));

			// setup call to the cennzx precompile
			let cpay_to_buy = 100 * DOLLARS;
			let input = encode_swap_input(cpay_to_buy, cpay_to_buy + 5 * DOLLARS);
			let gas_limit = 1_000_000_u64;
			let max_fee_per_gas = Runtime::gas_price();
			let max_priority_fee_per_gas = U256::zero();

			// give caller some CENNZ to fund the swap
			let caller: H160 = hex!("420aC537F1a4f78d4Dfb3A71e902be0E3d480AFB").into();
			let caller_ss58 = AddressMappingOf::<Runtime>::into_account_id(caller);
			let initial_cennz_balance = 105 * DOLLARS;
			let initial_cpay_balance = 50 * DOLLARS;
			let _ = GenericAsset::deposit_creating(&caller_ss58, CENNZ_ASSET_ID, initial_cennz_balance);
			let _ = GenericAsset::deposit_creating(&caller_ss58, CPAY_ASSET_ID, initial_cpay_balance);

			// Test
			assert_ok!(<Runtime as pallet_evm::Config>::Runner::call(
				caller,
				H160::from_low_u64_be(CENNZX_PRECOMPILE),
				input,
				U256::zero(),
				gas_limit,
				Some(max_fee_per_gas),
				Some(max_priority_fee_per_gas),
				None,
				Default::default(),
				&CENNZNET_EVM_CONFIG
			));

			let after_cpay_balance = GenericAsset::free_balance(CPAY_ASSET_ID, &caller_ss58);
			let after_cennz_balance = GenericAsset::free_balance(CENNZ_ASSET_ID, &caller_ss58);
			// cennz has been swapped in
			assert!(after_cennz_balance < initial_cennz_balance);
			// even after paying gas fees, cpay balance is higher after the swap
			assert!(after_cpay_balance > initial_cpay_balance);
		});
}

#[test]
fn buy_cpay_reverts() {
	ExtBuilder::default().build().execute_with(|| {
		// Setup call to the cennzx precompile
		let cpay_to_buy = 100 * DOLLARS;
		let input = encode_swap_input(cpay_to_buy, cpay_to_buy + 5 * DOLLARS);
		let gas_limit = 1_000_000_u64;
		let max_fee_per_gas = Runtime::gas_price();
		let max_priority_fee_per_gas = U256::zero();
		let caller: H160 = hex!("420aC537F1a4f78d4Dfb3A71e902be0E3d480AFB").into();
		let caller_ss58 = AddressMappingOf::<Runtime>::into_account_id(caller);
		let _ = GenericAsset::deposit_creating(&caller_ss58, CPAY_ASSET_ID, 100 * DOLLARS);

		// Tests
		assert_eq!(
			<Runtime as pallet_evm::Config>::Runner::call(
				caller,
				H160::from_low_u64_be(CENNZX_PRECOMPILE),
				input,
				U256::zero(),
				gas_limit,
				Some(max_fee_per_gas),
				Some(max_priority_fee_per_gas),
				None,
				Default::default(),
				&CENNZNET_EVM_CONFIG
			)
			.unwrap()
			.exit_reason,
			ExitReason::Revert(ExitRevert::Reverted),
		);
	});
}
