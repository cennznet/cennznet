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

use cennznet_primitives::types::{AssetId, Balance};
use codec::{Decode, Encode};
use crml_support::MultiCurrency;
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage, ensure, log,
	traits::{Get, OneSessionHandler, ValidatorSet},
	PalletId, Parameter,
};
use frame_system::{
	ensure_none, ensure_root, ensure_signed,
	offchain::{CreateSignedTransaction, SubmitTransaction},
};
use sp_core::H256;
use sp_runtime::{
	offchain as rt_offchain,
	traits::{AccountIdConversion, MaybeSerializeDeserialize, Member, Zero},
	transaction_validity::{InvalidTransaction, TransactionSource, TransactionValidity, ValidTransaction},
	KeyTypeId, Percent, RuntimeAppPublic, RuntimeDebug,
};
use sp_std::prelude::*;

pub const ETH_BRIDGE: KeyTypeId = KeyTypeId(*b"eth-");

pub mod crypto {
	mod app_crypto {
		use crate::ETH_BRIDGE;
		use sp_application_crypto::{app_crypto, ecdsa};
		app_crypto!(ecdsa, ETH_BRIDGE);
	}

	sp_application_crypto::with_pair! {
		/// An i'm online keypair using ecdsa as its crypto.
		pub type AuthorityPair = app_crypto::Pair;
	}

	/// An i'm online signature using ecdsa as its crypto.
	pub type AuthoritySignature = app_crypto::Signature;

	/// An i'm online identifier using ecdsa as its crypto.
	pub type AuthorityId = app_crypto::Public;
}

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
pub struct NotarizationPayload {
	/// The claim Id being notarized
	claim_id: ClaimId,
	/// The ordinal index of the signer in the notary set
	/// It may be used with chain storage to lookup the public key of the notary
	authority_index: u16,
	/// Whether the claim was validated or not
	is_valid: bool,
}

/// This is the pallet's configuration trait
pub trait Config: frame_system::Config + CreateSignedTransaction<Call<Self>> {
	/// An onchain address for this pallet
	type BridgePalletId: Get<PalletId>;
	/// Currency functions
	type MultiCurrency: MultiCurrency<AccountId = Self::AccountId, Balance = Balance, CurrencyId = AssetId>;
	/// Event signature of a deposit on the Ethereum bridge contract
	type DepositEventSignature: Get<[u8; 32]>;
	/// Eth bridge contract address
	type BridgeContractAddress: Get<[u8; 20]>;
	/// The minimum number of transaction confirmations needed to ratify an Eth deposit
	type RequiredConfirmations: Get<u16>;
	/// The threshold of notarizations required to approve an Eth deposit
	type DepositApprovalThreshold: Get<Percent>;
	/// Deposits cannot be claimed after this time # of Eth blocks)
	type DepositClaimPeriod: Get<u32>;
	/// The identifier type for an authority.
	type AuthorityId: Member + Parameter + RuntimeAppPublic + Default + Ord + MaybeSerializeDeserialize;
	/// Active notaries
	type NotarySet: ValidatorSet<Self::AccountId>;
	/// The overarching dispatch call type.
	type Call: From<Call<Self>>;
	/// The overarching event type.
	type Event: From<Event> + Into<<Self as frame_system::Config>::Event>;
}

decl_storage! {
	trait Store for Pallet<T: Config> as EthBridge {
		/// Id of a token claim
		NextClaimId get(fn next_claim_id): ClaimId;
		/// Info of a claim
		ClaimInfo get(fn claim_info): map hasher(twox_64_concat) ClaimId => Option<EthDepositEvent>;
		/// Pending claims
		PendingClaims get(fn pending_claims): map hasher(twox_64_concat) ClaimId => EthTxHash;
		/// Notarizations for pending claims
		/// Either: None = no notarization exist OR Some(yay/nay)
		ClaimNotarizations get(fn claim_notarizations): double_map hasher(twox_64_concat) ClaimId, hasher(twox_64_concat) T::AuthorityId => Option<bool>;
		/// Active notary (validator) public keys
		NotaryKeys get(fn notary_keys): Vec<T::AuthorityId>;
		/// Map ERC20 address to GA asset Id
		pub Erc20ToAsset get(fn erc20_to_asset): map hasher(twox_64_concat) EthAddress => Option<AssetId>;
		/// Map GA asset Id to ERC20 address
		pub AssetToErc20 get(fn asset_do_erc20): map hasher(twox_64_concat) AssetId => Option<EthAddress>;
		/// Metadata for well-known erc20 tokens
		Erc20Meta get(fn erc20_meta): map hasher(twox_64_concat) EthAddress => Option<(Vec<u8>, u8)>;
		// Track completed claims, keyed by era id
		// it is pruned after claims period expires
		// CompleteClaims get(fn complete_claims): double_map hasher(twox_64_concat) u64, hasher(blake2_128_concat) EthTxHash => ();
	}
	add_extra_genesis {
		config(erc20s): Vec<(EthAddress, Vec<u8>, u8)>;
		build(|config: &GenesisConfig| {
			for (address, symbol, decimals) in config.erc20s.iter() {
				Erc20Meta::insert(address, (symbol, decimals));
			}
		});
	}
}

decl_event! {
	pub enum Event {
		/// A bridge token claim succeeded (claim id)
		TokenClaim(ClaimId),
	}
}

decl_error! {
	pub enum Error for Pallet<T: Config> {
		// Error returned when making signed transactions in off-chain worker
		NoLocalSigningAccount,
		// Error returned when making unsigned transactions with signed payloads in off-chain worker
		OffchainUnsignedTxSignedPayloadError,
		/// A notarization was invalid
		InvalidNotarization,
		// Error returned when fetching github info
		HttpFetch,
		/// Could not create the bridged asset
		CreateAssetFailed,
		/// Claim was invalid
		InvalidClaim
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		#[weight = 100_000_000]
		/// Submit a bridge deposit claim for an ethereum tx hash
		/// The deposit details must be provided for cross-checking by notaries
		/// Any caller may initiate a claim while only the intended beneficiary will be paid.
		pub fn deposit_claim(origin, eth_tx_hash: H256, eth_deposit_event: EthDepositEvent) {
			// Note: require caller to provide the `eth_deposit_event` so we don't need to handle the-
			// complexities of notaries reporting differing deposit events
			// TODO: weight here should reflect the full amount of offchain work which is triggered as a result
			// TODO: need replay protection:
			// 1) check / store successfully claimed txHashes
			// 2) claims older than some time period should be invalid, allowing us to release claimed txHashes from storage at regular intervals
			let _ = ensure_signed(origin)?;
			// ensure!(!CompleteClaims::<T>::contains_key(eth_tx_hash), Error::<T>::AlreadyClaimed);
			let claim_id = Self::next_claim_id();
			// fail a claim early for an amount that is too large
			ensure!(eth_deposit_event.amount < ethereum_types::U256::from(u128::max_value()), Error::<T>::InvalidClaim);
			// fail a claim if beneficiary is not a valid CENNZnet address
			ensure!(T::AccountId::decode(&mut &eth_deposit_event.beneficiary.0[..]).is_ok(), Error::<T>::InvalidClaim);
			ClaimInfo::insert(claim_id, eth_deposit_event);
			PendingClaims::insert(claim_id, eth_tx_hash);
			NextClaimId::put(claim_id.wrapping_add(1));
		}

		#[weight = 1_000_000]
		/// Internal only
		/// Validators will call this with their notarization vote for a given claim
		pub fn submit_notarization(origin, payload: NotarizationPayload, _signature: <<T as Config>::AuthorityId as RuntimeAppPublic>::Signature) {
			let _ = ensure_none(origin)?;

			// we don't need to verify the signature here because it has been verified in
			//`validate_unsigned` function when sending out the unsigned tx.
			let notary_keys = Self::notary_keys();
			let notary_public_key = match notary_keys.get(payload.authority_index as usize) {
				Some(id) => id,
				None => return Err(Error::<T>::InvalidNotarization.into()),
			};
			<ClaimNotarizations<T>>::insert::<ClaimId, T::AuthorityId, bool>(payload.claim_id, notary_public_key.clone(), payload.is_valid);
			// - check if threshold reached for or against
			let notaries_count = notary_keys.len() as u32;
			let notarizations_count = <ClaimNotarizations<T>>::iter_prefix(payload.claim_id).count() as u32;

			if Percent::from_rational(notarizations_count, notaries_count) >= T::DepositApprovalThreshold::get() {
				// no need to track info on this claim any more since it's approved
				PendingClaims::remove(payload.claim_id);
				let claim_info = ClaimInfo::take(payload.claim_id);
				<ClaimNotarizations<T>>::remove_prefix(payload.claim_id, None);

				if claim_info.is_none() {
					// this should never happen
					log!(error, "💎 not an active notary, exiting");
					return Err(Error::<T>::InvalidClaim.into())
				}

				let claim_info = claim_info.unwrap();
				let asset_id = match Self::erc20_to_asset(claim_info.token_type) {
					None => {
						// create asset with known values from `Erc20Meta`
						// asset will be created with `0` decimal places and "" for symbol if the asset is unknown
						// dapps can also use `AssetToERC20` to retrieve the appropriate decimal places from ethereum
						let (symbol, decimals) = Erc20Meta::get(claim_info.token_type).unwrap_or_default();
						let asset_id = T::MultiCurrency::create(
							&T::BridgePalletId::get().into_account(),
							Zero::zero(), // 0 supply
							decimals,
							1, // minimum balance
							symbol,
						).map_err(|_| Error::<T>::CreateAssetFailed)?;
						Erc20ToAsset::insert(claim_info.token_type, asset_id);
						AssetToErc20::insert(asset_id, claim_info.token_type);

						asset_id
					},
					Some(asset_id) => asset_id,
				};

				// checked at the time of initiating the claim that beneficiary value is valid and this op will not fail qed.
				let beneficiary: T::AccountId = T::AccountId::decode(&mut &claim_info.beneficiary.0[..]).unwrap();
				let _imbalance = T::MultiCurrency::deposit_creating(
					&beneficiary,
					asset_id,
					claim_info.amount.as_u128()
				);

				Self::deposit_event(Event::TokenClaim(payload.claim_id));
			}
		}

		fn offchain_worker(_block_number: T::BlockNumber) {
			log!(info, "💎 entering off-chain worker");

			// check local `key` is a valid bridge notary
			if !sp_io::offchain::is_validator() {
				log!(error, "💎 not an active notary, exiting");
				return
			}

			// Get all signing keys for this protocol 'KeyTypeId'
			let keys = T::AuthorityId::all();
			// Only expect one key to be present
			if keys.iter().count() > 1 {
				log!(error, "💎 multiple signing keys detected for: {:?}, bailing...", T::AuthorityId::ID);
				return
			}
			let key = match keys.first() {
				Some(key) => key,
				None => {
					log!(error, "💎 no signing keys for: {:?}, cannot participate in notarization!", T::AuthorityId::ID);
					return
				}
			};

			// check all pending claims we have _yet_ to notarize and try to notarize them
			// this will be invoked once every block
			// we limit the total claims per invocation using `CLAIMS_PER_BLOCK` so we don't stall block production
			let mut budget = CLAIMS_PER_BLOCK;
			for (claim_id, tx_hash) in PendingClaims::iter() {
				// if we haven't voted on this claim yet, then try!
				if !<ClaimNotarizations<T>>::contains_key::<ClaimId, T::AuthorityId>(claim_id, key.clone()) {
					if let Some(claim_info) = Self::claim_info(claim_id) {
						let result = Self::offchain_verify_claim(tx_hash, claim_info);
						log!(trace, "💎 claim verification status: {:?}", result);
						let _ = Self::offchain_send_notarization(key, claim_id, result.is_ok())
							.map_err(|err| {
								log!(error, "💎 sending notarization failed 🙈, {:?}", err);
							})
							.map(|_| {
								log!(info, "💎 signed notarization: '{:?}' for claim: {:?}", result.is_ok(), claim_id);
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
}

impl<T: Config> Pallet<T> {
	/// Verify a claim
	/// - check Eth full node for transaction status
	/// - tx success
	/// - tx sent to deposit contract address
	/// - check for log with deposited amount and token type
	/// - confirmations >= T::RequiredConfirmations
	fn offchain_verify_claim(tx_hash: EthTxHash, reported_claim_event: EthDepositEvent) -> Result<(), ClaimFailReason> {
		let result = Self::get_transaction_receipt(tx_hash);
		if let Err(err) = result {
			log!(error, "💎 eth_getTransactionReceipt({:?}) failed: {:?}", tx_hash, err);
			return Err(ClaimFailReason::DataProvider);
		}

		let tx_receipt = result.unwrap(); // error handled above qed.
		let status = tx_receipt.status.unwrap_or_default();
		if status.is_zero() {
			return Err(ClaimFailReason::TxStatusFailed);
		}

		if tx_receipt.to != Some(T::BridgeContractAddress::get().into()) {
			return Err(ClaimFailReason::InvalidBridgeAddress);
		}

		let topic: H256 = T::DepositEventSignature::get().into();
		// search for a bridge deposit event in this tx receipt
		let matching_log = tx_receipt
			.logs
			.iter()
			.find(|log| log.transaction_hash == Some(tx_hash) && log.topics.contains(&topic.into()));

		if let Some(log) = matching_log {
			match EthDepositEvent::try_decode_from_log(log) {
				Some(event) => {
					// check if the ethereum deposit event matches what was reported
					// in the original claim
					if reported_claim_event != event {
						log!(
							trace,
							"💎 mismatch in claim vs. event: reported: {:?} real: {:?}",
							reported_claim_event,
							event
						);
						return Err(ClaimFailReason::ProvenInvalid);
					}
				}
				None => {
					return Err(ClaimFailReason::NoTxLogs);
				}
			}

			// lastly, have we got enough block confirmations to be re-org safe?
			let result = Self::get_block_number();
			if let Err(err) = result {
				log!(error, "💎 eth_getBlock failed: {:?}", err);
				return Err(ClaimFailReason::DataProvider);
			}
			let latest_block_number = result.unwrap().as_u64();
			if latest_block_number.saturating_sub(tx_receipt.block_number.as_u64())
				< T::RequiredConfirmations::get() as u64
			{
				return Err(ClaimFailReason::NotEnoughConfirmations);
			}

			// it's ok!
			return Ok(());
		}

		return Err(ClaimFailReason::NoTxLogs);
	}

	/// Get transaction receipt from eth client
	fn get_transaction_receipt(tx_hash: EthTxHash) -> Result<TransactionReceipt, Error<T>> {
		let request = GetTxReceiptRequest::new(tx_hash);
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
	fn get_block_number() -> Result<EthBlockNumber, Error<T>> {
		let request = GetBlockNumberRequest::new();
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
		// TODO: load this info from some client config.e.g. offchain indexed
		const ETH_HOST: &str = "http://localhost:8545";
		const HEADER_CONTENT_TYPE: &str = "application/json";
		log!(info, "💎 sending request to: {}", ETH_HOST);
		let body = serde_json::to_string::<R>(&request_body).unwrap();
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

		// Read the response body and check it's valid utf-8
		Ok(response.body().collect::<Vec<u8>>())
	}

	/// Send a notarization for the given claim
	fn offchain_send_notarization(key: &T::AuthorityId, claim_id: ClaimId, is_valid: bool) -> Result<(), Error<T>> {
		let authority_index = Self::notary_keys()
			.binary_search(key)
			.map(|pos| pos as u16)
			.map_err(|_| {
				log!(error, "💎 not found in authority set, this is a bug");
				return <Error<T>>::OffchainUnsignedTxSignedPayloadError;
			})?;
		let payload = NotarizationPayload {
			claim_id,
			authority_index,
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
			// notarization must be from an active notary
			let notary_keys = Self::notary_keys();
			let notary_public_key = match notary_keys.get(payload.authority_index as usize) {
				Some(id) => id,
				None => return InvalidTransaction::BadProof.into(),
			};
			// notarization must not be a duplicate/equivocation
			if <ClaimNotarizations<T>>::contains_key(payload.claim_id, &notary_public_key) {
				log!(
					error,
					"💎 received equivocation from: {:?} on {:?}",
					notary_public_key,
					payload.claim_id
				);
				return InvalidTransaction::BadProof.into();
			}
			// notarization is signed correctly
			if !(notary_public_key.verify(&payload.encode(), signature)) {
				return InvalidTransaction::BadProof.into();
			}
			// TODO: does 'provides' need to be unique for all validators?
			// txs with the same 'provides' produce: Error submitting a transaction to the pool: Pool(TooLowPriority { old: 100100, new: 100100 })
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

impl<T: Config> sp_runtime::BoundToRuntimeAppPublic for Pallet<T> {
	type Public = T::AuthorityId;
}

impl<T: Config> OneSessionHandler<T::AccountId> for Pallet<T> {
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

	fn on_new_session<'a, I: 'a>(_changed: bool, validators: I, _queued_validators: I)
	where
		I: Iterator<Item = (&'a T::AccountId, T::AuthorityId)>,
	{
		// Record authorities for the new session.
		NotaryKeys::<T>::put(validators.map(|x| x.1).collect::<Vec<_>>());
	}

	fn on_before_session_ending() {}
	fn on_disabled(_i: usize) {}
}
