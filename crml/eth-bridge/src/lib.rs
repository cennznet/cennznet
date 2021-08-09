/* Copyright 2021 Centrality Investments Limited
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

//! CENNZnet Eth Bridge
//!
//! Deposits Overview:
//!
//! 1) Claimants deposit ERC20 tokens to a paired bridging contract on Ethereum.
//! 2) After waiting for block confirmations, claimants submit the Ethereum transaction hash and deposit event info using `erc20_deposit_claim`
//! 3) Validators aka 'Notaries' in this context, run an OCW protocol using Ethereum full nodes to verify
//! the deposit has occurred
//! 4) after a threshold of notarizations have been received for a claim, the tokens are released to the beneficiary account
//!
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod tests;

mod types;
use types::*;

use cennznet_primitives::types::{AssetId, Balance, BlockNumber};
use codec::{Decode, Encode};
use crml_support::MultiCurrency;
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage, ensure, log,
	traits::{Get, OneSessionHandler, UnixTime, ValidatorSet},
	transactional,
	weights::Weight,
	Parameter,
};
use frame_system::{
	ensure_none, ensure_signed,
	offchain::{CreateSignedTransaction, SubmitTransaction},
};
use sp_runtime::{
	offchain as rt_offchain,
	offchain::StorageKind,
	traits::{AccountIdConversion, MaybeSerializeDeserialize, Member, SaturatedConversion, Zero},
	transaction_validity::{InvalidTransaction, TransactionSource, TransactionValidity, ValidTransaction},
	KeyTypeId, DispatchResult, Percent, RuntimeAppPublic,
};
use sp_std::{convert::TryInto, prelude::*};

pub const ETH_BRIDGE: KeyTypeId = KeyTypeId(*b"eth-");

pub mod crypto {
	mod app_crypto {
		use crate::ETH_BRIDGE;
		use sp_application_crypto::{app_crypto, ecdsa};
		app_crypto!(ecdsa, ETH_BRIDGE);
	}
	sp_application_crypto::with_pair! {
		/// An eth bridge keypair using ecdsa as its crypto.
		pub type AuthorityPair = app_crypto::Pair;
	}
	/// An eth bridge signature using ecdsa as its crypto.
	pub type AuthoritySignature = app_crypto::Signature;
	/// An eth bridge identifier using ecdsa as its crypto.
	pub type AuthorityId = app_crypto::Public;
}

/// The type to sign and send transactions.
const UNSIGNED_TXS_PRIORITY: u64 = 100;
/// Max notarization claims to attempt per block/OCW invocation
const CLAIMS_PER_BLOCK: u64 = 3;
/// Deadline for any network requests e.g.to Eth JSON-RPC endpoint
const REQUEST_TTL_MS: u64 = 1_500;
/// Bucket claims in intervals of this factor (seconds)
const BUCKET_FACTOR_S: u64 = 3_600; // 1 hour
/// Number of blocks between claim pruning
const CLAIM_PRUNING_INTERVAL: BlockNumber = BUCKET_FACTOR_S as u32 / 5_u32;

pub(crate) const LOG_TARGET: &'static str = "eth-bridge";

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

/// This is the pallet's configuration trait
pub trait Config: frame_system::Config + CreateSignedTransaction<Call<Self>> {
	/// The identifier type for an authority in this module (i.e. active validator session key)
	type AuthorityId: Member + Parameter + AsRef<[u8]> + RuntimeAppPublic + Default + Ord + MaybeSerializeDeserialize;
	/// Knows the active authority set (validator stash addresses)
	type AuthoritySet: ValidatorSet<Self::AccountId>;
	/// Returns the block timestamp
	type UnixTime: UnixTime;
	/// Currency functions
	type MultiCurrency: MultiCurrency<AccountId = Self::AccountId, Balance = Balance, CurrencyId = AssetId>;
	/// The minimum number of transaction confirmations needed to ratify an Eth message
	type MessageConfirmations: Get<u16>;
	/// Messages cannot be claimed after this time (seconds)
	type MessageDeadline: Get<u64>;
	/// The threshold of notarizations required to approve an Eth message
	type NotarizationThreshold: Get<Percent>;
	/// 
	type MessageCallbackRouter;
	/// The overarching call type.
	type Call: From<Call<Self>>;
	/// The overarching event type.
	type Event: From<Event> + Into<<Self as frame_system::Config>::Event>;
}

decl_storage! {
	trait Store for Module<T: Config> as EthBridge {
		/// Required % of validator support to signal readiness (default: 66%)
		ActivationThreshold get(fn activation_threshold) config(): Percent = Percent::from_parts(66);
		/// Message data
		MessageData get(fn message_data): map hasher(twox_64_concat) MessageId => Option<Vec<u8>>;
		/// Map from message type to handler config
		MessageHandlerConfig get(fn message_handler_config): map hasher(twox_64_concat) MessageType => Option<HandlerConfig>;
		/// Notarizations for queued messages
		/// Either: None = no notarization exists OR Some(yay/nay)
		MessageNotarizations get(fn message_notarizations): double_map hasher(twox_64_concat) MessageId, hasher(twox_64_concat) T::AuthorityId => Option<bool>;
		/// Queued messages, awaiting notarization
		MessageQueue get(fn message_queue): map hasher(twox_64_concat) MessageId => EthHash;
		/// Id of the next Eth bridge message
		NextMessageId get(fn next_message_id): MessageId;
		/// Active notary (validator) public keys
		NotaryKeys get(fn notary_keys): Vec<T::AuthorityId>;
		/// Processed messages bucketed by unix timestamp of the most recent hour.
		// Used in conjunction with `MessageDeadline` to prevent "double spends".
		// After a bucket is older than the deadline, any messages prior are considered expired.
		// This allows the record of processed messages to be pruned from state regularly
		ProcessedMessageBuckets get(fn processed_message_buckets): double_map hasher(twox_64_concat) u64, hasher(identity) EthHash => ();
	}
}

decl_event! {
	pub enum Event {
		/// Verifying an event succeeded
		Verified(MessageId),
		/// Verifying an event failed
		Invalid(MessageId),
	}
}

decl_error! {
	pub enum Error for Module<T: Config> {
		// Error returned when making signed transactions in off-chain worker
		NoLocalSigningAccount,
		// Error returned when making unsigned transactions with signed payloads in off-chain worker
		OffchainUnsignedTxSignedPayload,
		/// A notarization was invalid
		InvalidNotarization,
		// Error returned when fetching github info
		HttpFetch,
		/// Claim was invalid
		InvalidClaim,
		/// offchain worker not configured properly
		OcwConfig,
		/// This message has already been notarized
		AlreadyNotarized
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		fn on_initialize(block_number: T::BlockNumber) -> Weight {
			// Prune claim storage every hour on CENNZnet (BUCKET_FACTOR_S / 5 seconds = 720 blocks)
			if (block_number % T::BlockNumber::from(CLAIM_PRUNING_INTERVAL)).is_zero() {
				// Find the bucket to expire
				let now = T::UnixTime::now().as_millis().saturated_into::<u64>();
				let expired_bucket_index = (now - T::MessageDeadline::get()) % BUCKET_FACTOR_S;
				ProcessedMessageBuckets::remove_prefix(expired_bucket_index);
				// TODO: better estimate
				50_000_000 as Weight
			} else {
				Zero::zero()
			}
		}

		#[weight = 1_000_000]
		#[transactional]
		/// Internal only
		/// Validators will submit inherents with their notarization vote for a given claim
		pub fn submit_notarization(origin, payload: NotarizationPayload, _signature: <<T as Config>::AuthorityId as RuntimeAppPublic>::Signature) {
			let _ = ensure_none(origin)?;

			// we don't need to verify the signature here because it has been verified in
			//`validate_unsigned` function when sending out the unsigned tx.
			let notary_keys = Self::notary_keys();
			let notary_public_key = match notary_keys.get(payload.authority_index as usize) {
				Some(id) => id,
				None => return Err(Error::<T>::InvalidNotarization.into()),
			};
			<MessageNotarizations<T>>::insert::<MessageId, T::AuthorityId, bool>(payload.message_id, notary_public_key.clone(), payload.is_valid);

			// Count notarization votes
			let notary_count = T::AuthoritySet::validators().len() as u32;
			let mut yay_count = 0_u32;
			let mut nay_count = 0_u32;
			for (_id, is_valid) in <MessageNotarizations<T>>::iter_prefix(payload.message_id) {
				match is_valid {
					true => yay_count += 1,
					false => nay_count += 1,
				}
			}

			// Claim is invalid (nays > (100% - NotarizationThreshold))
			if Percent::from_rational_approximation(nay_count, notary_count) > (Percent::from_parts(100_u8 - T::NotarizationThreshold::get().deconstruct())) {
				// event did not notarize / failed, clean up
				let message_data = MessageData::take(payload.message_id);
				if message_data.is_none() {
					// this should never happen
					log!(error, "💎 unexpected empty claim");
					return Err(Error::<T>::InvalidClaim.into())
				}
				<MessageNotarizations<T>>::remove_prefix(payload.message_id);
				MessageQueue::remove(payload.message_id);
				Self::deposit_event(Event::Invalid(payload.message_id));
				return Ok(());
			}

			// Claim is valid
			if Percent::from_rational_approximation(yay_count, notary_count) >= T::NotarizationThreshold::get() {
				let message_data = MessageData::take(payload.message_id);
				if message_data.is_none() {
					// this should never happen
					log!(error, "💎 unexpected empty claim");
					return Err(Error::<T>::InvalidClaim.into())
				}
				// no need to track info on this claim any more since it's approved
				<MessageNotarizations<T>>::remove_prefix(payload.message_id);
				let eth_tx_hash = MessageQueue::take(payload.message_id);
				let message_data = message_data.unwrap();

				// note this tx as completed
				let bucket_index = T::UnixTime::now().as_millis().saturated_into::<u64>() % BUCKET_FACTOR_S;
				ProcessedMessageBuckets::insert(bucket_index, eth_tx_hash, ());

				// TODO: dispatch callback success
				if let Ok(call) = T::Call::decode(&mut &MessageHandlerConfig::get(message_type)[..]) {
					let ok = call.dispatch(frame_system::RawOrigin::Root.into()).is_ok();
				}

				Self::deposit_event(Event::Verified(payload.message_id));
			}
		}

		fn offchain_worker(block_number: T::BlockNumber) {
			log!(trace, "💎 entering off-chain worker: {:?}", block_number);
			// TODO: remove this
			log!(trace, "💎 active notaries: {:?}", Self::notary_keys());

			// check local `key` is a valid bridge notary
			if !sp_io::offchain::is_validator() {
				// this passes if flag `--validator` set not necessarily
				// in the active set
				log!(info, "💎 not a validator, exiting");
				return
			}

			let supports = NotaryKeys::<T>::decode_len().unwrap_or(0);
			let needed = Self::activation_threshold();
			let total = T::AuthoritySet::validators().len();
			if Percent::from_rational_approximation(supports, total) < needed {
				log!(info, "💎 waiting for validator support to activate eth bridge: {:?}/{:?}", supports, needed);
				return;
			}

			// Get all signing keys for this protocol 'KeyTypeId'
			let keys = T::AuthorityId::all();
			let key = match keys.len() {
				0 => {
					log!(error, "💎 no signing keys for: {:?}, cannot participate in notarization!", T::AuthorityId::ID);
					return
				},
				1 => keys[0].clone(),
				_ => {
					// expect at most one key to be present
					log!(error, "💎 multiple signing keys detected for: {:?}, bailing...", T::AuthorityId::ID);
					return
				},
			};

			// check if locally known keys are in the active validator set
			let authority_index = Self::notary_keys().iter().position(|k| k == &key);
			if authority_index.is_none() {
				log!(error, "💎 no active validator keys, exiting");
				return;
			}
			let authority_index = authority_index.unwrap() as u16;

			// check all pending claims we have _yet_ to notarize and try to notarize them
			// this will be invoked once every block
			// we limit the total claims per invocation using `CLAIMS_PER_BLOCK` so we don't stall block production
			let mut budget = CLAIMS_PER_BLOCK;
			for (message_id, tx_hash) in MessageQueue::iter() {
				if budget.is_zero() {
					log!(info, "💎 claims budget exceeded, exiting...");
					return
				}

				// check we haven't notarized this already
				if <MessageNotarizations<T>>::contains_key::<MessageId, T::AuthorityId>(message_id, key.clone()) {
					log!(trace, "💎 already cast notarization for claim: {:?}, ignoring...", message_id);
				}

				if let Some(message_data) = Self::message_data(message_id) {
					// TODO: pass details for this message type
					let result = Self::offchain_verify_event(tx_hash, message_data);
					log!(trace, "💎 claim verification status: {:?}", result);
					let payload = NotarizationPayload {
						message_id,
						authority_index,
						is_valid: result.is_ok()
					};
					let _ = Self::offchain_send_notarization(&key, payload)
						.map_err(|err| {
							log!(error, "💎 sending notarization failed 🙈, {:?}", err);
						})
						.map(|_| {
							log!(info, "💎 sent notarization: '{:?}' for claim: {:?}", result.is_ok(), message_id);
						});
					budget = budget.saturating_sub(1);
				} else {
					// should not happen, defensive only
					log!(error, "💎 empty claim data for: {:?}", message_id);
				}
			}

			log!(trace, "💎 exiting off-chain worker");
		}

	}
}

/// POssible failure outcomes from attempting to verify eth deposit claims
#[derive(Debug, PartialEq, Clone)]
enum ClaimFailReason {
	/// Couldn't request data from the Eth client
	DataProvider,
	/// The eth tx is marked failed
	TxStatusFailed,
	/// The transaction recipient was not the bridge contract
	InvalidBridgeAddress,
	/// The expected tx logs were not present
	NoTxLogs,
	/// Not enough block confirmations yet
	NotEnoughConfirmations,
	/// Tx event logs indicated this claim does not match the event
	ProvenInvalid,
	/// The deposit tx is past the expiration deadline
	Expired,
}

impl<T: Config> Module<T> {
	/// Submit a bridge deposit claim for an ethereum tx hash
	/// The deposit details must be provided for cross-checking by notaries
	/// Any caller may initiate a claim while only the intended beneficiary will be paid.
	pub fn verify_event(request: VerifyEventRequest) -> DispatchResult {
		// fail a claim if it's already been claimed
		let bucket_index = request.timestamp.as_u64() % BUCKET_FACTOR_S; // checked timestamp < u64
		ensure!(!ProcessedMessageBuckets::contains_key(bucket_index, request.tx_hash), Error::<T>::AlreadyNotarized);

		let message_id = Self::next_message_id();
		MessageData::insert(message_id, request.event_data);
		MessageQueue::insert(message_id, request.tx_hash);
		NextMessageId::put(message_id.wrapping_add(1));

		Ok(())
	}
	/// Verify a message
	/// `tx_hash` - The ethereum tx hash
	/// `message_data` - The claimed message data
	/// `message_handler_config` - Details of the message
	/// Checks:
	/// - check Eth full node for transaction status
	/// - tx success
	/// - tx sent to deposit contract address
	/// - check for log with deposited amount and token type
	/// - confirmations `>= T::MessageConfirmations`
	/// - message has not expired older than `T::MessageDeadline`
	fn offchain_verify_event(tx_hash: EthHash, message_data: Vec<u8>, message_handler_config: HandlerConfig) -> Result<(), ClaimFailReason> {
		let result = Self::get_transaction_receipt(tx_hash);
		if let Err(err) = result {
			log!(error, "💎 eth_getTransactionReceipt({:?}) failed: {:?}", tx_hash, err);
			return Err(ClaimFailReason::DataProvider);
		}

		let maybe_tx_receipt = result.unwrap(); // error handled above qed.
		let tx_receipt = match maybe_tx_receipt {
			Some(tx_receipt) => tx_receipt,
			None => return Err(ClaimFailReason::NoTxLogs),
		};
		let status = tx_receipt.status.unwrap_or_default();
		if status.is_zero() {
			return Err(ClaimFailReason::TxStatusFailed);
		}

		if tx_receipt.to != Some(message_handler_config.contract_address.into()) {
			return Err(ClaimFailReason::InvalidBridgeAddress);
		}

		let topic: EthHash = message_handler_config.event_signature.into();
		// search for a bridge deposit event in this tx receipt
		let matching_log = tx_receipt
			.logs
			.iter()
			.find(|log| log.transaction_hash == Some(tx_hash) && log.topics.contains(&topic));

		if let Some(log) = matching_log {
			// check if the ethereum deposit event matches what was reported
			// in the original claim
			if log.data != message_data {
				log!(
					trace,
					"💎 mismatch in message vs. event. reported: {:?} observed: {:?}",
					message_data,
					log.data,
				);
				return Err(ClaimFailReason::ProvenInvalid);
			}
		}

		// lastly, have we got enough block confirmations to be re-org safe?
		let observed_block = tx_receipt.block_number.saturated_into();
		let result = Self::get_block(observed_block);
		if let Err(err) = result {
			log!(error, "💎 eth_getBlockByNumber failed: {:?}", err);
			return Err(ClaimFailReason::DataProvider);
		}
		let maybe_block = result.unwrap();
		if maybe_block.is_none() {
			return Err(ClaimFailReason::DataProvider);
		}
		let block = maybe_block.unwrap();
		let latest_block_number: u64 = block.number.unwrap_or_default().as_u64();
		let block_confirmations = latest_block_number.saturating_sub(observed_block);
		if block_confirmations < T::MessageConfirmations::get() as u64 {
			return Err(ClaimFailReason::NotEnoughConfirmations);
		}
		// claim is past the expiration deadline
		// ` reported_claim_event.timestamp` < u64 checked in `erc20_deposit_claim`
		if T::UnixTime::now().as_millis().saturated_into::<u64>() - block.timestamp.as_u64()
			> T::MessageDeadline::get()
		{
			return Err(ClaimFailReason::Expired);
		}

		// it's ok!
		return Ok(());
	}

	/// Get transaction receipt from eth client
	fn get_transaction_receipt(tx_hash: EthHash) -> Result<Option<TransactionReceipt>, Error<T>> {
		let random_request_id = u32::from_be_bytes(sp_io::offchain::random_seed()[..4].try_into().unwrap());
		let request = GetTxReceiptRequest::new(tx_hash, random_request_id as usize);
		let resp_bytes = Self::query_eth_client(Some(request)).map_err(|e| {
			log!(error, "💎 read eth-rpc API error: {:?}", e);
			<Error<T>>::HttpFetch
		})?;

		let resp_str = core::str::from_utf8(&resp_bytes).map_err(|_| {
			log!(error, "💎 response invalid utf8: {:?}", resp_bytes);
			<Error<T>>::HttpFetch
		})?;

		// Deserialize JSON to struct
		serde_json::from_str::<EthResponse<TransactionReceipt>>(resp_str)
			.map(|resp| resp.result)
			.map_err(|err| {
				log!(error, "💎 deserialize json response error: {:?}", err);
				<Error<T>>::HttpFetch
			})
	}

		/// Get latest block number from eth client
		fn get_block(number: u64) -> Result<Option<EthBlock>, Error<T>> {
			// let random_request_id = u32::from_be_bytes(sp_io::offchain::random_seed()[..4].try_into().unwrap());
			let request = GetBlockByNumberRequest::new(1_usize, number);
			let resp_bytes = Self::query_eth_client(request).map_err(|e| {
				log!(error, "💎 read eth-rpc API error: {:?}", e);
				<Error<T>>::HttpFetch
			})?;
	
			let resp_str = core::str::from_utf8(&resp_bytes).map_err(|_| {
				log!(error, "💎 response invalid utf8: {:?}", resp_bytes);
				<Error<T>>::HttpFetch
			})?;
	
			// Deserialize JSON to struct
			serde_json::from_str::<EthResponse<EthBlock>>(resp_str)
				.map(|resp| resp.result)
				.map_err(|err| {
					log!(error, "💎 deserialize json response error: {:?}", err);
					<Error<T>>::HttpFetch
				})
		}
	

	/// Get latest block number from eth client
	fn get_block_number() -> Result<Option<EthBlockNumber>, Error<T>> {
		// let random_request_id = u32::from_be_bytes(sp_io::offchain::random_seed()[..4].try_into().unwrap());
		let request = GetBlockNumberRequest::new(1_usize);
		let resp_bytes = Self::query_eth_client(request).map_err(|e| {
			log!(error, "💎 read eth-rpc API error: {:?}", e);
			<Error<T>>::HttpFetch
		})?;

		let resp_str = core::str::from_utf8(&resp_bytes).map_err(|_| {
			log!(error, "💎 response invalid utf8: {:?}", resp_bytes);
			<Error<T>>::HttpFetch
		})?;

		// Deserialize JSON to struct
		serde_json::from_str::<EthResponse<EthBlockNumber>>(resp_str)
			.map(|resp| resp.result)
			.map_err(|err| {
				log!(error, "💎 deserialize json response error: {:?}", err);
				<Error<T>>::HttpFetch
			})
	}

	/// This function uses the `offchain::http` API to query the remote github information,
	/// and returns the JSON response as vector of bytes.
	fn query_eth_client<R: serde::Serialize>(request_body: R) -> Result<Vec<u8>, Error<T>> {
		// Load eth http URI from offchain storage
		// this should have been configured on start up by passing e.g. `--eth-http`
		// e.g. `--eth-http=http://localhost:8545`
		let eth_http_uri = if let Some(value) = sp_io::offchain::local_storage_get(StorageKind::PERSISTENT, b"ETH_HTTP")
		{
			value
		} else {
			log!(
				error,
				"💎 Eth http uri is not configured! set --eth-http=<value> on start up"
			);
			return Err(Error::<T>::OcwConfig.into());
		};
		let eth_http_uri = core::str::from_utf8(&eth_http_uri).map_err(|_| Error::<T>::OcwConfig)?;

		const HEADER_CONTENT_TYPE: &str = "application/json";
		log!(info, "💎 sending request to: {}", eth_http_uri);
		let body = serde_json::to_string::<R>(&request_body).unwrap();
		// Initiate an external HTTP GET request. This is using high-level wrappers from `sp_runtime`.
		let request = rt_offchain::http::Request::post(eth_http_uri, vec![body.as_bytes()]);
		log!(trace, "💎 request: {:?}", request);

		// Keeping the offchain worker execution time reasonable, so limiting the call to be within 3s.
		let timeout = sp_io::offchain::timestamp().add(rt_offchain::Duration::from_millis(REQUEST_TTL_MS));
		let pending = request
			.add_header("Content-Type", HEADER_CONTENT_TYPE)
			.deadline(timeout) // Setting the timeout time
			.send() // Sending the request out by the host
			.map_err(|err| {
				log!(error, "💎 http request error: {:?}", err);
				<Error<T>>::HttpFetch
			})?;

		// By default, the http request is async from the runtime perspective. So we are asking the
		// runtime to wait here.
		// The returning value here is a `Result` of `Result`, so we are unwrapping it twice by two `?`
		// ref: https://substrate.dev/rustdocs/v3.0.0/sp_runtime/offchain/http/struct.PendingRequest.html#method.try_wait
		let response = pending
			.try_wait(timeout)
			.map_err(|err| {
				log!(error, "💎 http request error: timeline reached: {:?}", err);
				<Error<T>>::HttpFetch
			})?
			.map_err(|err| {
				log!(error, "💎 http request error: timeline reached: {:?}", err);
				<Error<T>>::HttpFetch
			})?;
		log!(trace, "💎 response: {:?}", response);

		if response.code != 200 {
			log!(error, "💎 http request status code: {}", response.code);
			return Err(<Error<T>>::HttpFetch);
		}

		// Read the response body and check it's valid utf-8
		Ok(response.body().collect::<Vec<u8>>())
	}

	/// Send a notarization for the given claim
	fn offchain_send_notarization(key: &T::AuthorityId, payload: NotarizationPayload) -> Result<(), Error<T>> {
		let signature = key
			.sign(&payload.encode())
			.ok_or(<Error<T>>::OffchainUnsignedTxSignedPayload)?;

		let call = Call::submit_notarization(payload, signature);

		// Retrieve the signer to sign the payload
		SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into())
			.map_err(|_| <Error<T>>::OffchainUnsignedTxSignedPayload)?;

		Ok(())
	}
}

impl<T: Config> frame_support::unsigned::ValidateUnsigned for Module<T> {
	type Call = Call<T>;

	fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
		if let Call::submit_notarization(ref payload, ref signature) = call {
			// notarization must be from an active notary
			let notary_keys = Self::notary_keys();
			let notary_public_key = match notary_keys.get(payload.authority_index as usize) {
				Some(id) => id,
				None => return InvalidTransaction::BadProof.into(),
			};
			// notarization must not be a duplicate/equivocation
			if <MessageNotarizations<T>>::contains_key(payload.message_id, &notary_public_key) {
				log!(
					error,
					"💎 received equivocation from: {:?} on {:?}",
					notary_public_key,
					payload.message_id
				);
				return InvalidTransaction::BadProof.into();
			}
			// notarization is signed correctly
			if !(notary_public_key.verify(&payload.encode(), signature)) {
				return InvalidTransaction::BadProof.into();
			}
			ValidTransaction::with_tag_prefix("eth-bridge")
				.priority(UNSIGNED_TXS_PRIORITY)
				// 'provides' must be unique for each submission on the network (i.e. unique for each claim id and validator)
				.and_provides([
					b"notarize",
					&payload.message_id.to_be_bytes(),
					&(payload.authority_index as u64).to_be_bytes(),
				])
				.longevity(3)
				.propagate(true)
				.build()
		} else {
			InvalidTransaction::Call.into()
		}
	}
}

impl<T: Config> sp_runtime::BoundToRuntimeAppPublic for Module<T> {
	type Public = T::AuthorityId;
}

impl<T: Config> OneSessionHandler<T::AccountId> for Module<T> {
	type Key = T::AuthorityId;

	fn on_genesis_session<'a, I: 'a>(validators: I)
	where
		I: Iterator<Item = (&'a T::AccountId, T::AuthorityId)>,
	{
		let keys = validators.map(|x| x.1).collect::<Vec<_>>();
		if !keys.is_empty() {
			assert!(
				NotaryKeys::<T>::decode_len().is_none(),
				"NotaryKeys are already initialized!"
			);
			NotaryKeys::<T>::put(keys);
		}
	}

	fn on_new_session<'a, I: 'a>(changed: bool, validators: I, _queued_validators: I)
	where
		I: Iterator<Item = (&'a T::AccountId, T::AuthorityId)>,
	{
		// Record authorities for the new session.
		if changed {
			NotaryKeys::<T>::put(validators.map(|x| x.1).collect::<Vec<_>>());
		}
		// `changed` informs us the current `validators` has changed in some way as of right now
		// `queued_validators` will update the reflected set one session prior to activation
		// this gives us one session to notarize a proof of ancestry for the next set.
		// TODO: how does this interplay function with the election window?
		// e.g. PendingAncestryClaim::insert(queued_validators)
		// next block should trigger voting asap
	}

	fn on_before_session_ending() {}
	fn on_disabled(_i: usize) {}
}
