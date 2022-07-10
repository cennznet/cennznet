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

use fp_evm::{ExitSucceed, PrecompileFailure, PrecompileHandle, PrecompileOutput};
use pallet_evm::{AddressMapping, ExitRevert, GasWeightMapping, Precompile};
use sp_core::U256;
use sp_runtime::SaturatedConversion;
use sp_std::marker::PhantomData;

use cennznet_primitives::{
	traits::BuyFeeAsset,
	types::{AccountId, AssetId, Balance, FeeExchange, FeeExchangeV1},
};
use pallet_evm_precompiles_erc20::Erc20IdConversion;
use precompile_utils::prelude::*;

#[precompile_utils::generate_function_selector]
#[derive(Debug, PartialEq)]
pub enum Action {
	/// Swap some input asset for some exact CPAY amount, defining a limit on the max. input
	/// (assetIn, exactCpayOut, maxAssetIn)
	SwapForExactCPAY = "swapForExactCPAY(address,uint128,uint256)",
}

/// Provides access to the state oracle pallet
pub struct CennzxPrecompile<T, U, G, C>(PhantomData<(T, U, G, C)>);

impl<T, U, G, C> Precompile for CennzxPrecompile<T, U, G, C>
where
	T: BuyFeeAsset<AccountId = AccountId, Balance = Balance, FeeExchange = FeeExchange<AssetId, Balance>>,
	U: AddressMapping<AccountId>,
	G: GasWeightMapping,
	C: Erc20IdConversion<EvmId = Address, RuntimeId = AssetId>,
{
	fn execute(handle: &mut impl PrecompileHandle) -> EvmResult<PrecompileOutput> {
		let selector = match handle.read_selector() {
			Ok(selector) => selector,
			Err(e) => return Err(e),
		};

		if let Err(err) = handle.check_function_modifier(match selector {
			Action::SwapForExactCPAY => FunctionModifier::NonPayable,
		}) {
			return Err(err);
		}

		match selector {
			Action::SwapForExactCPAY => Self::swap_for_exact_cpay(handle),
		}
	}
}

impl<T, U, G, C> CennzxPrecompile<T, U, G, C>
where
	T: BuyFeeAsset<AccountId = AccountId, Balance = Balance, FeeExchange = FeeExchange<AssetId, Balance>>,
	U: AddressMapping<AccountId>,
	G: GasWeightMapping,
	C: Erc20IdConversion<EvmId = Address, RuntimeId = AssetId>,
{
	fn swap_for_exact_cpay(handle: &mut impl PrecompileHandle) -> EvmResult<PrecompileOutput> {
		let mut input = handle.read_input()?;
		input.expect_arguments(3)?;

		let input_asset: Address = input.read::<Address>()?.into();
		// the given `input_asset` address is not a valid (derived) generic asset address
		// it is not supported by cennzx
		let asset_id = C::evm_id_to_runtime_id(input_asset).ok_or(revert("unsupported asset"))?;
		// in CPAY units
		let exact_output: U256 = input.read::<U256>()?.into();
		// in ASSET units
		let max_input: U256 = input.read::<U256>()?.into();

		let fee_exchange = FeeExchange::V1(FeeExchangeV1::<AssetId, Balance> {
			asset_id,
			max_payment: max_input.saturated_into(),
		});

		let caller = U::into_account_id(handle.context().caller);

		handle.record_cost(G::weight_to_gas(T::buy_fee_weight()))?;
		let _ = T::buy_fee_asset(&caller, exact_output.saturated_into(), &fee_exchange).map_err(|err| {
			PrecompileFailure::Revert {
				exit_status: ExitRevert::Reverted,
				output: alloc::format!("swap failed: {:?}", err.stripped()).as_bytes().to_vec(),
			}
		})?;

		// Build output.
		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			output: Default::default(),
		})
	}
}
