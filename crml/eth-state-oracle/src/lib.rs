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
	scale_wei_to_4dp, ContractExecutor, EthAbiCodec, EthCallFailure, EthCallOracle, EthCallOracleSubscriber,
	EthereumStateOracle, MultiCurrency, H160,
};
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage, log,
	pallet_prelude::*,
	traits::{ExistenceRequirement, UnixTime, WithdrawReasons},
	weights::{constants::RocksDbWeight as DbWeight, Weight},
};
use frame_system::ensure_signed;
use pallet_evm::{AddressMapping, GasWeightMapping};
use sp_runtime::traits::Saturating;
use sp_runtime::traits::UniqueSaturatedInto;
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
	type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
	/// Returns current block time
	type UnixTime: UnixTime;
	/// Multi-currency system
	type MultiCurrency: MultiCurrency<AccountId = Self::AccountId, Balance = Balance, CurrencyId = AssetId>;
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
	/// Minimum bond amount for a relayer
	type RelayerBondAmount: Get<Balance>;
	/// Minimum bond amount for a single challenge
	type ChallengerBondAmount: Get<Balance>;
	/// Maximum number of requests allowed per block. Absolute max = 100
	type MaxRequestsPerBlock: Get<u32>;
}

decl_storage! {
	trait Store for Module<T: Config> as EthStateOracle {
		/// Map from challenge subscription Id to its request
		ChallengeSubscriptions: map hasher(twox_64_concat) ChallengeId => Option<RequestId>;
		/// Unique identifier for remote call requests
		NextRequestId get(fn next_request_id): RequestId;
		/// Requests for remote 'eth_call's keyed by request Id
		Requests get(fn requests): map hasher(twox_64_concat) RequestId => Option<CallRequest>;
		/// Maps from account to balance bonded for relayer responses
		RelayerBonds get(fn relayer_bonds): map hasher(twox_64_concat) T::AccountId => Balance;
		/// Maximum number of active relayers allowed at one time
		MaxRelayerCount get(fn max_relayer_count): u32 = 1;
		/// Maps from account to balance bonded for challengers
		ChallengerBonds get(fn challenger_bonds): map hasher(twox_64_concat) T::AccountId => Balance;
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
		/// Total number of requests that have been made in the current block. Resets in on_initialize
		RequestsThisBlock: u32 = 0;
	}
}

decl_event! {
	pub enum Event<T> where
		<T as frame_system::Config>::AccountId,
	{
		/// New state oracle request (Caller, Id)
		NewRequest(EthAddress, RequestId),
		/// executing the request callback failed (Id, Reason)
		CallbackErr(RequestId, DispatchError),
		/// executing the callback succeeded (Id, Weight)
		Callback(RequestId, Weight),
		/// An account has submitted a relayer bond (AccountId, Balance)
		RelayerBondSet(AccountId, Balance),
		/// An account has submitted a challenger bond (AccountId, Balance)
		ChallengerBondSet(AccountId, Balance),
		/// An account has removed their relayer bond (AccountId, Balance)
		RelayerBondRemoved(AccountId, Balance),
		/// An account has removed their challenger bond (AccountId, Balance)
		ChallengerBondRemoved(AccountId, Balance),
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
		/// Account already has CPay bonded
		AlreadyBonded,
		/// This account doesn't have enough CPay bonded
		NotEnoughBonded,
		/// The account has active responses so can't unbond
		CantUnbond,
		/// The max amount of relayers has been reached
		MaxRelayersReached,
		/// There are not enough challengers to challenge the response
		NoAvailableResponses,
		/// This challenger has an active challenge so can't unbond
		ActiveChallenger,
		/// There are no challengers available to challenge the request
		NoChallengers,
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		/// Promote any unchallenged responses as ready for callback and
		/// remove expired requests
		fn on_initialize(now: T::BlockNumber) -> Weight {
			let mut consumed_weight = DbWeight::get().reads(2) + DbWeight::get().writes(1);
			// Reset number of requests per block
			RequestsThisBlock::put(0);

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
					Self::deposit_event(Event::<T>::CallbackErr(
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
						Self::deposit_event(Event::<T>::Callback(
							call_request_id,
							callback_weight,
						));
						callback_weight
					},
					Err(info) => {
						let callback_weight = info.post_info.actual_weight.unwrap_or(0);
						Self::deposit_event(Event::<T>::CallbackErr(
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

		/// Deposits a bond which is required to submit call responses
		#[weight = 500_000]
		pub fn bond_relayer(origin) {
			let origin = ensure_signed(origin)?;

			// Check account doesn't already have a bond
			if !Self::relayer_bonds(&origin).is_zero() {
				// Account already has CPay bonded
				return Err(Error::<T>::AlreadyBonded.into())
			};

			// Make sure there are relayer slots available
			let max_relayers = Self::max_relayer_count();
			let current_relayer_count = RelayerBonds::<T>::iter().count();
			ensure!(current_relayer_count < max_relayers as usize, Error::<T>::MaxRelayersReached);

			// check user has the requisite funds to make this bid
			let fee_currency = T::MultiCurrency::fee_currency();
			let relayer_bond_amount = T::RelayerBondAmount::get();
			let balance = T::MultiCurrency::free_balance(&origin, fee_currency);
			if let Some(balance_after_bond) = balance.checked_sub(relayer_bond_amount) {
				// TODO: review behaviour with 3.0 upgrade: https://github.com/cennznet/cennznet/issues/414
				// - `amount` is unused
				// - if there are multiple locks on user asset this could return true inaccurately
				// - `T::MultiCurrency::reserve(origin, asset_id, amount)` should be checking this internally...
				let _ = T::MultiCurrency::ensure_can_withdraw(&origin, fee_currency, relayer_bond_amount, WithdrawReasons::RESERVE, balance_after_bond)?;
			}

			// try lock funds
			T::MultiCurrency::reserve(&origin, fee_currency, relayer_bond_amount)?;
			RelayerBonds::<T>::insert(&origin, relayer_bond_amount);
			Self::deposit_event(Event::<T>::RelayerBondSet(origin, relayer_bond_amount));
		}

		/// Unbonds an accounts assets
		#[weight = 500_000]
		pub fn unbond_relayer(origin) {
			let origin = ensure_signed(origin)?;
			// Ensure account has bonded amount
			let bonded_amount: Balance = Self::relayer_bonds(&origin);
			ensure!(!bonded_amount.is_zero(), Error::<T>::NotEnoughBonded);

			// Check that there isn't an existing request for the account
			let responses: Vec<(RequestId, CallResponse<T::AccountId>)> = Responses::<T>::iter().collect();
			for (_, call_response) in responses {
				if call_response.relayer == origin {
					return Err(Error::<T>::CantUnbond.into());
				}
			}

			// Unreserve bonded amount
			T::MultiCurrency::unreserve(&origin, T::MultiCurrency::fee_currency(), bonded_amount);
			RelayerBonds::<T>::remove(&origin);
			Self::deposit_event(Event::<T>::RelayerBondRemoved(origin, bonded_amount));
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
			let origin = ensure_signed(origin)?;
			ensure!(Self::relayer_bonds(&origin) == T::RelayerBondAmount::get(), Error::<T>::NotEnoughBonded);
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

		/// Deposits a bond which is required to submit challenges
		/// call requests can't be made if there are no challengers available to challenge them
		#[weight = 500_000]
		pub fn bond_challenger(origin) {
			let origin = ensure_signed(origin)?;

			// Check account doesn't already have a bond
			if !Self::challenger_bonds(&origin).is_zero() {
				// Account already has CPay bonded
				return Err(Error::<T>::AlreadyBonded.into())
			};

			// check user has the requisite funds to make this bond
			let fee_currency = T::MultiCurrency::fee_currency();
			let challenger_bond_amount = T::ChallengerBondAmount::get();
			// Calculate total bond for a challenger, this is the individual bond amount * the max relayer responses * max relayer count
			let max_concurrent_responses = u128::from(T::MaxRequestsPerBlock::get()).saturating_mul(T::ChallengePeriod::get().unique_saturated_into());
			let total_challenger_bond: Balance = challenger_bond_amount.saturating_mul(max_concurrent_responses);
			if let Some(balance_after_bond) = T::MultiCurrency::free_balance(&origin, fee_currency).checked_sub(total_challenger_bond) {
				// TODO: review behaviour with 3.0 upgrade: https://github.com/cennznet/cennznet/issues/414
				// - `amount` is unused
				// - if there are multiple locks on user asset this could return true inaccurately
				// - `T::MultiCurrency::reserve(origin, asset_id, amount)` should be checking this internally...
				let _ = T::MultiCurrency::ensure_can_withdraw(&origin, fee_currency, total_challenger_bond, WithdrawReasons::RESERVE, balance_after_bond)?;
			}

			// try lock funds
			T::MultiCurrency::reserve(&origin, fee_currency, total_challenger_bond)?;
			ChallengerBonds::<T>::insert(&origin, total_challenger_bond);
			Self::deposit_event(Event::<T>::ChallengerBondSet(origin, total_challenger_bond));
		}

		/// Unbonds an accounts bonded challenger assets
		#[weight = 500_000]
		pub fn unbond_challenger(origin) {
			let origin = ensure_signed(origin)?;
			// Ensure account has bonded amount
			let bonded_amount: Balance = Self::challenger_bonds(&origin);
			ensure!(!bonded_amount.is_zero(), Error::<T>::NotEnoughBonded);

			// Check that there isn't an existing challenge for the account
			let challenged_responses: Vec<(RequestId, T::AccountId)> = ResponsesChallenged::<T>::iter().collect();
			for (_, challenger) in challenged_responses {
				if challenger == origin {
					return Err(Error::<T>::ActiveChallenger.into());
				}
			}

			// Unreserve bonded amount
			T::MultiCurrency::unreserve(&origin, T::MultiCurrency::fee_currency(), bonded_amount);
			ChallengerBonds::<T>::remove(&origin);
			Self::deposit_event(Event::<T>::ChallengerBondRemoved(origin, bonded_amount));
		}

		/// Initiate a challenge on the current response for `request_id`
		/// Valid challenge scenarios are:
		/// - incorrect value
		/// - The block number of the response is stale or from the future
		/// - the block timestamp of the response is inaccurate
		#[weight = 500_000]
		pub fn submit_response_challenge(origin, request_id: RequestId) {
			let origin = ensure_signed(origin)?;
			ensure!(Requests::contains_key(request_id), Error::<T>::NoRequest);
			ensure!(!ResponsesChallenged::<T>::contains_key(request_id), Error::<T>::DuplicateChallenge);

			// Ensure challenger has enough bonded
			let challenger_bond_amount = T::ChallengerBondAmount::get();
			// Calculate total bond for a challenger, this is the individual bond amount * the max relayer responses * max relayer count
			let max_concurrent_responses = u128::from(T::MaxRequestsPerBlock::get()).saturating_mul(T::ChallengePeriod::get().unique_saturated_into());
			let total_challenger_bond: Balance = challenger_bond_amount.saturating_mul(max_concurrent_responses);

			ensure!(Self::challenger_bonds(&origin) == total_challenger_bond, Error::<T>::NotEnoughBonded);

			if let Some(response) = Responses::<T>::get(request_id) {
				let request = Requests::get(request_id).unwrap();
				let challenge_subscription_id = T::EthCallOracle::checked_eth_call(
					&request.destination,
					request.input_data.as_ref(),
					request.timestamp,
					response.eth_block_number,
					// TODO: configure
					// the latest possible challenge can happen after
					// ChallengePeriod blocks + 1
					3_u64,
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
	/// Either the challenger will be slashed or the relayer
	fn on_eth_call_complete(
		_call_id: Self::CallId,
		_validator_return_data: &[u8; 32],
		_block_number: u64,
		_block_timestamp: u64,
	) {
		unimplemented!();
	}

	fn on_eth_call_failed(_call_id: Self::CallId, _reason: EthCallFailure) {
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
	) -> Result<Self::RequestId, DispatchError> {
		// Limit number of requests per block
		ensure!(
			RequestsThisBlock::get() < T::MaxRequestsPerBlock::get(),
			Error::<T>::NoAvailableResponses
		);

		// Ensure there is at least one challenger to challenge the request
		let challenger_count = ChallengerBonds::<T>::iter().count();
		ensure!(!challenger_count.is_zero(), Error::<T>::NoChallengers);

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
		NextRequestId::mutate(|i| *i += U256::from(1u64));
		RequestsThisBlock::mutate(|i| *i += 1);
		RequestsExpiredAtBlock::<T>::append(expiry_block, request_id);

		Ok(request_id)
	}

	/// Return state oracle request fee
	/// This covers the worst case gas consumption
	fn new_request_fee() -> u64 {
		T::GasWeightMapping::weight_to_gas(DbWeight::get().writes(15).saturating_add(DbWeight::get().reads(15)))
	}
}
