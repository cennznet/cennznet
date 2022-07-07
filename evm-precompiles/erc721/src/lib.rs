// Copyright 2019-2022 Centrality Investments Ltd.
// This file is part of CENNZnet.

// CENNZnet is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CENNZnet is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CENNZnet.  If not, see <http://www.gnu.org/licenses/>.

#![cfg_attr(not(feature = "std"), no_std)]

use fp_evm::{Context, ExitSucceed, PrecompileHandle, PrecompileOutput};
use frame_support::{
	dispatch::{Dispatchable, GetDispatchInfo, PostDispatchInfo},
	traits::OriginTrait,
};
use pallet_evm::{AddressMapping, PrecompileSet};
use sp_core::{H160, H256, U256};
use sp_runtime::traits::SaturatedConversion;
use sp_std::{marker::PhantomData, vec};

use cennznet_primitives::types::{CollectionId, SerialNumber, SeriesId, TokenId};
pub use precompile_utils::{
	error, keccak256, revert, Address, AddressMappingReversibleExt, Bytes, EvmData, EvmDataReader, EvmDataWriter,
	EvmResult, FunctionModifier, PrecompileHandleExt, RuntimeHelper,
};

/// Solidity selector of the Transfer log, which is the Keccak of the Log signature.
pub const SELECTOR_LOG_TRANSFER: [u8; 32] = keccak256!("Transfer(address,address,uint256)");

/// Solidity selector of the Transfer log, which is the Keccak of the Log signature.
pub const SELECTOR_LOG_APPROVAL: [u8; 32] = keccak256!("Approval(address,address,uint256)");

#[precompile_utils::generate_function_selector]
#[derive(Debug, PartialEq)]
pub enum Action {
	BalanceOf = "balanceOf(address)",
	OwnerOf = "ownerOf(uint256)",
	TransferFrom = "transferFrom(address,address,uint256)",
	SafeTransferFrom = "safeTransferFrom(address,address,uint256)",
	SafeTransferFromCallData = "safeTransferFrom(address,address,uint256,bytes)",
	Approve = "approve(address,uint256)",
	GetApproved = "getApproved(uint256)",
	IsApprovedForAll = "isApprovedForAll(address,address)",
	SetApprovalForAll = "setApprovalForAll(address,bool)",
	// Metadata extensions
	Name = "name()",
	Symbol = "symbol()",
	TokenURI = "tokenURI(uint256)",
}

/// Convert EVM addresses into NFT module identifiers and vice versa
pub trait Erc721IdConversion {
	/// ID type used by EVM
	type EvmId;
	/// ID type used by runtime
	type RuntimeId;
	// Get runtime Id from EVM id
	fn evm_id_to_runtime_id(evm_id: Self::EvmId) -> Option<Self::RuntimeId>;
	// Get EVM id from runtime Id
	fn runtime_id_to_evm_id(runtime_id: Self::RuntimeId) -> Self::EvmId;
}

/// Calls to contracts starting with this prefix will be shim'd to the CENNZnet NFT module
/// via an ERC721 compliant interface (`Erc721PrecompileSet`)
pub const ERC721_PRECOMPILE_ADDRESS_PREFIX: &[u8] = &[0xAA; 4];

/// The following distribution has been decided for the precompiles
/// 0-1023: Ethereum Mainnet Precompiles
/// 1024-2047 Precompiles that are not in Ethereum Mainnet but are neither CENNZnet specific
/// 2048-4095 CENNZnet specific precompiles
/// NFT precompile addresses can only fall between
/// 	0xAAAAAAAA00000000000000000000000000000000 - 0xAAAAAAAAFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF
/// The precompile for NFT series (X,Y) where X & Y are a u32 (i.e.8 bytes), if 0XFFFFFFFF + Bytes(CollectionId) + Bytes(SeriesId)
/// In order to route the address to Erc721Precompile<R>, we check whether the CollectionId + SeriesId
/// exist in crml-nft pallet

/// This means that every address that starts with 0xAAAAAAAA will go through an additional db read,
/// but the probability for this to happen is 2^-32 for random addresses
pub struct Erc721PrecompileSet<Runtime>(PhantomData<Runtime>);

impl<Runtime> PrecompileSet for Erc721PrecompileSet<Runtime>
where
	Runtime::AccountId: Into<[u8; 32]>,
	Runtime: crml_nft::Config + pallet_evm::Config + frame_system::Config + crml_token_approvals::Config,
	Runtime::AddressMapping: AddressMappingReversibleExt<Runtime::AccountId>,
	Runtime::Call: Dispatchable<PostInfo = PostDispatchInfo> + GetDispatchInfo,
	Runtime::Call: From<crml_nft::Call<Runtime>> + From<crml_token_approvals::Call<Runtime>>,
	<Runtime::Call as Dispatchable>::Origin: From<Option<Runtime::AccountId>>,
	Runtime: Erc721IdConversion<RuntimeId = (CollectionId, SeriesId), EvmId = Address>,
	<<Runtime as frame_system::Config>::Call as Dispatchable>::Origin: OriginTrait,
{
	fn execute(&self, handle: &mut impl PrecompileHandle) -> Option<EvmResult<PrecompileOutput>> {
		// Convert target `address` into it's runtime NFT Id
		if let Some((collection_id, series_id)) = Runtime::evm_id_to_runtime_id(Address(address)) {
			// 'collection name' is empty when the collection doesn't exist yet
			if !crml_nft::Pallet::<Runtime>::collection_name(collection_id).is_empty() {
				let result = {
					let selector = match handle.read_selector() {
						Ok(selector) => selector,
						Err(e) => return Some(Err(e)),
					};

					if let Err(err) = handle.check_function_modifier(match selector {
						Action::Approve
						| Action::SafeTransferFrom
						| Action::TransferFrom
						| Action::SafeTransferFromCallData => FunctionModifier::NonPayable,
						_ => FunctionModifier::View,
					}) {
						return Some(Err(err));
					}

					let series_id_parts = (collection_id, series_id);
					match selector {
						Action::OwnerOf => Self::owner_of(series_id_parts, handle),
						Action::BalanceOf => Self::balance_of(series_id_parts, handle),
						Action::TransferFrom => Self::transfer_from(series_id_parts, handle),
						Action::Name => Self::name(series_id_parts, handle),
						Action::Symbol => Self::symbol(series_id_parts, handle),
						Action::TokenURI => Self::token_uri(series_id_parts, handle),
						Action::Approve => Self::approve(series_id_parts, handle),
						Action::GetApproved => Self::get_approved(series_id_parts, handle),
						Action::SafeTransferFrom
						| Action::SafeTransferFromCallData
						| Action::IsApprovedForAll
						| Action::SetApprovalForAll => {
							return Some(Err(error("function not implemented yet").into()));
						}
					}
				};
				return Some(result);
			}
		}
		None
	}

	fn is_precompile(&self, address: H160) -> bool {
		if let Some((collection_id, series_id)) = Runtime::evm_id_to_runtime_id(Address(address)) {
			// route to NFT module only if the (collection, series) exists
			crml_nft::Pallet::<Runtime>::series_exists(collection_id, series_id)
		} else {
			false
		}
	}
}

impl<Runtime> Erc721PrecompileSet<Runtime> {
	pub fn new() -> Self {
		Self(PhantomData)
	}
}

impl<Runtime> Erc721PrecompileSet<Runtime>
where
	Runtime::AccountId: Into<[u8; 32]>,
	Runtime: crml_nft::Config + pallet_evm::Config + frame_system::Config + crml_token_approvals::Config,
	Runtime::AddressMapping: AddressMappingReversibleExt<Runtime::AccountId>,
	Runtime::Call: Dispatchable<PostInfo = PostDispatchInfo> + GetDispatchInfo,
	Runtime::Call: From<crml_nft::Call<Runtime>> + From<crml_token_approvals::Call<Runtime>>,
	<Runtime::Call as Dispatchable>::Origin: From<Option<Runtime::AccountId>>,
	Runtime: Erc721IdConversion<RuntimeId = (CollectionId, SeriesId), EvmId = Address>,
	<<Runtime as frame_system::Config>::Call as Dispatchable>::Origin: OriginTrait,
{
	/// Returns the CENNZnet address which owns the given token
	/// The zero address is returned if it is unowned or does not exist
	/// ss58 `5C4hrfjw9DjXZTzV3MwzrrAr9P1MJhSrvWGWqi1eSuyUpnhM`
	fn owner_of(
		series_id_parts: (CollectionId, SeriesId),
		handle: &mut impl PrecompileHandle,
	) -> EvmResult<PrecompileOutput> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		// Parse input.
		let mut input = handle.read_input()?;
		input.expect_arguments(1)?;
		let serial_number: U256 = input.read::<U256>()?;

		// For now we only support Ids < u32 max
		// since `u32` is the native `SerialNumber` type used by the NFT module.
		// it's not possible for the module to issue Ids larger than this
		if serial_number > u32::max_value().into() {
			return Err(error("expected token id <= 2^32").into());
		}
		let serial_number: SerialNumber = serial_number.saturated_into();

		// Fetch info.
		let owner_account_id =
			H256::from(crml_nft::Pallet::<Runtime>::token_owner(series_id_parts, serial_number).into());

		// Build output.
		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			output: EvmDataWriter::new().write(owner_account_id).build(),
		})
	}

	fn balance_of(
		series_id_parts: (CollectionId, SeriesId),
		handle: &mut impl PrecompileHandle,
	) -> EvmResult<PrecompileOutput> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		// Read input.
		let mut input = handle.read_input()?;
		input.expect_arguments(1)?;

		let owner: H160 = input.read::<Address>()?.into();

		// Fetch info.
		let amount: U256 = {
			let owner: Runtime::AccountId = Runtime::AddressMapping::into_account_id(owner);
			(*crml_nft::Pallet::<Runtime>::token_balance(&owner)
				.get(&series_id_parts)
				.unwrap_or(&0))
			.into()
		};

		// Build output.
		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			output: EvmDataWriter::new().write(amount).build(),
		})
	}

	fn transfer_from(
		series_id_parts: (CollectionId, SeriesId),
		handle: &mut impl PrecompileHandle,
	) -> EvmResult<PrecompileOutput> {
		handle.record_log_costs_manual(3, 32)?;

		// Parse input.
		let mut input = handle.read_input()?;
		input.expect_arguments(3)?;

		let to: H160 = input.read::<Address>()?.into();
		let from: H160 = input.read::<Address>()?.into();
		let serial_number = input.read::<U256>()?;

		// For now we only support Ids < u32 max
		// since `u32` is the native `SerialNumber` type used by the NFT module.
		// it's not possible for the module to issue Ids larger than this
		if serial_number > u32::max_value().into() {
			return Err(error("expected token id <= 2^32").into());
		}
		let serial_number: SerialNumber = serial_number.saturated_into();
		let token_id = (series_id_parts.0, series_id_parts.1, serial_number);
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let approved_account: H160 = crml_token_approvals::Module::<Runtime>::erc721_approvals(token_id);

		// Build call with origin.
		if context.caller == from || context.caller == approved_account {
			let from = Runtime::AddressMapping::into_account_id(from);
			let to = Runtime::AddressMapping::into_account_id(to);

			// Dispatch call (if enough gas).
			RuntimeHelper::<Runtime>::try_dispatch(
				handle,
				Some(from).into(),
				crml_nft::Call::<Runtime>::transfer {
					token_id,
					new_owner: to,
				},
				gasometer,
			)?;
		} else {
			return Err(error("caller not approved").into());
		}

		log3(
			SELECTOR_LOG_TRANSFER,
			context.caller,
			to,
			EvmDataWriter::new().write(serial_number).build(),
		)
		.record(handle)?;

		// Build output.
		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			output: EvmDataWriter::new().write(true).build(),
		})
	}

	fn approve(
		series_id_parts: (CollectionId, SeriesId),
		handle: &mut impl PrecompileHandle,
	) -> EvmResult<PrecompileOutput> {
		handle.record_log_costs_manual(3, 32)?;

		// Parse input.
		let mut input = handle.read_input()?;
		input.expect_arguments(3)?;

		let to: H160 = input.read::<Address>()?.into();
		let from: H160 = input.read::<Address>()?.into();
		let serial_number = input.read::<U256>()?;

		// For now we only support Ids < u32 max
		// since `u32` is the native `SerialNumber` type used by the NFT module.
		// it's not possible for the module to issue Ids larger than this
		if serial_number > u32::max_value().into() {
			return Err(error("expected token id <= 2^32").into());
		}
		let serial_number: SerialNumber = serial_number.saturated_into();

		if context.caller == from {
			let token_id: TokenId = (series_id_parts.0, series_id_parts.1, serial_number);
			// Dispatch call (if enough gas).
			RuntimeHelper::<Runtime>::try_dispatch(
				handle,
				None.into(),
				crml_token_approvals::Call::<Runtime>::erc721_approval {
					caller: from,
					operator_account: to,
					token_id,
				},
				gasometer,
			)?;
		} else {
			return Err(error("caller must be from").into());
		};

		log3(
			SELECTOR_LOG_APPROVAL,
			context.caller,
			to,
			EvmDataWriter::new().write(serial_number).build(),
		)
		.record(handle)?;

		// Build output.
		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			output: EvmDataWriter::new().write(true).build(),
		})
	}

	fn get_approved(
		series_id_parts: (CollectionId, SeriesId),
		handle: &mut impl PrecompileHandle,
	) -> EvmResult<PrecompileOutput> {
		handle.record_log_costs_manual(3, 32)?;

		// Parse input.
		let mut input = handle.read_input()?;
		input.expect_arguments(1)?;
		let serial_number = input.read::<U256>()?;

		// For now we only support Ids < u32 max
		// since `u32` is the native `SerialNumber` type used by the NFT module.
		// it's not possible for the module to issue Ids larger than this
		if serial_number > u32::max_value().into() {
			return Err(error("expected token id <= 2^32").into());
		}
		let serial_number: SerialNumber = serial_number.saturated_into();
		let approved_account = crml_token_approvals::Module::<Runtime>::erc721_approvals((
			series_id_parts.0,
			series_id_parts.1,
			serial_number,
		));
		// Build output.
		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			output: EvmDataWriter::new()
				.write::<Bytes>(approved_account.as_bytes().into())
				.build(),
		})
	}

	fn name(
		series_id_parts: (CollectionId, SeriesId),
		handle: &mut impl PrecompileHandle,
	) -> EvmResult<PrecompileOutput> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		// Build output.
		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			output: EvmDataWriter::new()
				.write::<Bytes>(
					crml_nft::Pallet::<Runtime>::series_name(series_id_parts)
						.as_slice()
						.into(),
				)
				.build(),
		})
	}

	fn symbol(
		series_id_parts: (CollectionId, SeriesId),
		handle: &mut impl PrecompileHandle,
	) -> EvmResult<PrecompileOutput> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		// Build output.
		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			output: EvmDataWriter::new()
				.write::<Bytes>(
					// TODO: returns same as `name`
					crml_nft::Pallet::<Runtime>::series_name(series_id_parts)
						.as_slice()
						.into(),
				)
				.build(),
		})
	}

	fn token_uri(
		series_id_parts: (CollectionId, SeriesId),
		handle: &mut impl PrecompileHandle,
	) -> EvmResult<PrecompileOutput> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		let mut input = handle.read_input()?;
		input.expect_arguments(1)?;
		let serial_number = input.read::<U256>()?;

		// For now we only support Ids < u32 max
		// since `u32` is the native `SerialNumber` type used by the NFT module.
		// it's not possible for the module to issue Ids larger than this
		if serial_number > u32::max_value().into() {
			return Err(error("expected token id <= 2^32").into());
		}
		let serial_number: SerialNumber = serial_number.saturated_into();

		// Build output.
		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			output: EvmDataWriter::new()
				.write::<Bytes>(
					crml_nft::Pallet::<Runtime>::token_uri((series_id_parts.0, series_id_parts.1, serial_number))
						.as_slice()
						.into(),
				)
				.build(),
		})
	}
}
