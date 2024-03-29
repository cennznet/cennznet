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

use cennznet_primitives::types::{AssetId, Balance};
pub use fp_evm::{Context, ExitSucceed, PrecompileOutput};
use frame_support::dispatch::{Dispatchable, GetDispatchInfo, PostDispatchInfo};
use frame_support::traits::OriginTrait;
pub use pallet_evm::{AddressMapping, PrecompileSet};
pub use precompile_utils::{
	error, keccak256, Address, Bytes, EvmDataReader, EvmDataWriter, EvmResult, FunctionModifier, Gasometer,
	LogsBuilder, RuntimeHelper,
};
use sp_core::{H160, U256};
use sp_runtime::traits::{SaturatedConversion, Zero};
use sp_std::{marker::PhantomData, vec};

/// Calls to contracts starting with this prefix will be shim'd to the CENNZnet GA module
/// via an ERC20 compliant interface (`Erc20PrecompileSet`)
pub const ERC20_PRECOMPILE_ADDRESS_PREFIX: &[u8] = &[0xCC; 4];

/// Solidity selector of the Transfer log, which is the Keccak of the Log signature
pub const SELECTOR_LOG_TRANSFER: [u8; 32] = keccak256!("Transfer(address,address,uint256)");

/// Solidity selector of the Approval log, which is the Keccak of the Log signature
pub const SELECTOR_LOG_APPROVAL: [u8; 32] = keccak256!("Approval(address,address,uint256)");

#[precompile_utils::generate_function_selector]
#[derive(Debug, PartialEq)]
pub enum Action {
	TotalSupply = "totalSupply()",
	BalanceOf = "balanceOf(address)",
	Allowance = "allowance(address,address)",
	Transfer = "transfer(address,uint256)",
	Approve = "approve(address,uint256)",
	TransferFrom = "transferFrom(address,address,uint256)",
	Name = "name()",
	Symbol = "symbol()",
	Decimals = "decimals()",
}

/// Convert EVM addresses into GA module identifiers and vice versa
pub trait Erc20IdConversion {
	/// ID type used by EVM
	type EvmId;
	/// ID type used by runtime
	type RuntimeId;
	// Get runtime Id from EVM id
	fn evm_id_to_runtime_id(evm_id: Self::EvmId) -> Option<Self::RuntimeId>;
	// Get EVM id from runtime Id
	fn runtime_id_to_evm_id(runtime_id: Self::RuntimeId) -> Self::EvmId;
}

/// The following distribution has been decided for the precompiles
/// The precompile for AssetId X, where X is a u128 (i.e.16 bytes), if 0XCCCCCCCC + Bytes(AssetId)
/// In order to route the address to Erc20Precompile<R>, we first check whether the AssetId
/// exists in crml-generic-asset
/// This means that every address that starts with 0xCCCCCCCC will go through an additional db read,
/// but the probability for this to happen is 2^-32 for random addresses
pub struct Erc20PrecompileSet<Runtime>(PhantomData<Runtime>);

impl<Runtime> PrecompileSet for Erc20PrecompileSet<Runtime>
where
	Runtime: crml_generic_asset::Config<AssetId = AssetId, Balance = Balance>
		+ pallet_evm::Config
		+ frame_system::Config
		+ crml_token_approvals::Config,
	Runtime::Call: Dispatchable<PostInfo = PostDispatchInfo> + GetDispatchInfo,
	Runtime::Call: From<crml_generic_asset::Call<Runtime>> + From<crml_token_approvals::Call<Runtime>>,
	<Runtime::Call as Dispatchable>::Origin: From<Option<Runtime::AccountId>>,
	Runtime: Erc20IdConversion<EvmId = Address, RuntimeId = AssetId>,
	<<Runtime as frame_system::Config>::Call as Dispatchable>::Origin: OriginTrait,
{
	fn execute(
		&self,
		address: H160,
		input: &[u8],
		target_gas: Option<u64>,
		context: &Context,
		is_static: bool,
	) -> Option<EvmResult<PrecompileOutput>> {
		if let Some(asset_id) = Runtime::evm_id_to_runtime_id(address.into()) {
			// If the assetId has non-zero supply
			// "total_supply" returns both 0 if the assetId does not exist or if the supply is 0
			// The assumption I am making here is that a 0 supply asset is not interesting from
			// the perspective of the precompiles. Once pallet-assets has more publicly accesible
			// storage we can use another function for this, like check_asset_existence.
			// The other options is to check the asset existence in pallet-asset-manager, but
			// this makes the precompiles dependent on such a pallet, which is not ideal
			if !crml_generic_asset::Pallet::<Runtime>::total_issuance(asset_id).is_zero() {
				let result = {
					let mut gasometer = Gasometer::new(target_gas);
					let gasometer = &mut gasometer;

					let (mut input, selector) = match EvmDataReader::new_with_selector(gasometer, input) {
						Ok((input, selector)) => (input, selector),
						Err(e) => return Some(Err(e)),
					};
					let input = &mut input;

					if let Err(err) = gasometer.check_function_modifier(
						context,
						is_static,
						match selector {
							Action::Approve | Action::Transfer | Action::TransferFrom => FunctionModifier::NonPayable,
							_ => FunctionModifier::View,
						},
					) {
						return Some(Err(err));
					}

					match selector {
						Action::TotalSupply => Self::total_supply(asset_id, input, gasometer),
						Action::BalanceOf => Self::balance_of(asset_id, input, gasometer),
						Action::Transfer => Self::transfer(asset_id, input, gasometer, context),
						Action::TransferFrom => Self::transfer_from(asset_id, input, gasometer, context),
						Action::Name => Self::name(asset_id, gasometer),
						Action::Symbol => Self::symbol(asset_id, gasometer),
						Action::Decimals => Self::decimals(asset_id, gasometer),
						Action::Allowance => Self::allowance(asset_id, input, gasometer),
						Action::Approve => Self::approve(asset_id, input, gasometer, context),
					}
				};
				return Some(result);
			}
		}
		None
	}

	fn is_precompile(&self, address: H160) -> bool {
		if let Some(asset_id) = Runtime::evm_id_to_runtime_id(address.into()) {
			// totaly supply `0` is a good enough check for asset existence
			!crml_generic_asset::Pallet::<Runtime>::total_issuance(asset_id).is_zero()
		} else {
			false
		}
	}
}

impl<Runtime> Erc20PrecompileSet<Runtime> {
	pub fn new() -> Self {
		Self(PhantomData)
	}
}

impl<Runtime> Erc20PrecompileSet<Runtime>
where
	Runtime: crml_generic_asset::Config<AssetId = AssetId, Balance = Balance>
		+ pallet_evm::Config
		+ frame_system::Config
		+ crml_token_approvals::Config,
	Runtime::Call: Dispatchable<PostInfo = PostDispatchInfo> + GetDispatchInfo,
	Runtime::Call: From<crml_generic_asset::Call<Runtime>> + From<crml_token_approvals::Call<Runtime>>,
	<Runtime::Call as Dispatchable>::Origin: From<Option<Runtime::AccountId>>,
	Runtime: Erc20IdConversion<EvmId = Address, RuntimeId = AssetId>,
	<<Runtime as frame_system::Config>::Call as Dispatchable>::Origin: OriginTrait,
{
	fn total_supply(
		asset_id: AssetId,
		input: &mut EvmDataReader,
		gasometer: &mut Gasometer,
	) -> EvmResult<PrecompileOutput> {
		gasometer.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		// Parse input.
		input.expect_arguments(gasometer, 0)?;

		// Fetch info.
		let amount: U256 = crml_generic_asset::Pallet::<Runtime>::total_issuance(asset_id).into();

		// Build output.
		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			cost: gasometer.used_gas(),
			output: EvmDataWriter::new().write(amount).build(),
			logs: vec![],
		})
	}

	fn balance_of(
		asset_id: AssetId,
		input: &mut EvmDataReader,
		gasometer: &mut Gasometer,
	) -> EvmResult<PrecompileOutput> {
		gasometer.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost() * 2)?;

		// Read input.
		input.expect_arguments(gasometer, 1)?;

		let owner: H160 = input.read::<Address>(gasometer)?.into();

		// Fetch info.
		let amount: U256 = {
			let owner: Runtime::AccountId = Runtime::AddressMapping::into_account_id(owner);
			// CENNZ tokens must consider staking locked amounts
			if asset_id == crml_generic_asset::Pallet::<Runtime>::staking_asset_id() {
				crml_generic_asset::Pallet::<Runtime>::get_all_balances(&owner, asset_id)
					.available
					.into()
			} else {
				crml_generic_asset::Pallet::<Runtime>::free_balance(asset_id, &owner).into()
			}
		};

		// Build output.
		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			cost: gasometer.used_gas(),
			output: EvmDataWriter::new().write(amount).build(),
			logs: vec![],
		})
	}

	fn allowance(
		asset_id: AssetId,
		input: &mut EvmDataReader,
		gasometer: &mut Gasometer,
	) -> EvmResult<PrecompileOutput> {
		gasometer.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		// Read input.
		input.expect_arguments(gasometer, 2)?;

		let owner: H160 = input.read::<Address>(gasometer)?.into();
		let spender: H160 = input.read::<Address>(gasometer)?.into();

		// Fetch info.
		let amount: U256 = {
			// Fetch info.
			crml_token_approvals::Module::<Runtime>::erc20_approvals((&owner, &asset_id), &spender).into()
		};

		// Build output.
		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			cost: gasometer.used_gas(),
			output: EvmDataWriter::new().write(amount).build(),
			logs: vec![],
		})
	}

	fn approve(
		asset_id: AssetId,
		input: &mut EvmDataReader,
		gasometer: &mut Gasometer,
		context: &Context,
	) -> EvmResult<PrecompileOutput> {
		gasometer.record_log_costs_manual(3, 32)?;

		// Parse input.
		input.expect_arguments(gasometer, 2)?;

		let spender: H160 = input.read::<Address>(gasometer)?.into();
		let amount: U256 = input.read(gasometer)?;

		// Amount saturate if too high.
		let amount: Balance = amount.saturated_into();

		// Dispatch call (if enough gas).
		RuntimeHelper::<Runtime>::try_dispatch(
			None.into(),
			crml_token_approvals::Call::<Runtime>::erc20_approval {
				caller: context.caller.clone(),
				spender,
				asset_id,
				amount,
			},
			gasometer,
		)?;

		// Build output.
		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			cost: gasometer.used_gas(),
			output: EvmDataWriter::new().write(true).build(),
			logs: LogsBuilder::new(context.address)
				.log3(
					SELECTOR_LOG_APPROVAL,
					context.caller,
					spender,
					EvmDataWriter::new().write(amount).build(),
				)
				.build(),
		})
	}

	fn transfer(
		asset_id: AssetId,
		input: &mut EvmDataReader,
		gasometer: &mut Gasometer,
		context: &Context,
	) -> EvmResult<PrecompileOutput> {
		gasometer.record_log_costs_manual(3, 32)?;

		// Parse input.
		input.expect_arguments(gasometer, 2)?;

		let to: H160 = input.read::<Address>(gasometer)?.into();
		let amount: Balance = input.read::<U256>(gasometer)?.saturated_into();

		// Build call with origin.
		{
			let origin = Runtime::AddressMapping::into_account_id(context.caller);
			let to = Runtime::AddressMapping::into_account_id(to);

			// Dispatch call (if enough gas).
			RuntimeHelper::<Runtime>::try_dispatch(
				Some(origin).into(),
				crml_generic_asset::Call::<Runtime>::transfer { asset_id, to, amount },
				gasometer,
			)?;
		}

		// Build output.
		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			cost: gasometer.used_gas(),
			output: EvmDataWriter::new().write(true).build(),
			logs: LogsBuilder::new(context.address)
				.log3(
					SELECTOR_LOG_TRANSFER,
					context.caller,
					to,
					EvmDataWriter::new().write(amount).build(),
				)
				.build(),
		})
	}

	fn transfer_from(
		asset_id: AssetId,
		input: &mut EvmDataReader,
		gasometer: &mut Gasometer,
		context: &Context,
	) -> EvmResult<PrecompileOutput> {
		gasometer.record_log_costs_manual(3, 32)?;

		// Parse input.
		input.expect_arguments(gasometer, 3)?;
		let from: H160 = input.read::<Address>(gasometer)?.into();
		let to: H160 = input.read::<Address>(gasometer)?.into();
		let amount: Balance = input.read::<U256>(gasometer)?.saturated_into();

		// If caller is "from", it can spend as much as it wants from its own balance.
		if context.caller == from {
			let from: Runtime::AccountId = Runtime::AddressMapping::into_account_id(from.clone());
			let to: Runtime::AccountId = Runtime::AddressMapping::into_account_id(to);
			// Dispatch call (if enough gas).
			RuntimeHelper::<Runtime>::try_dispatch(
				Some(from).into(),
				crml_generic_asset::Call::<Runtime>::transfer { asset_id, to, amount },
				gasometer,
			)?;
		} else {
			// caller not from, check if caller is approved
			gasometer.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
			let current_approved_amount: Balance =
				crml_token_approvals::Module::<Runtime>::erc20_approvals((&from.clone(), &asset_id), &context.caller);
			let new_approved_amount: Balance = current_approved_amount
				.checked_sub(amount)
				.ok_or(error("Caller not approved for amount").into())?;

			if new_approved_amount.is_zero() {
				// New balance is 0, remove approval
				RuntimeHelper::<Runtime>::try_dispatch(
					None.into(),
					crml_token_approvals::Call::<Runtime>::erc20_remove_approval {
						caller: from.clone(),
						asset_id,
						spender: context.caller,
					},
					gasometer,
				)?;
			} else {
				// New balance has changed, update approval to represent difference
				RuntimeHelper::<Runtime>::try_dispatch(
					None.into(),
					crml_token_approvals::Call::<Runtime>::erc20_approval {
						caller: from.clone(),
						spender: context.caller,
						asset_id,
						amount: new_approved_amount,
					},
					gasometer,
				)?;
			}

			let from: Runtime::AccountId = Runtime::AddressMapping::into_account_id(from.clone());
			let to: Runtime::AccountId = Runtime::AddressMapping::into_account_id(to);
			RuntimeHelper::<Runtime>::try_dispatch(
				Some(from).into(),
				crml_generic_asset::Call::<Runtime>::transfer { asset_id, to, amount },
				gasometer,
			)?;
		}

		// Build output.
		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			cost: gasometer.used_gas(),
			output: EvmDataWriter::new().write(true).build(),
			logs: LogsBuilder::new(context.address)
				.log3(
					SELECTOR_LOG_TRANSFER,
					from,
					to,
					EvmDataWriter::new().write(amount).build(),
				)
				.build(),
		})
	}

	fn name(asset_id: AssetId, gasometer: &mut Gasometer) -> EvmResult<PrecompileOutput> {
		gasometer.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		// Build output.
		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			cost: gasometer.used_gas(),
			output: EvmDataWriter::new()
				.write::<Bytes>(
					crml_generic_asset::Pallet::<Runtime>::asset_meta(asset_id)
						.symbol()
						.as_slice()
						.into(),
				)
				.build(),
			logs: Default::default(),
		})
	}

	fn symbol(asset_id: AssetId, gasometer: &mut Gasometer) -> EvmResult<PrecompileOutput> {
		gasometer.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		// Build output.
		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			cost: gasometer.used_gas(),
			output: EvmDataWriter::new()
				.write::<Bytes>(
					crml_generic_asset::Pallet::<Runtime>::asset_meta(asset_id)
						.symbol()
						.as_slice()
						.into(),
				)
				.build(),
			logs: Default::default(),
		})
	}

	fn decimals(asset_id: AssetId, gasometer: &mut Gasometer) -> EvmResult<PrecompileOutput> {
		gasometer.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		// Build output.
		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			cost: gasometer.used_gas(),
			output: EvmDataWriter::new()
				.write::<u8>(crml_generic_asset::Pallet::<Runtime>::asset_meta(asset_id).decimal_places())
				.build(),
			logs: Default::default(),
		})
	}
}
