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
use cennznet_runtime::constants::asset::CPAY_ASSET_ID;
use cennznet_runtime::constants::currency::DOLLARS;
use cennznet_runtime::{GenericAsset, Runtime, TokenApprovals};
use crml_support::{MultiCurrency, PrefixedAddressMapping};
use frame_support::assert_ok;
use hex_literal::hex;
use pallet_evm::AddressMapping;
use pallet_evm_precompiles_erc20::{Action, Erc20IdConversion};
use precompile_utils::prelude::*;
use precompile_utils::AddressMappingReversibleExt;
use sp_core::{H160, U256};

mod common;
use common::mock::ExtBuilder;
use common::precompiles_builder::RunnerCallBuilder;

const STAKING_ASSET_ID: AssetId = 16000;

#[test]
fn erc20_transfer() {
	let initial_balance = 1000;
	let caller = AccountId::from(hex!("63766d3a00000000000000a86e122edbdcba4bf24a2abf89f5c230b37df49d4a"));
	ExtBuilder::default()
		.initialise_eth_accounts(vec![caller.clone()])
		.initial_balance(initial_balance)
		.build()
		.execute_with(|| {
			let caller_eth = PrefixedAddressMapping::from_account_id(caller.clone());
			let receiver_eth = H160::from_slice(&hex!("0000022EdbDcBA4bF24a2Abf89F5C230b3700000"));
			let receiver: AccountId = PrefixedAddressMapping::into_account_id(receiver_eth.clone());
			let transfer_amount: Balance = 100;
			let _ = GenericAsset::deposit_creating(&caller, CPAY_ASSET_ID, 100 * DOLLARS);

			// Check initial balances
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&caller, STAKING_ASSET_ID),
				initial_balance
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&receiver, STAKING_ASSET_ID),
				0
			);
			let address: H160 = Runtime::runtime_id_to_evm_id(STAKING_ASSET_ID).into();
			let input_data = EvmDataWriter::new_with_selector(Action::Transfer)
				.write::<Address>(receiver_eth.into())
				.write::<U256>(transfer_amount.into())
				.build();
			assert_ok!(RunnerCallBuilder::new(caller_eth, input_data, address).run());

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
fn erc20_transfer_from() {
	let initial_balance = 1000;
	let caller = AccountId::from(hex!("63766d3a00000000000000a86e122edbdcba4bf24a2abf89f5c230b37df49d4a"));
	ExtBuilder::default()
		.initialise_eth_accounts(vec![caller.clone()])
		.initial_balance(initial_balance)
		.build()
		.execute_with(|| {
			let caller_eth = PrefixedAddressMapping::from_account_id(caller.clone());
			let receiver_eth = H160::from_slice(&hex!("0000022EdbDcBA4bF24a2Abf89F5C230b3700000"));
			let receiver: AccountId = PrefixedAddressMapping::into_account_id(receiver_eth.clone());
			let transfer_amount: Balance = 100;
			let _ = GenericAsset::deposit_creating(&caller, CPAY_ASSET_ID, 100 * DOLLARS);

			// Check initial balances
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&caller, STAKING_ASSET_ID),
				initial_balance
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&receiver, STAKING_ASSET_ID),
				0
			);

			let address: H160 = Runtime::runtime_id_to_evm_id(STAKING_ASSET_ID).into();
			let input_data = EvmDataWriter::new_with_selector(Action::TransferFrom)
				.write::<Address>(caller_eth.into())
				.write::<Address>(receiver_eth.into())
				.write::<U256>(transfer_amount.into())
				.build();
			assert_ok!(RunnerCallBuilder::new(caller_eth, input_data, address).run());

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
	let caller = AccountId::from(hex!("63766d3a00000000000000a86e122edbdcba4bf24a2abf89f5c230b37df49d4a"));
	ExtBuilder::default()
		.initialise_eth_accounts(vec![caller.clone()])
		.initial_balance(initial_balance)
		.build()
		.execute_with(|| {
			let caller_eth = PrefixedAddressMapping::from_account_id(caller.clone());
			let receiver_eth = H160::from_slice(&hex!("0000022EdbDcBA4bF24a2Abf89F5C230b3700000"));
			let receiver: AccountId = PrefixedAddressMapping::into_account_id(receiver_eth.clone());
			let transfer_amount: Balance = 100;
			let _ = GenericAsset::deposit_creating(&receiver, CPAY_ASSET_ID, 100 * DOLLARS);

			let address: H160 = Runtime::runtime_id_to_evm_id(STAKING_ASSET_ID).into();
			let input_data = EvmDataWriter::new_with_selector(Action::TransferFrom)
				.write::<Address>(caller_eth.into())
				.write::<Address>(receiver_eth.into())
				.write::<U256>(transfer_amount.into())
				.build();
			assert_ok!(RunnerCallBuilder::new(receiver_eth, input_data, address).run());

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
	let initial_balance = 100000;
	let owner = AccountId::from(hex!("63766d3a00000000000000a86e122edbdcba4bf24a2abf89f5c230b37df49d4a"));
	ExtBuilder::default()
		.initialise_eth_accounts(vec![owner.clone()])
		.initial_balance(initial_balance)
		.build()
		.execute_with(|| {
			let owner_eth = PrefixedAddressMapping::from_account_id(owner.clone());
			let receiver_eth = H160::from_slice(&hex!("0000022EdbDcBA4bF24a2Abf89F5C230b3700000"));
			let approved_eth = H160::from_slice(&hex!("0000022EdbDcBA4bF24a2Abf89F5C230b3700000"));
			let receiver: AccountId = PrefixedAddressMapping::into_account_id(receiver_eth.clone());
			let approved: AccountId = PrefixedAddressMapping::into_account_id(approved_eth.clone());
			let _ = GenericAsset::deposit_creating(&owner, CPAY_ASSET_ID, 100 * DOLLARS);
			let _ = GenericAsset::deposit_creating(&approved, CPAY_ASSET_ID, 100 * DOLLARS);

			let approved_amount: Balance = 200;
			let transfer_amount: Balance = 100;

			// Set Approval
			let address: H160 = Runtime::runtime_id_to_evm_id(STAKING_ASSET_ID).into();
			let input_data = EvmDataWriter::new_with_selector(Action::Approve)
				.write::<Address>(approved_eth.into())
				.write::<U256>(approved_amount.into())
				.build();
			assert_ok!(RunnerCallBuilder::new(owner_eth, input_data, address).run());

			// Check approvals module
			assert_eq!(
				TokenApprovals::erc20_approvals((owner_eth, STAKING_ASSET_ID), approved_eth),
				approved_amount
			);

			// Transfer
			let input_data = EvmDataWriter::new_with_selector(Action::TransferFrom)
				.write::<Address>(owner_eth.into())
				.write::<Address>(receiver_eth.into())
				.write::<U256>(transfer_amount.into())
				.build();
			assert_ok!(RunnerCallBuilder::new(approved_eth, input_data, address).run());

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
fn erc20_approve_and_transfer_removes_approval() {
	let initial_balance = 1000;
	let owner = AccountId::from(hex!("63766d3a00000000000000a86e122edbdcba4bf24a2abf89f5c230b37df49d4a"));
	ExtBuilder::default()
		.initialise_eth_accounts(vec![owner.clone()])
		.initial_balance(initial_balance)
		.build()
		.execute_with(|| {
			let owner_eth = PrefixedAddressMapping::from_account_id(owner.clone());
			let receiver_eth = H160::from_slice(&hex!("0000022EdbDcBA4bF24a2Abf89F5C230b3700000"));
			let approved_eth = H160::from_slice(&hex!("0000022EdbDcBA4bF24a2Abf89F5C230b3700000"));
			let approved: AccountId = PrefixedAddressMapping::into_account_id(approved_eth.clone());
			let receiver: AccountId = PrefixedAddressMapping::into_account_id(receiver_eth.clone());
			let approved_amount: Balance = 200;
			let transfer_amount: Balance = 200; // The same as approved amount should clear approval
			let _ = GenericAsset::deposit_creating(&approved, CPAY_ASSET_ID, 100 * DOLLARS);
			let _ = GenericAsset::deposit_creating(&owner, CPAY_ASSET_ID, 100 * DOLLARS);

			// Set Approval
			let address: H160 = Runtime::runtime_id_to_evm_id(STAKING_ASSET_ID).into();
			let input_data = EvmDataWriter::new_with_selector(Action::Approve)
				.write::<Address>(approved_eth.into())
				.write::<U256>(approved_amount.into())
				.build();
			assert_ok!(RunnerCallBuilder::new(owner_eth, input_data, address).run());

			// Check approvals module
			assert_eq!(
				TokenApprovals::erc20_approvals((owner_eth, STAKING_ASSET_ID), approved_eth),
				approved_amount
			);

			// Transfer
			let input_data = EvmDataWriter::new_with_selector(Action::TransferFrom)
				.write::<Address>(owner_eth.into())
				.write::<Address>(receiver_eth.into())
				.write::<U256>(transfer_amount.into())
				.build();
			assert_ok!(RunnerCallBuilder::new(approved_eth, input_data, address).run());

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
				0
			);
		})
}

#[test]
fn erc20_not_enough_approved_should_fail() {
	let initial_balance = 1000;
	let owner = AccountId::from(hex!("63766d3a00000000000000a86e122edbdcba4bf24a2abf89f5c230b37df49d4a"));
	ExtBuilder::default()
		.initialise_eth_accounts(vec![owner.clone()])
		.initial_balance(initial_balance)
		.build()
		.execute_with(|| {
			let owner_eth = PrefixedAddressMapping::from_account_id(owner.clone());
			let receiver_eth = H160::from_slice(&hex!("0000022EdbDcBA4bF24a2Abf89F5C230b3700000"));
			let approved_eth = H160::from_slice(&hex!("0000022EdbDcBA4bF24a2Abf89F5C230b3700000"));
			let approved: AccountId = PrefixedAddressMapping::into_account_id(approved_eth.clone());
			let receiver: AccountId = PrefixedAddressMapping::into_account_id(receiver_eth.clone());
			let approved_amount: Balance = 100;
			let transfer_amount: Balance = 101; // Higher than approved amount
			let _ = GenericAsset::deposit_creating(&approved, CPAY_ASSET_ID, 100 * DOLLARS);
			let _ = GenericAsset::deposit_creating(&owner, CPAY_ASSET_ID, 100 * DOLLARS);

			// Set Approval
			let address: H160 = Runtime::runtime_id_to_evm_id(STAKING_ASSET_ID).into();
			let input_data = EvmDataWriter::new_with_selector(Action::Approve)
				.write::<Address>(approved_eth.into())
				.write::<U256>(approved_amount.into())
				.build();
			assert_ok!(RunnerCallBuilder::new(owner_eth, input_data, address).run());

			// Check approvals module
			assert_eq!(
				TokenApprovals::erc20_approvals((owner_eth, STAKING_ASSET_ID), approved_eth),
				approved_amount
			);

			// Transfer
			let input_data = EvmDataWriter::new_with_selector(Action::TransferFrom)
				.write::<Address>(owner_eth.into())
				.write::<Address>(receiver_eth.into())
				.write::<U256>(transfer_amount.into())
				.build();
			assert_ok!(RunnerCallBuilder::new(approved_eth, input_data, address).run());

			// Check final balances
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&owner, STAKING_ASSET_ID),
				initial_balance
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&receiver, STAKING_ASSET_ID),
				0
			);
			// Check approvals module hasn't changed
			assert_eq!(
				TokenApprovals::erc20_approvals((owner_eth, STAKING_ASSET_ID), approved_eth),
				approved_amount
			);
		})
}

#[test]
fn erc20_update_existing_approval() {
	let initial_balance = 1000;
	let owner = AccountId::from(hex!("63766d3a00000000000000a86e122edbdcba4bf24a2abf89f5c230b37df49d4a"));
	ExtBuilder::default()
		.initialise_eth_accounts(vec![owner.clone()])
		.initial_balance(initial_balance)
		.build()
		.execute_with(|| {
			let owner_eth = PrefixedAddressMapping::from_account_id(owner.clone());
			let approved_eth = H160::from_slice(&hex!("0000022EdbDcBA4bF24a2Abf89F5C230b3700000"));
			let initial_approved_amount: Balance = 200;
			let updated_approved_amount: Balance = 100;
			let _ = GenericAsset::deposit_creating(&owner, CPAY_ASSET_ID, 100 * DOLLARS);

			// Set Approval
			let address: H160 = Runtime::runtime_id_to_evm_id(STAKING_ASSET_ID).into();
			let input_data = EvmDataWriter::new_with_selector(Action::Approve)
				.write::<Address>(approved_eth.into())
				.write::<U256>(initial_approved_amount.into())
				.build();
			assert_ok!(RunnerCallBuilder::new(owner_eth, input_data, address).run());

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
			assert_ok!(RunnerCallBuilder::new(owner_eth, input_data, address).run());

			// Check approvals amount has changed
			assert_eq!(
				TokenApprovals::erc20_approvals((owner_eth, STAKING_ASSET_ID), approved_eth),
				updated_approved_amount
			);
		})
}
