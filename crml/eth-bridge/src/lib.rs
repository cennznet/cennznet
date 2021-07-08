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
//! 2) After waiting for block confirmations, claimants submit the Ethereum transaction hash and deposit event info using `deposit_claim`
//! 3) Validators aka 'Notaries' in this context, run an OCW protocol using Ethereum full nodes to verify
//! the deposit has occurred
//! 4) after a threshold of notarizations have been received for a claim, the tokens are released to the beneficiary account
//!
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod tests;

mod types;
use types::*;

use codec::{Codec, Decode, Encode};
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage,
	dispatch::DispatchResult,
	log,
	traits::{Get, OneSessionHandler},
	Parameter,
};
use frame_system::{
	ensure_none, ensure_signed,
	offchain::{CreateSignedTransaction, SignedPayload, SigningTypes, SubmitTransaction},
};
use sp_core::H256;
use sp_runtime::{
	offchain as rt_offchain,
	traits::{MaybeSerializeDeserialize, Member, Zero},
	transaction_validity::{InvalidTransaction, TransactionSource, TransactionValidity, ValidTransaction},
	Percent, RuntimeAppPublic, RuntimeDebug,
};
use sp_std::prelude::*;

/// The type to sign and send transactions.
const UNSIGNED_TXS_PRIORITY: u64 = 100;
/// Max notarization claims to attempt per block/OCW invocation
const CLAIMS_PER_BLOCK: u64 = 3;
/// Deadline for any network requests e.g.to Eth JSON-RPC endpoint
const REQUEST_TTL_MS: u64 = 1_500;

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

pub enum NotarizationResult {
	/// The notarization was invalid
	Valid,
	/// The notarization was invalid
	Invalid,
	/// Checking the notarization failed
	Failed,
}

/// An independent notarization vote on a claim
/// This is signed and shared with the runtime after verification by a particular validator
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct NotarizationPayload<Public: Codec> {
	/// The claim Id being notarized
	claim_id: ClaimId,
	/// The public key of the authority that will sign this
	public: Public,
	/// Whether the claim was validated or not
	is_valid: bool,
}

impl<T: SigningTypes> SignedPayload<T> for NotarizationPayload<T::Public> {
	fn public(&self) -> T::Public {
		self.public.clone()
	}
}

/// This is the pallet's configuration trait
pub trait Config: frame_system::Config + CreateSignedTransaction<Call<Self>> {
	/// Event signature of a deposit on the Ethereum bridge contract
	type DepositEventSignature: Get<[u8; 32]>;
	/// Eth bridge contract address
	type BridgeContractAddress: Get<[u8; 20]>;
	/// The minimum number of transaction confirmations needed to ratify an Eth deposit
	type RequiredConfirmations: Get<u16>;
	/// The threshold of notarizations required to approve an Eth deposit
	// TODO: enable this
	// type DepositApprovalThreshold: Get<Percent>;
	/// Deposits cannot be claimed after this time # of Eth blocks)
	type DepositClaimPeriod: Get<u32>;
	/// The identifier type for an authority.
	type AuthorityId: Member + Parameter + RuntimeAppPublic + Default + Ord + MaybeSerializeDeserialize;
	/// The overarching dispatch call type.
	type Call: From<Call<Self>>;
	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
}

decl_storage! {
	trait Store for Pallet<T: Config> as EthBridge {
		/// Id of a token claim
		NextClaimId get(fn next_claim_id): ClaimId;
		/// Info of a claim
		ClaimInfo get(fn claim_info): map hasher(twox_64_concat) ClaimId => Option<EthDepositEvent>;
		/// Pending claims
		PendingClaims get(fn pending_claims): map hasher(twox_64_concat) ClaimId => H256;
		/// Notarizations for pending claims
		/// Either: None = no notarization exist OR Some(yay/nay)
		ClaimNotarizations get(fn claim_notarizations): double_map hasher(twox_64_concat) ClaimId, hasher(twox_64_concat) T::AuthorityId => Option<bool>;
		/// Active notary (validator) public keys
		NotaryKeys get(fn notary_keys): Vec<T::AuthorityId>;
	}
}

decl_event! {
	pub enum Event<T> where
		<T as frame_system::Config>::AccountId,
		ClaimId = ClaimId,
	{
		/// A bridge token claim succeeded (claim id)
		TokenClaim(ClaimId),
		// TODO: unused, just make compiler happy
		Dummy(AccountId),
	}
}

decl_error! {
	pub enum Error for Pallet<T: Config> {
		// Error returned when making signed transactions in off-chain worker
		NoLocalSigningAccount,
		// Error returned when making unsigned transactions with signed payloads in off-chain worker
		OffchainUnsignedTxSignedPayloadError,
		// Error returned when fetching github info
		HttpFetch,
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		#[weight = 100_000_000]
		/// Submit a bridge deposit claim for an ethereum tx hash
		/// The deposit details must be provided for cross-checking by notaries
		/// Any caller may initiate a claim while only the intended beneficiary will be paid.
		pub fn deposit_claim(origin, tx_hash: H256, deposit_event: EthDepositEvent) {
			// Note: require caller to provide the `deposit_event` so we don't need to handle the-
			// complexities of notaries reporting differing deposit events
			// TODO: weight here should reflect the full amount of offchain work which is triggered as a result
			// TODO: need replay protection:
			// 1) check / store successfully claimed txHashes
			// 2) claims older than some time period should be invalid, allowing us to release claimed txHashes from storage at regular intervals
			let _ = ensure_signed(origin)?;
			let claim_id = Self::next_claim_id();
			ClaimInfo::insert(claim_id, deposit_event);
			PendingClaims::insert(claim_id, tx_hash);
			NextClaimId::put(claim_id.wrapping_add(1));
		}

		#[weight = 100_000]
		/// Internal only
		/// Validators will call this with their notarization vote for a given claim
		pub fn submit_notarization(origin, payload: NotarizationPayload<T::AuthorityId>, _signature: <<T as Config>::AuthorityId as RuntimeAppPublic>::Signature) -> DispatchResult {
			let _ = ensure_none(origin)?;

			// we don't need to verify the signature here because it has been verified in
			//`validate_unsigned` function when sending out the unsigned tx.
			<ClaimNotarizations<T>>::insert::<ClaimId, T::AuthorityId, bool>(payload.claim_id, payload.public, payload.is_valid);
			// - check if threshold reached for or against
			let notarizations = <ClaimNotarizations<T>>::iter_prefix(payload.claim_id).count() as u32;

			// TODO: Keys::<T>::decode_len().unwrap_or_default() as u32
			// TODO: T::DepositApprovalThreshold::get()
			let validators_len = 1_u32;
			if Percent::from_rational(notarizations, validators_len) >= Percent::from_rational(51_u32, 100_u32) {
				// no need to track info on this claim any more since it's approved
				PendingClaims::remove(payload.claim_id);
				let claim_info = ClaimInfo::take(payload.claim_id);
				<ClaimNotarizations<T>>::remove_prefix(payload.claim_id, None);
				// TODO:
				// 1) maybe mint new asset (check first)
				// 2) release tokens
				Self::deposit_event(RawEvent::TokenClaim(payload.claim_id));
			}

			Ok(())
		}

		fn offchain_worker(_block_number: T::BlockNumber) {
			log!(info, "💎 entering off-chain worker");

			// check local `key` is a valid bridge notary
			if !sp_io::offchain::is_validator() {
				log!(error, "💎 not an active notary this session, exiting");
				return
			}

			// Get all signing keys for this protocol e.g. we piggyback the 'imon' key
			let keys = T::AuthorityId::all();
			// We only expect one key to be present
			if keys.iter().count() > 1 {
				log!(error, "💎 multiple signing keys detected for: {:?}, bailing...", T::AuthorityId::ID);
				return
			}
			let key = match keys.first() {
				Some(key) => key,
				None => {
					log!(error, "💎 no signing keys for: {:?}, will not participate in notarization!", T::AuthorityId::ID);
					return
				}
			};

			// check all pending claims we have _yet_ to notarize and try to notarize them
			// this will be invoked once every block
			// we limit the total claims per invocation using `CLAIMS_PER_BLOCK` so we don't stall block production
			let mut budget = CLAIMS_PER_BLOCK;
			for (claim_id, tx_hash) in PendingClaims::iter() {
				// if we haven't voted on this claim yet, then try!
				// TODO: need to track local in-flight claims
				// - don't modify state until consensus
				// - allow notarization to continue in event of a restart
				if !<ClaimNotarizations<T>>::contains_key::<ClaimId, T::AuthorityId>(claim_id, key.clone()) {
					if let Some(claim_info) = Self::claim_info(claim_id) {
						let is_valid = Self::offchain_verify_claim(tx_hash, claim_info);
						let _ = Self::offchain_send_notarization(key, claim_id, is_valid)
							.map_err(|err| {
								log!(error, "💎 sending notarization failed 🙈, {:?}", err);
							})
							.map(|_| {
								log!(info, "💎 signed notarization: '{:?}' for claim: {:?}", is_valid, claim_id);
							});
						}
						budget = budget.saturating_sub(1);
					} else {
						// this should not happen, just handling the case
						log!(error, "💎 cannot notarize empty claim {:?}", claim_id);
					}

				if budget.is_zero() {
					log!(info, "💎 met claims budget exiting...");
					return
				}
			}

			log!(info, "💎 exiting off-chain worker");
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Verify a claim
	/// - check Eth full node for transaction status
	/// - tx success
	/// - tx sent to deposit contract address
	/// - check for log with deposited amount and token type
	/// - confirmations >= T::RequiredConfirmations
	fn offchain_verify_claim(tx_hash: H256, reported_claim_event: EthDepositEvent) -> bool {
		let result = Self::get_transaction_receipt(tx_hash);
		if let Err(err) = result {
			log!(error, "💎 get tx receipt: {:?}, failed: {:?}", tx_hash, err);
			return false;
		}

		let tx_receipt = result.unwrap(); // error handled above qed.
		let status = tx_receipt.status.unwrap_or_default();
		if status.is_zero() {
			return false;
		}

		if tx_receipt.to != Some(T::BridgeContractAddress::get().into()) {
			return false;
		}

		let topic: H256 = T::DepositEventSignature::get().into();
		let matching_log = tx_receipt
			.logs
			.iter()
			.find(|log| log.transaction_hash == Some(tx_hash) && log.topics.contains(&topic.into()));

		if let Some(log) = matching_log {
			match EthDepositEvent::try_decode_from_log(log) {
				Some(event) => {
					// this checks if the ethereum deposit event matches
					// what was reported in the claim
					if reported_claim_event != event {
						return false;
					}
				}
				None => return false,
			}

			// finally, have we got enough block confirmations to be re-org safe?
			let latest_block_number = Self::get_block_number().unwrap_or_default();
			let tx_block_number = tx_receipt.block_number;
			return latest_block_number.as_u64().saturating_sub(tx_block_number.as_u64())
				>= T::RequiredConfirmations::get() as u64;
		}

		return false;
	}

	/// Get transaction receipt from eth client
	fn get_transaction_receipt(tx_hash: H256) -> Result<TransactionReceipt, Error<T>> {
		let request = GetTxReceiptRequest::new(tx_hash);
		let resp_bytes = Self::query_eth_client(Some(request)).map_err(|e| {
			log!(error, "💎 read eth-rpc API error: {:?}", e);
			<Error<T>>::HttpFetch
		})?;

		// Deserialize JSON to struct
		serde_json_core::from_slice(&resp_bytes)
			.map(|resp: EthResponse<TransactionReceipt>| resp.result)
			.map_err(|err| {
				log!(error, "💎 deserialize json response error: {:?}", err);
				<Error<T>>::HttpFetch
			})
	}

	/// Get latest block number from eth client
	fn get_block_number() -> Result<EthBlockNumber, Error<T>> {
		let request = GetBlockNumberRequest::new();
		let resp_bytes = Self::query_eth_client(request).map_err(|e| {
			log!(error, "💎 read eth-rpc API error: {:?}", e);
			<Error<T>>::HttpFetch
		})?;

		// Deserialize JSON to struct
		serde_json_core::from_slice(&resp_bytes)
			.map(|resp: EthResponse<EthBlockNumber>| resp.result)
			.map_err(|err| {
				log!(error, "💎 deserialize json response error: {:?}", err);
				<Error<T>>::HttpFetch
			})
	}

	/// This function uses the `offchain::http` API to query the remote github information,
	/// and returns the JSON response as vector of bytes.
	fn query_eth_client<R: serde::Serialize>(request_body: R) -> Result<Vec<u8>, Error<T>> {
		// TODO: load this info from some client config.e.g. offchain indexed
		const ETH_HOST: &str = "http://localhost:8545";
		const HEADER_CONTENT_TYPE: &str = "application/json";
		log!(info, "💎 sending request to: {}", ETH_HOST);
		let body = serde_json_core::to_string::<serde_json_core::consts::U512, R>(&request_body).unwrap();
		// Initiate an external HTTP GET request. This is using high-level wrappers from `sp_runtime`.
		let request = rt_offchain::http::Request::post(ETH_HOST, vec![body.as_bytes()]);
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

		// Next we fully read the response body and collect it to a vector of bytes.
		Ok(response.body().collect::<Vec<u8>>())
	}

	/// Send a notarization for the given claim
	fn offchain_send_notarization(key: &T::AuthorityId, claim_id: ClaimId, is_valid: bool) -> Result<(), Error<T>> {
		let payload = NotarizationPayload {
			claim_id,
			public: key.clone(),
			is_valid,
		};
		let signature = key
			.sign(&payload.encode())
			.ok_or(<Error<T>>::OffchainUnsignedTxSignedPayloadError)?;
		let call = Call::submit_notarization(payload, signature);
		// Retrieve the signer to sign the payload
		SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into())
			.map_err(|_| <Error<T>>::OffchainUnsignedTxSignedPayloadError)?;

		Ok(())
	}
}

impl<T: Config> frame_support::unsigned::ValidateUnsigned for Pallet<T> {
	type Call = Call<T>;

	fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
		if let Call::submit_notarization(ref payload, ref signature) = call {
			// TODO: !check `payload.public` is a valid authority
			// TODO: check `payload.public` has not voted already
			if !(payload.public.verify(&payload.encode(), signature)) {
				return InvalidTransaction::BadProof.into();
			}

			// TODO: does 'provides' need to be unique for all validators?
			// Error submitting a transaction to the pool: Pool(TooLowPriority { old: 100100, new: 100100 })
			ValidTransaction::with_tag_prefix("eth-bridge")
				.priority(UNSIGNED_TXS_PRIORITY)
				.and_provides([&b"notarize", &payload.claim_id.to_be_bytes()])
				.longevity(3)
				.propagate(true)
				.build()
		} else {
			InvalidTransaction::Call.into()
		}
	}
}
