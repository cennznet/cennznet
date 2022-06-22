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

use crate::{constants::evm::FEE_PROXY, Cennzx, FEE_FUNCTION_SELECTOR};
use cennznet_primitives::{
	traits::BuyFeeAsset,
	types::{AccountId, AssetId, Balance, FeeExchange},
};
use crml_support::{log, scale_wei_to_4dp, H160, H256, U256};
use ethabi::{ParamType, Token};
use frame_support::ensure;
use pallet_evm::{
	runner::stack::Runner, AddressMapping, CallInfo, CreateInfo, EvmConfig, FeeCalculator, Runner as RunnerT,
};
use pallet_evm_precompiles_erc20::Erc20IdConversion;
use precompile_utils::Address as EthAddress;
use sp_runtime::{traits::UniqueSaturatedInto, Permill};
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

impl<T> Into<pallet_evm::Error<T>> for FeePreferencesError {
	fn into(self: Self) -> pallet_evm::Error<T> {
		match self {
			FeePreferencesError::WithdrawFailed => pallet_evm::Error::WithdrawFailed,
			FeePreferencesError::GasPriceTooLow => pallet_evm::Error::GasPriceTooLow,
			FeePreferencesError::FeeOverflow => pallet_evm::Error::FeeOverflow,
			_ => pallet_evm::Error::WithdrawFailed,
		}
	}
}

/// CENNZnet implementation of the evm runner which handles the case where users are attempting
/// to set their payment asset. In this case, we will exchange their desired asset into CPAY to
/// complete the transaction
pub struct FeePreferencesRunner<T, U>(PhantomData<(T, U)>);

impl<T, U> FeePreferencesRunner<T, U>
where
	T: pallet_evm::Config<AccountId = AccountId>,
	U: Erc20IdConversion<EvmId = EthAddress, RuntimeId = AssetId>,
{
	/// Decodes the input for call_with_fee_preferences
	pub fn decode_input(input: Vec<u8>) -> Result<(AssetId, u32, H160, Vec<u8>), FeePreferencesError> {
		ensure!(input.len() >= 4, FeePreferencesError::InvalidInputArguments);
		ensure!(
			input[..4] == FEE_FUNCTION_SELECTOR,
			FeePreferencesError::InvalidFunctionSelector
		);

		let types = [
			ParamType::Address,
			ParamType::Uint(32),
			ParamType::Address,
			ParamType::Bytes,
		];
		let tokens = ethabi::decode(&types, &input[4..]).map_err(|_| FeePreferencesError::FailedToDecodeInput)?;

		if let [Token::Address(payment_asset_address), Token::Uint(slippage), Token::Address(new_target), Token::Bytes(new_input)] =
			tokens.as_slice()
		{
			let payment_asset = U::evm_id_to_runtime_id((*payment_asset_address).into());
			ensure!(payment_asset.is_some(), FeePreferencesError::InvalidPaymentAsset);

			Ok((
				payment_asset.unwrap(),
				slippage.low_u32(),
				*new_target,
				new_input.clone(),
			))
		} else {
			Err(FeePreferencesError::InvalidInputArguments)
		}
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
			None => Default::default(),
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
			.unique_saturated_into();

		Ok(total_fee)
	}
}

impl<T, U> RunnerT<T> for FeePreferencesRunner<T, U>
where
	T: pallet_evm::Config<AccountId = AccountId>,
	U: Erc20IdConversion<EvmId = EthAddress, RuntimeId = AssetId>,
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

			let total_fee = Self::calculate_total_gas(gas_limit, max_fee_per_gas, max_priority_fee_per_gas)
				.map_err(|err| err.into())?;
			let total_fee_required = scale_wei_to_4dp(total_fee); // this is the CPAY amount required for exchange of fee asset
			// get decimals of payment asset and use it in a function along with slippage to calculate max_payment
			// const decimal_places = crml_generic_asset::Pallet::<Runtime>::asset_meta(payment_asset).decimal_places();
			let max_payment = total_fee.saturating_add(Permill::from_rational(slippage, 1_000) * total_fee); // max payment needs to consider the decimals of payment_asset
			let exchange = FeeExchange::new_v1(payment_asset, max_payment);
			// Buy the CENNZnet fee currency paying with the user's nominated fee currency
			let account = <T as pallet_evm::Config>::AddressMapping::into_account_id(source);
			<Cennzx as BuyFeeAsset>::buy_fee_asset(&account, total_fee_required, &exchange).map_err(|err| {
				log!(
					debug,
					"⛽️ swapping {:?} (max {:?} units) for fee {:?} units failed: {:?}",
					payment_asset,
					max_payment,
					total_fee,
					err
				);
				// Using general error to cover all cases due to fixed return type of pallet_evm::Error
				Self::Error::WithdrawFailed
			})?;
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
	use crate::{BaseFee, Runtime};
	use frame_support::{assert_noop, assert_ok};
	use hex_literal::hex;

	/// type alias for runtime configured FeePreferencesRunner
	type Runner = FeePreferencesRunner<Runtime, Runtime>;

	#[test]
	fn decode_input() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			// Abi generated from below parameters using the following function name:
			// callWithFeePreferences
			// abi can be easily generated here https://abi.hashex.org/
			let exp_payment_asset = 16000_u32;
			let exp_slippage: u32 = 50;
			let exp_target = H160::from_slice(&hex!("cCccccCc00003E80000000000000000000000000"));
			let exp_input: Vec<u8> = hex!("a9059cbb0000000000000000000000007a107fc1794f505cb351148f529accae12ffbcd8000000000000000000000000000000000000000000000000000000000000007b").to_vec();
			let mut input= FEE_FUNCTION_SELECTOR.to_vec();
			input.append(&mut ethabi::encode(&[
				Token::Address(Runtime::runtime_id_to_evm_id(exp_payment_asset).0),
				Token::Uint(exp_slippage.into()),
				Token::Address(exp_target),
				Token::Bytes(exp_input.clone())],
			));

			assert_eq!(
				Runner::decode_input(input),
				Ok((exp_payment_asset, exp_slippage, exp_target, exp_input))
			);
		});
	}

	#[test]
	fn decode_input_invalid_function_selector_should_fail() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			let bad_selector_input = vec![0x01, 0x02, 0x03, 0x04];
			assert_noop!(
				Runner::decode_input(bad_selector_input),
				FeePreferencesError::InvalidFunctionSelector
			);
		});
	}

	#[test]
	fn decode_input_empty_input_should_fail() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			assert_noop!(
				Runner::decode_input(Default::default()),
				FeePreferencesError::InvalidInputArguments
			);
		});
	}

	#[test]
	fn decode_input_invalid_input_args_should_fail() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			let mut input = FEE_FUNCTION_SELECTOR.to_vec();
			input.append(&mut ethabi::encode(&[
				Token::Bytes(vec![1_u8, 2, 3, 4, 5]),
				Token::Array(vec![
					Token::Uint(1u64.into()),
					Token::Uint(2u64.into()),
					Token::Uint(3u64.into()),
					Token::Uint(4u64.into()),
					Token::Uint(5u64.into()),
				]),
			]));

			assert_noop!(Runner::decode_input(input), FeePreferencesError::FailedToDecodeInput);
		});
	}

	#[test]
	fn decode_input_zero_payment_asset_should_fail() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			let mut input = FEE_FUNCTION_SELECTOR.to_vec();
			input.append(&mut ethabi::encode(&[
				Token::Address(H160::zero()),
				Token::Uint(5u64.into()),
				Token::Address(H160::default()),
				Token::Bytes(vec![1_u8, 2, 3, 4, 5]),
			]));

			assert_noop!(
				Runner::decode_input(input.to_vec()),
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

			assert_ok!(Runner::calculate_total_gas(
				gas_limit,
				Some(max_fee_per_gas),
				Some(max_priority_fee_per_gas),
			));
		});
	}

	#[test]
	fn calculate_total_gas_low_max_fee_should_fail() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			let gas_limit = 100_000_u64;
			let max_fee_per_gas = BaseFee::min_gas_price().saturating_sub(1_u64.into());

			assert_noop!(
				Runner::calculate_total_gas(gas_limit, Some(max_fee_per_gas), None),
				FeePreferencesError::GasPriceTooLow
			);
		});
	}

	#[test]
	fn calculate_total_gas_no_max_fee_ok() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			let gas_limit = 100_000_u64;
			let max_fee_per_gas = None;
			let max_priority_fee_per_gas = U256::from(1_000_000_u64);

			assert_ok!(Runner::calculate_total_gas(
				gas_limit,
				max_fee_per_gas,
				Some(max_priority_fee_per_gas)
			));
		});
	}

	#[test]
	fn calculate_total_gas_max_priority_fee_too_large_should_fail() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			let gas_limit: u64 = 100000;
			let max_fee_per_gas = U256::from(20000000000000u64);
			let max_priority_fee_per_gas = U256::MAX;

			assert_noop!(
				Runner::calculate_total_gas(gas_limit, Some(max_fee_per_gas), Some(max_priority_fee_per_gas),),
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
				Runner::calculate_total_gas(gas_limit, Some(max_fee_per_gas), Some(max_priority_fee_per_gas),),
				FeePreferencesError::FeeOverflow
			);
		});
	}
}
