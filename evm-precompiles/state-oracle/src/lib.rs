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

use cennznet_primitives::types::FeePreferences;
use crml_support::EthereumStateOracle;
use fp_evm::{Context, ExitSucceed, PrecompileOutput};
pub use pallet_evm::{AddressMapping, Precompile, PrecompileResult};
pub use precompile_utils::{
	error, keccak256, Address, Bytes, EvmDataReader, EvmDataWriter, EvmResult, FunctionModifier, Gasometer,
	LogsBuilder, RuntimeHelper,
};
use sp_core::{H160, H256, U256};
use sp_runtime::Permill;
use sp_std::{convert::TryInto, marker::PhantomData};

#[precompile_utils::generate_function_selector]
#[derive(Debug, PartialEq)]
pub enum Action {
	/// Issue a remote 'call' to ethereum contract
	/// target contract, call input, callback selector, callback gas limit, callback bounty
	RemoteCall = "remoteCall(address,bytes,bytes4,uint256,uint256)",
	/// Issue a remote 'call' to ethereum contract
	/// allowing fee swaps in the callback
	/// target contract, call input, callback selector, callback gas limit, callback bounty, fee asset, fee slippage
	RemoteCallWithFeeSwap = "remoteCallWithFeeSwap(address,bytes,bytes4,uint256,uint256,uint32,uint32)",
}

/// Provides access to the state oracle pallet
pub struct StateOraclePrecompile<Runtime>(PhantomData<Runtime>);

impl<Runtime> Precompile for StateOraclePrecompile<Runtime>
where
	Runtime: pallet_evm::Config + frame_system::Config + crml_eth_state_oracle::Config,
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
				Action::RemoteCall => FunctionModifier::NonPayable,
				Action::RemoteCallWithFeeSwap => FunctionModifier::NonPayable,
			},
		) {
			return Err(err);
		}

		match selector {
			Action::RemoteCall => Self::remote_call(input, gasometer, &context.address),
			Action::RemoteCallWithFeeSwap => Self::remote_call_with_fee_swap(input, gasometer, &context.address),
		}
	}
}

impl<Runtime> StateOraclePrecompile<Runtime> {
	pub fn new() -> Self {
		Self(PhantomData)
	}
}

impl<Runtime> StateOraclePrecompile<Runtime>
where
	Runtime: pallet_evm::Config + frame_system::Config + crml_eth_state_oracle::Config,
{
	fn remote_call_with_fee_swap(
		input: &mut EvmDataReader,
		gasometer: &mut Gasometer,
		caller: &H160,
	) -> EvmResult<PrecompileOutput> {
		input.expect_arguments(gasometer, 7)?;
		let destination: H160 = input.read::<Address>(gasometer)?.into();
		let input_data: Bytes = input.read::<Bytes>(gasometer)?.into();
		// valid selectors are 4 bytes
		let callback_signature: Bytes = input.read::<Bytes>(gasometer)?.into();
		if callback_signature.as_bytes().len() != 4 {
			return Err(gasometer.revert("invalid callback sig"));
		}
		let callback_gas_limit: U256 = input.read::<U256>(gasometer)?.into();
		let callback_bounty: U256 = input.read::<U256>(gasometer)?.into();
		let fee_asset_id: U256 = input.read::<U256>(gasometer)?.into();
		let slippage: U256 = input.read::<U256>(gasometer)?.into();
		let mut fee_preferences = None;
		if fee_asset_id > U256::zero() {
			fee_preferences = Some(FeePreferences {
				asset_id: fee_asset_id.low_u32(),
				slippage: Permill::from_rational(slippage.low_u32(), 1_000),
			});
		};

		let request_id: U256 = crml_eth_state_oracle::Pallet::<Runtime>::new_request(
			caller,
			&destination,
			input_data.as_bytes(),
			callback_signature.as_bytes().try_into().unwrap(), // checked len == 4 above qed
			// TODO: check these conversations are saturating (low_u32, low_u64, low_u128)
			callback_gas_limit.low_u64(),
			fee_preferences,
			callback_bounty.low_u128(),
		);

		// TODO: log the request id
		// Build output.
		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			cost: gasometer.used_gas(),
			output: EvmDataWriter::new().write(request_id).build(),
			logs: Default::default(),
		})
	}

	/// Proxy state requests to the Eth state oracle pallet
	/// caller should be `msg.sender`
	fn remote_call(input: &mut EvmDataReader, gasometer: &mut Gasometer, caller: &H160) -> EvmResult<PrecompileOutput> {
		// Parse input.
		input.expect_arguments(gasometer, 5)?;
		let destination: H160 = input.read::<Address>(gasometer)?.into();
		let input_data: Bytes = input.read::<Bytes>(gasometer)?.into();
		let callback_signature: H256 = input.read::<H256>(gasometer)?.into();
		// extract selector from the first 4 bytes
		let callback_signature: [u8; 4] = callback_signature.as_fixed_bytes()[..4].try_into().unwrap(); // H256 has 32 bytes, cannot fail qed.
		let callback_gas_limit: U256 = input.read::<U256>(gasometer)?.into();
		let callback_bounty: U256 = input.read::<U256>(gasometer)?.into();
		// scale to 4dp for consistency with other CPAY balance apis
		let callback_bounty = scale_to_4dp(U256.low_u128());

		let request_id: U256 = crml_eth_state_oracle::Pallet::<Runtime>::new_request(
			caller,
			&destination,
			input_data.as_bytes(),
			&callback_signature,
			// TODO: check these conversations are saturating (low_u32, low_u64, low_u128)
			callback_gas_limit.low_u64(),
			None,
			callback_bounty.low_u128(),
		);

		// TODO: log the request id
		// Build output.
		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			cost: gasometer.used_gas(),
			output: EvmDataWriter::new().write(request_id).build(),
			logs: Default::default(),
		})
	}
}

/// Constant factor for scaling CPAY to its smallest indivisible unit
const CPAY_UNIT_VALUE: u128 = 10_u128.pow(14);

/// Convert 18dp wei values to 4dp equivalents (CPAY)
/// fractional amounts < `CPAY_UNIT_VALUE` are rounded up by adding 1 / 0.0001 cpay
pub fn scale_to_4dp(value: u128) -> u128 {
	let (quotient, remainder) = (value / CPAY_UNIT_VALUE, value % CPAY_UNIT_VALUE);
	if remainder.is_zero() {
		quotient
	} else {
		// if value has a fractional part < CPAY unit value
		// it is lost in this divide operation
		quotient + 1
	}
}

