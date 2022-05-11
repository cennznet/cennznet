/* Copyright 2022 Centrality Investments Limited
*
* Licensed under the LGPL, Version 3.0 (the "License");
* you may not use this file except in compliance with the License.
* Unless required by applicable law or agreed to in writing, software
* distributed under the License is distributed on an "AS IS" BASIS,
* WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
* See the License for the specific language governing permissions and
* limitations under the License.
* You may obtain a copy of the License at the root of this project source code,
* or at:
*     https://centrality.ai/licenses/gplv3.txt
*     https://centrality.ai/licenses/lgplv3.txt
*/
#![cfg_attr(not(feature = "std"), no_std)]

use cennznet_primitives::types::{Balance, FeePreferences};
use crml_support::{ContractExecutor, EthAbiCodec, EthereumStateOracle, MultiCurrency};
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage,
	pallet_prelude::*,
	traits::ExistenceRequirement,
	transactional,
	weights::{constants::RocksDbWeight as DbWeight, Weight},
};
use frame_system::ensure_signed;
use pallet_evm::AddressMapping;
use sp_runtime::traits::Zero;
use sp_std::prelude::*;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
mod types;
use types::*;

pub trait Config: frame_system::Config {
	/// Map evm address into ss58 address
	type AddressMapping: AddressMapping<Self::AccountId>;
	/// Challenge period in blocks for state oracle responses
	type ChallengePeriod: Get<Self::BlockNumber>;
	/// Handles invoking request callbacks
	type ContractExecutor: ContractExecutor<Address = EthAddress>;
	/// The overarching event type.
	type Event: From<Event> + IsType<<Self as frame_system::Config>::Event>;
	/// Multi-currency system
	type MultiCurrency: MultiCurrency<AccountId = Self::AccountId, Balance = Balance>;
	/// Returns the network min gas price
	type MinGasPrice: Get<u64>;
}

decl_storage! {
	trait Store for Module<T: Config> as EthStateOracle {
		/// Unique identifier for remote call requests
		NextRequestId get(fn next_request_id): RequestId;
		/// Requests for remote 'eth_call's keyed by request Id
		Requests get(fn requests): map hasher(twox_64_concat) RequestId => Option<CallRequest>;
		/// Input data for remote calls keyed by request Id
		RequestInputData: map hasher(twox_64_concat) RequestId => Vec<u8>;
		/// Reported return data keyed by request Id
		ResponseReturnData: map hasher(twox_64_concat) RequestId => Vec<u8>;
		/// Reported response details keyed by request Id
		/// These are not necessarily valid until passed the challenge period
		Responses: map hasher(twox_64_concat) RequestId => Option<CallResponse<T::AccountId>>;
		/// Map from block numbers to a list of responses that will be valid at the block (i.e. past the challenged period)
		ResponsesValidAtBlock: map hasher(twox_64_concat) T::BlockNumber => Vec<RequestId>;
		/// Queue of validated responses ready to issue callbacks
		ResponsesForCallback: Vec<RequestId>;
	}
}

decl_event! {
	pub enum Event {
		/// New state oracle request (Caller, Id)
		NewRequest(EthAddress, RequestId),
		/// executing the request callback failed (Id, Reason)
		CallbackErr(RequestId, DispatchError),
		/// executing the callback succeeded (Id, Weight)
		Callback(RequestId, Weight),
	}
}

decl_error! {
	pub enum Error for Module<T: Config> {
		/// A response has already been submitted for this request
		ResponseExists,
		/// Paying for callback gas failed
		InsufficientFundsGas,
		/// Paying the callback bounty to relayer failed
		InsufficientFundsBounty,
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		/// Promote any unchallenged responses as ready for callback
		fn on_initialize(now: T::BlockNumber) -> Weight {
			let mut consumed_weight = DbWeight::get().reads(1);
			if ResponsesValidAtBlock::<T>::contains_key(now) {
				// there responses have passed the challenge period successfully and
				// can be scheduled for callback immediately.
				// The exact timing depends on the capacity idle block space
				for call_request_id in ResponsesValidAtBlock::<T>::take(now) {
					ResponsesForCallback::append(call_request_id);
					consumed_weight += DbWeight::get().writes(1);
				}
			}

			consumed_weight
		}

		/// Try run request callbacks using idle blockspace
		fn on_idle(_now: T::BlockNumber, remaining_weight: Weight) -> Weight {
			if ResponsesForCallback::decode_len().unwrap_or(0).is_zero() {
				return DbWeight::get().reads(1);
			}

			let mut consumed_weight = DbWeight::get().reads(2) + DbWeight::get().writes(2);
			let mut callbacks = ResponsesForCallback::get();
			for call_request_id in callbacks.drain(..) {
				// TODO: improve weight accuracies
				// + remove request/response storages (x4)
				// + 2 GA transfers + maybe fee swaps
				// + EVM execution upto `request.callback_gas_limit`
				let next_weight = DbWeight::get().writes(5) + DbWeight::get().reads(5);
				if consumed_weight + next_weight > remaining_weight {
					return consumed_weight;
				}

				// remove all state related to the request and try to run the callback
				let request = Requests::take(call_request_id).unwrap();
				let response = Responses::<T>::take(call_request_id).unwrap();
				let return_data = ResponseReturnData::take(call_request_id);
				RequestInputData::remove(call_request_id);

				match Self::try_callback(
					call_request_id,
					&request,
					&response.reporter,
					return_data.as_ref(),
				) {
					Ok(info) => {
						let callback_weight = info.actual_weight.unwrap_or(0);
						consumed_weight += callback_weight;
						Self::deposit_event(Event::Callback(
							call_request_id,
							callback_weight,
						));
					},
					Err(info) => {
						let callback_weight = info.post_info.actual_weight.unwrap_or(0);
						consumed_weight += callback_weight;
						Self::deposit_event(Event::CallbackErr(
							call_request_id,
							info.error,
						));
					}
				}
			}
			// write remaining callbacks
			ResponsesForCallback::put(callbacks);

			consumed_weight
		}

		/// Submit response for a a remote call request
		/// `return_data` - the rlp encoded output of the remote call
		/// `eth_block_number` - the ethereum block number the request was made
		///
		/// Caller should be a configured relayer (i.e. authorized or staked)
		/// Only accepts the first response for a given request
		///
		#[weight = 500_000]
		pub fn submit_call_response(origin, request_id: RequestId, return_data: Vec<u8>, eth_block_number: u64) {
			// TODO: check registered relayer
			let origin = ensure_signed(origin)?;

			ensure!(!<Responses<T>>::contains_key(request_id), Error::<T>::ResponseExists);

			let response = CallResponse {
				return_data_digest: sp_io::hashing::blake2_256(return_data.as_slice()),
				eth_block_number,
				reporter: origin,
			};
			<Responses<T>>::insert(request_id, response);
			ResponseReturnData::insert(request_id, return_data);

			let execute_block = <frame_system::Pallet<T>>::block_number() + T::ChallengePeriod::get();
			<ResponsesValidAtBlock<T>>::append(execute_block, request_id);
		}
	}
}

impl<T: Config> Module<T> {
	/// Try to execute a callback
	/// `request` - the original request
	/// `relayer` - the address of the relayer
	/// `return_data` - the returndata of the request (fulfilled by the relayer)
	#[transactional]
	fn try_callback(
		request_id: RequestId,
		request: &CallRequest,
		relayer: &T::AccountId,
		return_data: &[u8],
	) -> DispatchResultWithPostInfo {
		// 1) requestor pays the state oracle request fee to network via gas fees and sets a bounty amount
		// 		bounty should be in cpay, relayers can take jobs at their discretion (free market)
		//
		// we can't buy gas in advance due to the cost of gas changing between blocks
		// so then we must
		// 2) caller precommit to a future gas_limit at time of request
		// 3) pay for this gas_limit at the time of execution from caller account

		let caller_ss58_address = T::AddressMapping::into_account_id(request.caller);
		// 2) payout bounty to the relayer
		// TODO: enable multi-currency payment for bounty & gas
		let _ = T::MultiCurrency::transfer(
			&caller_ss58_address,
			relayer,
			T::MultiCurrency::fee_currency(),
			request.bounty,
			ExistenceRequirement::AllowDeath,
		)
		.map_err(|_| Error::<T>::InsufficientFundsBounty)?;

		// state oracle precompile address
		// TODO: use some shared var from the precompile crate
		let state_oracle_precompile = EthAddress::from_low_u64_be(27572);
		let state_oracle_precompile_ss58_address = T::AddressMapping::into_account_id(state_oracle_precompile);
		// calculate required gas
		let max_fee_per_gas = T::MinGasPrice::get() * request.callback_gas_limit;
		let max_priority_fee_per_gas = 1 * request.callback_gas_limit;
		let total_fee = max_fee_per_gas + max_priority_fee_per_gas;

		// 3) fund `state_oracle_address` for `gas_limit`
		let _ = T::MultiCurrency::transfer(
			&caller_ss58_address,
			&state_oracle_precompile_ss58_address,
			T::MultiCurrency::fee_currency(),
			total_fee.into(),
			ExistenceRequirement::AllowDeath,
		)
		.map_err(|_| Error::<T>::InsufficientFundsGas)?;

		// TODO: return data length > 32 should fail early
		// abi encode callback input
		let mut return_data_ = return_data.to_vec();
		return_data_.resize(32, 0); // cheap way to abi encode bytes32
		let callback_input = [
			request.callback_signature.as_ref(),
			EthAbiCodec::encode(&request_id).as_ref(),
			return_data_.as_ref(),
		]
		.concat();

		T::ContractExecutor::execute(
			&state_oracle_precompile, // evm caller is the state oracle
			&request.caller,          // callback address is the original caller
			callback_input.as_ref(),
			request.callback_gas_limit,
		)
	}
}

impl<T: Config> EthereumStateOracle for Module<T> {
	type Address = EthAddress;
	type RequestId = RequestId;
	/// Create a new remote call request
	/// `caller` - should be `msg.sender` and will pay for the callback fulfillment
	/// `bounty` - CPAY amount as incentive for relayer to fulfil the job
	fn new_request(
		caller: &Self::Address,
		destination: &Self::Address,
		input_data: &[u8],
		callback_signature: &[u8; 4],
		callback_gas_limit: u64,
		fee_preferences: Option<FeePreferences>,
		bounty: Balance,
	) -> Self::RequestId {
		let request_id = NextRequestId::get();
		let request_info = CallRequest {
			caller: *caller,
			destination: *destination,
			input_digest: sp_io::hashing::blake2_256(input_data),
			callback_signature: *callback_signature,
			callback_gas_limit,
			fee_preferences,
			bounty,
		};
		Requests::insert(request_id, request_info);
		RequestInputData::insert(request_id, input_data.to_vec());
		NextRequestId::mutate(|i| *i += U256::from(1));

		request_id
	}
}
