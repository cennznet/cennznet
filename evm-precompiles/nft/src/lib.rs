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
// along with CENNZnet. If not, see <http://www.gnu.org/licenses/>.

#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

use cennznet_primitives::types::{AssetId, CollectionId, SeriesId, TokenCount};
use crml_nft::{weights::WeightInfo, MetadataScheme, RoyaltiesSchedule};
use fp_evm::{Context, ExitSucceed, PrecompileOutput};
use frame_support::{dispatch::PostDispatchInfo, weights::GetDispatchInfo};
use pallet_evm::{AddressMapping, ExitRevert, GasWeightMapping, Precompile};
use pallet_evm_precompiles_erc20::Erc20IdConversion;
use precompile_utils::{
	error, Address, Bytes, EvmDataReader, EvmDataWriter, EvmResult, FunctionModifier, Gasometer, PrecompileFailure,
	RuntimeHelper,
};
use sp_core::{H160, U256};
use sp_runtime::traits::Dispatchable;
use sp_runtime::{Permill, SaturatedConversion};
use sp_std::marker::PhantomData;
use sp_std::{vec, vec::Vec};

#[precompile_utils::generate_function_selector]
#[derive(Debug, PartialEq)]
pub enum Action {
	/// Create a new NFT series
	/// collection_id, metadata_type, metadata_path, royalty_addresses, royalty_entitlements
	InitializeSeries = "initializeSeries(uint32,uint8,bytes,address[],uint32[])",
	/// Mint an NFT in a series
	/// collection_id, series_id, quantity, owner
	Mint = "mint(uint32,uint32,uint32,address)",
}

/// Provides access to the NFT pallet
pub struct NftPrecompile<T>(PhantomData<T>);

impl<T> Precompile for NftPrecompile<T>
where
	T: frame_system::Config
		+ crml_nft::Config
		+ pallet_evm::Config
		+ Erc20IdConversion<EvmId = Address, RuntimeId = AssetId>,
	T::Call: Dispatchable<PostInfo = PostDispatchInfo> + GetDispatchInfo,
	T::Call: From<crml_nft::Call<T>>,
	<T::Call as Dispatchable>::Origin: From<Option<T::AccountId>>,
{
	fn execute(
		input: &[u8],
		target_gas: Option<u64>,
		context: &Context,
		is_static: bool,
	) -> EvmResult<PrecompileOutput> {
		let mut gasometer = Gasometer::new(target_gas);
		let gasometer = &mut gasometer;

		let (mut input, selector) = match EvmDataReader::new_with_selector(gasometer, input) {
			Ok((input, selector)) => (input, selector),
			Err(err) => return Err(err),
		};
		let input = &mut input;

		if let Err(err) = gasometer.check_function_modifier(context, is_static, FunctionModifier::NonPayable) {
			return Err(err);
		}

		match selector {
			Action::InitializeSeries => Self::initialize_series(input, gasometer, &context.caller),
			Action::Mint => Self::mint(input, gasometer, &context.caller),
		}
	}
}

impl<T> NftPrecompile<T> {
	pub fn new() -> Self {
		Self(PhantomData)
	}
}

impl<T> NftPrecompile<T>
where
	T: frame_system::Config
		+ crml_nft::Config
		+ pallet_evm::Config
		+ Erc20IdConversion<EvmId = Address, RuntimeId = AssetId>,
	T::Call: Dispatchable<PostInfo = PostDispatchInfo> + GetDispatchInfo,
	T::Call: From<crml_nft::Call<T>>,
	<T::Call as Dispatchable>::Origin: From<Option<T::AccountId>>,
{
	fn initialize_series(
		input: &mut EvmDataReader,
		gasometer: &mut Gasometer,
		caller: &H160,
	) -> EvmResult<PrecompileOutput> {
		// Parse input.
		input.expect_arguments(gasometer, 5)?;

		let collection_id: U256 = input.read::<U256>(gasometer)?.into();
		if collection_id > CollectionId::MAX.into() {
			return Err(error("expected collection ID <= 2^32").into());
		}
		let collection_id: CollectionId = collection_id.saturated_into();

		let metadata_type: U256 = input.read::<U256>(gasometer)?.into();
		if metadata_type > u8::MAX.into() {
			return Err(error("Invalid metadata_type, expected u8").into());
		}
		let metadata_type: u8 = metadata_type.saturated_into();

		let metadata_path: Bytes = input.read::<Bytes>(gasometer)?.into();
		let metadata_path: Vec<u8> = metadata_path.as_bytes().to_vec();

		let metadata_scheme = MetadataScheme::from_index(metadata_type, metadata_path)
			.map_err(|_| error("Invalid metadata_type, expected u8 <= 3").into())?;

		let royalty_addresses: Vec<Address> = input.read::<Vec<Address>>(gasometer)?.into();
		let royalty_entitlements: Vec<U256> = input.read::<Vec<U256>>(gasometer)?.into();
		if royalty_addresses.len() != royalty_entitlements.len() {
			return Err(error("Royalty addresses and entitlements must be the same length").into());
		}
		let royalty_entitlements = royalty_entitlements.into_iter().map(|entitlement| {
			let entitlement: u32 = entitlement.saturated_into();
			Permill::from_parts(entitlement)
		});
		let royalties_schedule: Option<RoyaltiesSchedule<T::AccountId>> = if royalty_addresses.len() > 0 {
			let entitlements = royalty_addresses
				.into_iter()
				.map(|address| T::AddressMapping::into_account_id(address.into()))
				.zip(royalty_entitlements)
				.collect();
			Some(RoyaltiesSchedule { entitlements })
		} else {
			None
		};

		let origin = T::AddressMapping::into_account_id(*caller);
		gasometer.record_cost(<T as pallet_evm::Config>::GasWeightMapping::weight_to_gas(
			<T as crml_nft::Config>::WeightInfo::mint_series(0),
		))?;

		// Dispatch call (if enough gas).
		let series_id =
			crml_nft::Module::<T>::do_mint_series(origin, collection_id, 0, None, metadata_scheme, royalties_schedule);

		// Build output.
		match series_id {
			Ok(series_id) => Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				cost: gasometer.used_gas(),
				output: EvmDataWriter::new().write(U256::from(series_id)).build(),
				logs: Default::default(),
			}),
			Err(err) => Err(PrecompileFailure::Revert {
				exit_status: ExitRevert::Reverted,
				output: alloc::format!("Initialize series failed: {:?}", err.stripped())
					.as_bytes()
					.to_vec(),
				cost: gasometer.used_gas(),
			}),
		}
	}

	fn mint(input: &mut EvmDataReader, gasometer: &mut Gasometer, caller: &H160) -> EvmResult<PrecompileOutput> {
		gasometer.record_log_costs_manual(3, 32)?;

		// Parse input.
		input.expect_arguments(gasometer, 4)?;

		let collection_id: U256 = input.read::<U256>(gasometer)?.into();
		if collection_id > CollectionId::MAX.into() {
			return Err(error("expected collection ID <= 2^32").into());
		}
		let collection_id: CollectionId = collection_id.saturated_into();

		let series_id: U256 = input.read::<U256>(gasometer)?.into();
		if series_id > SeriesId::MAX.into() {
			return Err(error("expected series ID <= 2^32").into());
		}
		let series_id: SeriesId = series_id.saturated_into();

		let quantity: U256 = input.read::<U256>(gasometer)?.into();
		if quantity > TokenCount::MAX.into() {
			return Err(error("expected quantity <= 2^32").into());
		}
		let quantity: TokenCount = quantity.saturated_into();

		let owner: H160 = input.read::<Address>(gasometer)?.into();
		let owner = if owner == H160::default() {
			None
		} else {
			Some(T::AddressMapping::into_account_id(owner))
		};

		let origin = T::AddressMapping::into_account_id(*caller);

		// Dispatch call (if enough gas).
		RuntimeHelper::<T>::try_dispatch(
			Some(origin).into(),
			crml_nft::Call::<T>::mint_additional {
				collection_id,
				series_id,
				quantity,
				owner,
			},
			gasometer,
		)?;

		// Build output.
		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			cost: gasometer.used_gas(),
			output: EvmDataWriter::new().write(true).build(),
			logs: vec![],
		})
	}
}
