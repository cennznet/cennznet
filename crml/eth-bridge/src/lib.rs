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
//! Module for witnessing/notarizing events on the Ethereum blockchain
//!
//! CENNZnet validators use an offchain worker and Ethereum full node connections to independently
//! verify and observe events happened on Ethereum.
//! Once a threshold of validators sign a notarization having witnessed the event it is considered verified.
//!
//! Events are opaque to this module, other modules handle submitting "event claims" and "callbacks" to handle success

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod tests;

mod types;
use types::*;

use cennznet_primitives::{
	eth::{ConsensusLog, ValidatorSet, ETHY_ENGINE_ID},
	types::BlockNumber,
};
use codec::Encode;
use crml_support::{EventClaimSubscriber, EventClaimVerifier, NotarizationRewardHandler};
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage, ensure, log,
	traits::{Get, OneSessionHandler, UnixTime, ValidatorSet as ValidatorSetT},
	transactional,
	weights::Weight,
	Parameter,
};
use frame_system::{
	ensure_none,
	offchain::{CreateSignedTransaction, SubmitTransaction},
};
use sp_runtime::{
	generic::DigestItem,
	offchain as rt_offchain,
	offchain::StorageKind,
	traits::{MaybeSerializeDeserialize, Member, SaturatedConversion, Zero},
	transaction_validity::{InvalidTransaction, TransactionSource, TransactionValidity, ValidTransaction},
	DispatchError, Percent, RuntimeAppPublic,
};
use sp_std::{convert::TryInto, prelude::*};

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

pub(crate) const LOG_TARGET: &str = "eth-bridge";

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
	type AuthoritySet: ValidatorSetT<Self::AccountId, ValidatorId = Self::AccountId>;
	/// The minimum number of block confirmations needed to ratify an Ethereum event
	type EventConfirmations: Get<u16>;
	/// Events cannot be claimed after this time (seconds)
	type EventDeadline: Get<u64>;
	/// The threshold of notarizations required to approve an Ethereum
	type NotarizationThreshold: Get<Percent>;
	/// Rewards notaries for participating in claims
	type RewardHandler: NotarizationRewardHandler<AccountId = Self::AccountId>;
	/// Things subscribing to event claims
	type Subscribers: EventClaimSubscriber;
	/// Returns the block timestamp
	type UnixTime: UnixTime;
	/// The overarching call type.
	type Call: From<Call<Self>>;
	/// The overarching event type.
	type Event: From<Event> + Into<<Self as frame_system::Config>::Event>;
}

decl_storage! {
	trait Store for Module<T: Config> as EthBridge {
		/// Required % of validator support to signal readiness (default: 66%)
		ActivationThreshold get(fn activation_threshold) config(): Percent = Percent::from_parts(66);
		/// Queued event claims, awaiting notarization
		EventClaims get(fn event_claims): map hasher(twox_64_concat) EventClaimId => (EthHash, EventTypeId);
		/// Event data for a given claim
		EventData get(fn event_data): map hasher(twox_64_concat) EventClaimId => Option<Vec<u8>>;
		/// Notarizations for queued messages
		/// Either: None = no notarization exists OR Some(yay/nay)
		EventNotarizations get(fn event_notarizations): double_map hasher(twox_64_concat) EventClaimId, hasher(twox_64_concat) T::AuthorityId => Option<EventClaimResult>;
		/// Maps event types seen by the bridge ((contract address, event signature)) to unique type Ids
		EventTypeToTypeId get(fn event_type_to_type_id): map hasher(blake2_128_concat) (EthAddress, EthHash) => EventTypeId;
		/// Maps event type ids to ((contract address, event signature))
		TypeIdToEventType get(fn type_id_to_event_type): map hasher(blake2_128_concat) EventTypeId => (EthAddress, EthHash);
		/// Id of the next Eth bridge event claim
		NextEventClaimId get(fn next_event_claim_id): EventClaimId;
		/// Id of the next event type (internal)
		NextEventTypeId get(fn next_event_type_id): EventTypeId;
		/// Id of the next event proof
		NextProofId get(fn next_proof_id): EventProofId;
		/// Active notary (validator) public keys
		NotaryKeys get(fn notary_keys): Vec<T::AuthorityId>;
		/// Scheduled notary (validator) public keys for the next session
		NextNotaryKeys get(fn next_notary_keys): Vec<T::AuthorityId>;
		/// Processed tx hashes bucketed by unix timestamp (`BUCKET_FACTOR_S`)
		// Used in conjunction with `EventDeadline` to prevent "double spends".
		// After a bucket is older than the deadline, any events prior are considered expired.
		// This allows the record of processed events to be pruned from state regularly
		ProcessedTxBuckets get(fn processed_tx_buckets): double_map hasher(twox_64_concat) u64, hasher(identity) EthHash => ();
		/// Map from processed tx hash to status
		/// Periodically cleared after `EventDeadline` expires
		ProcessedTxHashes get(fn processed_tx_hashes): map hasher(twox_64_concat) EthHash => ();
		/// The current validator set id
		NotarySetId get(fn notary_set_id): u64;
	}
}

decl_event! {
	pub enum Event {
		/// Verifying an event succeeded
		Verified(EventClaimId),
		/// Verifying an event failed
		Invalid(EventClaimId),
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
				let now = T::UnixTime::now().as_secs().saturated_into::<u64>();
				let expired_bucket_index = (now - T::EventDeadline::get()) % BUCKET_FACTOR_S;
				for (expired_tx_hash, _empty_value) in ProcessedTxBuckets::iter_prefix(expired_bucket_index) {
					ProcessedTxHashes::remove(expired_tx_hash);
				}
				ProcessedTxBuckets::remove_prefix(expired_bucket_index);

				// TODO: better estimate
				50_000_000_u64
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
			// `validate_unsigned` function when sending out the unsigned tx.
			let notary_keys = Self::notary_keys();
			let notary_public_key = match notary_keys.get(payload.authority_index as usize) {
				Some(id) => id,
				None => return Err(Error::<T>::InvalidNotarization.into()),
			};
			<EventNotarizations<T>>::insert::<EventClaimId, T::AuthorityId, EventClaimResult>(payload.event_claim_id, notary_public_key.clone(), payload.result);

			T::AuthoritySet::validators().get(payload.authority_index as usize)
				.map(|v| T::RewardHandler::reward_notary(v));

			// Count notarization votes
			let notary_count = T::AuthoritySet::validators().len() as u32;
			let mut yay_count = 0_u32;
			let mut nay_count = 0_u32;
			for (_id, result) in <EventNotarizations<T>>::iter_prefix(payload.event_claim_id) {
				match result {
					EventClaimResult::Valid => yay_count += 1,
					_ => nay_count += 1,
				}
			}

			// Claim is invalid (nays > (100% - NotarizationThreshold))
			if Percent::from_rational_approximation(nay_count, notary_count) > (Percent::from_parts(100_u8 - T::NotarizationThreshold::get().deconstruct())) {
				// event did not notarize / failed, clean up
				let event_data = EventData::take(payload.event_claim_id);
				if event_data.is_none() {
					// this should never happen
					log!(error, "💎 unexpected empty claim");
					return Err(Error::<T>::InvalidClaim.into())
				}
				<EventNotarizations<T>>::remove_prefix(payload.event_claim_id);
				EventClaims::remove(payload.event_claim_id);
				Self::deposit_event(Event::Invalid(payload.event_claim_id));
				return Ok(());
			}

			// Claim is valid
			if Percent::from_rational_approximation(yay_count, notary_count) >= T::NotarizationThreshold::get() {
				let event_data = EventData::take(payload.event_claim_id);
				if event_data.is_none() {
					// this should never happen
					log!(error, "💎 unexpected empty claim");
					return Err(Error::<T>::InvalidClaim.into())
				}
				// no need to track info on this claim any more since it's approved
				<EventNotarizations<T>>::remove_prefix(payload.event_claim_id);
				let (eth_tx_hash, event_type_id) = EventClaims::take(payload.event_claim_id);
				let (contract_address, event_signature) = TypeIdToEventType::get(event_type_id);
				let event_data = event_data.unwrap();

				// note this tx as completed
				let bucket_index = T::UnixTime::now().as_secs().saturated_into::<u64>() % BUCKET_FACTOR_S;
				ProcessedTxBuckets::insert(bucket_index, eth_tx_hash, ());
				ProcessedTxHashes::insert(eth_tx_hash, ());
				Self::deposit_event(Event::Verified(payload.event_claim_id));

				T::Subscribers::on_success(payload.event_claim_id, &contract_address, &event_signature, &event_data);
			}
		}

		fn offchain_worker(block_number: T::BlockNumber) {
			log!(trace, "💎 entering off-chain worker: {:?}", block_number);
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
			for (event_claim_id, (tx_hash, event_type_id)) in EventClaims::iter() {
				if budget.is_zero() {
					log!(info, "💎 claims budget exceeded, exiting...");
					return
				}

				// check we haven't notarized this already
				if <EventNotarizations<T>>::contains_key::<EventClaimId, T::AuthorityId>(event_claim_id, key.clone()) {
					log!(trace, "💎 already cast notarization for claim: {:?}, ignoring...", event_claim_id);
				}

				if let Some(event_data) = Self::event_data(event_claim_id) {
					let (contract_address, event_signature) = TypeIdToEventType::get(event_type_id);
					let event_claim = EventClaim { tx_hash, data: event_data, contract_address, event_signature };
					let result = Self::offchain_try_notarize_event(event_claim);
					log!(trace, "💎 claim verification status: {:?}", &result);
					let payload = NotarizationPayload {
						event_claim_id,
						authority_index,
						result: result.clone(),
					};
					let _ = Self::offchain_send_notarization(&key, payload)
						.map_err(|err| {
							log!(error, "💎 sending notarization failed 🙈, {:?}", err);
						})
						.map(|_| {
							log!(info, "💎 sent notarization: '{:?}' for claim: {:?}", result, event_claim_id);
						});
					budget = budget.saturating_sub(1);
				} else {
					// should not happen, defensive only
					log!(error, "💎 empty claim data for: {:?}", event_claim_id);
				}
			}

			log!(trace, "💎 exiting off-chain worker");
		}

	}
}

impl<T: Config> EventClaimVerifier for Module<T> {
	/// Submit an event claim against an ethereum tx hash
	// tx hashes may only be claimed once
	fn submit_event_claim(
		contract_address: &H160,
		event_signature: &H256,
		tx_hash: &H256,
		event_data: &[u8],
	) -> Result<EventClaimId, DispatchError> {
		ensure!(!ProcessedTxHashes::contains_key(tx_hash), Error::<T>::AlreadyNotarized);

		// check if we've seen this event type before
		// if not we assign it a type Id (saves us storing the (contract address, event signature) each time)
		let event_type_id = if !EventTypeToTypeId::contains_key((contract_address, event_signature)) {
			let next_event_type_id = Self::next_event_type_id();
			EventTypeToTypeId::insert((contract_address, event_signature), next_event_type_id);
			TypeIdToEventType::insert(next_event_type_id, (contract_address, event_signature));
			NextEventTypeId::put(next_event_type_id.wrapping_add(1));
			next_event_type_id
		} else {
			EventTypeToTypeId::get((contract_address, event_signature))
		};

		let event_claim_id = Self::next_event_claim_id();
		EventData::insert(event_claim_id, event_data);
		EventClaims::insert(event_claim_id, (tx_hash, event_type_id));
		NextEventClaimId::put(event_claim_id.wrapping_add(1));

		Ok(event_claim_id)
	}

	fn generate_event_proof(message: &[u8]) -> Result<u64, DispatchError> {
		let event_proof_id = Self::next_proof_id();

		// TODO: does this support multiple consensus logs in a block?
		// save this for `on_finalize` and insert many
		let log: DigestItem<T::Hash> = DigestItem::Consensus(
			ETHY_ENGINE_ID,
			ConsensusLog::<T::AccountId>::OpaqueSigningRequest((message.to_vec(), event_proof_id)).encode(),
		);
		<frame_system::Pallet<T>>::deposit_log(log);

		NextProofId::put(event_proof_id.wrapping_add(1));

		Ok(event_proof_id)
	}
}

impl<T: Config> Module<T> {
	/// Verify a message
	/// `tx_hash` - The ethereum tx hash
	/// `event_data` - The claimed message data
	/// `event_handler_config` - Details of the message
	/// Checks:
	/// - check Eth full node for transaction status
	/// - tx success
	/// - tx sent to deposit contract address
	/// - check for log with deposited amount and token type
	/// - confirmations `>= T::EventConfirmations`
	/// - message has not expired older than `T::EventDeadline`
	fn offchain_try_notarize_event(event_claim: EventClaim) -> EventClaimResult {
		let EventClaim {
			tx_hash,
			data,
			contract_address,
			event_signature,
		} = event_claim;
		let result = Self::get_transaction_receipt(tx_hash);
		if let Err(err) = result {
			log!(error, "💎 eth_getTransactionReceipt({:?}) failed: {:?}", tx_hash, err);
			return EventClaimResult::DataProviderErr;
		}

		let maybe_tx_receipt = result.unwrap(); // error handled above qed.
		let tx_receipt = match maybe_tx_receipt {
			Some(tx_receipt) => tx_receipt,
			None => return EventClaimResult::NoTxLogs,
		};
		let status = tx_receipt.status.unwrap_or_default();
		if status.is_zero() {
			return EventClaimResult::TxStatusFailed;
		}

		if tx_receipt.to != Some(contract_address) {
			return EventClaimResult::UnexpectedContractAddress;
		}

		let topic: EthHash = event_signature;
		// search for a bridge deposit event in this tx receipt
		let matching_log = tx_receipt
			.logs
			.iter()
			.find(|log| log.transaction_hash == Some(tx_hash) && log.topics.contains(&topic));

		if let Some(log) = matching_log {
			// check if the ethereum deposit event matches what was reported
			// in the original claim
			if log.data != data {
				log!(
					trace,
					"💎 mismatch in provided data vs. observed data. provided: {:?} observed: {:?}",
					data,
					log.data,
				);
				return EventClaimResult::UnexpectedData;
			}
		}

		// lastly, have we got enough block confirmations to be re-org safe?
		let observed_block = tx_receipt.block_number.saturated_into();
		let result = Self::get_block(observed_block);
		if let Err(err) = result {
			log!(error, "💎 eth_getBlockByNumber failed: {:?}", err);
			return EventClaimResult::DataProviderErr;
		}
		let maybe_block = result.unwrap();
		if maybe_block.is_none() {
			return EventClaimResult::DataProviderErr;
		}
		let block = maybe_block.unwrap();
		let latest_block_number: u64 = block.number.unwrap_or_default().as_u64();
		let block_confirmations = latest_block_number.saturating_sub(observed_block as u64);
		if block_confirmations < T::EventConfirmations::get() as u64 {
			return EventClaimResult::NotEnoughConfirmations;
		}
		// claim is past the expiration deadline
		// eth. block timestamp (seconds)
		// deadline (seconds)
		if T::UnixTime::now().as_secs().saturated_into::<u64>() - block.timestamp.saturated_into::<u64>()
			> T::EventDeadline::get()
		{
			return EventClaimResult::Expired;
		}

		EventClaimResult::Valid
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
	fn get_block(number: u32) -> Result<Option<EthBlock>, Error<T>> {
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
			return Err(Error::<T>::OcwConfig);
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

	/// Return the active Ethy validator set.
	pub fn validator_set() -> ValidatorSet<T::AuthorityId> {
		ValidatorSet::<T::AuthorityId> {
			validators: Self::notary_keys(),
			id: Self::notary_set_id(),
		}
	}

	fn change_authorities(new: Vec<T::AuthorityId>, queued: Vec<T::AuthorityId>) {
		// As in GRANDPA, we trigger a validator set change only if the the validator
		// set has actually changed.
		if new != Self::notary_keys() {
			<NotaryKeys<T>>::put(&new);

			let next_id = Self::notary_set_id().wrapping_add(1);
			NotarySetId::put(next_id);

			let log: DigestItem<T::Hash> = DigestItem::Consensus(
				ETHY_ENGINE_ID,
				ConsensusLog::AuthoritiesChange(ValidatorSet {
					validators: new,
					id: next_id,
				})
				.encode(),
			);
			<frame_system::Pallet<T>>::deposit_log(log);
		}

		<NextNotaryKeys<T>>::put(&queued);
	}

	fn initialize_authorities(authorities: &[T::AuthorityId]) {
		if authorities.is_empty() {
			return;
		}

		assert!(<NotaryKeys<T>>::get().is_empty(), "NotaryKeys are already initialized!");

		<NotaryKeys<T>>::put(authorities);
		NotarySetId::put(0);
		// Like `pallet_session`, initialize the next validator set as well.
		<NextNotaryKeys<T>>::put(authorities);
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
			if <EventNotarizations<T>>::contains_key(payload.event_claim_id, &notary_public_key) {
				log!(
					error,
					"💎 received equivocation from: {:?} on {:?}",
					notary_public_key,
					payload.event_claim_id
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
					&payload.event_claim_id.to_be_bytes(),
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

	fn on_new_session<'a, I: 'a>(changed: bool, validators: I, queued_validators: I)
	where
		I: Iterator<Item = (&'a T::AccountId, T::AuthorityId)>,
	{
		// Record authorities for the new session.
		if changed {
			let next_authorities = validators.map(|(_, k)| k).collect::<Vec<_>>();
			let next_queued_authorities = queued_validators.map(|(_, k)| k).collect::<Vec<_>>();

			Self::change_authorities(next_authorities, next_queued_authorities);
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
