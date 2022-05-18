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
use crml_support::{
	scale_wei_to_4dp, ContractExecutor, EthAbiCodec, EthCallOracle, EthCallOracleSubscriber, EthereumStateOracle,
	MultiCurrency, H160,
};
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage, log,
	pallet_prelude::*,
	traits::{ExistenceRequirement, UnixTime},
	weights::{constants::RocksDbWeight as DbWeight, Weight},
};
use frame_system::ensure_signed;
use pallet_evm::{AddressMapping, GasWeightMapping};
use sp_runtime::traits::{SaturatedConversion, Zero};
use sp_std::prelude::*;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
mod types;
use types::*;

pub(crate) const LOG_TARGET: &str = "state-oracle";

// syntactic sugar for logging.
#[macro_export]
macro_rules! log {
	($level:tt, $patter:expr $(, $values:expr)* $(,)?) => {
		log::$level!(
			target: crate::LOG_TARGET,
			$patter $(, $values)*
		)
	};
}

pub trait Config: frame_system::Config {
	/// Map evm address into ss58 address
	type AddressMapping: AddressMapping<Self::AccountId>;
	/// Challenge period in blocks for state oracle responses
	type ChallengePeriod: Get<Self::BlockNumber>;
	/// Handles invoking request callbacks
	type ContractExecutor: ContractExecutor<Address = EthAddress>;
	/// Configured address for the state oracle precompile
	type StateOraclePrecompileAddress: Get<H160>;
	/// Handles verifying challenged responses
	type EthCallOracle: EthCallOracle<Address = EthAddress, CallId = u64>;
	/// The overarching event type.
	type Event: From<Event> + IsType<<Self as frame_system::Config>::Event>;
	/// Returns current block time
	type UnixTime: UnixTime;
	/// Multi-currency system
	type MultiCurrency: MultiCurrency<AccountId = Self::AccountId, Balance = Balance>;
	/// Returns the network min gas price
	type MinGasPrice: Get<u64>;
	/// Convert gas to weight according to runtime config
	type GasWeightMapping: GasWeightMapping;
}

decl_storage! {
	trait Store for Module<T: Config> as EthStateOracle {
		/// Map from challenge subscription Id to its request
		ChallengeSubscriptions: map hasher(twox_64_concat) ChallengeId => Option<RequestId>;
		/// Unique identifier for remote call requests
		NextRequestId get(fn next_request_id): RequestId;
		/// Requests for remote 'eth_call's keyed by request Id
		Requests get(fn requests): map hasher(twox_64_concat) RequestId => Option<CallRequest>;
		/// Input data for remote calls keyed by request Id
		RequestInputData: map hasher(twox_64_concat) RequestId => Vec<u8>;
		/// Reported response details keyed by request Id
		/// These are not necessarily valid until passed the challenge period
		Responses get(fn responses): map hasher(twox_64_concat) RequestId => Option<CallResponse<T::AccountId>>;
		/// Responses that are being actively challenged (value is the challenger)
		ResponsesChallenged: map hasher(twox_64_concat) RequestId => Option<T::AccountId>;
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
		/// The request does not exist (either fulfilled or never did)
		NoRequest,
		/// No response exists
		NoResponse,
		/// Paying for callback gas failed
		InsufficientFundsGas,
		/// Paying the callback bounty to relayer failed
		InsufficientFundsBounty,
		/// Challenge already in progress
		DuplicateChallenge,
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
				// The exact timing depends on the capacity of idle block space
				for call_request_id in ResponsesValidAtBlock::<T>::take(now) {
					ResponsesForCallback::append(call_request_id);
					consumed_weight += DbWeight::get().writes(1);
				}
			}

			consumed_weight
		}

		/// Try run request callbacks using idle block space
		fn on_idle(_now: T::BlockNumber, remaining_weight: Weight) -> Weight {
			if ResponsesForCallback::decode_len().unwrap_or(0).is_zero() {
				return DbWeight::get().reads(1);
			}

			// (weight): + 2 read responses for Callback + ResponsesChallenged
			//           + 1 write ResponsesForCallback will be updated at the end
			let mut consumed_weight = DbWeight::get().reads(2) + DbWeight::get().writes(1);
			let mut processed_callbacks = 0_usize;
			let weight_per_callback = Self::per_callback_weight();
			let callbacks = ResponsesForCallback::get();

			for call_request_id in callbacks.clone() {
				if ResponsesChallenged::<T>::contains_key(call_request_id) {
					// skip running challenged callbacks
					// they'll be rescheduled by the challenge protocol
					continue;
				}
				let request = Requests::get(call_request_id).unwrap();
				// (weight) + max weight the EVM callback will consume
				let next_callback_weight_limit = T::GasWeightMapping::gas_to_weight(request.callback_gas_limit);
				// Check we can process the next callback in this block?
				if consumed_weight
					.saturating_add(weight_per_callback)
					.saturating_add(next_callback_weight_limit) > remaining_weight {
					break;
				}

				// Remove all state related to the request and try to run the callback
				let request = Requests::take(call_request_id).unwrap();
				let response = Responses::<T>::take(call_request_id).unwrap();
				RequestInputData::remove(call_request_id);

				// Try run the callback, recording weight consumed
				let callback_weight = match Self::try_callback(
					call_request_id,
					&request,
					&response.reporter,
					&response.return_data,
				) {
					Ok(info) => {
						let callback_weight = info.actual_weight.unwrap_or(0);
						Self::deposit_event(Event::Callback(
							call_request_id,
							callback_weight,
						));
						callback_weight
					},
					Err(info) => {
						let callback_weight = info.post_info.actual_weight.unwrap_or(0);
						Self::deposit_event(Event::CallbackErr(
							call_request_id,
							info.error,
						));
						callback_weight
					}
				};
				// add total weight for processing this callback
				consumed_weight = consumed_weight
					.saturating_add(weight_per_callback)
					.saturating_add(callback_weight);
				processed_callbacks += 1;
			}
			// write remaining callbacks
			ResponsesForCallback::put(&callbacks[processed_callbacks..]);

			consumed_weight
		}

		/// Submit response for a a remote call request
		/// `return_data` - the rlp encoded output of the remote call (must be padded or truncated to exactly 32 bytes)
		/// `eth_block_number` - the ethereum block number the request was made
		///
		/// Caller should be a configured relayer (i.e. authorized or staked)
		/// Only accepts the first response for a given request
		///
		#[weight = 500_000]
		pub fn submit_call_response(origin, request_id: RequestId, return_data: Vec<u8>, eth_block_number: u64) {
			// TODO: relayer should have some bond
			let origin = ensure_signed(origin)?;

			ensure!(Requests::contains_key(request_id), Error::<T>::NoRequest);
			ensure!(!<Responses<T>>::contains_key(request_id), Error::<T>::ResponseExists);

			let response = CallResponse {
				return_data: return_data_to_bytes32(return_data),
				eth_block_number,
				reporter: origin,
			};
			<Responses<T>>::insert(request_id, response);

			let execute_block = <frame_system::Pallet<T>>::block_number() + T::ChallengePeriod::get();
			<ResponsesValidAtBlock<T>>::append(execute_block, request_id);
		}

		/// Initiate a challenge on the current response for `request_id`
		/// Valid challenge scenarios are:
		/// - incorrect value
		/// - The block number of the response is stale or from the future
		///   request.timestamp - lenience > block.timestamp > request.timestamp + lenience
		#[weight = 500_000]
		pub fn submit_response_challenge(origin, request_id: RequestId) {
			// TODO: challenger should have some bond
			let origin = ensure_signed(origin)?;
			ensure!(Requests::contains_key(request_id), Error::<T>::NoRequest);
			ensure!(!ResponsesChallenged::<T>::contains_key(request_id), Error::<T>::DuplicateChallenge);

			if let Some(response) = Responses::<T>::get(request_id) {
				// after the challenge, either reporter or challenger is slashed and the other rewarded
				// the response will be updated with the value from the oracle check process
				let request = Requests::get(request_id).unwrap();
				let request_input = RequestInputData::get(request_id);
				// SKETCH
				/*
					validators receive request timestamp
					need to decide which block to query for the response so as to minimize queries to Ethereum

					1) query block timestamp at relayer reported block_number
					2a) if block timestamp is in the lenience range then do call_at at the relayer reported block
					2b) if block timestamp is outside the lenience range (the reporter is going to be slashed) we still need to find the right block to query for the true value
					process to find right block number:
					- query the current latest block number from Ethereum
					- assuming avg blocktime eth blocktime of 15 seconds calculate x blocks backwards
					- query the block number closest to and higher than request timestamp i.e. prefer block after the time of request
					3) do the `eth_call` at the correct block
				 */
				let challenge_subscription_id = T::EthCallOracle::call_at(
					&request.destination,
					request_input.as_ref(),
					request.timestamp,
				);
				ResponsesChallenged::<T>::insert(request_id, origin);
				ChallengeSubscriptions::insert(challenge_subscription_id, request_id);
			} else {
				return Err(Error::<T>::NoResponse.into());
			}
		}
	}
}

impl<T: Config> EthCallOracleSubscriber for Module<T> {
	type CallId = u64;
	/// Compare response from relayer with response from validators
	/// Either the challenger will be slashed or
	fn on_call_at_complete(
		call_id: Self::CallId,
		validator_return_data: &[u8; 32],
		block_number: u64,
		block_timestamp: u64,
	) {
		if let Some(request_id) = ChallengeSubscriptions::get(call_id) {
			let response = if let Some(response) = <Responses<T>>::get(request_id) {
				response
			} else {
				return;
			};
			if let Some(challenger) = ResponsesChallenged::<T>::get(request_id) {
				// Validators digest of the return data and the block timestamp
				// - store timestamp with request
				// - trigger validator checks for the correct value
				if &response.return_data != validator_return_data {
					// return data at block did not match what was reported
					// TODO: slash reporter, reward challenger
				}
				let request_timestamp = Requests::get(request_id).unwrap().timestamp;
				// maximum lenience allowed between oracle request time and ethereum block timestamp
				// where the result was reported from
				let LENIENCE = 15_u64;
				// check if block of the report was too far in the past
				let is_stale = block_timestamp < (request_timestamp - LENIENCE);
				if is_stale {
					// TODO: slash reporter, reward challenger
				}
				// check if block of the report was too far in the future
				let is_future = block_timestamp > (request_timestamp + LENIENCE);
				if is_future {
					// TODO: slash reporter, reward challenger
				}

				// nothing wrong with this request
				// TODO: slash challenger

				// TODO: schedule the callback using the oracle values
			}
		}
	}
}

impl<T: Config> Module<T> {
	/// Try to execute a callback
	/// `request` - the original request
	/// `relayer` - the address of the relayer
	/// `return_data` - the returndata of the request (fulfilled by the relayer)
	fn try_callback(
		request_id: RequestId,
		request: &CallRequest,
		relayer: &T::AccountId,
		return_data: &[u8; 32],
	) -> DispatchResultWithPostInfo {
		// The overall gas payment process for state oracle interaction is as follows:
		// 1) requestor pays the state oracle request fee to network via gas fees and sets a bounty amount
		// 	  bounty should be in cpay, relayers can take jobs at their discretion (free market)
		//
		// we can't buy gas in advance due to the potential price of gas changing between blocks, therefore:
		// 2) require caller to precommit to a future gas_limit at time of request
		// 3) pay for this gas_limit at the current price at the time of execution from caller account

		let caller_ss58_address = T::AddressMapping::into_account_id(request.caller);
		log!(
			debug,
			"🔮 preparing callback for: {:?}, caller: {:?}, caller(ss58): {:?}",
			request_id,
			request.caller,
			caller_ss58_address
		);
		// 2) payout bounty to the relayer
		// TODO: enable multi-currency payment for bounty & gas
		log!(
			debug,
			"🔮 paying bounty for callback({:?}), bounty: {:?}",
			request_id,
			request.bounty
		);
		let _ = T::MultiCurrency::transfer(
			&caller_ss58_address,
			relayer,
			T::MultiCurrency::fee_currency(),
			request.bounty,
			ExistenceRequirement::AllowDeath,
		)
		.map_err(|_| Error::<T>::InsufficientFundsBounty)?;

		// state oracle precompile address
		let state_oracle_precompile = T::StateOraclePrecompileAddress::get();
		let state_oracle_precompile_ss58_address = T::AddressMapping::into_account_id(state_oracle_precompile);
		// calculate required gas
		let max_fee_per_gas = U256::from(T::MinGasPrice::get());
		let max_priority_fee_per_gas = U256::one();

		// `min_fee_per_gas` and `max_priority_fee_per_gas` are expressed in wei, scale to 4dp to work with CPAY amounts
		let total_fee: Balance = scale_wei_to_4dp(
			(max_fee_per_gas * request.callback_gas_limit + max_priority_fee_per_gas).saturated_into(),
		);

		// 3) fund `state_oracle_address` for `gas_limit`
		// The caller could be underpaying for gas here, if so the execution will fail when the EVM handles the fee withdrawal
		log!(
			debug,
			"🔮 required gas fee for callback({:?}), total_fee: {:?}, gas_limit: {:?}",
			request_id,
			total_fee,
			request.callback_gas_limit
		);
		let _ = T::MultiCurrency::transfer(
			&caller_ss58_address,
			&state_oracle_precompile_ss58_address,
			T::MultiCurrency::fee_currency(),
			total_fee,
			ExistenceRequirement::AllowDeath,
		)
		.map_err(|_| Error::<T>::InsufficientFundsGas)?;

		// abi encode callback input
		let callback_input = [
			request.callback_signature.as_ref(),
			EthAbiCodec::encode(&request_id).as_ref(),
			return_data, // bytes32 are encoded as is
		]
		.concat();

		log!(
			debug,
			"🔮 execute callback {:?}, input: {:?}",
			request_id,
			callback_input
		);
		T::ContractExecutor::execute(
			&state_oracle_precompile, // evm caller is the state oracle
			&request.caller,          // callback address is the original caller
			callback_input.as_ref(),
			request.callback_gas_limit,
			max_fee_per_gas,
			max_priority_fee_per_gas,
		)
	}
	/// Returns the ~weight per callback
	fn per_callback_weight() -> Weight {
		// (weight) + all reads and clean up of request/response storage
		DbWeight::get().writes(3) +
		DbWeight::get().reads(4) +
		// (weight) + 2x GA transfer weight (bounty + gas fee transfer)
		(2 * 203_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(8 as Weight))
			.saturating_add(DbWeight::get().writes(5 as Weight))
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
			callback_signature: *callback_signature,
			callback_gas_limit,
			fee_preferences,
			bounty,
			timestamp: T::UnixTime::now().as_secs(),
		};
		Requests::insert(request_id, request_info);
		RequestInputData::insert(request_id, input_data.to_vec());
		NextRequestId::mutate(|i| *i += U256::from(1));

		request_id
	}

	/// Return state oracle request fee
	/// This covers the worst case gas consumption
	fn new_request_fee() -> u64 {
		T::GasWeightMapping::weight_to_gas(DbWeight::get().writes(15).saturating_add(DbWeight::get().reads(15)))
	}
}
