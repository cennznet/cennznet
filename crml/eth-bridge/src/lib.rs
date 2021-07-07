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

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod tests;

mod types;
use types::*;

use codec::{Codec, Decode, Encode};
use frame_support::{decl_error, decl_event, decl_module, decl_storage, dispatch::DispatchResult, log, traits::{Get, OneSessionHandler}, Parameter};
use frame_system::{
	ensure_none, ensure_signed,
	offchain::{CreateSignedTransaction, SignedPayload, SigningTypes, SubmitTransaction},
};
use sp_core::{H160, H256};
use sp_runtime::{
	offchain as rt_offchain,
	traits::{MaybeSerializeDeserialize, Member},
	transaction_validity::{InvalidTransaction, TransactionSource, TransactionValidity, ValidTransaction},
	Percent, RuntimeAppPublic, RuntimeDebug,
};
use sp_std::{
	prelude::*,
	str::{self, FromStr},
};

/// The type to sign and send transactions.
const UNSIGNED_TXS_PRIORITY: u64 = 100;

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

/// An independent notarization vote on a claim
/// This is signed and shared with the runtime after verification by a particular validator
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct NotarizationPayload<Public: Codec> {
	/// The claim Id being notarized
	claim_id: u64,
	/// The public key of the authority that will sign this
	public: Public,
	/// Whether the claim was validated or not
	// TODO: status enum instead of bool
	is_valid: bool,
}

impl<T: SigningTypes> SignedPayload<T> for NotarizationPayload<T::Public> {
	fn public(&self) -> T::Public {
		self.public.clone()
	}
}

/// This is the pallet's configuration trait
pub trait Config: frame_system::Config + CreateSignedTransaction<Call<Self>> {
	// config values
	/// The deposited event topic of a deposit on Ethereum
	// type EthDepositContractTopic: Get<H256>;
	/// The Eth deposit contract address
	// type EthDepositContractAddress: Get<H160>;
	/// The minimum number of transaction confirmations needed to ratify an Eth deposit
	type RequiredConfirmations: Get<u16>;
	/// The threshold of notarizations required to approve an Eth deposit
	// type DepositApprovalThreshold: Get<Percent>;
	/// Deposits cannot be claimed after this time # of Eth blocks)
	type DepositClaimPeriod: Get<u32>;

	// config types
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
		NextClaimId get(fn next_claim_id): u64;
		/// Pending claims
		PendingClaims get(fn pending_claims): map hasher(twox_64_concat) u64 => H256;
		/// Notarizations for pending claims
		/// None, no notarization or Some(yay/nay)
		ClaimNotarizations get(fn claim_notarizations): double_map hasher(twox_64_concat) u64, hasher(twox_64_concat) T::AuthorityId => Option<bool>;
		/// Active notary (Validator) public keys
		NotaryKeys get(fn notary_keys): Vec<T::AuthorityId>;
	}
}

decl_event! {
	pub enum Event<T> where
		<T as frame_system::Config>::AccountId,
	{
		/// A bridge token claim succeeded (address, claim id)
		TokenClaim(AccountId, u64),
	}
}

decl_error! {
	pub enum Error for Pallet<T: Config> {
		// Error returned when making signed transactions in off-chain worker
		NoLocalSigningAccount,
		// Error returned when making unsigned transactions with signed payloads in off-chain worker
		OffchainUnsignedTxSignedPayloadError,
		// Error returned when fetching github info
		HttpFetchingError,
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		#[weight = 100_000_000]
		// TODO: weight here should reflect the offchain work which is triggered as a result
		/// Submit a bridge deposit claim for an ethereum tx hash
		pub fn deposit_claim(origin, tx_hash: H256) {
			let _ = ensure_signed(origin)?;
			let claim_id = Self::next_claim_id();
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
			<ClaimNotarizations<T>>::insert::<u64, T::AuthorityId, bool>(payload.claim_id, payload.public, payload.is_valid);
			// - check if threshold reached for or against
			let notarizations = <ClaimNotarizations<T>>::iter_prefix(payload.claim_id).count() as u32;

			// TODO: Keys::<T>::decode_len().unwrap_or_default() as u32
			let validators_len = 1_u32;
			if Percent::from_rational(notarizations, validators_len) >= Percent::from_rational(51_u32, 100_u32) {
				// - clean up + release tokens
				// Self::deposit_event(RawEvent::TokenClaim(claim_id));
			}

			Ok(())
		}

		fn offchain_worker(_block_number: T::BlockNumber) {
			log!(info, "💎 entering off-chain worker");

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

			// check local `key` is a valid bridge notary
			// TODO: optimise this as it's O(N) with the validator set
			if !<NotaryKeys<T>>::get().iter().any(|notary| notary == key) {
				log!(error, "💎 not an active notary this session, exiting: {:?}", key);
			}

			// check all pending claims we have _yet_ to notarize and try to notarize them
			// this will be invoked once every block
			for (claim_id, tx_hash) in PendingClaims::iter() {
				if !<ClaimNotarizations<T>>::contains_key::<u64, T::AuthorityId>(claim_id, key.clone()) {
					let is_valid = Self::offchain_verify_claim(tx_hash);
					let _ = Self::offchain_send_notarization(key, claim_id, is_valid).map_err(|err| {
						log!(error, "💎 sending notarization failed 🙈, {:?}", err);
					});
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
	fn offchain_verify_claim(tx_hash: H256) -> bool {
		let result = Self::get_transaction_receipt(tx_hash);
		if let Err(err) = result {
			log!(error, "💎 get tx receipt: {:?}, failed: {:?}", tx_hash, err);
			return false;
		}
		let tx_receipt = result.unwrap();
		let status = tx_receipt.status.unwrap_or_default();
		if status.is_zero() {
			return false;
		}

		// transaction should be to our configured bridge contract
		if tx_receipt.to.unwrap_or_default() != H160::from_str("0x0c823526689243a45c315d12c5a634a2670843e4837cc1d51af12d25aad839dc").unwrap() {
			return false;
		}

		if let Some(log) = tx_receipt.logs.iter().find(|log| log.transaction_hash == Some(tx_hash)) {
			// TODO:
			// 1) log.topics == our topic
			// 0x0c823526689243a45c315d12c5a634a2670843e4837cc1d51af12d25aad839dc
			// T::DepositEventTopic::get() == keccack256("Deposit(address,address,uint256,bytes32")
			// https://ethereum.stackexchange.com/questions/7835/what-is-topics0-in-event-logs
			// 2) log.data == our expected data
			// rlp crate: https://github.com/paritytech/frontier/blob/1b810cf8143bc545955459ae1e788ef23e627050/frame/ethereum/Cargo.toml#L25
			// https://docs.rs/rlp/0.3.0/src/rlp/lib.rs.html#77-80
			// rlp::decode_list()
			// e.g. MyEventType::rlp_decode(log.data)
			let log_exists = log
				.topics
				.iter()
				.find(|t| {
					**t == H256::from_str("0x0c823526689243a45c315d12c5a634a2670843e4837cc1d51af12d25aad839dc").unwrap()
				})
				.is_some();
			if !log_exists {
				return false;
			}
		} else {
			// no log found
			return false;
		}

		let latest_block_number = Self::get_block_number().unwrap_or_default();
		let tx_block_number = tx_receipt.block_number.unwrap_or_default();
		// have we got enough block confirmations
		if latest_block_number.as_u64().saturating_sub(tx_block_number.as_u64())
			>= T::RequiredConfirmations::get() as u64
		{
			return false;
		}

		// TODO: need replay protection
		// - require nonce in log on eth side per withdrawing address
		// - store bridge nonce on this side, check it's increasing
		// store claimed txHashes
		true
	}

	/// Get transaction receipt from eth client
	fn get_transaction_receipt(tx_hash: H256) -> Result<TransactionReceipt, Error<T>> {
		// '{"jsonrpc":"2.0","method":"eth_getTransactionReceipt","params":["0xb903239f8543d04b5dc1ba6579132b143087c68db1b2168786408fcbce568238"],"id":1}'
		let request = GetTxReceiptRequest::new(tx_hash);
		let resp_bytes = Self::query_eth_client(Some(request)).map_err(|e| {
			log!(error, "💎 Read from eth-rpc API error: {:?}", e);
			<Error<T>>::HttpFetchingError
		})?;

		// Deserialize JSON to struct
		let tx_receipt: TransactionReceipt = serde_json_core::from_slice(&resp_bytes).map_err(|err| {
			log!(error, "💎 deserialize json response error: {:?}", err);
			<Error<T>>::HttpFetchingError
		})?;

		Ok(tx_receipt)
	}

	/// Get latest block number from eth client
	fn get_block_number() -> Result<EthBlockNumber, Error<T>> {
		let request = GetBlockNumberRequest::new();
		let resp_bytes = Self::query_eth_client(request).map_err(|e| {
			log!(error, "💎 Read from eth-rpc API error: {:?}", e);
			<Error<T>>::HttpFetchingError
		})?;

		// Deserialize JSON to struct
		let eth_block_number: EthBlockNumber = serde_json_core::from_slice(&resp_bytes).map_err(|err| {
			log!(error, "💎 deserialize json response error: {:?}", err);
			<Error<T>>::HttpFetchingError
		})?;

		Ok(eth_block_number)
	}

	/// This function uses the `offchain::http` API to query the remote github information,
	/// and returns the JSON response as vector of bytes.
	fn query_eth_client<R: serde::Serialize>(request_body: R) -> Result<Vec<u8>, Error<T>> {
		// TODO: load this info from some client config.e.g. offchain indexed
		const HTTP_REMOTE_REQUEST: &str = "http://localhost:8545";
		const HTTP_HEADER_USER_AGENT: &str = "application/json";
		const FETCH_TIMEOUT_PERIOD: u64 = 3_000; // in milli-seconds
		log!(info, "💎 sending request to: {}", HTTP_REMOTE_REQUEST);

		// Initiate an external HTTP GET request. This is using high-level wrappers from `sp_runtime`.
		let request = rt_offchain::http::Request::get(HTTP_REMOTE_REQUEST);

		// Keeping the offchain worker execution time reasonable, so limiting the call to be within 3s.
		let timeout = sp_io::offchain::timestamp().add(rt_offchain::Duration::from_millis(FETCH_TIMEOUT_PERIOD));

		let pending = request
			.body(vec![serde_json_core::to_string::<serde_json_core::consts::U512, R>(
				&request_body,
			)
			.unwrap()
			.as_bytes()])
			.add_header("Content-Type", HTTP_HEADER_USER_AGENT)
			.deadline(timeout) // Setting the timeout time
			.send() // Sending the request out by the host
			.map_err(|_| {
				log!(error, "💎 unexpected http request error");
				<Error<T>>::HttpFetchingError
			})?;

		// By default, the http request is async from the runtime perspective. So we are asking the
		// runtime to wait here.
		// The returning value here is a `Result` of `Result`, so we are unwrapping it twice by two `?`
		// ref: https://substrate.dev/rustdocs/v3.0.0/sp_runtime/offchain/http/struct.PendingRequest.html#method.try_wait
		let response = pending
			.try_wait(timeout)
			.map_err(|_| {
				log!(error, "💎 unexpected http request error: timeline reached?");
				<Error<T>>::HttpFetchingError
			})?
			.map_err(|_| {
				log!(error, "💎 unexpected http request error: timeline reached 2");
				<Error<T>>::HttpFetchingError
			})?;

		if response.code != 200 {
			log!(error, "💎 unexpected http request status code: {}", response.code);
			return Err(<Error<T>>::HttpFetchingError);
		}

		// Next we fully read the response body and collect it to a vector of bytes.
		Ok(response.body().collect::<Vec<u8>>())
	}

	/// Send a notarization for the given claim
	fn offchain_send_notarization(key: &T::AuthorityId, claim_id: u64, is_valid: bool) -> Result<(), Error<T>> {
		let payload = NotarizationPayload {
			claim_id,
			public: key.clone(),
			is_valid,
		};
		let signature = key.sign(&payload.encode()).ok_or(<Error<T>>::OffchainUnsignedTxSignedPayloadError)?;
		let call = Call::submit_notarization(payload, signature);
		// Retrieve the signer to sign the payload
		SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into()).map_err(|_| <Error<T>>::OffchainUnsignedTxSignedPayloadError)?;

		Ok(())
	}
}

impl<T: Config> frame_support::unsigned::ValidateUnsigned for Pallet<T> {
	type Call = Call<T>;

	fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
		if let Call::submit_notarization(ref payload, ref signature) = call {
			// TODO: ! check `payload.public` is a valid authority
			// TODO: check `payload.public` has not voted already
			if !(payload.public.verify(&payload.encode(), signature)) {
				return InvalidTransaction::BadProof.into();
			}
			ValidTransaction::with_tag_prefix("eth-bridge")
				.priority(UNSIGNED_TXS_PRIORITY)
				// TODO: does this need to be unique in the tx pool?
				.and_provides([&b"notarize"])
				.longevity(3)
				.propagate(true)
				.build()
		} else {
			InvalidTransaction::Call.into()
		}
	}
}

impl<T: Config> sp_runtime::BoundToRuntimeAppPublic for Pallet<T> {
	type Public = T::AuthorityId;
}

/// Tracks notary public keys (i.e. the active validator set keys)
impl<T: Config> OneSessionHandler<T::AccountId> for Pallet<T> {
	type Key = T::AuthorityId;

	fn on_genesis_session<'a, I: 'a>(validators: I)
		where I: Iterator<Item=(&'a T::AccountId, T::AuthorityId)>
	{
		let keys = validators.map(|x| x.1).collect::<Vec<_>>();
		NotaryKeys::<T>::put(keys);
	}

	fn on_new_session<'a, I: 'a>(_changed: bool, validators: I, _queued_validators: I)
		where I: Iterator<Item=(&'a T::AccountId, T::AuthorityId)>
	{
		// Remember who the authorities are for the new session.
		NotaryKeys::<T>::put(validators.map(|x| x.1).collect::<Vec<_>>());
	}

	fn on_before_session_ending() {
		// TODO: enable offence reporting here
	}

	fn on_disabled(_i: usize) {}
}