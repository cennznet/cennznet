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

use cennznet_primitives::types::AssetId;
pub use fp_evm::Precompile;
pub use fp_evm::{Context, ExitSucceed, PrecompileOutput};
use frame_support::{
	dispatch::{Dispatchable, GetDispatchInfo, PostDispatchInfo},
	traits::OriginTrait,
};
pub use pallet_evm::AddressMapping;
pub use precompile_utils::{
	error, keccak256, Address, Bytes, EvmDataReader, EvmDataWriter, EvmResult, FunctionModifier, Gasometer,
	LogsBuilder, RuntimeHelper,
};
use sp_core::{H160, U256};
use sp_runtime::SaturatedConversion;
use sp_std::marker::PhantomData;

#[precompile_utils::generate_function_selector]
#[derive(Debug, PartialEq)]
pub enum Action {
	SetFeeAsset = "setFeeAsset(uint256)",
}

/// Provides access to the eth-wallet pallet
pub struct FeePaymentPrecompile<Runtime>(PhantomData<Runtime>);

impl<Runtime> Precompile for FeePaymentPrecompile<Runtime>
where
	Runtime: pallet_evm::Config + frame_system::Config + crml_eth_wallet::Config,
	<Runtime as frame_system::Config>::Call: Dispatchable<PostInfo = PostDispatchInfo> + GetDispatchInfo,
	<Runtime as frame_system::Config>::Call: From<crml_eth_wallet::Call<Runtime>>,
	<<Runtime as frame_system::Config>::Call as Dispatchable>::Origin: From<Option<Runtime::AccountId>>,
	<<Runtime as frame_system::Config>::Call as Dispatchable>::Origin: OriginTrait,
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
			Err(e) => return Err(e),
		};
		let input = &mut input;

		if let Err(err) = gasometer.check_function_modifier(
			context,
			is_static,
			match selector {
				Action::SetFeeAsset => FunctionModifier::NonPayable,
			},
		) {
			return Err(err);
		}

		match selector {
			Action::SetFeeAsset => Self::set_fee_asset(input, gasometer, &context.caller),
		}
	}
}

impl<Runtime> FeePaymentPrecompile<Runtime> {
	pub fn new() -> Self {
		Self(PhantomData)
	}
}

impl<Runtime> FeePaymentPrecompile<Runtime>
where
	Runtime: pallet_evm::Config + frame_system::Config + crml_eth_wallet::Config,
	<Runtime as frame_system::Config>::Call: Dispatchable<PostInfo = PostDispatchInfo> + GetDispatchInfo,
	<Runtime as frame_system::Config>::Call: From<crml_eth_wallet::Call<Runtime>>,
	<<Runtime as frame_system::Config>::Call as Dispatchable>::Origin: From<Option<Runtime::AccountId>>,
	<<Runtime as frame_system::Config>::Call as Dispatchable>::Origin: OriginTrait,
{
	/// Proxy remote call requests to the eth-wallet pallet
	fn set_fee_asset(
		input: &mut EvmDataReader,
		gasometer: &mut Gasometer,
		caller: &H160,
	) -> EvmResult<PrecompileOutput> {
		// Parse input.
		input.expect_arguments(gasometer, 1)?;
		let payment_asset: AssetId = input.read::<U256>(gasometer)?.saturated_into();
		let origin = <Runtime as pallet_evm::Config>::AddressMapping::into_account_id(*caller);

		RuntimeHelper::<Runtime>::try_dispatch(
			Some(origin).into(),
			crml_eth_wallet::Call::<Runtime>::set_payment_asset {
				payment_asset: Some(payment_asset),
			},
			gasometer,
		)?;

		// Build output.
		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			cost: gasometer.used_gas(),
			output: EvmDataWriter::new().write(true).build(),
			logs: Default::default(),
		})
	}
}
