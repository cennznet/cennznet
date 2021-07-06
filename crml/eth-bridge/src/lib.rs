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

use frame_support::{
	log, decl_error, decl_event, decl_module, decl_storage,
	dispatch::DispatchResult,
	traits::Get,
};
use parity_scale_codec::{Codec, Decode, Encode};

use frame_system::{
	ensure_none, ensure_signed,
	offchain::{
		AppCrypto, CreateSignedTransaction, SendUnsignedTransaction,
		SignedPayload, Signer, SigningTypes,
	},
};
use sp_core::{H160, H256, crypto::KeyTypeId};
use sp_runtime::{
	Percent, RuntimeDebug, offchain as rt_offchain,
	transaction_validity::{
		InvalidTransaction, TransactionSource, TransactionValidity, ValidTransaction,
	}
};
use sp_std::{prelude::*, str};

/// Defines application identifier for crypto keys of this module.
///
/// Every module that deals with signatures needs to declare its unique identifier for
/// its crypto keys.
/// When an offchain worker is signing transactions it's going to request keys from type
/// `KeyTypeId` via the keystore to sign the transaction.
/// The keys can be inserted manually via RPC (see `author_insertKey`).
pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"eth-");
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


/// Based on the above `KeyTypeId` we need to generate a pallet-specific crypto type wrapper.
/// We can utilize the supported crypto kinds (`sr25519`, `ed25519` and `ecdsa`) and augment
/// them with the pallet-specific identifier.
pub mod crypto {
	use crate::KEY_TYPE;
	use sp_core::sr25519::Signature as Sr25519Signature;
	use sp_runtime::app_crypto::{app_crypto, sr25519};
	use sp_runtime::{traits::Verify, MultiSignature, MultiSigner};

	app_crypto!(sr25519, KEY_TYPE);

	pub struct TestAuthId;
	// implemented for ocw-runtime
	impl frame_system::offchain::AppCrypto<MultiSigner, MultiSignature> for TestAuthId {
		type RuntimeAppPublic = Public;
		type GenericSignature = sp_core::sr25519::Signature;
		type GenericPublic = sp_core::sr25519::Public;
	}

	// implemented for mock runtime in test
	impl frame_system::offchain::AppCrypto<<Sr25519Signature as Verify>::Signer, Sr25519Signature>
		for TestAuthId
	{
		type RuntimeAppPublic = Public;
		type GenericSignature = sp_core::sr25519::Signature;
		type GenericPublic = sp_core::sr25519::Public;
	}
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
	is_valid: bool
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
	type EthDepositContractTopic: Get<H256>;
	/// The Eth deposit contract address
	type EthDepositContractAddress: Get<H160>;
	/// The minimum number of transaction confirmations needed to ratify an Eth deposit
	type RequiredConfirmations: Get<u16>;
	/// The threshold of notarizations required to approve an Eth deposit
	type DepositApprovalThreshold: Get<Percent>;
	/// Deposits cannot be claimed after this time # of Eth blocks)
	type DepositClaimPeriod: Get<u32>;

	// config types
	/// The identifier type for an offchain worker.
	type AuthorityId: AppCrypto<Self::Public, Self::Signature>;
	/// Returns the count of active network authorities (validators)
	type ActiveAuthoritiesCounter: Get<u32>;
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
		ClaimNotarizations get(fn claim_notarizations): double_map hasher(twox_64_concat) u64, hasher(twox_64_concat) T::AccountId => Option<bool>
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
		// Error returned when not sure which ocw function to executed
		UnknownOffchainMux,

		// Error returned when making signed transactions in off-chain worker
		NoLocalSigningAccount,
		OffchainSignedTxError,

		// Error returned when making unsigned transactions in off-chain worker
		OffchainUnsignedTxError,

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
			let origin = ensure_signed(origin)?;
			let claim_id = Self::next_claim_id();
			PendingClaims::insert(claim_id, tx_hash);
			NextClaimId::put(claim_id.wrapping_add(1));
		}

		#[weight = 100_000]
		/// Internal only
		/// Validators will call this with their notarization vote for a given claim
		pub fn submit_notarization(origin, payload: NotarizationPayload<T::Public>, _signature: T::Signature) -> DispatchResult {
			let _ = ensure_none(origin)?;

			// we don't need to verify the signature here because it has been verified in
			//`validate_unsigned` function when sending out the unsigned tx.
			// TODO: fix types here
			<ClaimNotarizations<T>>::insert(payload.claim_id, payload.public.into(), payload.is_valid);
			// - check if threshold reached for or against
			let notarizations = ClaimNotarizations::iter_prefix(payload.claim_id).count() as u32;

			if Percent::from_rational(notarizations, T::ActiveAuthoritiesCounter::get()) > T::DepositApprovalThreshold::get() {
				// - clean up + release tokens
				// Self::deposit_event(RawEvent::TokenClaim(claim_id));
			}

			Ok(())
		}

		fn offchain_worker(block_number: T::BlockNumber) {
			// TODO: check only validators run this
			log!(info, "💎 entering off-chain worker");
			// check pending claims
			// if empty return
			// if not empty
			// check if voted on the first claim
			// submit notarization for the first claim

			// check all pending claims we have _yet_ to notarize and try to notarize them
			// this will be invoked once every block
			// TODO: is this run async or can it stall a block?
			// if it's async fire all the claims we can otherwise be conservative e.g .limit to 1

			// TODO: only need one account here...
			let account = Signer::<T, T::AuthorityId>::any_account();
			for (claim_id, tx_hash) in PendingClaims::iter() {
				if Self::claim_notarizations(claim_id, account.into()).is_none() {
					let is_valid = Self::offchain_verify_claim(claim_id, tx_hash);
					Self::offchain_send_notarization(claim_id, is_valid);
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
	fn offchain_verify_claim(claim_id: u64, tx_hash: H256) -> bool {
		let result = Self::get_transaction_receipt(tx_hash);
		if let Err(err) = result {
			log!(error, "💎 get tx receipt: {:?}, failed: {:?}", tx_hash, err);
			return false;
		}
		let tx_receipt = result.unwrap();
		let status = tx_receipt.status.unwrap_or_default();
		if status.is_zero() {
			return false
		}

		// transaction should be to our configured bridge contract
		if tx_receipt.to.unwrap_or_default() != T::EthDepositContractAddress::get() {
			return false
		}

		if let Some(log) = tx_receipt.logs
			.iter()
			.find(|log| log.transaction_hash == Some(tx_hash)) {
				// TODO:
				// 1) log.topics == our topic
				// T::DepositEventTopic::get() == keccack256("MyEventName(address,bytes32,uint256")
				// https://ethereum.stackexchange.com/questions/7835/what-is-topics0-in-event-logs
				// 2) log.data == our expected data
				// rlp crate: https://github.com/paritytech/frontier/blob/1b810cf8143bc545955459ae1e788ef23e627050/frame/ethereum/Cargo.toml#L25
				// https://docs.rs/rlp/0.3.0/src/rlp/lib.rs.html#77-80
				// rlp::decode_list()
				// 3) check log.removed is false
				// e.g. MyEventType::rlp_decode(log.data)
			}
		else {
			// no log found
			return false;
		}

		// TODO: fetch latest block number from node
		let latest_block_number = 1_000_u64;
		let tx_block_number = tx_receipt.block_number.unwrap_or_default();
		// have we got enough block confirmations
		if latest_block_number.saturating_sub(tx_block_number.as_u64()) >= T::RequiredConfirmations::get() as u64 {
			return false
		}

		// TODO: need replay protection
		// - require nonce in log on eth side per withdrawing address
		// - store bridge nonce on this side, check it's increasing
		true
	}

	/// Fetch from remote and deserialize the JSON to a struct
	fn get_transaction_receipt(tx_hash: H256) -> Result<TransactionReceipt, Error<T>> {
		let resp_bytes = Self::fetch_from_remote().map_err(|e| {
			log!(error, "💎 Read from eth-rpc API error: {:?}", e);
			<Error<T>>::HttpFetchingError
		})?;

		// Deserialize JSON string to struct
		let resp_str = str::from_utf8(&resp_bytes).map_err(|_| <Error<T>>::HttpFetchingError)?;
		let tx_receipt: TransactionReceipt =
			serde_json::from_str(&resp_str).map_err(|_| <Error<T>>::HttpFetchingError)?;

		Ok(tx_receipt)
	}

	/// This function uses the `offchain::http` API to query the remote github information,
	/// and returns the JSON response as vector of bytes.
	fn fetch_from_remote() -> Result<Vec<u8>, Error<T>> {
		// TODO: load this info from some client config...
		// e.g. offchain indexed
		const HTTP_REMOTE_REQUEST: &str = "https://api.github.com/orgs/substrate-developer-hub";
		const HTTP_HEADER_USER_AGENT: &str = "jimmychu0807";
		const FETCH_TIMEOUT_PERIOD: u64 = 3_000; // in milli-seconds
		log!(info, "💎 sending request to: {}", HTTP_REMOTE_REQUEST);

		// Initiate an external HTTP GET request. This is using high-level wrappers from `sp_runtime`.
		let request = rt_offchain::http::Request::get(HTTP_REMOTE_REQUEST);

		// Keeping the offchain worker execution time reasonable, so limiting the call to be within 3s.
		let timeout = sp_io::offchain::timestamp()
			.add(rt_offchain::Duration::from_millis(FETCH_TIMEOUT_PERIOD));

		// For github API request, we also need to specify `user-agent` in http request header.
		// See: https://developer.github.com/v3/#user-agent-required
		let pending = request
			.add_header("User-Agent", HTTP_HEADER_USER_AGENT)
			.deadline(timeout) // Setting the timeout time
			.send() // Sending the request out by the host
			.map_err(|_| <Error<T>>::HttpFetchingError)?;

		// By default, the http request is async from the runtime perspective. So we are asking the
		// runtime to wait here.
		// The returning value here is a `Result` of `Result`, so we are unwrapping it twice by two `?`
		// ref: https://substrate.dev/rustdocs/v3.0.0/sp_runtime/offchain/http/struct.PendingRequest.html#method.try_wait
		let response = pending
			.try_wait(timeout)
			.map_err(|_| <Error<T>>::HttpFetchingError)?
			.map_err(|_| <Error<T>>::HttpFetchingError)?;

		if response.code != 200 {
			log!(error, "💎 unexpected http request status code: {}", response.code);
			return Err(<Error<T>>::HttpFetchingError);
		}

		// Next we fully read the response body and collect it to a vector of bytes.
		Ok(response.body().collect::<Vec<u8>>())
	}

	/// Send a notarization for the given claim
	fn offchain_send_notarization(claim_id: u64, is_valid: bool) -> Result<(), Error<T>> {
		// Retrieve the signer to sign the payload
		let signer = Signer::<T, T::AuthorityId>::any_account();

		// `send_unsigned_transaction` is returning a type of `Option<(Account<T>, Result<(), ()>)>`.
		//   Similar to `send_signed_transaction`, they account for:
		//   - `None`: no account is available for sending transaction
		//   - `Some((account, Ok(())))`: transaction is successfully sent
		//   - `Some((account, Err(())))`: error occured when sending the transaction
		if let Some((_, res)) = signer.send_unsigned_transaction(
			|account| NotarizationPayload {
				claim_id,
				public: account.public.clone(),
				is_valid,
			},
			Call::submit_notarization,
		) {
			return res.map_err(|_| {
				log!(error, "💎 signing send notarization failed");
				<Error<T>>::OffchainUnsignedTxSignedPayloadError
			});
		} else {
			// The case of `None`: no account is available for sending
			log!(error, "💎 no signing account available");
			Err(<Error<T>>::NoLocalSigningAccount)
		}
	}
}

impl<T: Config> frame_support::unsigned::ValidateUnsigned for Pallet<T> {
	type Call = Call<T>;

	fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
		if let Call::submit_notarization(ref payload, ref signature) = call {
				if !SignedPayload::<T>::verify::<T::AuthorityId>(payload, *signature) {
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
