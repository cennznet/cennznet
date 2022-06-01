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

use cennznet_primitives::types::{AssetId, Balance, FeeExchange, FeePreferences};
use crml_support::{
	scale_wei_to_4dp, ContractExecutor, EthAbiCodec, EthCallOracle, EthCallOracleSubscriber, EthereumStateOracle,
	MultiCurrency, ReturnDataClaim, H160,
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
mod tests;
mod types;
use cennznet_primitives::traits::BuyFeeAsset;
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
	/// Convert fee preference into payment asset
	type BuyFeeAsset: BuyFeeAsset<
		AccountId = Self::AccountId,
		Balance = Balance,
		FeeExchange = FeeExchange<AssetId, Balance>,
	>;
}

decl_storage! {
	trait Store for Module<T: Config> as EthStateOracle {
		/// Map from challenge subscription Id to its request
		ChallengeSubscriptions: map hasher(twox_64_concat) ChallengeId => Option<RequestId>;
		/// Unique identifier for remote call requests
		NextRequestId get(fn next_request_id): RequestId;
		/// Requests for remote 'eth_call's keyed by request Id
		Requests get(fn requests): map hasher(twox_64_concat) RequestId => Option<CallRequest>;
		/// Reported response details keyed by request Id
		/// These are not necessarily valid until passed the challenge period
		Responses get(fn responses): map hasher(twox_64_concat) RequestId => Option<CallResponse<T::AccountId>>;
		/// Responses that are being actively challenged (value is the challenger)
		ResponsesChallenged: map hasher(twox_64_concat) RequestId => Option<T::AccountId>;
		/// Map from block number to a list of responses that will be valid at the block (i.e. past the challenged period)
		ResponsesValidAtBlock: map hasher(twox_64_concat) T::BlockNumber => Vec<RequestId>;
		/// Map from block number to a list of requests that will expire at the block (if no responses submitted)
		RequestsExpiredAtBlock: map hasher(twox_64_concat) T::BlockNumber => Vec<RequestId>;
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
		/// The request does not exist (either expired, fulfilled, or never did)
		NoRequest,
		/// No response exists
		NoResponse,
		/// Paying for callback gas failed
		InsufficientFundsGas,
		/// Paying the callback bounty to relayer failed
		InsufficientFundsBounty,
		/// Timestamp is either stale or from the future
		InvalidResponseTimestamp,
		/// Challenge already in progress
		DuplicateChallenge,
		/// Return data exceeded the 32 byte limit
		ReturnDataExceedsLimit,
		/// The request did not receive any relayed response in the alloted time
		RequestExpired,
		/// Failed to exchange fee preference to CPay
		FeeExchangeFailed,
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		/// Promote any unchallenged responses as ready for callback and
		/// remove expired requests
		fn on_initialize(now: T::BlockNumber) -> Weight {
			let mut consumed_weight = DbWeight::get().reads(2);
			if ResponsesValidAtBlock::<T>::contains_key(now) {
				// these responses have passed the challenge period successfully and
				// can be scheduled for callback immediately.
				// The exact timing depends on the capacity of idle block space
				for call_request_id in ResponsesValidAtBlock::<T>::take(now) {
					ResponsesForCallback::append(call_request_id);
					consumed_weight += DbWeight::get().writes(1);
				}
			}
			// requests marked to expire
			if RequestsExpiredAtBlock::<T>::contains_key(now) {
				for expired_request_id in RequestsExpiredAtBlock::<T>::take(now) {
					Requests::remove(expired_request_id);
					Self::deposit_event(Event::CallbackErr(
						expired_request_id,
						Error::<T>::RequestExpired.into(),
					));
					consumed_weight += DbWeight::get().writes(3);
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
				} else {
					processed_callbacks += 1;
				}

				// Remove all state related to the request and try to run the callback
				Requests::remove(call_request_id);
				let response = Responses::<T>::take(call_request_id).unwrap();

				// Try run the callback, recording weight consumed
				let callback_weight = match Self::try_callback(
					call_request_id,
					&request,
					&response,
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
			}
			// write remaining callbacks
			ResponsesForCallback::put(&callbacks[processed_callbacks..]);

			consumed_weight
		}

		/// Submit response for a a remote call request
		///
		/// `return_data` - the claimed `returndata` of the `eth_call` RPC using the requested contract and input buffer
		/// `eth_block_number` - the ethereum block number where the 'returndata' was obtained
		/// `eth_block_timestamp` - the ethereum block timestamp where the 'returndata' was obtained (unix timestamp, seconds)
		///
		/// Caller should be a configured relayer (i.e. authorized or staked)
		/// Only accepts the first response for a given request
		/// The response is not valid until `T::ChallengePeriod` blocks have passed
		///
		#[weight = 500_000]
		pub fn submit_call_response(origin, request_id: RequestId, return_data: ReturnDataClaim, eth_block_number: u64, eth_block_timestamp: u64) {
			// TODO: relayer should have some bond
			let origin = ensure_signed(origin)?;

			ensure!(Requests::contains_key(request_id), Error::<T>::NoRequest);
			ensure!(!<Responses<T>>::contains_key(request_id), Error::<T>::ResponseExists);

			// The `return_data` from a valid contract call is always Ethereum abi encoded as a 32 byte word for _fixed size_ data types
			// e.g. bool, uint<N>, bytes<N>, address etc.
			//
			// This table describes the transformation and proper decoding approaches:
			//
			// | abi type             | real value                                   | abi encoded value                                                    | decoding approach              |
			// |----------------------|----------------------------------------------|----------------------------------------------------------------------|--------------------------------|
			// | `uint256`            | `10113`                                      | `0x0000000000000000000000000000000000000000000000000000000000002781` | `uint256(returnData)`          |
			// | `address`(`uint160`) | `0x111122223333444455556666777788889999aAaa` | `0x000000000000000000000000111122223333444455556666777788889999aAaa` | `address(uint160(returnData))` |
			// | `bool`(`uint8`)      | `true`                                       | `0x0000000000000000000000000000000000000000000000000000000000000001` | `bool(uint8(returnData))`      |
			// | `bytes4`             | `0x61620000`                                 | `0x6162000000000000000000000000000000000000000000000000000000000000` | `bytes4(returnData)`           |
			//
			// if the type is dynamic i.e `[]T` or `bytes`, `returndata` length will be 32 * (offset + length + n)

			if let Some(request) = Requests::get(request_id) {
				// ~ average ethereum block time
				let eth_block_time_s = 15;
				// check response timestamp is within a sensible bound +3/-2 Ethereum blocks from the request timestamp
				ensure!(
					eth_block_timestamp >= (request.timestamp.saturating_sub(2 * eth_block_time_s)) &&
					eth_block_timestamp <= (request.timestamp.saturating_add(3 * eth_block_time_s)),
					Error::<T>::InvalidResponseTimestamp
				);

				let response = CallResponse {
					return_data,
					eth_block_number,
					eth_block_timestamp,
					relayer: origin,
				};
				<Responses<T>>::insert(request_id, response);
				let execute_block = <frame_system::Pallet<T>>::block_number() + T::ChallengePeriod::get();
				<ResponsesValidAtBlock<T>>::append(execute_block, request_id);

				// The request will not be expired
				RequestsExpiredAtBlock::<T>::mutate(T::BlockNumber::from(request.expiry_block), |r| {
					r.retain(|&x| x != request_id);
				});
			}
		}

		/// Initiate a challenge on the current response for `request_id`
		/// Valid challenge scenarios are:
		/// - incorrect value
		/// - The block number of the response is stale or from the future
		/// - the block timestamp of the response is inaccurate
		#[weight = 500_000]
		pub fn submit_response_challenge(origin, request_id: RequestId) {
			// TODO: challenger should have some bond
			let origin = ensure_signed(origin)?;
			ensure!(Requests::contains_key(request_id), Error::<T>::NoRequest);
			ensure!(!ResponsesChallenged::<T>::contains_key(request_id), Error::<T>::DuplicateChallenge);

			if Responses::<T>::contains_key(request_id) {
				let request = Requests::get(request_id).unwrap();
				let challenge_subscription_id = T::EthCallOracle::call_at(
					&request.destination,
					request.input_data.as_ref(),
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
		validator_return_data: &ReturnDataClaim,
		block_number: u64,
		block_timestamp: u64,
	) {
		unimplemented!();
	}
}

impl<T: Config> Module<T> {
	/// Try to execute a callback
	/// `request_id` - the request identifier
	/// `request` - the original request
	/// `response` - the response info (validated)
	fn try_callback(
		request_id: RequestId,
		request: &CallRequest,
		response: &CallResponse<T::AccountId>,
	) -> DispatchResultWithPostInfo {
		// check returndata type
		let return_data = match response.return_data {
			ReturnDataClaim::Ok(return_data) => return_data,
			// this returndata exceeded the length limit so it will not be processed
			ReturnDataClaim::ExceedsLengthLimit => return Err(Error::<T>::ReturnDataExceedsLimit.into()),
		};

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

		// Exchange for fee asset if fee preferences have been set
		if let Some(fee_preferences) = &request.fee_preferences {
			let exchange_total = request.bounty + total_fee;
			let max_payment = exchange_total.saturating_add(fee_preferences.slippage * exchange_total);
			let exchange = FeeExchange::new_v1(fee_preferences.asset_id, max_payment);
			T::BuyFeeAsset::buy_fee_asset(&caller_ss58_address, exchange_total, &exchange)?;
			log!(
				debug,
				"🔮 exchanging fee preference asset: {:?}",
				fee_preferences.asset_id,
			);
		}

		// 2) payout bounty to the relayer
		log!(
			debug,
			"🔮 paying bounty for callback({:?}), bounty: {:?}",
			request_id,
			request.bounty
		);
		let _ = T::MultiCurrency::transfer(
			&caller_ss58_address,
			&response.relayer,
			T::MultiCurrency::fee_currency(),
			request.bounty,
			ExistenceRequirement::AllowDeath,
		)
		.map_err(|_| Error::<T>::InsufficientFundsBounty)?;

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

		// abi encode callback input `<callbackSelector>(uint256 requestId, uint256 timestamp, bytes32 returnData)`
		let callback_input = [
			request.callback_signature.as_ref(),
			EthAbiCodec::encode(&request_id).as_ref(),
			EthAbiCodec::encode(&response.eth_block_timestamp).as_ref(),
			&return_data, // bytes32 are encoded as is
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
		// The request will expire after `ChallengePeriod` blocks if no response it submitted
		let expiry_block = <frame_system::Pallet<T>>::block_number() + T::ChallengePeriod::get();
		let request_info = CallRequest {
			caller: *caller,
			destination: *destination,
			callback_signature: *callback_signature,
			callback_gas_limit,
			fee_preferences,
			bounty,
			timestamp: T::UnixTime::now().as_secs(),
			input_data: input_data.to_vec(),
			expiry_block: expiry_block.saturated_into(),
		};
		Requests::insert(request_id, request_info);
		NextRequestId::mutate(|i| *i += U256::from(1));
		RequestsExpiredAtBlock::<T>::append(expiry_block, request_id);

		request_id
	}

	/// Return state oracle request fee
	/// This covers the worst case gas consumption
	fn new_request_fee() -> u64 {
		T::GasWeightMapping::weight_to_gas(DbWeight::get().writes(15).saturating_add(DbWeight::get().reads(15)))
	}
}
