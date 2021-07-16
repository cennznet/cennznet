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

use cennznet_primitives::types::{AssetId, Balance, BlockNumber};
use codec::{Decode, Encode};
use crml_support::MultiCurrency;
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage, ensure, log,
	traits::{Get, OneSessionHandler, UnixTime, ValidatorSet},
	weights::{constants::RocksDbWeight, Weight},
	PalletId, Parameter,
};
use frame_system::{
	ensure_none, ensure_signed,
	offchain::{CreateSignedTransaction, SubmitTransaction},
};
use sp_core::H256;
use sp_io::KillStorageResult;
use sp_runtime::{
	offchain as rt_offchain,
	offchain::StorageKind,
	traits::{AccountIdConversion, MaybeSerializeDeserialize, Member, SaturatedConversion, Zero},
	transaction_validity::{InvalidTransaction, TransactionSource, TransactionValidity, ValidTransaction},
	KeyTypeId, Percent, RuntimeAppPublic, RuntimeDebug,
};
use sp_std::{
	convert::TryInto,
	prelude::*,
};

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
	/// Returns the block timestamp
	type UnixTime: UnixTime;
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
	/// Deposits cannot be claimed after this time (seconds)
	type DepositDeadline: Get<u64>;
	/// The identifier type for an authority.
	type AuthorityId: Member + Parameter + AsRef<[u8]> + RuntimeAppPublic + Default + Ord + MaybeSerializeDeserialize;
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
		/// Active notary (validator) public keys
		NotaryKeys get(fn notary_keys): Vec<T::AuthorityId>;
		/// Map ERC20 address to GA asset Id
		Erc20ToAsset get(fn erc20_to_asset): map hasher(twox_64_concat) EthAddress => Option<AssetId>;
		/// Map GA asset Id to ERC20 address
		AssetToErc20 get(fn asset_do_erc20): map hasher(twox_64_concat) AssetId => Option<EthAddress>;
		/// Metadata for well-known erc20 tokens
		// TODO: put this info into offchain storage or build into client as constant (not onchain)
		Erc20Meta get(fn erc20_meta): map hasher(twox_64_concat) EthAddress => Option<(Vec<u8>, u8)>;
		/// Info of a claim
		ClaimInfo get(fn claim_info): map hasher(twox_64_concat) ClaimId => Option<EthDepositEvent>;
		/// Pending claims
		PendingClaims get(fn pending_claims): map hasher(twox_64_concat) ClaimId => EthTxHash;
		/// Notarizations for pending claims
		/// Either: None = no notarization exist OR Some(yay/nay)
		ClaimNotarizations get(fn claim_notarizations): double_map hasher(twox_64_concat) ClaimId, hasher(twox_64_concat) T::AuthorityId => Option<bool>;
		/// Completed claims bucketed by unix timestamp of the most recent hour
		// Used in conjunction with `DepositDeadline` to prevent double spends.
		// After a bucket is older than the deadline, any deposits prior are considered expired.
		// This allows the record of claimed transactions to be pruned from state regularly
		CompleteClaimBuckets get(fn complete_claims): double_map hasher(twox_64_concat) u64, hasher(identity) EthTxHash => ();
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
		/// A bridge token claim succeeded
		TokenClaim(ClaimId),
		/// A bridge token claim failed
		TokenClaimFail(ClaimId),
	}
}

decl_error! {
	pub enum Error for Pallet<T: Config> {
		// Error returned when making signed transactions in off-chain worker
		NoLocalSigningAccount,
		// Error returned when making unsigned transactions with signed payloads in off-chain worker
		OffchainUnsignedTxSignedPayload,
		/// A notarization was invalid
		InvalidNotarization,
		// Error returned when fetching github info
		HttpFetch,
		/// Could not create the bridged asset
		CreateAssetFailed,
		/// Claim was invalid
		InvalidClaim,
		/// offchain worker not configured properly
		OcwConfig,
		/// This deposit was already claimed
		AlreadyClaimed
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
				let expired_bucket_index = (now - T::DepositDeadline::get()) % BUCKET_FACTOR_S;
				match CompleteClaimBuckets::remove_prefix(expired_bucket_index, None) {
					KillStorageResult::AllRemoved(count) => RocksDbWeight::get().reads(count as Weight),
					// this won't happen, just handling the case
					KillStorageResult::SomeRemaining(count) => RocksDbWeight::get().reads(count as Weight),
				}
			} else {
				Zero::zero()
			}
		}

		#[weight = 50_000_000]
		/// Submit a bridge deposit claim for an ethereum tx hash
		/// The deposit details must be provided for cross-checking by notaries
		/// Any caller may initiate a claim while only the intended beneficiary will be paid.
		// TODO: weight here should reflect the full amount of offchain work which is triggered as a result
		pub fn deposit_claim(origin, eth_tx_hash: EthTxHash, claim: EthDepositEvent) {
			// Note: require caller to provide the `claim` so we don't need to handle the-
			// complexities of notaries reporting differing deposit events
			let _ = ensure_signed(origin)?;
			// fail a claim early for an amount that is too large
			ensure!(claim.amount < ethereum_types::U256::from(u128::max_value()), Error::<T>::InvalidClaim);
			// fail a claim early for a timestamp that is too large
			ensure!(claim.timestamp < ethereum_types::U256::from(u64::max_value()), Error::<T>::InvalidClaim);
			// fail a claim if it's already been claimed
			let bucket_index = claim.timestamp.as_u64() % BUCKET_FACTOR_S; // checked timestamp < u64
			ensure!(!CompleteClaimBuckets::contains_key(bucket_index, eth_tx_hash), Error::<T>::AlreadyClaimed);
			// fail a claim if beneficiary is not a valid CENNZnet address
			ensure!(T::AccountId::decode(&mut &claim.beneficiary.0[..]).is_ok(), Error::<T>::InvalidClaim);

			let claim_id = Self::next_claim_id();
			ClaimInfo::insert(claim_id, claim);
			PendingClaims::insert(claim_id, eth_tx_hash);
			NextClaimId::put(claim_id.wrapping_add(1));
		}

		#[weight = 1_000_000]
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
			<ClaimNotarizations<T>>::insert::<ClaimId, T::AuthorityId, bool>(payload.claim_id, notary_public_key.clone(), payload.is_valid);

			// Count notarization votes
			let notaries_count = notary_keys.len() as u32;
			let mut yay_count = 0_u32;
			let mut nay_count = 0_u32;
			for (_id, is_valid) in <ClaimNotarizations<T>>::iter_prefix(payload.claim_id) {
				match is_valid {
					true => yay_count += 1,
					false => nay_count += 1,
				}
			}

			// Claim is invalid (nays > 100% - DepositApprovalThreshold)
			if Percent::from_rational(nay_count, notaries_count) > (Percent::from_parts(100_u8 - T::DepositApprovalThreshold::get().deconstruct())) {
				let claim_info = Self::claim_info(payload.claim_id);
				if claim_info.is_none() {
					// this should never happen
					log!(error, "💎 unexpected empty claim");
					return Err(Error::<T>::InvalidClaim.into())
				}

				// free temporary storage
				<ClaimNotarizations<T>>::remove_prefix(payload.claim_id, None);
				PendingClaims::remove(payload.claim_id);
				Self::deposit_event(Event::TokenClaimFail(payload.claim_id));
				return Ok(());
			}

			// Claim is valid
			if Percent::from_rational(yay_count, notaries_count) >= T::DepositApprovalThreshold::get() {
				let claim_info = Self::claim_info(payload.claim_id);
				if claim_info.is_none() {
					// this should never happen
					log!(error, "💎 unexpected empty claim");
					return Err(Error::<T>::InvalidClaim.into())
				}

				<ClaimNotarizations<T>>::remove_prefix(payload.claim_id, None);
				let eth_tx_hash = PendingClaims::take(payload.claim_id);
				let claim_info = claim_info.unwrap();
				// note this tx as complete
				let bucket_index = claim_info.timestamp.as_u64() % BUCKET_FACTOR_S;  // checked amount < u64 in `deposit_claim`
				// no need to track info on this claim any more since it's approved
				CompleteClaimBuckets::insert(bucket_index, eth_tx_hash, ());

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
					claim_info.amount.as_u128() // checked amount < u128 in `deposit_claim`
				);

				Self::deposit_event(Event::TokenClaim(payload.claim_id));
			}
		}

		fn offchain_worker(block_number: T::BlockNumber) {
			log!(trace, "💎 entering off-chain worker: {:?}", block_number);
			log!(info, "💎 active notaries: {:?}", Self::notary_keys());

			// check local `key` is a valid bridge notary
			if !sp_io::offchain::is_validator() {
				// this passes if flag `--validator` set not necessarily
				// in the active set
				log!(info, "💎 not a validator, exiting");
				return
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
			for (claim_id, tx_hash) in PendingClaims::iter() {

				if budget.is_zero() {
					log!(info, "💎 claims budget exceeded, exiting...");
					return
				}

				// check we haven't notarized this already
				if <ClaimNotarizations<T>>::contains_key::<ClaimId, T::AuthorityId>(claim_id, key.clone()) {
					log!(trace, "💎 already cast notarization for claim: {:?}, ignoring...", claim_id);
				}

				if let Some(claim_info) = Self::claim_info(claim_id) {
					let result = Self::offchain_verify_claim(tx_hash, claim_info);
					log!(trace, "💎 claim verification status: {:?}", result);
					let payload = NotarizationPayload {
						claim_id,
						authority_index,
						is_valid: result.is_ok()
					};
					let _ = Self::offchain_send_notarization(&key, payload)
						.map_err(|err| {
							log!(error, "💎 sending notarization failed 🙈, {:?}", err);
						})
						.map(|_| {
							log!(info, "💎 sent notarization: '{:?}' for claim: {:?}", result.is_ok(), claim_id);
						});
					budget = budget.saturating_sub(1);
				} else {
					// should not happen, defensive only
					log!(error, "💎 empty claim data for: {:?}", claim_id);
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

		let maybe_tx_receipt = result.unwrap(); // error handled above qed.
		let tx_receipt = match maybe_tx_receipt {
			Some(tx_receipt) => tx_receipt,
			None => return Err(ClaimFailReason::NoTxLogs),
		};
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
			.find(|log| log.transaction_hash == Some(tx_hash) && log.topics.contains(&topic));

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
					// claim is past the expiration deadline
					// ` reported_claim_event.timestamp` < u64 checked in `deposit_claim`
					if T::UnixTime::now().as_millis().saturated_into::<u64>() - reported_claim_event.timestamp.as_u64()
						> T::DepositDeadline::get()
					{
						return Err(ClaimFailReason::Expired);
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
			let maybe_block_number = result.unwrap();
			if maybe_block_number.is_none() {
				return Err(ClaimFailReason::DataProvider);
			}
			let latest_block_number = maybe_block_number.unwrap().as_u64();
			let block_confirmations = latest_block_number.saturating_sub(tx_receipt.block_number.as_u64());
			if block_confirmations < T::RequiredConfirmations::get() as u64 {
				return Err(ClaimFailReason::NotEnoughConfirmations);
			}

			// it's ok!
			return Ok(());
		}

		return Err(ClaimFailReason::NoTxLogs);
	}

	/// Get transaction receipt from eth client
	fn get_transaction_receipt(tx_hash: EthTxHash) -> Result<Option<TransactionReceipt>, Error<T>> {
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
	fn get_block_number() -> Result<Option<EthBlockNumber>, Error<T>> {
		let random_request_id = u32::from_be_bytes(sp_io::offchain::random_seed()[..4].try_into().unwrap());
		let request = GetBlockNumberRequest::new(random_request_id as usize);
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
			ValidTransaction::with_tag_prefix("eth-bridge")
				.priority(UNSIGNED_TXS_PRIORITY)
				// 'provides' must be unique for each submission on the network (i.e. unique for each claim id and validator)
				.and_provides([
					b"notarize",
					&payload.claim_id.to_be_bytes(),
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
