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

use crate::{constants::asset::SPENDING_ASSET_ID, impls::scale_to_4dp, Cennzx, FEE_FUNCTION_SELECTOR};
use cennznet_primitives::{
	traits::BuyFeeAsset,
	types::{AccountId, AssetId, Balance, FeeExchange},
};
use crml_support::{H160, H256, U256};
use ethabi::{ParamType, Token};
use frame_support::ensure;
use pallet_evm::{
	runner::stack::Runner, AddressMapping, CallInfo, CreateInfo, EvmConfig, FeeCalculator, Runner as RunnerT,
};
use sp_runtime::{
	traits::{SaturatedConversion, UniqueSaturatedInto},
	Permill,
};
use sp_std::{marker::PhantomData, prelude::*};

#[derive(Debug, Eq, PartialEq)]
pub enum FeePreferencesError {
	InvalidFunctionSelector,
	WithdrawFailed,
	GasPriceTooLow,
	FeeOverflow,
	InvalidInputArguments,
	FailedToDecodeInput,
	InvalidPaymentAsset,
}

// Precompile address for fee preferences
pub const FEE_PROXY: u64 = 1211;

/// CENNZnet implementation of the evm runner which handles the case where users are attempting
/// to set their payment asset. In this case, we will exchange their desired asset into CPAY to
/// complete the transaction
pub struct FeePreferencesRunner<T>
where
	T: pallet_evm::Config<AccountId = AccountId>,
{
	_marker: PhantomData<T>,
}

impl<T> FeePreferencesRunner<T>
where
	T: pallet_evm::Config<AccountId = AccountId>,
{
	/// Decodes the input for call_with_fee_preferences
	pub fn decode_input(input: Vec<u8>) -> Result<(AssetId, u32, H160, Vec<u8>), FeePreferencesError> {
		ensure!(input.len() >= 4, FeePreferencesError::InvalidInputArguments);
		ensure!(
			input[..4] == FEE_FUNCTION_SELECTOR,
			FeePreferencesError::InvalidFunctionSelector
		);

		let types = [
			ParamType::Uint(32),
			ParamType::Uint(32),
			ParamType::Address,
			ParamType::Bytes,
		];
		let tokens = ethabi::decode(&types, &input[4..]);

		let (payment_asset, slippage, new_target, new_input) = match tokens {
			Ok(token_vec) => match &token_vec[..] {
				[Token::Uint(payment_asset), Token::Uint(slippage), Token::Address(new_target), Token::Bytes(new_input)] => {
					(
						payment_asset.clone().low_u128().saturated_into::<AssetId>(),
						slippage.clone().low_u128().saturated_into::<u32>(),
						H160::from(new_target.clone().to_fixed_bytes()),
						new_input.clone().to_vec(),
					)
				}
				_ => return Err(FeePreferencesError::InvalidInputArguments),
			},
			_ => return Err(FeePreferencesError::FailedToDecodeInput),
		};

		ensure!(payment_asset != 0, FeePreferencesError::InvalidPaymentAsset);

		Ok((payment_asset, slippage, new_target, new_input))
	}

	/// Calculate gas price for transaction to use for exchanging asset into CPAY
	pub fn calculate_total_gas(
		gas_limit: u64,
		max_fee_per_gas: Option<U256>,
		max_priority_fee_per_gas: Option<U256>,
	) -> Result<Balance, FeePreferencesError> {
		let base_fee = T::FeeCalculator::min_gas_price();

		let max_fee_per_gas = match max_fee_per_gas {
			Some(max_fee_per_gas) => {
				ensure!(max_fee_per_gas >= base_fee, FeePreferencesError::GasPriceTooLow);
				max_fee_per_gas
			}
			None => return Err(FeePreferencesError::GasPriceTooLow),
		};
		let max_base_fee = max_fee_per_gas
			.checked_mul(U256::from(gas_limit))
			.ok_or(FeePreferencesError::FeeOverflow)?;
		let max_priority_fee = if let Some(max_priority_fee) = max_priority_fee_per_gas {
			max_priority_fee
				.checked_mul(U256::from(gas_limit))
				.ok_or(FeePreferencesError::FeeOverflow)?
		} else {
			U256::zero()
		};
		let total_fee: Balance = max_base_fee
			.checked_add(max_priority_fee)
			.ok_or(FeePreferencesError::FeeOverflow)?
			.low_u128()
			.unique_saturated_into();

		Ok(total_fee)
	}
}

impl<T> RunnerT<T> for FeePreferencesRunner<T>
where
	T: pallet_evm::Config<AccountId = AccountId>,
{
	type Error = pallet_evm::Error<T>;

	fn call(
		source: H160,
		target: H160,
		input: Vec<u8>,
		value: U256,
		gas_limit: u64,
		max_fee_per_gas: Option<U256>,
		max_priority_fee_per_gas: Option<U256>,
		nonce: Option<U256>,
		access_list: Vec<(H160, Vec<H256>)>,
		config: &EvmConfig,
	) -> Result<CallInfo, Self::Error> {
		// These values may change if we are using the fee_preferences precompile
		let mut input = input;
		let mut target = target;

		// Check if we are calling with fee preferences
		if target == H160::from_low_u64_be(FEE_PROXY) {
			let (payment_asset, slippage, new_target, new_input) =
				Self::decode_input(input).map_err(|_| Self::Error::WithdrawFailed)?;
			// set input and target to new input and actual target for passthrough
			input = new_input;
			target = new_target;

			// If payment_asset isn't CPAY, calculate gas and exchange for payment asset
			if payment_asset != SPENDING_ASSET_ID {
				let total_fee = Self::calculate_total_gas(gas_limit, max_fee_per_gas, max_priority_fee_per_gas)
					.map_err(|err| match err {
						FeePreferencesError::WithdrawFailed => Self::Error::WithdrawFailed,
						FeePreferencesError::GasPriceTooLow => Self::Error::GasPriceTooLow,
						FeePreferencesError::FeeOverflow => Self::Error::FeeOverflow,
						_ => Self::Error::WithdrawFailed,
					})?;
				let total_fee = scale_to_4dp(total_fee);
				let max_payment = total_fee.saturating_add(Permill::from_rational(slippage, 1_000) * total_fee);
				let exchange = FeeExchange::new_v1(payment_asset, max_payment);
				// Buy the CENNZnet fee currency paying with the user's nominated fee currency
				let account = <T as pallet_evm::Config>::AddressMapping::into_account_id(source);
				<Cennzx as BuyFeeAsset>::buy_fee_asset(&account, total_fee, &exchange).map_err(|_| {
					// Using general error to cover all cases due to fixed return type of pallet_evm::Error
					Self::Error::WithdrawFailed
				})?;
			}
		}

		<Runner<T> as RunnerT<T>>::call(
			source,
			target,
			input,
			value,
			gas_limit,
			max_fee_per_gas,
			max_priority_fee_per_gas,
			nonce,
			access_list,
			config,
		)
	}

	fn create(
		source: H160,
		init: Vec<u8>,
		value: U256,
		gas_limit: u64,
		max_fee_per_gas: Option<U256>,
		max_priority_fee_per_gas: Option<U256>,
		nonce: Option<U256>,
		access_list: Vec<(H160, Vec<H256>)>,
		config: &EvmConfig,
	) -> Result<CreateInfo, Self::Error> {
		<Runner<T> as RunnerT<T>>::create(
			source,
			init,
			value,
			gas_limit,
			max_fee_per_gas,
			max_priority_fee_per_gas,
			nonce,
			access_list,
			config,
		)
	}

	fn create2(
		source: H160,
		init: Vec<u8>,
		salt: H256,
		value: U256,
		gas_limit: u64,
		max_fee_per_gas: Option<U256>,
		max_priority_fee_per_gas: Option<U256>,
		nonce: Option<U256>,
		access_list: Vec<(H160, Vec<H256>)>,
		config: &EvmConfig,
	) -> Result<CreateInfo, Self::Error> {
		<Runner<T> as RunnerT<T>>::create2(
			source,
			init,
			salt,
			value,
			gas_limit,
			max_fee_per_gas,
			max_priority_fee_per_gas,
			nonce,
			access_list,
			config,
		)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::Runtime;
	use frame_support::{assert_noop, assert_ok};
	use hex_literal::hex;

	#[test]
	fn decode_input() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			// Abi generated from below parameters using the following function name:
			// callWithFeePreferences
			// abi can be easily generated here https://abi.hashex.org/
			let abi = hex!("ccf39ea90000000000000000000000000000000000000000000000000000000000003e800000000000000000000000000000000000000000000000000000000000000032000000000000000000000000cccccccc00003e8000000000000000000000000000000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000044a9059cbb0000000000000000000000007a107fc1794f505cb351148f529accae12ffbcd8000000000000000000000000000000000000000000000000000000000000007b00000000000000000000000000000000000000000000000000000000");
			let exp_payment_asset: u32 = 16000;
			let exp_slippage: u32 = 50;
			let exp_target = H160::from_slice(&hex!("cCccccCc00003E80000000000000000000000000"));
			let exp_input: Vec<u8> = hex!("a9059cbb0000000000000000000000007a107fc1794f505cb351148f529accae12ffbcd8000000000000000000000000000000000000000000000000000000000000007b").to_vec();
			let (payment_asset, slippage, new_target, new_input) =
				<FeePreferencesRunner<Runtime>>::decode_input(abi.to_vec()).unwrap();

			// Ensure the values decode correctly
			assert_eq!(payment_asset, exp_payment_asset);
			assert_eq!(slippage, exp_slippage);
			assert_eq!(new_target, exp_target);
			assert_eq!(new_input, exp_input);
		});
	}

	#[test]
	fn decode_input_invalid_function_selector_should_fail() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			let abi = hex!("11111111000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000320000000000000000000000001122334455667788991122334455667788990000000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000051234567890000000000000000000000000000000000000000000000000000000");
			assert_noop!(
				<FeePreferencesRunner<Runtime>>::decode_input(abi.to_vec()),
				FeePreferencesError::InvalidFunctionSelector
			);
		});
	}

	#[test]
	fn decode_input_empty_input_should_fail() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			let abi = hex!("");
			assert_noop!(
				<FeePreferencesRunner<Runtime>>::decode_input(abi.to_vec()),
				FeePreferencesError::InvalidInputArguments
			);
		});
	}

	#[test]
	fn decode_input_invalid_input_args_should_fail() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			let abi = hex!("ccf39ea9000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000320000000000000000000000001122334455667788991122334455667788990000");
			assert_noop!(
			<FeePreferencesRunner<Runtime>>::decode_input(abi.to_vec()),
				FeePreferencesError::FailedToDecodeInput
			);
		});
	}

	#[test]
	fn decode_input_zero_payment_asset_should_fail() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			let abi = hex!("ccf39ea9000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000320000000000000000000000001122334455667788991122334455667788990000000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000051234567890000000000000000000000000000000000000000000000000000000");
			assert_noop!(
				<FeePreferencesRunner<Runtime>>::decode_input(abi.to_vec()),
				FeePreferencesError::InvalidPaymentAsset
			);
		});
	}

	#[test]
	fn calculate_total_gas() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			let gas_limit: u64 = 100000;
			let max_fee_per_gas = U256::from(20000000000000u64);
			let max_priority_fee_per_gas = U256::from(1000000u64);

			assert_ok!(<FeePreferencesRunner<Runtime>>::calculate_total_gas(
				gas_limit,
				Some(max_fee_per_gas),
				Some(max_priority_fee_per_gas),
			));
		});
	}

	#[test]
	fn calculate_total_gas_low_max_fee_should_fail() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			let gas_limit: u64 = 100000;
			let max_fee_per_gas = U256::from(200000u64);
			let max_priority_fee_per_gas = U256::from(1000000u64);

			assert_noop!(
				<FeePreferencesRunner<Runtime>>::calculate_total_gas(
					gas_limit,
					Some(max_fee_per_gas),
					Some(max_priority_fee_per_gas),
				),
				FeePreferencesError::GasPriceTooLow
			);
		});
	}

	#[test]
	fn calculate_total_gas_no_max_fee_should_fail() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			let gas_limit: u64 = 100000;
			let max_fee_per_gas = None;
			let max_priority_fee_per_gas = U256::from(1000000u64);

			assert_noop!(
				<FeePreferencesRunner<Runtime>>::calculate_total_gas(
					gas_limit,
					max_fee_per_gas,
					Some(max_priority_fee_per_gas),
				),
				FeePreferencesError::GasPriceTooLow
			);
		});
	}

	#[test]
	fn calculate_total_gas_max_priority_fee_too_large_should_fail() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			let gas_limit: u64 = 100000;
			let max_fee_per_gas = U256::from(20000000000000u64);
			let max_priority_fee_per_gas = U256::MAX;

			assert_noop!(
				<FeePreferencesRunner<Runtime>>::calculate_total_gas(
					gas_limit,
					Some(max_fee_per_gas),
					Some(max_priority_fee_per_gas),
				),
				FeePreferencesError::FeeOverflow
			);
		});
	}

	#[test]
	fn calculate_total_gas_max_fee_too_large_should_fail() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			let gas_limit: u64 = 100000;
			let max_fee_per_gas = U256::MAX;
			let max_priority_fee_per_gas = U256::from(1000000u64);

			assert_noop!(
				<FeePreferencesRunner<Runtime>>::calculate_total_gas(
					gas_limit,
					Some(max_fee_per_gas),
					Some(max_priority_fee_per_gas),
				),
				FeePreferencesError::FeeOverflow
			);
		});
	}
}
