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

use fp_evm::{ExitSucceed, PrecompileFailure, PrecompileOutput};
use pallet_evm::{AddressMapping, ExitRevert, Precompile};
use sp_core::{H160, U256};
use sp_runtime::traits::UniqueSaturatedInto;
use sp_std::marker::PhantomData;

use cennznet_primitives::types::AssetId;
use crml_erc20_peg::types::WithdrawCallOrigin;
use pallet_evm_precompiles_erc20::Erc20IdConversion;
use precompile_utils::prelude::*;

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
	fn execute(handle: &mut impl PrecompileHandle) -> EvmResult<PrecompileOutput> {
		let selector = match handle.read_selector() {
			Ok(selector) => selector,
			Err(e) => return Err(e),
		};

		if let Err(err) = handle.check_function_modifier(FunctionModifier::NonPayable) {
			return Err(err);
		}

		match selector {
			Action::Withdraw => Self::withdraw(handle),
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
	fn withdraw(handle: &mut impl PrecompileHandle) -> EvmResult<PrecompileOutput> {
		// Parse input.
		let mut input = handle.read_input()?;
		input.expect_arguments(3)?;

		let withdraw_asset: Address = input.read::<Address>()?.into();
		// the given `input_asset` address is not a valid (derived) generic asset address
		let asset_id = T::evm_id_to_runtime_id(withdraw_asset).ok_or(revert("unsupported asset"))?;
		let withdraw_amount: U256 = input.read::<U256>()?.into();
		let beneficiary: H160 = input.read::<Address>()?.into();

		handle.record_cost(RuntimeHelper::<T>::db_read_gas_cost() * 6)?;
		let caller: T::AccountId = T::AddressMapping::into_account_id(handle.context().caller);
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
				output: EvmDataWriter::new().write(U256::from(proof_id)).build(),
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
