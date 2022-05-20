/* Copyright 2021-2022 Centrality Investments Limited
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

//! CENNZnet Eth Bridge 🌉
//!
//! This pallet defines notarization protocols for CENNZnet validators to agree on values from a bridged Ethereum chain (Ethereum JSON-RPC compliant),
//! and conversely, generate proofs of events that have occurred on CENNZnet.
//!
//! The proofs are a collection of signatures which can be verified by a bridged contract on Ethereum with awareness of the
//! current validator set.
//!
//! There are types of Ethereum values the bridge can verify:
//! 1) verify a transaction hash exists that executed a specific contract producing a specific event log
//! 2) verify the `returndata` of executing a contract at some time _t_ with input `i`
//!
//! CENNZnet validators use an offchain worker and Ethereum full node connections to independently
//! verify and observe events happened on Ethereum.
//! Once a threshold of validators sign a notarization having witnessed the event it is considered verified.
//!
//! Events are opaque to this module, other modules handle submitting "event claims" and "callbacks" to handle success

#![cfg_attr(not(feature = "std"), no_std)]

mod impls;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
pub use impls::EthereumRpcClient;
mod types;
use types::*;

use cennznet_primitives::eth::Message;
use cennznet_primitives::{
	eth::{ConsensusLog, ValidatorSet, ETHY_ENGINE_ID},
	types::BlockNumber,
};
use codec::Encode;
use crml_support::{
	EthAbiCodec, EventClaimSubscriber, EventClaimVerifier, FinalSessionTracker as FinalSessionTrackerT,
	NotarizationRewardHandler,
};
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage, log,
	pallet_prelude::*,
	traits::{OneSessionHandler, UnixTime, ValidatorSet as ValidatorSetT},
	transactional,
	weights::constants::RocksDbWeight as DbWeight,
	Parameter,
};
use frame_system::{
	offchain::{CreateSignedTransaction, SubmitTransaction},
	pallet_prelude::*,
};
use sp_runtime::{
	generic::DigestItem,
	offchain as rt_offchain,
	traits::{MaybeSerializeDeserialize, Member, SaturatedConversion, Zero},
	transaction_validity::{InvalidTransaction, TransactionSource, TransactionValidity, ValidTransaction},
	DispatchError, Percent, RuntimeAppPublic,
};
use sp_std::prelude::*;

/// The type to sign and send transactions.
const UNSIGNED_TXS_PRIORITY: u64 = 100;
/// Max notarization claims to attempt per block/OCW invocation
const CLAIMS_PER_BLOCK: u64 = 3;
/// Deadline for any network requests e.g.to Eth JSON-RPC endpoint
const REQUEST_TTL_MS: u64 = 2_500;
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
	/// 33 byte ECDSA public key
	type EthyId: Member + Parameter + AsRef<[u8]> + RuntimeAppPublic + Default + Ord + MaybeSerializeDeserialize;
	/// Provides an api for Ethereum JSON-RPC request/responses to the bridged ethereum network
	type EthereumRpcClient: BridgeEthereumRpcApi;
	/// Knows the active authority set (validator stash addresses)
	type AuthoritySet: ValidatorSetT<Self::AccountId, ValidatorId = Self::AccountId>;
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
	/// Tracks the status of sessions/eras
	type FinalSessionTracker: FinalSessionTrackerT;
}

decl_storage! {
	trait Store for Module<T: Config> as EthBridge {
		/// Required % of validator support to signal readiness (default: 66%)
		ActivationThreshold get(fn activation_threshold) config(): Percent = Percent::from_parts(66);
		/// Queued event claims, awaiting notarization
		EventClaims get(fn event_claims): map hasher(twox_64_concat) EventClaimId => (EthHash, EventTypeId);
		/// Event data for a given proof
		EventData get(fn event_data): map hasher(twox_64_concat) EventClaimId => Option<Vec<u8>>;
		/// Event proofs to be processed once bridge has been re-enabled
		DelayedEventProofs get (fn delayed_event_proofs): map hasher(twox_64_concat) EventClaimId => Option<Message>;
		/// Notarizations for queued messages
		/// Either: None = no notarization exists OR Some(yay/nay)
		EventNotarizations get(fn event_notarizations): double_map hasher(twox_64_concat) EventClaimId, hasher(twox_64_concat) T::EthyId => Option<EventClaimResult>;
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
		NotaryKeys get(fn notary_keys): Vec<T::EthyId>;
		/// Scheduled notary (validator) public keys for the next session
		NextNotaryKeys get(fn next_notary_keys): Vec<T::EthyId>;
		/// Processed tx hashes bucketed by unix timestamp (`BUCKET_FACTOR_S`)
		// Used in conjunction with `EventDeadlineSeconds` to prevent "double spends".
		// After a bucket is older than the deadline, any events prior are considered expired.
		// This allows the record of processed events to be pruned from state regularly
		ProcessedTxBuckets get(fn processed_tx_buckets): double_map hasher(twox_64_concat) u64, hasher(identity) EthHash => ();
		/// Set of processed tx hashes
		/// Periodically cleared after `EventDeadlineSeconds` expires
		ProcessedTxHashes get(fn processed_tx_hashes): map hasher(twox_64_concat) EthHash => ();
		/// Map of pending tx hashes to claim Id
		PendingTxHashes get(fn pending_tx_hashes): map hasher(twox_64_concat) EthHash => EventClaimId;
		/// The current validator set id
		NotarySetId get(fn notary_set_id): u64;
		/// The event proof Id generated by the previous validator set to notarize the current set.
		/// Useful for syncing the latest proof to Ethereum
		NotarySetProofId get(fn notary_set_proof_id): EventProofId;
		/// Whether the bridge is paused (for validator transitions)
		BridgePaused get(fn bridge_paused): bool;
		/// The minimum number of block confirmations needed to notarize an Ethereum event
		EventConfirmations get(fn event_confirmations): u64 = 3;
		/// The maximum number of delayed events that can be processed in on_initialize()
		DelayedEventProofsPerBlock get(fn delayed_event_proofs_per_block): u8 = 5;
		/// Events cannot be claimed after this time (seconds)
		EventDeadlineSeconds get(fn event_deadline_seconds): u64 = 604_800; // 1 week
		/// Subscription Id for EthCall requests
		NextEthCallId: EthCallId;
		/// Queue of pending EthCallOracle requests
		EthCallRequests get(fn eth_call_requests): Vec<EthCallId>;
		/// EthCallOracle responses keyed by (Id, Notary)
		EthCallResponses: double_map hasher(twox_64_concat) EthCallId, hasher(twox_64_concat) T::EthyId => Option<EthCallResponse>;
		/// EthCallOracle request info
		EthCallRequestInfo get(fn eth_call_request_info): map hasher(twox_64_concat) EthCallId => Option<EthCallRequest>;
		
	}
}

decl_event! {
	pub enum Event {
		/// Verifying an event succeeded
		Verified(EventClaimId),
		/// Verifying an event failed
		Invalid(EventClaimId),
		/// A notary (validator) set change is in motion (event_id, new_validator_set_id)
		/// A proof for the change will be generated with the given `event_id`
		AuthoritySetChange(EventProofId, u64),
		/// Generating event proof delayed as bridge is paused
		ProofDelayed(EventProofId),
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
		AlreadyNotarized,
		/// Claim in progress
		DuplicateClaim,
		/// The bridge is paused pending validator set changes (once every era / 24 hours)
		/// It will reactive after ~10 minutes
		BridgePaused,
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		fn on_initialize(block_number: T::BlockNumber) -> Weight {
			let mut weight: Weight = DbWeight::get().reads(1 as Weight);
			// Prune claim storage every hour on CENNZnet (BUCKET_FACTOR_S / 5 seconds = 720 blocks)
			if (block_number % T::BlockNumber::from(CLAIM_PRUNING_INTERVAL)).is_zero() {
				// Find the bucket to expire
				let now = T::UnixTime::now().as_secs().saturated_into::<u64>();
				let expired_bucket_index = (now - Self::event_deadline_seconds()) % BUCKET_FACTOR_S;
				for (expired_tx_hash, _empty_value) in ProcessedTxBuckets::iter_prefix(expired_bucket_index) {
					ProcessedTxHashes::remove(expired_tx_hash);
				}
				ProcessedTxBuckets::remove_prefix(expired_bucket_index, None);

				// TODO: better estimate
				weight += 50_000_000 as Weight;
			}

			if DelayedEventProofs::iter().next().is_none() {
				return weight;
			}
			if !Self::bridge_paused() {
				let max_delayed_events = Self::delayed_event_proofs_per_block();
				weight = weight.saturating_add(DbWeight::get().reads(2 as Weight) + max_delayed_events as Weight * DbWeight::get().writes(2 as Weight));
				for (event_proof_id, packed_event_with_id) in DelayedEventProofs::iter().take(max_delayed_events as usize) {
					Self::do_generate_event_proof(event_proof_id, packed_event_with_id);
					DelayedEventProofs::remove(event_proof_id);
				}
			}
			weight
		}

		#[weight = 100_000]
		/// Set event confirmations (blocks). Required block confirmations for an Ethereum event to be notarized by CENNZnet
		pub fn set_event_confirmations(origin, confirmations: u64) {
			ensure_root(origin)?;
			EventConfirmations::put(confirmations)
		}

		#[weight = 100_000]
		/// Set event deadline (seconds). Events cannot be notarized after this time has elapsed
		pub fn set_event_deadline(origin, seconds: u64) {
			ensure_root(origin)?;
			EventDeadlineSeconds::put(seconds);
		}

		#[weight = 100_000]
		/// Set max number of delayed events that can be processed in a block
		pub fn set_delayed_event_proofs_per_block(origin, count: u8) {
			ensure_root(origin)?;
			DelayedEventProofsPerBlock::put(count);
		}

		#[weight = 1_000_000]
		#[transactional]
		/// Internal only
		/// Validators will submit inherents with their notarization vote for a given claim
		pub fn submit_notarization(origin, payload: NotarizationPayload, _signature: <<T as Config>::EthyId as RuntimeAppPublic>::Signature) {
			let _ = ensure_none(origin)?;

			// we don't need to verify the signature here because it has been verified in
			// `validate_unsigned` function when sending out the unsigned tx.
			let notary_keys = Self::notary_keys();
			let notary_public_key = match notary_keys.get(payload.authority_index as usize) {
				Some(id) => id,
				None => return Err(Error::<T>::InvalidNotarization.into()),
			};
			<EventNotarizations<T>>::insert::<EventClaimId, T::EthyId, EventClaimResult>(payload.event_claim_id, notary_public_key.clone(), payload.result);

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
			if Percent::from_rational(nay_count, notary_count) > (Percent::from_parts(100_u8 - T::NotarizationThreshold::get().deconstruct())) {
				// event did not notarize / failed, clean up
				let event_data = EventData::take(payload.event_claim_id);
				if event_data.is_none() {
					// this should never happen
					log!(error, "💎 unexpected empty claim");
					return Err(Error::<T>::InvalidClaim.into())
				}
				<EventNotarizations<T>>::remove_prefix(payload.event_claim_id, None);
				let (_eth_tx_hash, event_type_id) = EventClaims::take(payload.event_claim_id);
				let (contract_address, event_signature) = TypeIdToEventType::get(event_type_id);
				let event_data = event_data.unwrap();
				Self::deposit_event(Event::Invalid(payload.event_claim_id));

				T::Subscribers::on_failure(payload.event_claim_id, &contract_address, &event_signature, &event_data);
				return Ok(());
			}

			// Claim is valid
			if Percent::from_rational(yay_count, notary_count) >= T::NotarizationThreshold::get() {
				let event_data = EventData::take(payload.event_claim_id);
				if event_data.is_none() {
					// this should never happen
					log!(error, "💎 unexpected empty claim");
					return Err(Error::<T>::InvalidClaim.into())
				}
				// no need to track info on this claim any more since it's approved
				<EventNotarizations<T>>::remove_prefix(payload.event_claim_id, None);
				let (eth_tx_hash, event_type_id) = EventClaims::take(payload.event_claim_id);
				let (contract_address, event_signature) = TypeIdToEventType::get(event_type_id);
				let event_data = event_data.unwrap();

				// note this tx as completed
				let bucket_index = T::UnixTime::now().as_secs().saturated_into::<u64>() % BUCKET_FACTOR_S;
				ProcessedTxBuckets::insert(bucket_index, eth_tx_hash, ());
				ProcessedTxHashes::insert(eth_tx_hash, ());
				PendingTxHashes::remove(eth_tx_hash);
				Self::deposit_event(Event::Verified(payload.event_claim_id));

				T::Subscribers::on_success(payload.event_claim_id, &contract_address, &event_signature, &event_data);
			}
		}

		fn offchain_worker(block_number: T::BlockNumber) {
			log!(trace, "💎 entering off-chain worker: {:?}", block_number);
			log!(trace, "💎 active notaries: {:?}", Self::notary_keys());

			// this passes if flag `--validator` set, not necessarily in the active set
			if !sp_io::offchain::is_validator() {
				log!(info, "💎 not a validator, exiting");
				return
			}

			// check a local key exists for a valid bridge notary
			if let Some((active_key, authority_index)) = Self::find_active_ethy_key() {
				Self::do_event_notarization_ocw(&active_key, authority_index);
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
		ensure!(!PendingTxHashes::contains_key(tx_hash), Error::<T>::DuplicateClaim);

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
		PendingTxHashes::insert(tx_hash, event_claim_id);

		Ok(event_claim_id)
	}

	fn generate_event_proof<E: EthAbiCodec>(event: &E) -> Result<u64, DispatchError> {
		let event_proof_id = Self::next_proof_id();
		NextProofId::put(event_proof_id.wrapping_add(1));

		// TODO: does this support multiple consensus logs in a block?
		// save this for `on_finalize` and insert many
		let packed_event_with_id = [
			&event.encode()[..],
			&EthAbiCodec::encode(&Self::validator_set().id)[..],
			&EthAbiCodec::encode(&event_proof_id)[..],
		]
		.concat();

		if Self::bridge_paused() {
			// Delay proof
			DelayedEventProofs::insert(event_proof_id, packed_event_with_id);
			Self::deposit_event(Event::ProofDelayed(event_proof_id));
		} else {
			Self::do_generate_event_proof(event_proof_id, packed_event_with_id);
		}

		Ok(event_proof_id)
	}
}

impl<T: Config> Module<T> {
	/// Check the nodes local keystore for an active (staked) Ethy session key
	/// Returns the public key and index of the key in the current notary set
	fn find_active_ethy_key() -> Option<(T::EthyId, u16)> {
		// Get all signing keys for this protocol 'KeyTypeId'
		let local_keys = T::EthyId::all();
		if local_keys.is_empty() {
			log!(
				error,
				"💎 no signing keys for: {:?}, cannot participate in notarization!",
				T::EthyId::ID
			);
			return None;
		};

		let mut maybe_active_key: Option<(T::EthyId, usize)> = None;
		// search all local ethy keys
		for key in local_keys {
			if let Some(active_key_index) = Self::notary_keys().iter().position(|k| k == &key) {
				maybe_active_key = Some((key, active_key_index));
				break;
			}
		}

		// check if locally known keys are in the active validator set
		if maybe_active_key.is_none() {
			log!(error, "💎 no active ethy keys, exiting");
			return None;
		}
		maybe_active_key.map(|(key, idx)| (key, idx as u16))
	}
	/// Handle OCW event notarization protocol for validators
	/// Receives the node's local notary session key and index in the set
	fn do_event_notarization_ocw(active_key: &T::EthyId, authority_index: u16) {
		// do not try to notarize events while the bridge is paused
		if Self::bridge_paused() {
			return;
		}

		let supports = NotaryKeys::<T>::decode_len().unwrap_or(0);
		let needed = Self::activation_threshold();
		let total = T::AuthoritySet::validators().len();
		if Percent::from_rational(supports, total) < needed {
			log!(
				info,
				"💎 waiting for validator support to activate eth bridge: {:?}/{:?}",
				supports,
				needed
			);
			return;
		}

		// check all pending claims we have _yet_ to notarize and try to notarize them
		// this will be invoked once every block
		// we limit the total claims per invocation using `CLAIMS_PER_BLOCK` so we don't stall block production
		let mut budget = CLAIMS_PER_BLOCK;
		for (event_claim_id, (tx_hash, event_type_id)) in EventClaims::iter() {
			if budget.is_zero() {
				log!(info, "💎 claims budget exceeded, exiting...");
				return;
			}

			// skip if we've notarized it previously
			if <EventNotarizations<T>>::contains_key::<EventClaimId, T::EthyId>(event_claim_id, active_key.clone()) {
				log!(
					trace,
					"💎 already cast notarization for claim: {:?}, ignoring...",
					event_claim_id
				);
				continue;
			}

			if let Some(event_data) = Self::event_data(event_claim_id) {
				let (contract_address, event_signature) = TypeIdToEventType::get(event_type_id);
				let event_claim = EventClaim {
					tx_hash,
					data: event_data,
					contract_address,
					event_signature,
				};
				let result = Self::offchain_try_notarize_event(event_claim);
				log!(trace, "💎 claim verification status: {:?}", &result);
				let payload = NotarizationPayload {
					event_claim_id,
					authority_index,
					result: result.clone(),
				};
				let _ = Self::offchain_send_notarization(&active_key, payload)
					.map_err(|err| {
						log!(error, "💎 sending notarization failed 🙈, {:?}", err);
					})
					.map(|_| {
						log!(
							info,
							"💎 sent notarization: '{:?}' for claim: {:?}",
							result,
							event_claim_id
						);
					});
				budget = budget.saturating_sub(1);
			} else {
				// should not happen, defensive only
				log!(error, "💎 empty claim data for: {:?}", event_claim_id);
			}
		}
	}
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
		let result = T::EthereumRpcClient::get_transaction_receipt(tx_hash);
		if let Err(err) = result {
			log!(error, "💎 eth_getTransactionReceipt({:?}) failed: {:?}", tx_hash, err);
			return EventClaimResult::DataProviderErr;
		}

		let maybe_tx_receipt = result.unwrap(); // error handled above qed.
		let tx_receipt = match maybe_tx_receipt {
			Some(t) => t,
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
			if log.address != contract_address {
				return EventClaimResult::UnexpectedContractAddress;
			}
		} else {
			return EventClaimResult::NoTxLogs;
		}

		//  have we got enough block confirmations to be re-org safe?
		let observed_block_number: u64 = tx_receipt.block_number.saturated_into();

		let latest_block: EthBlock = match T::EthereumRpcClient::get_block_by_number(LatestOrNumber::Latest) {
			Ok(None) => return EventClaimResult::DataProviderErr,
			Ok(Some(block)) => block,
			Err(err) => {
				log!(error, "💎 eth_getBlockByNumber latest failed: {:?}", err);
				return EventClaimResult::DataProviderErr;
			}
		};

		let latest_block_number = latest_block.number.unwrap_or_default().as_u64();
		let block_confirmations = latest_block_number.saturating_sub(observed_block_number);
		if block_confirmations < Self::event_confirmations() {
			return EventClaimResult::NotEnoughConfirmations;
		}

		// we can calculate if the block is expired w some high degree of confidence without making a query.
		// time since the event = block_confirmations * ~16 seconds avg
		// using slightly less to be conservative
		if block_confirmations * 14 > Self::event_deadline_seconds() {
			return EventClaimResult::Expired;
		}

		//  check the block this tx is in if the timestamp > deadline
		let observed_block: EthBlock =
			match T::EthereumRpcClient::get_block_by_number(LatestOrNumber::Number(observed_block_number as u32)) {
				Ok(None) => return EventClaimResult::DataProviderErr,
				Ok(Some(block)) => block,
				Err(err) => {
					log!(error, "💎 eth_getBlockByNumber observed failed: {:?}", err);
					return EventClaimResult::DataProviderErr;
				}
			};

		// claim is past the expiration deadline
		// eth. block timestamp (seconds)
		// deadline (seconds)
		if T::UnixTime::now().as_secs().saturated_into::<u64>() - observed_block.timestamp.saturated_into::<u64>()
			> Self::event_deadline_seconds()
		{
			return EventClaimResult::Expired;
		}

		EventClaimResult::Valid
	}

	/// Try issuing an `eth_call` request to the bridged ethereum network
	fn offchain_try_eth_call(request: &EthCallRequest) -> EthCallResponse {
		// validator OCW process
		// 1) get latest eth block
		// 2) extract number and timestamp
		// 3) calculate best block to issue query
		// 4) invoke eth_call and return response (what to do with large response*)
		// 5) submit response
		// 6) consensus on response value (distribute reward points to those that voted together*). Return info to the subscriber
		/*

		// validators receive request timestamp
		// need to decide which block to query for the response so as to minimize queries to Ethereum
		1) query block timestamp at relayer reported block_number
		2a) if block timestamp is in the lenience range then do call_at at the relayer reported block
		2b) if block timestamp is outside the lenience range (the reporter is going to be slashed) we still need to find the right block to query for the true value
		process to find right block number:
		- query the current latest block number from Ethereum
		- assuming avg blocktime eth blocktime of 15 seconds calculate x blocks backwards
		- query the block number closest to and higher than request timestamp i.e. prefer block after the time of request
		3) do the `eth_call` at the correct block
		*/
		let latest_block: EthBlock = match T::EthereumRpcClient::get_block(LatestOrNumber::Latest) {
			Ok(None) => return EthCallResponse::DataProviderErr,
			Ok(Some(block)) => block,
			Err(err) => {
				log!(error, "💎 eth_getBlockByNumber latest failed: {:?}", err);
				return EthCallResponse::DataProviderErr;
			}
		};
		let LENIENCE_MS = 15_000;

		// now figure out how to get the block closest to the timestamp...
		// TODO: ensure these are both in the same units (milliseconds or seconds)
		let request_cennznet_block_timestamp = request.timestamp;
		let latest_eth_block_timestamp = latest_block.timestamp;

		/*
		[r]-[]-[]-[]-[]-[]
		[    r'] - [    ] -
		[r]-[]-[]-[]-[]-[]
		[   r' ] - [r'   ] -
		[r]-[]-[]-[]-[]-[]
		[     ] - [r'   ] -
		*/

		// relayer must ensure it does `eth_call` at the block at or after `request.timestamp`

		// eth - lenience < req < eth + eth_block_time
		// if request.timestamp <= eth_block_time + LENIENCE_MS && request.timestamp >= eth_block_time - LENIENCE_MS {
		// 	// request.timestamp is in this range
		// }

		// U256::from(request.timestamp)
		EthCallResponse::ExceedsLengthLimit
	}

	/// Send a notarization for the given claim
	fn offchain_send_notarization(key: &T::EthyId, payload: NotarizationPayload) -> Result<(), Error<T>> {
		let signature = key
			.sign(&payload.encode())
			.ok_or(<Error<T>>::OffchainUnsignedTxSignedPayload)?;

		let call = Call::submit_notarization {
			payload,
			_signature: signature,
		};

		// Retrieve the signer to sign the payload
		SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into())
			.map_err(|_| <Error<T>>::OffchainUnsignedTxSignedPayload)?;

		Ok(())
	}

	/// Return the active Ethy validator set.
	pub fn validator_set() -> ValidatorSet<T::EthyId> {
		ValidatorSet::<T::EthyId> {
			validators: Self::notary_keys(),
			id: Self::notary_set_id(),
		}
	}

	/// Handle changes to the authority set
	/// This could be called when validators rotate their keys, we don't want to
	/// change this until the era has changed to avoid generating proofs for small set changes or too frequently
	/// - `new`: The validator set that is active right now
	/// - `queued`: The validator set that will activate next session
	fn handle_authorities_change(new: Vec<T::EthyId>, queued: Vec<T::EthyId>) {
		// ### Session life cycle
		// block on_initialize if ShouldEndSession(n)
		//  rotate_session
		//    before_end_session
		//    end_session (end just been)
		//    start_session (start now)
		//    new_session (start now + 1)
		//   -> on_new_session <- this function is CALLED here

		let log_notary_change = |next_keys: &[T::EthyId]| {
			// Store the keys for usage next session
			<NextNotaryKeys<T>>::put(next_keys);
			// Signal the Event Id that will be used for the proof of validator set change.
			// Any observer can subscribe to this event and submit the resulting proof to keep the
			// validator set on the Ethereum bridge contract updated.
			let event_proof_id = NextProofId::get();
			let next_validator_set_id = Self::notary_set_id().wrapping_add(1);
			Self::deposit_event(Event::AuthoritySetChange(event_proof_id, next_validator_set_id));
			NotarySetProofId::put(event_proof_id);
			NextProofId::put(event_proof_id.wrapping_add(1));
			let log: DigestItem = DigestItem::Consensus(
				ETHY_ENGINE_ID,
				ConsensusLog::PendingAuthoritiesChange((
					ValidatorSet {
						validators: next_keys.to_vec(),
						id: next_validator_set_id,
					},
					event_proof_id,
				))
				.encode(),
			);
			<frame_system::Pallet<T>>::deposit_log(log);
		};

		// signal 1 session early about the `queued` validator set change for the next era so there's time to generate a proof
		if T::FinalSessionTracker::is_next_session_final().0 {
			log!(trace, "💎 next session final");
			log_notary_change(queued.as_ref());
		} else if T::FinalSessionTracker::is_active_session_final() {
			// Pause bridge claim/proofs
			// Prevents claims/proofs being partially processed and failing if the validator set changes
			// significantly
			// Note: the bridge will be reactivated at the end of the session
			log!(trace, "💎 active session final");
			BridgePaused::put(true);

			if Self::next_notary_keys().is_empty() {
				// if we're here the era was forced, we need to generate a proof asap
				log!(warn, "💎 urgent notary key rotation");
				log_notary_change(new.as_ref());
			}
		}
	}

	fn do_generate_event_proof(event_proof_id: EventClaimId, packed_event_with_id: Message) {
		let log: DigestItem = DigestItem::Consensus(
			ETHY_ENGINE_ID,
			ConsensusLog::<T::AccountId>::OpaqueSigningRequest((packed_event_with_id, event_proof_id)).encode(),
		);
		<frame_system::Pallet<T>>::deposit_log(log);
	}
}

impl<T: Config> frame_support::unsigned::ValidateUnsigned for Module<T> {
	type Call = Call<T>;

	fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
		if let Call::submit_notarization {
			ref payload,
			_signature: ref signature,
		} = call
		{
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
	type Public = T::EthyId;
}

impl<T: Config> OneSessionHandler<T::AccountId> for Module<T> {
	type Key = T::EthyId;

	fn on_genesis_session<'a, I: 'a>(validators: I)
	where
		I: Iterator<Item = (&'a T::AccountId, T::EthyId)>,
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

	fn on_new_session<'a, I: 'a>(_changed: bool, validators: I, queued_validators: I)
	where
		I: Iterator<Item = (&'a T::AccountId, T::EthyId)>,
	{
		// Only run change process at the end of an era
		if T::FinalSessionTracker::is_next_session_final().0 || T::FinalSessionTracker::is_active_session_final() {
			// Record authorities for the new session.
			let next_authorities = validators.map(|(_, k)| k).collect::<Vec<_>>();
			let next_queued_authorities = queued_validators.map(|(_, k)| k).collect::<Vec<_>>();

			Self::handle_authorities_change(next_authorities, next_queued_authorities);
		}
	}

	/// A notification for end of the session.
	///
	/// Note it is triggered before any [`SessionManager::end_session`] handlers,
	/// so we can still affect the validator set.
	fn on_before_session_ending() {
		// Re-activate the bridge, allowing claims & proofs again
		if T::FinalSessionTracker::is_active_session_final() {
			log!(trace, "💎 session & era ending, set new validator keys");
			// A proof should've been generated now so we can reactivate the bridge with the new validator set
			BridgePaused::kill();
			// Time to update the bridge validator keys.
			let next_notary_keys = NextNotaryKeys::<T>::take();
			// Store the new keys and increment the validator set id
			// Next notary keys should be unset, until populated by new session logic
			<NotaryKeys<T>>::put(&next_notary_keys);
			NotarySetId::mutate(|next_set_id| *next_set_id = next_set_id.wrapping_add(1));
		}
	}

	fn on_disabled(_i: u32) {
		// TODO: remove disabled validator from claim voting?
	}
}

impl<T: Config> EthCallOracle for Module<T> {
	type Address = EthAddress;
	type CallId = EthCallId;
	/// Invoke `target` contract with `input` on the bridged ethereum network
	/// the call will be executed at the block number closest to `timestamp ` (during or after but not before)
	///
	/// Returns a call Id for subscribers
	fn call_at(target: &Self::Address, input: &[u8], timestamp: u64) -> Self::CallId {
		// store the job for validators to process async
		let call_id = NextEthCallId::get();
		EthCallRequestInfo::insert(
			call_id,
			EthCallRequest {
				target: *target,
				input: input.to_vec(),
				timestamp,
			},
		);
		EthCallRequests::append(call_id);
		NextEthCallId::put(call_id + 1);

		call_id
	}
}