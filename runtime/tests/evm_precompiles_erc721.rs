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

use cennznet_primitives::types::{AccountId, CollectionId, SerialNumber, SeriesId};
use cennznet_runtime::constants::asset::CPAY_ASSET_ID;
use cennznet_runtime::constants::currency::DOLLARS;
use cennznet_runtime::{GenericAsset, Nft, Runtime, TokenApprovals};
use crml_nft::MetadataScheme;
use crml_support::{MultiCurrency, PrefixedAddressMapping};
use frame_support::assert_ok;
use pallet_evm::{AddressMapping, ExitError, ExitReason};
use pallet_evm_precompiles_erc721::{Action, Erc721IdConversion};
use precompile_utils::prelude::*;
use sp_core::{H160, U256};

mod common;
use common::mock::ExtBuilder;
use common::precompiles_builder::RunnerCallBuilder;
use precompile_utils::ExitRevert;

fn setup_nft_series(token_owner: AccountId) -> (CollectionId, SeriesId, SerialNumber) {
	let collection_owner_eth: H160 = b"test1000000000000000".into();
	let collection_owner: AccountId = PrefixedAddressMapping::into_account_id(collection_owner_eth.clone());
	let collection_id = Nft::next_collection_id();

	assert_ok!(Nft::create_collection(
		Some(collection_owner.clone()).into(),
		b"test-collection".to_vec(),
		None
	));
	assert_ok!(Nft::mint_series(
		Some(collection_owner).into(),
		collection_id,
		1,
		Some(token_owner),
		MetadataScheme::IpfsDir(b"<CID>".to_vec()),
		None,
	));
	(collection_id, 0, 0)
}

fn setup_input_data(serial_number: SerialNumber, to: H160, from: H160, selector: Action) -> Vec<u8> {
	// Write to input data
	EvmDataWriter::new_with_selector(selector)
		.write::<Address>(to.into())
		.write::<Address>(from.into())
		.write::<U256>(serial_number.into())
		.build()
}

#[test]
fn erc721_transfer_from() {
	ExtBuilder::default().initial_balance(1).build().execute_with(|| {
		let token_owner_eth: H160 = b"test2000000000000000".into();
		let new_owner_eth: H160 = b"test3000000000000000".into();

		let token_owner: AccountId = PrefixedAddressMapping::into_account_id(token_owner_eth.clone());
		let new_owner: AccountId = PrefixedAddressMapping::into_account_id(new_owner_eth.clone());
		let _ = GenericAsset::deposit_creating(&token_owner, CPAY_ASSET_ID, 100 * DOLLARS);

		let (collection_id, series_id, serial_number) = setup_nft_series(token_owner.clone());
		let address: H160 = Runtime::runtime_id_to_evm_id((collection_id, series_id)).into();
		let input_data = setup_input_data(serial_number, new_owner_eth, token_owner_eth, Action::TransferFrom);

		assert_eq!(
			Nft::token_owner((collection_id, series_id), serial_number).unwrap(),
			token_owner.clone()
		);
		assert_ok!(RunnerCallBuilder::new(token_owner_eth, input_data, address).run());

		// NFT changed ownership
		assert_eq!(
			Nft::token_owner((collection_id, series_id), serial_number).unwrap(),
			new_owner
		);
	})
}

#[test]
fn erc721_transfer_from_caller_not_approved_should_fail() {
	ExtBuilder::default().initial_balance(1).build().execute_with(|| {
		let token_owner_eth: H160 = b"test2000000000000000".into();
		let new_owner_eth: H160 = b"test3000000000000000".into();
		let token_owner: AccountId = PrefixedAddressMapping::into_account_id(token_owner_eth.clone());
		let new_owner: AccountId = PrefixedAddressMapping::into_account_id(new_owner_eth.clone());
		let _ = GenericAsset::deposit_creating(&new_owner, CPAY_ASSET_ID, 100 * DOLLARS);

		let (collection_id, series_id, serial_number) = setup_nft_series(token_owner.clone());
		let address: H160 = Runtime::runtime_id_to_evm_id((collection_id, series_id)).into();
		let input_data = setup_input_data(serial_number, new_owner_eth, token_owner_eth, Action::TransferFrom);

		assert_eq!(
			RunnerCallBuilder::new(new_owner_eth, input_data, address)
				.run()
				.unwrap()
				.exit_reason,
			ExitReason::Error(ExitError::Other(("caller not approved").into()))
		);

		// Ownership shouldn't have transferred
		assert_eq!(
			Nft::token_owner((collection_id, series_id), serial_number).unwrap(),
			token_owner
		);
	})
}

#[test]
fn erc721_approve_and_transfer() {
	ExtBuilder::default().initial_balance(1).build().execute_with(|| {
		let token_owner_eth: H160 = b"test2000000000000000".into();
		let approved_account_eth: H160 = b"test3000000000000000".into();
		let new_owner_eth: H160 = b"test4000000000000000".into();
		let token_owner: AccountId = PrefixedAddressMapping::into_account_id(token_owner_eth.clone());
		let new_owner: AccountId = PrefixedAddressMapping::into_account_id(new_owner_eth.clone());
		let approved_account: AccountId = PrefixedAddressMapping::into_account_id(approved_account_eth.clone());
		let _ = GenericAsset::deposit_creating(&token_owner, CPAY_ASSET_ID, 100 * DOLLARS);
		let _ = GenericAsset::deposit_creating(&approved_account, CPAY_ASSET_ID, 100 * DOLLARS);

		let (collection_id, series_id, serial_number) = setup_nft_series(token_owner.clone());
		let address: H160 = Runtime::runtime_id_to_evm_id((collection_id, series_id)).into();
		let input_data = setup_input_data(serial_number, approved_account_eth, token_owner_eth, Action::Approve);

		assert_ok!(RunnerCallBuilder::new(token_owner_eth, input_data, address).run());

		assert_eq!(
			TokenApprovals::erc721_approvals((collection_id, series_id, serial_number)),
			approved_account_eth.clone()
		);

		// Transfer NFT from approved account
		let input_data = setup_input_data(serial_number, new_owner_eth, token_owner_eth, Action::TransferFrom);

		assert_ok!(RunnerCallBuilder::new(approved_account_eth, input_data, address).run());

		// NFT changed ownership
		assert_eq!(
			Nft::token_owner((collection_id, series_id), serial_number).unwrap(),
			new_owner
		);
		// Approval should be removed
		assert_eq!(
			TokenApprovals::erc721_approvals((collection_id, series_id, serial_number)),
			H160::default()
		);
	})
}

#[test]
fn erc721_approve_caller_not_from_should_fail() {
	ExtBuilder::default().initial_balance(1).build().execute_with(|| {
		let token_owner_eth: H160 = b"test2000000000000000".into();
		let approved_account_eth: H160 = b"test3000000000000000".into();
		let token_owner: AccountId = PrefixedAddressMapping::into_account_id(token_owner_eth.clone());
		let approved_account: AccountId = PrefixedAddressMapping::into_account_id(approved_account_eth.clone());
		let _ = GenericAsset::deposit_creating(&approved_account, CPAY_ASSET_ID, 100 * DOLLARS);

		let (collection_id, series_id, serial_number) = setup_nft_series(token_owner.clone());
		let address: H160 = Runtime::runtime_id_to_evm_id((collection_id, series_id)).into();
		let input_data = setup_input_data(serial_number, approved_account_eth, token_owner_eth, Action::Approve);

		assert_eq!(
			RunnerCallBuilder::new(approved_account_eth, input_data, address)
				.run()
				.unwrap()
				.exit_reason,
			ExitReason::Error(ExitError::Other(("caller must be from").into()))
		);

		assert_eq!(
			TokenApprovals::erc721_approvals((collection_id, series_id, serial_number)),
			H160::default()
		);
	})
}

#[test]
fn erc721_approve_caller_not_token_owner_should_fail() {
	ExtBuilder::default().initial_balance(1).build().execute_with(|| {
		let token_owner_eth: H160 = b"test2000000000000000".into();
		let approved_account_eth: H160 = b"test3000000000000000".into();
		let new_owner_eth: H160 = b"test4000000000000000".into();
		let token_owner: AccountId = PrefixedAddressMapping::into_account_id(token_owner_eth.clone());
		let approved_account: AccountId = PrefixedAddressMapping::into_account_id(approved_account_eth.clone());
		let _ = GenericAsset::deposit_creating(&approved_account, CPAY_ASSET_ID, 100 * DOLLARS);

		let (collection_id, series_id, serial_number) = setup_nft_series(token_owner.clone());
		let address: H160 = Runtime::runtime_id_to_evm_id((collection_id, series_id)).into();
		let input_data = setup_input_data(serial_number, new_owner_eth, approved_account_eth, Action::Approve);

		assert_eq!(
			RunnerCallBuilder::new(approved_account_eth, input_data, address)
				.run()
				.unwrap()
				.exit_reason,
			ExitReason::Revert(ExitRevert::Reverted)
		);

		assert_eq!(
			TokenApprovals::erc721_approvals((collection_id, series_id, serial_number)),
			H160::default()
		);
	})
}
