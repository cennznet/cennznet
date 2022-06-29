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

use cennznet_primitives::types::AssetId;
use crml_erc20_peg::types::WithdrawCallOrigin;
use fp_evm::{Context, ExitSucceed, PrecompileOutput};
use pallet_evm::{AddressMapping, ExitRevert, Precompile};
use pallet_evm_precompiles_erc20::Erc20IdConversion;
use precompile_utils::{
	Address, EvmDataReader, EvmDataWriter, EvmResult, FunctionModifier, Gasometer, PrecompileFailure, RuntimeHelper,
};
use sp_core::{H160, U256};
use sp_runtime::traits::UniqueSaturatedInto;
use sp_std::marker::PhantomData;

#[precompile_utils::generate_function_selector]
#[derive(Debug, PartialEq)]
pub enum Action {
	/// Submit a withdrawal through erc20-peg
	/// asset id, withdraw amount, beneficiary
	Withdraw = "withdraw(address,uint256,address)",
}

/// Provides access to the state oracle pallet
pub struct Erc20PegPrecompile<T>(PhantomData<T>);

impl<T> Precompile for Erc20PegPrecompile<T>
where
	T: frame_system::Config
		+ crml_erc20_peg::Config
		+ pallet_evm::Config
		+ Erc20IdConversion<EvmId = Address, RuntimeId = AssetId>,
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
			Action::Withdraw => Self::withdraw(input, gasometer, &context.caller),
		}
	}
}

impl<T> Erc20PegPrecompile<T> {
	pub fn new() -> Self {
		Self(PhantomData)
	}
}

impl<T> Erc20PegPrecompile<T>
where
	T: frame_system::Config
		+ crml_erc20_peg::Config
		+ pallet_evm::Config
		+ Erc20IdConversion<EvmId = Address, RuntimeId = AssetId>,
{
	/// erc20-peg withdrawal
	fn withdraw(input: &mut EvmDataReader, gasometer: &mut Gasometer, caller: &H160) -> EvmResult<PrecompileOutput> {
		// Parse input.
		input.expect_arguments(gasometer, 3)?;
		let withdraw_asset: Address = input.read::<Address>(gasometer)?.into();
		// the given `input_asset` address is not a valid (derived) generic asset address
		let asset_id = T::evm_id_to_runtime_id(withdraw_asset).ok_or(gasometer.revert("unsupported asset"))?;
		let withdraw_amount: U256 = input.read::<U256>(gasometer)?.into();
		let beneficiary: H160 = input.read::<Address>(gasometer)?.into();

		gasometer.record_cost(RuntimeHelper::<T>::db_read_gas_cost() * 6)?;
		let caller: T::AccountId = T::AddressMapping::into_account_id(*caller);
		let event_proof_id = crml_erc20_peg::Module::<T>::do_withdrawal(
			caller,
			asset_id,
			withdraw_amount.unique_saturated_into(),
			beneficiary,
			WithdrawCallOrigin::Evm,
		);

		// Build output.
		match event_proof_id {
			Ok(proof_id) => Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				cost: gasometer.used_gas(),
				output: EvmDataWriter::new().write(U256::from(proof_id)).build(),
				logs: Default::default(),
			}),
			Err(err) => Err(PrecompileFailure::Revert {
				exit_status: ExitRevert::Reverted,
				output: alloc::format!("withdraw failed: {:?}", err.stripped())
					.as_bytes()
					.to_vec(),
			}),
		}
	}
}
