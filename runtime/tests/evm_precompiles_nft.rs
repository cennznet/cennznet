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

//! EVM NFT integration tests

use cennznet_primitives::types::{CollectionId, SeriesId, TokenCount};
use cennznet_runtime::{
	constants::{asset::*, currency::*, evm::*},
	AddressMappingOf, GenericAsset, Nft, Origin, Runtime, CENNZNET_EVM_CONFIG,
};
use crml_nft::{MetadataScheme, RoyaltiesSchedule};
use crml_support::MultiCurrency;
use crml_support::{H160, U256};
use ethabi::Token;
use fp_rpc::runtime_decl_for_EthereumRuntimeRPCApi::EthereumRuntimeRPCApi;
use frame_support::assert_ok;
use hex_literal::hex;
use pallet_evm::{AddressMapping, Runner as RunnerT};

mod common;
use common::mock::ExtBuilder;
use sp_runtime::Permill;
use std::collections::BTreeMap;

fn encode_initialize_series_input(
	collection_id: CollectionId,
	metadata_scheme: MetadataScheme,
	royalty_addresses: Vec<H160>,
	royalty_entitlements: Vec<u32>,
) -> Vec<u8> {
	// keccak('initializeSeries(uint32,uint8,bytes,address[],uint32[])')[..4]
	let selector = [0x03, 0x0a, 0x69, 0x02];

	let (metadata_type, metadata_path) = match metadata_scheme {
		MetadataScheme::Https(path) => (0_u8, path),
		MetadataScheme::Http(path) => (1_u8, path),
		MetadataScheme::IpfsDir(path) => (2_u8, path),
		MetadataScheme::IpfsShared(path) => (3_u8, path),
	};

	let royalty_addresses: Vec<Token> = royalty_addresses.iter().map(|x| Token::Address(*x)).collect();
	let royalty_entitlements: Vec<Token> = royalty_entitlements
		.iter()
		.map(|x| Token::Uint(U256::from(*x)))
		.collect();

	let parameters = ethabi::encode(&[
		Token::Uint(U256::from(collection_id)),
		Token::Uint(U256::from(metadata_type)),
		Token::Bytes(metadata_path),
		Token::Array(royalty_addresses.clone()),
		Token::Array(royalty_entitlements.clone()),
	]);

	let input_length = 4_usize + parameters.as_slice().len();
	let mut input = vec![0_u8; input_length];
	input[..4].copy_from_slice(&selector);
	input[4..].copy_from_slice(parameters.as_slice());
	input.clone()
}

fn encode_mint_input(collection_id: CollectionId, series_id: SeriesId, quantity: TokenCount, owner: H160) -> Vec<u8> {
	// keccak('mint(uint32,uint32,uint32,address')[..4]
	let selector = [0xa0, 0x66, 0xf8, 0xd8];

	let parameters = ethabi::encode(&[
		Token::Uint(U256::from(collection_id)),
		Token::Uint(U256::from(series_id)),
		Token::Uint(U256::from(quantity)),
		Token::Address(owner),
	]);

	let input_length = 4_usize + parameters.as_slice().len();
	let mut input = vec![0_u8; input_length];
	input[..4].copy_from_slice(&selector);
	input[4..].copy_from_slice(parameters.as_slice());
	input.clone()
}

#[test]
fn initialize_series() {
	ExtBuilder::default()
		.initial_balance(1_000_000 * DOLLARS)
		.build()
		.execute_with(|| {
			// give caller some CPAY to fund the swap
			let caller: H160 = hex!("7a107Fc1794f505Cb351148F529AcCae12fFbcD8").into();
			let caller_ss58 = AddressMappingOf::<Runtime>::into_account_id(caller);
			let initial_cpay_balance = 50000 * DOLLARS;
			let _ = GenericAsset::deposit_creating(&caller_ss58, CPAY_ASSET_ID, initial_cpay_balance);

			// setup call to the nft precompile
			let collection_id = Nft::next_collection_id();
			let collection_name = b"test-evm-collection".to_vec();
			let metadata_scheme = MetadataScheme::Https(b"example.com/metadata".to_vec());
			let royalty_addresses: Vec<H160> = vec![hex!("420aC537F1a4f78d4Dfb3A71e902be0E3d480AFB").into()];
			let royalty_entitlements: Vec<u32> = vec![100];
			let royalties_schedule = RoyaltiesSchedule {
				entitlements: vec![(
					AddressMappingOf::<Runtime>::into_account_id(royalty_addresses[0]),
					Permill::from_parts(royalty_entitlements[0]),
				)],
			};
			assert_ok!(Nft::create_collection(
				Origin::signed(caller_ss58),
				collection_name,
				None
			));
			let series_id = Nft::next_series_id(collection_id);

			let input = encode_initialize_series_input(
				collection_id,
				metadata_scheme.clone(),
				royalty_addresses,
				royalty_entitlements,
			);
			let gas_limit = 1_000_000_000_u64;
			let max_fee_per_gas = Runtime::gas_price();
			let max_priority_fee_per_gas = U256::zero();

			// Test
			assert_ok!(<Runtime as pallet_evm::Config>::Runner::call(
				caller,
				H160::from_low_u64_be(NFT_PRECOMPILE),
				input,
				U256::zero(),
				gas_limit,
				Some(max_fee_per_gas),
				Some(max_priority_fee_per_gas),
				None,
				Default::default(),
				&CENNZNET_EVM_CONFIG
			));

			// Initial issuance of NFT series should be 0
			assert_eq!(Nft::series_issuance(collection_id, series_id), 0);
			// Series Id should have been incremented
			assert_eq!(Nft::next_series_id(collection_id), series_id + 1);
			// Royalties schedule should be correct
			assert_eq!(
				Nft::series_royalties(collection_id, series_id),
				Some(royalties_schedule)
			);
			// Metadata scheme should be correct
			assert_eq!(
				Nft::series_metadata_scheme(collection_id, series_id),
				Some(metadata_scheme)
			);
		});
}

#[test]
fn mint() {
	ExtBuilder::default()
		.initial_balance(1_000_000 * DOLLARS)
		.build()
		.execute_with(|| {
			// give caller some CPAY to fund the swap
			let caller: H160 = hex!("7a107Fc1794f505Cb351148F529AcCae12fFbcD8").into();
			let caller_ss58 = AddressMappingOf::<Runtime>::into_account_id(caller);
			let initial_cpay_balance = 500 * DOLLARS;
			let _ = GenericAsset::deposit_creating(&caller_ss58, CPAY_ASSET_ID, initial_cpay_balance);

			// setup Collection
			let collection_id = Nft::next_collection_id();
			let collection_name = b"test-evm-collection".to_vec();
			let metadata_scheme = MetadataScheme::Https(b"example.com/metadata".to_vec());
			assert_ok!(Nft::create_collection(
				Origin::signed(caller_ss58.clone()),
				collection_name,
				None
			));
			let series_id = Nft::next_series_id(collection_id);
			let royalties_schedule = RoyaltiesSchedule {
				entitlements: vec![(caller_ss58.clone(), Permill::one())],
			};

			// mint series with 0 tokens
			assert_ok!(Nft::mint_series(
				Some(caller_ss58.clone()).into(),
				collection_id,
				0,
				None,
				metadata_scheme.clone(),
				Some(royalties_schedule.clone()),
			));

			// Setup call for EVM Precompile
			let mint_quantity: TokenCount = 5;
			let input = encode_mint_input(collection_id, series_id, mint_quantity, caller);
			let gas_limit = 1_000_000_u64;
			let max_fee_per_gas = Runtime::gas_price();
			let max_priority_fee_per_gas = U256::zero();

			// Test
			assert_ok!(<Runtime as pallet_evm::Config>::Runner::call(
				caller,
				H160::from_low_u64_be(NFT_PRECOMPILE),
				input,
				U256::zero(),
				gas_limit,
				Some(max_fee_per_gas),
				Some(max_priority_fee_per_gas),
				None,
				Default::default(),
				&CENNZNET_EVM_CONFIG
			));

			// Initial issuance of NFT series should be 0
			assert_eq!(Nft::series_issuance(collection_id, series_id), mint_quantity);
			// Check token balance is correct
			let mut owner_map = BTreeMap::new();
			owner_map.insert((collection_id, series_id), mint_quantity);
			assert_eq!(Nft::token_balance(caller_ss58), owner_map);
		});
}
