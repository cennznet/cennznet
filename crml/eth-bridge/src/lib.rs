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
	traits::{MaybeSerializeDeserialize, Member},
	transaction_validity::{InvalidTransaction, TransactionSource, TransactionValidity, ValidTransaction},
	Percent, RuntimeAppPublic, RuntimeDebug,
};
use sp_std::prelude::*;

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
		HttpFetch,
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		#[weight = 100_000_000]
		// TODO: weight here should reflect the offchain work which is triggered as a result
		/// Submit a bridge deposit claim for an ethereum tx hash
		pub fn deposit_claim(origin, tx_hash: H256) {
			// TODO: need replay protection
			// check / store claimed txHashes
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
				// TODO:
				// - clean up + release tokens
				// - if token doesn't exist we need to mint it now also
				// - find token name/symbol from the eth client
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
			if !sp_io::offchain::is_validator() {
				log!(error, "💎 not an active notary this session, exiting: {:?}", key);
				return
			}

			// check all pending claims we have _yet_ to notarize and try to notarize them
			// this will be invoked once every block

			// TODO: need to track local in-flight claims
			// - don't modify state until consensus
			// - allow notarization to continue in event of a restart
			for (claim_id, tx_hash) in PendingClaims::iter() {
				if !<ClaimNotarizations<T>>::contains_key::<u64, T::AuthorityId>(claim_id, key.clone()) {
					let is_valid = Self::offchain_verify_claim(tx_hash);
					let _ = Self::offchain_send_notarization(key, claim_id, is_valid)
						.map_err(|err| {
							log!(error, "💎 sending notarization failed 🙈, {:?}", err);
						})
						.map(|_| {
							log!(info, "💎 signed notarization: '{:?}' for claim: {:?}", is_valid, claim_id);
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

		// transaction must be to the configured bridge contract
		// 0x87015d61b82a3808d9720a79573bf75deb8a1e90
		let contract_address: [u8; 20] = [
			0x87, 0x01, 0x5d, 0x61, 0xb8, 0x2a, 0x38, 0x08, 0xd9, 0x72, 0x0a, 0x79, 0x57, 0x3b, 0xf7, 0x5d, 0xeb, 0x8a, 0x1e, 0x90
		];
		if tx_receipt.to != Some(contract_address.into()) {
			return false;
		}

		// transaction must have event/log of the deposit
		let topic: [u8; 32] = [
			0x76,0xbb,0x91,0x1c,0x36,0x2d,0x5b,0x1f,0xeb,0x30,0x58,0xbc,0x7d,0xc9,0x35,0x47,0x03,0xe4,0xb6,0xeb,0x9c,0x61,0xcc,0x84,0x5f,0x73,0xda,0x88,0x0c,0xf6,0x2f,0x61
		];
		let matching_log = tx_receipt.logs
			.iter()
			.find(|log| log.transaction_hash == Some(tx_hash) && log.topics.contains(&topic.into()));

		if let Some(log) = matching_log {
			// TODO: check `target_log.data`
			// T::DepositEventTopic::get() == keccack256("Deposit(address,address,uint256,bytes32")
			// https://ethereum.stackexchange.com/questions/7835/what-is-topics0-in-event-logs
			// 2) log.data == our expected data
			// rlp crate: https://github.com/paritytech/frontier/blob/1b810cf8143bc545955459ae1e788ef23e627050/frame/ethereum/Cargo.toml#L25
			// https://docs.rs/rlp/0.3.0/src/rlp/lib.rs.html#77-80
			// rlp::decode_list()
			// e.g. MyEventType::rlp_decode(log.data)

			// finally, have we got enough block confirmations to be re-org safe?
			let latest_block_number = Self::get_block_number().unwrap_or_default();
			let tx_block_number = tx_receipt.block_number;
			return latest_block_number.as_u64().saturating_sub(tx_block_number.as_u64()) >= T::RequiredConfirmations::get() as u64;
		}

		return false
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
		const FETCH_TIMEOUT_PERIOD: u64 = 3_000; // in milli-seconds
		log!(info, "💎 sending request to: {}", ETH_HOST);
		let body = serde_json_core::to_string::<serde_json_core::consts::U512, R>(&request_body).unwrap();
		// Initiate an external HTTP GET request. This is using high-level wrappers from `sp_runtime`.
		let request = rt_offchain::http::Request::post(
			ETH_HOST,
			vec![body.as_bytes()]
		);
		log!(trace, "💎 request: {:?}", request);

		// Keeping the offchain worker execution time reasonable, so limiting the call to be within 3s.
		let timeout = sp_io::offchain::timestamp().add(rt_offchain::Duration::from_millis(FETCH_TIMEOUT_PERIOD));
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
	fn offchain_send_notarization(key: &T::AuthorityId, claim_id: u64, is_valid: bool) -> Result<(), Error<T>> {
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

#[cfg(test)]
mod tests2 {
	use std::str::FromStr;
	use ethereum_types::H256;
	use crate::types::{EthBlockNumber, EthResponse, GetBlockNumberRequest, GetTxReceiptRequest, TransactionReceipt};

	#[test]
	fn serialize_eth_block_number_request() {
		let result = serde_json_core::to_string::<serde_json_core::consts::U512, _>(&GetBlockNumberRequest::new()).unwrap();
		assert_eq!(
			result,
			r#"{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}"#
		)
	}

	#[test]
	fn serialize_eth_tx_receipt_request() {
		let result = serde_json_core::to_string::<serde_json_core::consts::U512, _>(&GetTxReceiptRequest::new(H256::from_str("0x185e85beb3296c7339954811cc682e3f992573ad3eecd37409e0ed763448d303").unwrap())).unwrap();
		assert_eq!(
			result,
			r#"{"jsonrpc":"2.0","method":"eth_getTransactionReceipt","params":["0x185e85beb3296c7339954811cc682e3f992573ad3eecd37409e0ed763448d303"],"id":1}"#
		)
	}

	#[test]
	fn deserialize_eth_block_number() {
		let response = r#"
		{
			"jsonrpc":"2.0",
			"id":1,
  			"result": "0x65a8db"
		}
		"#;

		let _result: EthResponse<EthBlockNumber> = serde_json_core::from_str(response).expect("it deserializes");
	}

	#[test]
	fn deserialize_eth_transaction_receipt() {
		let response = r#"
			{
				"jsonrpc":"2.0",
				"id":1,
				"result":{
					"blockHash":"0xa97fa85e0f38526be39a29eb77c07ad9f18c315f8eb6ab7d44028581c1518ec1",
					"blockNumber":"0x5",
					"contractAddress":null,
					"cumulativeGasUsed":"0x1685c",
					"effectiveGasPrice":"0x30cb962f",
					"from":"0xec2c80a819ee8e42c624f6a5de930e8184c0801f",
					"gasUsed":"0x1685c",
					"logs":[
						{"address":"0x17c54edee4d6bccf2379daa328dcc0fbd9c6ce2b",
						"topics":["0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef","0x000000000000000000000000ec2c80a819ee8e42c624f6a5de930e8184c0801f","0x00000000000000000000000087015d61b82a3808d9720a79573bf75deb8a1e90"],
						"data":"0x000000000000000000000000000000000000000000000000000000000000007b","blockNumber":"0x5","transactionHash":"0x185e85beb3296c7339954811cc682e3f992573ad3eecd37409e0ed763448d303","transactionIndex":"0x0",
						"blockHash":"0xa97fa85e0f38526be39a29eb77c07ad9f18c315f8eb6ab7d44028581c1518ec1","logIndex":"0x0","removed":false},{"address":"0x17c54edee4d6bccf2379daa328dcc0fbd9c6ce2b",
						"topics":["0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925","0x000000000000000000000000ec2c80a819ee8e42c624f6a5de930e8184c0801f","0x00000000000000000000000087015d61b82a3808d9720a79573bf75deb8a1e90"],
						"data":"0x000000000000000000000000000000000000000000000000000000000001e1c5",
						"blockNumber":"0x5",
						"transactionHash":"0x185e85beb3296c7339954811cc682e3f992573ad3eecd37409e0ed763448d303",
						"transactionIndex":"0x0",
						"blockHash":"0xa97fa85e0f38526be39a29eb77c07ad9f18c315f8eb6ab7d44028581c1518ec1",
						"logIndex":"0x1",
						"removed":false},
						{"address":"0x87015d61b82a3808d9720a79573bf75deb8a1e90",
						"topics":["0x76bb911c362d5b1feb3058bc7dc9354703e4b6eb9c61cc845f73da880cf62f61","0x000000000000000000000000ec2c80a819ee8e42c624f6a5de930e8184c0801f"],
						"data":"0x00000000000000000000000017c54edee4d6bccf2379daa328dcc0fbd9c6ce2b000000000000000000000000000000000000000000000000000000000000007bacd6118e217e552ba801f7aa8a934ea6a300a5b394e7c3f42cd9d6dd9a457c10","blockNumber":"0x5","transactionHash":"0x185e85beb3296c7339954811cc682e3f992573ad3eecd37409e0ed763448d303","transactionIndex":"0x0","blockHash":"0xa97fa85e0f38526be39a29eb77c07ad9f18c315f8eb6ab7d44028581c1518ec1","logIndex":"0x2","removed":false}],
						"logsBloom":"0x00000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000010000000200200000000000000000000000008000000000000000000000000000000000000000000000000000000001000000000000000000000000010000000000010000000800000000000000000000000000002000000000000000000000000000040000000020000000000000000000010000000000000000000000000000000000000000000000002000000000000000000000000200000000000008000000004000000000010001000000000000000020000000000000000000000000000001000000000",
						"status":"0x1",
						"to":"0x87015d61b82a3808d9720a79573bf75deb8a1e90",
						"transactionHash":"0x185e85beb3296c7339954811cc682e3f992573ad3eecd37409e0ed763448d303",
						"transactionIndex":"0x0",
						"type":"0x0"
				}
			}
		"#;

		let _result: EthResponse<TransactionReceipt> = serde_json_core::from_str(response).expect("it deserializes");
	}
}
