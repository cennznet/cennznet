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
use cennznet_runtime::{Nft, Runtime, TokenApprovals};
use crml_nft::MetadataScheme;
use crml_support::PrefixedAddressMapping;
use frame_support::assert_ok;
use pallet_evm_precompiles_erc721::{
	Action, Address, AddressMapping, Context, Erc721IdConversion, Erc721PrecompileSet, EvmDataWriter, PrecompileSet,
};

use sp_core::{H160, U256};

mod common;
use common::mock::ExtBuilder;

fn setup_nft_series(collection_owner: AccountId, token_owner: AccountId) -> (CollectionId, SeriesId, SerialNumber) {
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

#[test]
fn erc721_transfer_from() {
	ExtBuilder::default().initial_balance(1).build().execute_with(|| {
		let collection_owner_eth: H160 = b"test1000000000000000".into();
		let token_owner_eth: H160 = b"test2000000000000000".into();
		let new_owner_eth: H160 = b"test3000000000000000".into();
		let collection_owner: AccountId = PrefixedAddressMapping::into_account_id(collection_owner_eth.clone());
		let token_owner: AccountId = PrefixedAddressMapping::into_account_id(token_owner_eth.clone());
		let new_owner: AccountId = PrefixedAddressMapping::into_account_id(new_owner_eth.clone());

		let (collection_id, series_id, serial_number) = setup_nft_series(collection_owner, token_owner.clone());
		let serial_number_eth: U256 = serial_number.into();
		assert_eq!(
			Nft::token_owner((collection_id, series_id), serial_number),
			token_owner.clone()
		);

		let address: H160 = Runtime::runtime_id_to_evm_id((collection_id, series_id)).into();
		let context: Context = Context {
			address,
			caller: token_owner_eth,
			apparent_value: U256::default(),
		};

		// Write to input data
		let input_data = EvmDataWriter::new_with_selector(Action::TransferFrom)
			.write::<Address>(new_owner_eth.into())
			.write::<Address>(token_owner_eth.into())
			.write::<U256>(serial_number_eth)
			.build();

		let precompile_set = Erc721PrecompileSet::<Runtime>::new();
		assert_ok!(precompile_set
			.execute(
				address.into(),
				&input_data, //Build input data to convert to bytes
				None,
				&context,
				false,
			)
			.unwrap());
		// NFT changed ownership
		assert_eq!(Nft::token_owner((collection_id, series_id), serial_number), new_owner);
	})
}

#[test]
fn erc721_transfer_from_caller_not_approved_should_fail() {
	ExtBuilder::default().initial_balance(1).build().execute_with(|| {
		let collection_owner_eth: H160 = b"test1000000000000000".into();
		let token_owner_eth: H160 = b"test2000000000000000".into();
		let new_owner_eth: H160 = b"test3000000000000000".into();
		let collection_owner: AccountId = PrefixedAddressMapping::into_account_id(collection_owner_eth.clone());
		let token_owner: AccountId = PrefixedAddressMapping::into_account_id(token_owner_eth.clone());

		let (collection_id, series_id, serial_number) = setup_nft_series(collection_owner, token_owner.clone());
		let serial_number_eth: U256 = serial_number.into();
		assert_eq!(
			Nft::token_owner((collection_id, series_id), serial_number),
			token_owner.clone()
		);

		let address: H160 = Runtime::runtime_id_to_evm_id((collection_id, series_id)).into();
		let context: Context = Context {
			address,
			caller: new_owner_eth, //Not approved caller
			apparent_value: U256::default(),
		};

		// Write to input data
		let input_data = EvmDataWriter::new_with_selector(Action::TransferFrom)
			.write::<Address>(new_owner_eth.into()) //to
			.write::<Address>(token_owner_eth.into()) //from
			.write::<U256>(serial_number_eth)
			.build();

		let precompile_set = Erc721PrecompileSet::<Runtime>::new();

		assert!(precompile_set
			.execute(
				address.into(),
				&input_data, //Build input data to convert to bytes
				None,
				&context,
				false,
			)
			.unwrap()
			.is_err());

		// Ownership shouldn't have transferred
		assert_eq!(Nft::token_owner((collection_id, series_id), serial_number), token_owner);
	})
}

#[test]
fn erc721_approve_and_transfer() {
	ExtBuilder::default().initial_balance(1).build().execute_with(|| {
		let collection_owner_eth: H160 = b"test1000000000000000".into();
		let token_owner_eth: H160 = b"test2000000000000000".into();
		let approved_account_eth: H160 = b"test3000000000000000".into();
		let new_owner_eth: H160 = b"test4000000000000000".into();
		let collection_owner: AccountId = PrefixedAddressMapping::into_account_id(collection_owner_eth.clone());
		let token_owner: AccountId = PrefixedAddressMapping::into_account_id(token_owner_eth.clone());
		let approved_account: AccountId = PrefixedAddressMapping::into_account_id(approved_account_eth.clone());
		let new_owner: AccountId = PrefixedAddressMapping::into_account_id(new_owner_eth.clone());

		let (collection_id, series_id, serial_number) = setup_nft_series(collection_owner, token_owner.clone());
		let serial_number_eth: U256 = serial_number.into();
		assert_eq!(
			Nft::token_owner((collection_id, series_id), serial_number),
			token_owner.clone()
		);

		let address: H160 = Runtime::runtime_id_to_evm_id((collection_id, series_id)).into();
		let context: Context = Context {
			address,
			caller: token_owner_eth,
			apparent_value: U256::default(),
		};

		// Write to input data
		let input_data = EvmDataWriter::new_with_selector(Action::Approve)
			.write::<Address>(approved_account_eth.into()) //to
			.write::<Address>(token_owner_eth.into()) //from
			.write::<U256>(serial_number_eth)
			.build();

		//Set approval
		let precompile_set = Erc721PrecompileSet::<Runtime>::new();
		assert_ok!(precompile_set
			.execute(
				address.into(),
				&input_data, //Build input data to convert to bytes
				None,
				&context,
				false,
			)
			.unwrap());

		assert_eq!(
			TokenApprovals::erc721_approvals((collection_id, series_id, serial_number)),
			approved_account.clone()
		);

		// Transfer NFT from approved account
		let context: Context = Context {
			address,
			caller: approved_account_eth,
			apparent_value: U256::default(),
		};

		// Write to input data
		let input_data = EvmDataWriter::new_with_selector(Action::TransferFrom)
			.write::<Address>(new_owner_eth.into()) //to
			.write::<Address>(token_owner_eth.into()) //from
			.write::<U256>(serial_number_eth)
			.build();

		let precompile_set = Erc721PrecompileSet::<Runtime>::new();
		assert_ok!(precompile_set
			.execute(
				address.into(),
				&input_data, //Build input data to convert to bytes
				None,
				&context,
				false,
			)
			.unwrap());

		// NFT changed ownership
		assert_eq!(Nft::token_owner((collection_id, series_id), serial_number), new_owner);
		// Approval should be removed
		assert_eq!(
			TokenApprovals::erc721_approvals((collection_id, series_id, serial_number)),
			AccountId::default()
		);
	})
}

#[test]
fn erc721_approve_caller_not_from_should_fail() {
	ExtBuilder::default().initial_balance(1).build().execute_with(|| {
		let collection_owner_eth: H160 = b"test1000000000000000".into();
		let token_owner_eth: H160 = b"test2000000000000000".into();
		let approved_account_eth: H160 = b"test3000000000000000".into();
		let collection_owner: AccountId = PrefixedAddressMapping::into_account_id(collection_owner_eth.clone());
		let token_owner: AccountId = PrefixedAddressMapping::into_account_id(token_owner_eth.clone());

		let (collection_id, series_id, serial_number) = setup_nft_series(collection_owner, token_owner.clone());
		let serial_number_eth: U256 = serial_number.into();
		assert_eq!(
			Nft::token_owner((collection_id, series_id), serial_number),
			token_owner.clone()
		);

		let address: H160 = Runtime::runtime_id_to_evm_id((collection_id, series_id)).into();
		let context: Context = Context {
			address,
			caller: approved_account_eth,
			apparent_value: U256::default(),
		};

		// Write to input data
		let input_data = EvmDataWriter::new_with_selector(Action::Approve)
			.write::<Address>(approved_account_eth.into()) //to
			.write::<Address>(token_owner_eth.into()) //from
			.write::<U256>(serial_number_eth)
			.build();

		//Set approval
		let precompile_set = Erc721PrecompileSet::<Runtime>::new();
		assert!(precompile_set
			.execute(
				address.into(),
				&input_data, //Build input data to convert to bytes
				None,
				&context,
				false,
			)
			.unwrap()
			.is_err());

		assert_eq!(
			TokenApprovals::erc721_approvals((collection_id, series_id, serial_number)),
			AccountId::default()
		);
	})
}

#[test]
fn erc721_approve_caller_not_token_owner_should_fail() {
	ExtBuilder::default().initial_balance(1).build().execute_with(|| {
		let collection_owner_eth: H160 = b"test1000000000000000".into();
		let token_owner_eth: H160 = b"test2000000000000000".into();
		let approved_account_eth: H160 = b"test3000000000000000".into();
		let new_owner_eth: H160 = b"test4000000000000000".into();

		let collection_owner: AccountId = PrefixedAddressMapping::into_account_id(collection_owner_eth.clone());
		let token_owner: AccountId = PrefixedAddressMapping::into_account_id(token_owner_eth.clone());

		let (collection_id, series_id, serial_number) = setup_nft_series(collection_owner, token_owner.clone());
		let serial_number_eth: U256 = serial_number.into();
		assert_eq!(
			Nft::token_owner((collection_id, series_id), serial_number),
			token_owner.clone()
		);

		let address: H160 = Runtime::runtime_id_to_evm_id((collection_id, series_id)).into();
		let context: Context = Context {
			address,
			caller: approved_account_eth,
			apparent_value: U256::default(),
		};

		// Write to input data
		let input_data = EvmDataWriter::new_with_selector(Action::Approve)
			.write::<Address>(new_owner_eth.into()) //to
			.write::<Address>(approved_account_eth.into()) //from
			.write::<U256>(serial_number_eth)
			.build();

		//Set approval
		let precompile_set = Erc721PrecompileSet::<Runtime>::new();
		assert!(precompile_set
			.execute(
				address.into(),
				&input_data, //Build input data to convert to bytes
				None,
				&context,
				false,
			)
			.unwrap()
			.is_err());

		assert_eq!(
			TokenApprovals::erc721_approvals((collection_id, series_id, serial_number)),
			AccountId::default()
		);
	})
}
