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

use cennznet_primitives::{
	traits::BuyFeeAsset,
	types::{AccountId, AssetId, Balance, FeeExchange, FeeExchangeV1},
};
use fp_evm::{Context, ExitSucceed, PrecompileFailure, PrecompileOutput};
use pallet_evm::{AddressMapping, ExitRevert, GasWeightMapping, Precompile};
use pallet_evm_precompiles_erc20::Erc20IdConversion;
use precompile_utils::{Address, EvmDataReader, EvmResult, FunctionModifier, Gasometer};
use sp_core::{H160, U256};
use sp_runtime::SaturatedConversion;
use sp_std::marker::PhantomData;

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

		if let Err(err) = gasometer.check_function_modifier(
			context,
			is_static,
			match selector {
				Action::SwapForExactCPAY => FunctionModifier::NonPayable,
			},
		) {
			return Err(err);
		}

		match selector {
			Action::SwapForExactCPAY => Self::swap_for_exact_cpay(input, gasometer, &context.caller),
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
	fn swap_for_exact_cpay(
		input: &mut EvmDataReader,
		gasometer: &mut Gasometer,
		caller: &H160,
	) -> EvmResult<PrecompileOutput> {
		input.expect_arguments(gasometer, 3)?;

		let input_asset: Address = input.read::<Address>(gasometer)?.into();
		// the given `input_asset` address is not a valid (derived) generic asset address
		// it is not supported by cennzx
		let asset_id = C::evm_id_to_runtime_id(input_asset).ok_or(gasometer.revert("unsupported asset"))?;
		// in CPAY units
		let exact_output: U256 = input.read::<U256>(gasometer)?.into();
		// in ASSET units
		let max_input: U256 = input.read::<U256>(gasometer)?.into();

		let fee_exchange = FeeExchange::V1(FeeExchangeV1::<AssetId, Balance> {
			asset_id,
			max_payment: max_input.saturated_into(),
		});

		let caller = U::into_account_id(*caller);

		gasometer.record_cost(G::weight_to_gas(T::buy_fee_weight()))?;
		let _ = T::buy_fee_asset(&caller, exact_output.saturated_into(), &fee_exchange).map_err(|err| {
			PrecompileFailure::Revert {
				exit_status: ExitRevert::Reverted,
				output: alloc::format!("swap failed: {:?}", err.stripped()).as_bytes().to_vec(),
				cost: 0_u64,
			}
		})?;

		// Build output.
		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			cost: gasometer.used_gas(),
			output: Default::default(),
			logs: Default::default(),
		})
	}
}
