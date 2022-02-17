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

use cennznet_primitives::types::{AccountId, AssetId, Balance};
use cennznet_runtime::{GenericAsset, Runtime, TokenApprovals};
use crml_support::{MultiCurrency, PrefixedAddressMapping};
use frame_support::assert_ok;
use hex_literal::hex;
use pallet_evm_precompiles_erc20::{
	Action, Address, AddressMapping, Context, Erc20IdConversion, Erc20PrecompileSet, EvmDataWriter, PrecompileSet,
};
use sp_core::{H160, U256};

mod common;
use common::mock::ExtBuilder;

const STAKING_ASSET_ID: AssetId = 16000;

fn setup_context(asset_id: AssetId, caller: H160) -> (H160, Context) {
	let address: H160 = Runtime::runtime_id_to_evm_id(asset_id).into();
	let context: Context = Context {
		address,
		caller,
		apparent_value: U256::default(),
	};
	(address, context)
}

#[test]
fn erc20_transfer_from() {
	let initial_balance = 1000;
	ExtBuilder::default()
		.initial_balance(initial_balance)
		.build()
		.execute_with(|| {
			let caller_eth = H160::from_slice(&hex!("a86e122EdbDcBA4bF24a2Abf89F5C230b37DF49d"));
			let receiver_eth = H160::from_slice(&hex!("0000022EdbDcBA4bF24a2Abf89F5C230b3700000"));
			let caller: AccountId = PrefixedAddressMapping::into_account_id(caller_eth.clone());
			let receiver: AccountId = PrefixedAddressMapping::into_account_id(receiver_eth.clone());
			let transfer_amount: Balance = 100;

			// Check initial balances
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&caller, STAKING_ASSET_ID),
				initial_balance
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&receiver, STAKING_ASSET_ID),
				0
			);

			let (address, context) = setup_context(STAKING_ASSET_ID, caller_eth);
			let input_data = EvmDataWriter::new_with_selector(Action::TransferFrom)
				.write::<Address>(caller_eth.into())
				.write::<Address>(receiver_eth.into())
				.write::<U256>(transfer_amount.into())
				.build();
			let precompile_set = Erc20PrecompileSet::<Runtime>::new();

			assert_ok!(precompile_set
				.execute(address.into(), &input_data, None, &context, false,)
				.unwrap());

			// Check final balances
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&caller, STAKING_ASSET_ID),
				initial_balance - transfer_amount
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&receiver, STAKING_ASSET_ID),
				transfer_amount
			);
		})
}

#[test]
fn erc20_transfer_from_not_caller_should_fail() {
	let initial_balance = 1000;
	ExtBuilder::default()
		.initial_balance(initial_balance)
		.build()
		.execute_with(|| {
			let caller_eth = H160::from_slice(&hex!("a86e122EdbDcBA4bF24a2Abf89F5C230b37DF49d"));
			let receiver_eth = H160::from_slice(&hex!("0000022EdbDcBA4bF24a2Abf89F5C230b3700000"));
			let caller: AccountId = PrefixedAddressMapping::into_account_id(caller_eth.clone());
			let receiver: AccountId = PrefixedAddressMapping::into_account_id(receiver_eth.clone());
			let transfer_amount: Balance = 100;

			let (address, context) = setup_context(STAKING_ASSET_ID, receiver_eth);
			let input_data = EvmDataWriter::new_with_selector(Action::TransferFrom)
				.write::<Address>(caller_eth.into())
				.write::<Address>(receiver_eth.into())
				.write::<U256>(transfer_amount.into())
				.build();
			let precompile_set = Erc20PrecompileSet::<Runtime>::new();

			assert!(precompile_set
				.execute(address.into(), &input_data, None, &context, false,)
				.unwrap()
				.is_err());

			// Check final balances haven't changed
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&caller, STAKING_ASSET_ID),
				initial_balance,
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&receiver, STAKING_ASSET_ID),
				0,
			);
		})
}

#[test]
fn erc20_approve_and_transfer() {
	let initial_balance = 1000;
	ExtBuilder::default()
		.initial_balance(initial_balance)
		.build()
		.execute_with(|| {
			let owner_eth = H160::from_slice(&hex!("a86e122EdbDcBA4bF24a2Abf89F5C230b37DF49d"));
			let receiver_eth = H160::from_slice(&hex!("0000022EdbDcBA4bF24a2Abf89F5C230b3700000"));
			let approved_eth = H160::from_slice(&hex!("0000022EdbDcBA4bF24a2Abf89F5C230b3700000"));
			let owner: AccountId = PrefixedAddressMapping::into_account_id(owner_eth.clone());
			let receiver: AccountId = PrefixedAddressMapping::into_account_id(receiver_eth.clone());
			let approved_amount: Balance = 200;
			let transfer_amount: Balance = 100;

			// Set Approval
			let (address, context) = setup_context(STAKING_ASSET_ID, owner_eth);
			let input_data = EvmDataWriter::new_with_selector(Action::Approve)
				.write::<Address>(approved_eth.into())
				.write::<U256>(approved_amount.into())
				.build();
			let precompile_set = Erc20PrecompileSet::<Runtime>::new();
			assert_ok!(precompile_set
				.execute(address.into(), &input_data, None, &context, false)
				.unwrap());

			// Check approvals module
			assert_eq!(
				TokenApprovals::erc20_approvals((owner_eth, STAKING_ASSET_ID), approved_eth),
				approved_amount
			);

			// Transfer
			let (address, context) = setup_context(STAKING_ASSET_ID, approved_eth);
			let input_data = EvmDataWriter::new_with_selector(Action::TransferFrom)
				.write::<Address>(owner_eth.into())
				.write::<Address>(receiver_eth.into())
				.write::<U256>(transfer_amount.into())
				.build();
			let precompile_set = Erc20PrecompileSet::<Runtime>::new();
			assert_ok!(precompile_set
				.execute(address.into(), &input_data, None, &context, false,)
				.unwrap());

			// Check final balances
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&owner, STAKING_ASSET_ID),
				initial_balance - transfer_amount
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&receiver, STAKING_ASSET_ID),
				transfer_amount
			);
			// Check approvals module has been updated after transfer
			assert_eq!(
				TokenApprovals::erc20_approvals((owner_eth, STAKING_ASSET_ID), approved_eth),
				approved_amount - transfer_amount
			);
		})
}

#[test]
fn erc20_update_existing_approval() {
	let initial_balance = 1000;
	ExtBuilder::default()
		.initial_balance(initial_balance)
		.build()
		.execute_with(|| {
			let owner_eth = H160::from_slice(&hex!("a86e122EdbDcBA4bF24a2Abf89F5C230b37DF49d"));
			let approved_eth = H160::from_slice(&hex!("0000022EdbDcBA4bF24a2Abf89F5C230b3700000"));
			let initial_approved_amount: Balance = 200;
			let updated_approved_amount: Balance = 100;

			// Set Approval
			let (address, context) = setup_context(STAKING_ASSET_ID, owner_eth);
			let input_data = EvmDataWriter::new_with_selector(Action::Approve)
				.write::<Address>(approved_eth.into())
				.write::<U256>(initial_approved_amount.into())
				.build();
			let precompile_set = Erc20PrecompileSet::<Runtime>::new();
			assert_ok!(precompile_set
				.execute(address.into(), &input_data, None, &context, false)
				.unwrap());

			// Check approvals module
			assert_eq!(
				TokenApprovals::erc20_approvals((owner_eth, STAKING_ASSET_ID), approved_eth),
				initial_approved_amount
			);

			// Update approval amount
			let input_data = EvmDataWriter::new_with_selector(Action::Approve)
				.write::<Address>(approved_eth.into())
				.write::<U256>(updated_approved_amount.into())
				.build();
			let precompile_set = Erc20PrecompileSet::<Runtime>::new();
			assert_ok!(precompile_set
				.execute(address.into(), &input_data, None, &context, false)
				.unwrap());

			// Check approvals amount has changed
			assert_eq!(
				TokenApprovals::erc20_approvals((owner_eth, STAKING_ASSET_ID), approved_eth),
				updated_approved_amount
			);
		})
}
