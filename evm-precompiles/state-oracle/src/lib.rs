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

use cennznet_primitives::types::{AssetId, FeePreferences};
use crml_support::{scale_wei_to_4dp, EthereumStateOracle};
use fp_evm::{ExitSucceed, PrecompileFailure, PrecompileHandle, PrecompileOutput};
use frame_support::traits::Get;
use pallet_evm::{ExitRevert, Precompile};
use pallet_evm_precompiles_erc20::Erc20IdConversion;
use precompile_utils::prelude::*;
use sp_core::{H160, H256, U256};
use sp_runtime::{traits::UniqueSaturatedInto, Permill};
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
	RemoteCallWithFeeSwap = "remoteCallWithFeeSwap(address,bytes,bytes4,uint256,uint256,address,uint32)",
}

/// Provides access to the state oracle pallet
pub struct StateOraclePrecompile<T, C>(PhantomData<(T, C)>);

impl<T, C> Precompile for StateOraclePrecompile<T, C>
where
	T: EthereumStateOracle<Address = H160, RequestId = U256>,
	C: Erc20IdConversion<EvmId = Address, RuntimeId = AssetId> + crml_eth_state_oracle::Config,
{
	fn execute(handle: &mut impl PrecompileHandle) -> EvmResult<PrecompileOutput> {
		// Check that State Oracle is active
		if !<C as crml_eth_state_oracle::Config>::StateOracleIsActive::get() {
			return Err(PrecompileFailure::Revert {
				exit_status: ExitRevert::Reverted,
				output: ("Request Failed, State Oracle not enabled").as_bytes().to_vec(),
			});
		}

		let selector = match handle.read_selector() {
			Ok(selector) => selector,
			Err(e) => return Err(e),
		};

		if let Err(err) = handle.check_function_modifier(match selector {
			Action::RemoteCall => FunctionModifier::NonPayable,
			Action::RemoteCallWithFeeSwap => FunctionModifier::NonPayable,
		}) {
			return Err(err);
		}

		match selector {
			Action::RemoteCall => Self::remote_call(handle),
			Action::RemoteCallWithFeeSwap => Self::remote_call_with_fee_swap(handle),
		}
	}
}

impl<T, C> StateOraclePrecompile<T, C> {
	pub fn new() -> Self {
		Self(PhantomData)
	}
}

impl<T, C> StateOraclePrecompile<T, C>
where
	T: EthereumStateOracle<Address = H160, RequestId = U256>,
	C: Erc20IdConversion<EvmId = Address, RuntimeId = AssetId> + crml_eth_state_oracle::Config,
{
	fn remote_call_with_fee_swap(handle: &mut impl PrecompileHandle) -> EvmResult<PrecompileOutput> {
		let mut input = handle.read_input()?;
		input.expect_arguments(7)?;

		let destination: H160 = input.read::<Address>()?.into();
		let input_data: Bytes = input.read::<Bytes>()?.into();
		// valid selectors are 4 bytes
		let callback_signature: H256 = input.read::<H256>()?.into();
		let callback_signature: [u8; 4] = callback_signature.as_fixed_bytes()[..4].try_into().unwrap(); // H256 has 32 bytes, cannot fail qed.
		let callback_gas_limit: U256 = input.read::<U256>()?.into();
		let callback_bounty: U256 = input.read::<U256>()?.into();
		// scale to 4dp for consistency with other CPAY balance apis
		let callback_bounty = scale_wei_to_4dp(callback_bounty.unique_saturated_into());
		let fee_asset_id: Address = input.read::<Address>()?.into();
		// the given `input_asset` address is not a valid (derived) generic asset address
		// it is not supported by cennzx
		let asset_id = C::evm_id_to_runtime_id(fee_asset_id).ok_or(revert("unsupported asset"))?;
		let slippage: U256 = input.read::<U256>()?.into();
		let fee_preferences = Some(FeePreferences {
			asset_id,
			slippage: Permill::from_rational(slippage.low_u32(), 1_000),
		});

		handle.record_cost(T::new_request_fee())?;
		let request_id = T::new_request(
			&handle.context().caller,
			&destination,
			input_data.as_bytes(),
			&callback_signature, // checked len == 4 above qed
			callback_gas_limit.low_u64(),
			fee_preferences,
			callback_bounty,
		);

		// Build output.
		match request_id {
			Ok(request_id) => Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output: EvmDataWriter::new().write(request_id).build(),
			}),
			Err(err) => Err(PrecompileFailure::Revert {
				exit_status: ExitRevert::Reverted,
				output: alloc::format!("Request failed: {:?}", err.stripped())
					.as_bytes()
					.to_vec(),
			}),
		}
	}

	/// Proxy state requests to the Eth state oracle pallet
	/// caller should be `msg.sender`
	fn remote_call(handle: &mut impl PrecompileHandle) -> EvmResult<PrecompileOutput> {
		// Parse input.
		let mut input = handle.read_input()?;
		input.expect_arguments(5)?;

		let destination: H160 = input.read::<Address>()?.into();
		let input_data: Bytes = input.read::<Bytes>()?.into();
		let callback_signature: H256 = input.read::<H256>()?.into();
		// extract selector from the first 4 bytes
		let callback_signature: [u8; 4] = callback_signature.as_fixed_bytes()[..4].try_into().unwrap(); // H256 has 32 bytes, cannot fail qed.
		let callback_gas_limit: U256 = input.read::<U256>()?.into();
		let callback_bounty: U256 = input.read::<U256>()?.into();
		// scale to 4dp for consistency with other CPAY balance apis
		let callback_bounty = scale_wei_to_4dp(callback_bounty.unique_saturated_into());

		handle.record_cost(T::new_request_fee())?;
		let request_id = T::new_request(
			&handle.context().caller,
			&destination,
			input_data.as_bytes(),
			&callback_signature,
			callback_gas_limit.low_u64(),
			None,
			callback_bounty,
		);

		// Build output.
		match request_id {
			Ok(request_id) => Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output: EvmDataWriter::new().write(request_id).build(),
			}),
			Err(err) => Err(PrecompileFailure::Revert {
				exit_status: ExitRevert::Reverted,
				output: alloc::format!("Request failed: {:?}", err.stripped())
					.as_bytes()
					.to_vec(),
			}),
		}
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use cennznet_primitives::types::Balance;
	use ethabi::Token;
	use fp_evm::{ExitError, PrecompileFailure};
	use frame_support::assert_err;
	use sp_runtime::DispatchError;

	pub struct MockErc20IdConversion;

	// Mock conversion of runtime id and evm id.
	// Ignores prefix and just convert the types
	impl Erc20IdConversion for MockErc20IdConversion {
		type EvmId = Address;
		type RuntimeId = AssetId;

		// Get runtime Id from EVM address
		fn evm_id_to_runtime_id(evm_id: Self::EvmId) -> Option<Self::RuntimeId> {
			Some(evm_id.0.to_low_u64_be() as u32)
		}
		// Get EVM address from series Id parts (collection_id, series_id)
		fn runtime_id_to_evm_id(asset_id: Self::RuntimeId) -> Self::EvmId {
			H160::from_low_u64_be(asset_id as u64).into()
		}
	}

	#[test]
	fn new_remote_call_request() {
		struct MockEthereumStateOracle;
		impl EthereumStateOracle for MockEthereumStateOracle {
			type Address = H160;
			type RequestId = U256;
			fn new_request_fee() -> u64 {
				1_000_u64
			}
			/// assert inputs are correct
			fn new_request(
				caller_: &Self::Address,
				destination_: &Self::Address,
				input_data_: &[u8],
				callback_signature_: &[u8; 4],
				callback_gas_limit_: u64,
				fee_preferences: Option<FeePreferences>,
				bounty_: Balance,
			) -> Result<Self::RequestId, DispatchError> {
				let caller: H160 = H160::from_low_u64_be(555);
				let destination: H160 = H160::from_low_u64_be(23);
				let callback_signature: Vec<u8> = vec![1u8, 2, 3, 4];
				let callback_gas_limit: u64 = 200_000;
				let input_data: Vec<u8> = vec![55u8, 66, 77, 88, 99];

				assert_eq!(caller, *caller_);
				assert_eq!(destination, *destination_);
				assert_eq!(input_data, input_data_);
				assert_eq!(callback_signature, callback_signature_);
				assert_eq!(bounty_, 20_000_u128); // 2 CPAY 4dp, scaled down from 18dp input
				assert_eq!(callback_gas_limit, callback_gas_limit_);
				assert!(fee_preferences.is_none());

				Ok(U256::from(123u32))
			}
		}

		let caller: H160 = H160::from_low_u64_be(555);
		let abi_encoded_input = ethabi::encode(&[
			Token::Address(H160::from_low_u64_be(23)),
			Token::Bytes(vec![55u8, 66, 77, 88, 99]),
			Token::FixedBytes(vec![1u8, 2, 3, 4]),
			Token::Uint(U256::from(200_000_u64)),
			Token::Uint(U256::from(2_000_000_000_000_000_000_u128)), // 2 * 10**18
		]);
		let mut input = EvmDataReader::new(abi_encoded_input.as_ref());
		let mut gasometer = Gasometer::new(Some(MockEthereumStateOracle::new_request_fee()));

		// Test
		let result = StateOraclePrecompile::<MockEthereumStateOracle, MockErc20IdConversion>::remote_call(
			&mut input,
			&mut gasometer,
			&caller,
		);

		assert_eq!(
			result.unwrap(),
			PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output: EvmDataWriter::new().write(U256::from(123u32)).build(),
			},
		);
	}

	#[test]
	fn new_remote_call_with_fee_swap_request() {
		struct MockEthereumStateOracle;
		impl EthereumStateOracle for MockEthereumStateOracle {
			type Address = H160;
			type RequestId = U256;
			fn new_request_fee() -> u64 {
				1_000_u64
			}
			/// assert inputs are correct
			fn new_request(
				caller_: &Self::Address,
				destination_: &Self::Address,
				input_data_: &[u8],
				callback_signature_: &[u8; 4],
				callback_gas_limit_: u64,
				fee_preferences_: Option<FeePreferences>,
				bounty_: Balance,
			) -> Result<Self::RequestId, DispatchError> {
				let caller: H160 = H160::from_low_u64_be(555);
				let destination: H160 = H160::from_low_u64_be(23);
				let callback_signature: Vec<u8> = vec![1u8, 2, 3, 4];
				let callback_gas_limit: u64 = 200_000;
				let input_data: Vec<u8> = vec![55u8, 66, 77, 88, 99];
				let fee_preferences = Some(FeePreferences {
					asset_id: 100,
					slippage: Permill::from_rational(50u32, 1_000),
				});
				assert_eq!(caller, *caller_);
				assert_eq!(destination, *destination_);
				assert_eq!(input_data, input_data_);
				assert_eq!(callback_signature, callback_signature_);
				assert_eq!(bounty_, 20_000_u128); // 2 CPAY 4dp, scaled down from 18dp input
				assert_eq!(callback_gas_limit, callback_gas_limit_);
				assert_eq!(fee_preferences, fee_preferences_);

				Ok(U256::from(123u32))
			}
		}

		let caller: H160 = H160::from_low_u64_be(555);
		let abi_encoded_input = ethabi::encode(&[
			Token::Address(H160::from_low_u64_be(23)),
			Token::Bytes(vec![55u8, 66, 77, 88, 99]),
			Token::FixedBytes(vec![1u8, 2, 3, 4]),
			Token::Uint(U256::from(200_000_u64)),
			Token::Uint(U256::from(2_000_000_000_000_000_000_u128)), // 2 * 10**18
			Token::Address(H160::from(MockErc20IdConversion::runtime_id_to_evm_id(100))),
			Token::Uint(U256::from(50)),
		]);
		let mut input = EvmDataReader::new(abi_encoded_input.as_ref());
		let mut gasometer = Gasometer::new(Some(MockEthereumStateOracle::new_request_fee()));

		// Test
		let result = StateOraclePrecompile::<MockEthereumStateOracle, MockErc20IdConversion>::remote_call_with_fee_swap(
			&mut input,
		);

		assert_eq!(
			result.unwrap(),
			PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output: EvmDataWriter::new().write(U256::from(123u32)).build(),
			},
		);
	}

	#[test]
	fn uint_inputs_saturate_at_bounds() {
		// Setup
		struct MockEthereumStateOracle;
		impl EthereumStateOracle for MockEthereumStateOracle {
			type Address = H160;
			type RequestId = U256;
			fn new_request_fee() -> u64 {
				0_u64
			}
			fn new_request(
				_caller: &Self::Address,
				_destination: &Self::Address,
				_input_data: &[u8],
				_callback_signature: &[u8; 4],
				callback_gas_limit: u64,
				_fee_preferences: Option<FeePreferences>,
				bounty: Balance,
			) -> Result<Self::RequestId, DispatchError> {
				// gas_limit saturates at u64
				assert_eq!(callback_gas_limit, u64::max_value());
				// bounty saturates at balance type and scales down
				assert_eq!(bounty, scale_wei_to_4dp(u128::max_value()));
				Ok(U256::zero())
			}
		}
		let caller = H160::from_low_u64_be(1);
		let abi_encoded_input = ethabi::encode(&[
			Token::Address(H160::from_low_u64_be(2)),
			Token::Bytes(vec![1u8]),
			Token::FixedBytes(vec![55u8, 66, 77, 88, 99]),
			Token::Uint(U256::max_value()),
			Token::Uint(U256::max_value()),
		]);
		let mut input = EvmDataReader::new(abi_encoded_input.as_ref());
		let mut gasometer = Gasometer::new(Some(MockEthereumStateOracle::new_request_fee()));

		// Test
		let _ = StateOraclePrecompile::<MockEthereumStateOracle, MockErc20IdConversion>::remote_call(&mut input);
	}

	#[test]
	fn cannot_pay_new_request_fee() {
		struct MockEthereumStateOracle;
		impl EthereumStateOracle for MockEthereumStateOracle {
			type Address = H160;
			type RequestId = U256;
			fn new_request_fee() -> u64 {
				1_000_000u64
			}
			fn new_request(
				_caller_: &Self::Address,
				_destination_: &Self::Address,
				_input_data_: &[u8],
				_callback_signature_: &[u8; 4],
				_callback_gas_limit_: u64,
				_fee_preferences: Option<FeePreferences>,
				_bounty_: Balance,
			) -> Result<Self::RequestId, DispatchError> {
				Ok(U256::zero())
			}
		}
		let caller: H160 = H160::from_low_u64_be(555);
		let abi_encoded_input = ethabi::encode(&[
			Token::Address(H160::from_low_u64_be(23)),
			Token::Bytes(vec![55u8, 66, 77, 88, 99]),
			Token::FixedBytes(vec![1u8, 2, 3, 4]),
			Token::Uint(U256::from(200_000_u64)),
			Token::Uint(U256::zero()),
		]);
		let mut input = EvmDataReader::new(abi_encoded_input.as_ref());
		let mut gasometer = Gasometer::new(
			// not quite enough gas for the request fee
			Some(MockEthereumStateOracle::new_request_fee() - 1),
		);

		// Test
		assert_err!(
			StateOraclePrecompile::<MockEthereumStateOracle, MockErc20IdConversion>::remote_call(
				&mut gasometer,
				&caller
			),
			PrecompileFailure::Error {
				exit_status: ExitError::OutOfGas,
			},
		);
	}
}
