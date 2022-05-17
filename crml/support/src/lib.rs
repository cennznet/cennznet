// Copyright 2020-2021 Plug New Zealand Limited & Centrality Investments Limited
// This file is part of Plug.

// Plug is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Plug is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Plug.  If not, see <http://www.gnu.org/licenses/>.

#![cfg_attr(not(feature = "std"), no_std)]

//! # Common crml types and traits

use cennznet_primitives::types::{Balance, FeePreferences, TokenId};
use codec::Encode;
use frame_support::{
	dispatch::GetDispatchInfo,
	pallet_prelude::DispatchResultWithPostInfo,
	traits::{ExistenceRequirement, Imbalance, SignedImbalance, WithdrawReasons},
};
use pallet_evm::AddressMapping;
use precompile_utils::AddressMappingReversibleExt;
pub use primitive_types::{H160, H256, U256};
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, Dispatchable, MaybeSerializeDeserialize, Saturating},
	DispatchError, DispatchResult,
};
use sp_std::{fmt::Debug, marker::PhantomData, prelude::*, result};

/// EVM to CENNZnet address mapping impl
pub struct PrefixedAddressMapping<AccountId>(PhantomData<AccountId>);

/// Converts 20 byte EVM address to 32 byte CENNZnet/substrate address
/// Conversion process is:
/// 1. AccountId prefix: concat("cvm:", "0x00000000000000"), length: 11 bytes
/// 2. EVM address: the original evm address, length: 20 bytes
/// 3. CheckSum:  byte_xor(AccountId prefix + EVM address), length: 1 byte
///
/// e.g.given input EVM address `0x9d6a93a45c9372cc46c9bacfbdb0a2a9398ca903` -
/// output `0x63766d3a000000000000009d6a93a45c9372cc46c9bacfbdb0a2a9398ca90310` cennznet address (hex-ified)
/// breakdown:
/// 63766d3a   00000000000000 9d6a93a45c9372cc46c9bacfbdb0a2a9398ca903 10
/// [ prefix ] [  padding  ]  [            ethereum address          ] [checksum]
impl<AccountId> AddressMapping<AccountId> for PrefixedAddressMapping<AccountId>
where
	AccountId: From<[u8; 32]>,
{
	fn into_account_id(address: H160) -> AccountId {
		let mut raw_account = [0u8; 32];
		raw_account[0..4].copy_from_slice(b"cvm:");
		raw_account[11..31].copy_from_slice(&address[..]);
		let checksum: u8 = raw_account[1..31].iter().fold(raw_account[0], |sum, &byte| sum ^ byte);
		raw_account[31] = checksum;

		raw_account.into()
	}
}

impl<AccountId> AddressMappingReversibleExt<AccountId> for PrefixedAddressMapping<AccountId>
where
	AccountId: From<[u8; 32]> + Into<[u8; 32]>,
{
	fn from_account_id(address: AccountId) -> H160 {
		let mut check_prefix = [0u8; 11];
		check_prefix[0..4].copy_from_slice(b"cvm:");

		let raw_account: [u8; 32] = address.into();

		return if raw_account[..11] == check_prefix {
			let new_account: [u8; 20] = raw_account[11..31].try_into().expect("expected 32 bytes"); // Guaranteed in bounds
			new_account.into()
		} else if raw_account == [0u8; 32] {
			H160::default()
		} else {
			let mut return_account = [0u8; 20];
			return_account[0..4].copy_from_slice(b"crt:");
			return_account.into()
		};
	}
}

/// Tracks the status of sessions in an era
pub trait FinalSessionTracker {
	/// Returns whether the next session the final session of an era
	/// (is_final, was_forced)
	fn is_next_session_final() -> (bool, bool);
	/// Returns whether the active session is final of an era
	fn is_active_session_final() -> bool;
}

/// Something that can be decoded from eth log data/ ABI
/// TODO: ethabi crate would be better for this however no support for `no_std`
pub trait EthAbiCodec: Sized {
	fn encode(&self) -> Vec<u8>;
	/// Decode `Self` from Eth log data
	fn decode(data: &[u8]) -> Option<Self>;
}

impl EthAbiCodec for U256 {
	fn encode(&self) -> Vec<u8> {
		Into::<[u8; 32]>::into(*self).to_vec()
	}

	fn decode(_data: &[u8]) -> Option<Self> {
		unimplemented!();
	}
}

impl EthAbiCodec for u64 {
	fn encode(&self) -> Vec<u8> {
		let uint = U256::from(*self);
		Into::<[u8; 32]>::into(uint).to_vec()
	}

	fn decode(_data: &[u8]) -> Option<Self> {
		unimplemented!();
	}
}

/// Reward validators for notarizations
pub trait NotarizationRewardHandler {
	type AccountId;
	/// Note that the given account ID witnessed an eth-bridge claim
	fn reward_notary(notary: &Self::AccountId);
}

/// Something that subscribes to bridge event claims
#[impl_trait_for_tuples::impl_for_tuples(10)]
pub trait EventClaimSubscriber {
	/// Notify subscriber about a successful event claim for the given event data
	fn on_success(event_claim_id: u64, contract_address: &H160, event_signature: &H256, event_data: &[u8]);
	/// Notify subscriber about a failed event claim for the given event data
	fn on_failure(event_claim_id: u64, contract_address: &H160, event_signature: &H256, event_data: &[u8]);
}

/// Something that verifies event claims
pub trait EventClaimVerifier {
	/// Submit an event claim to the verifier
	/// Returns a unique claim Id on success
	fn submit_event_claim(
		contract_address: &H160,
		event_signature: &H256,
		tx_hash: &H256,
		event_data: &[u8],
	) -> Result<u64, DispatchError>;
	/// Generate proof of the given message
	/// Returns a unique proof Id on success
	fn generate_event_proof<M: EthAbiCodec>(message: &M) -> Result<u64, DispatchError>;
}

// Note: in the following traits the terms:
// - 'token' / 'asset' / 'currency' and
// - 'balance' / 'value' / 'amount'
// are used interchangeably as they make more sense in certain contexts.

/// Something which provides an ID with authority from chain storage
pub trait AssetIdAuthority {
	/// The asset ID type e.g a `u32`
	type AssetId;
	/// Return the authoritative asset ID (no `&self`)
	fn asset_id() -> Self::AssetId;
}

/// Handles transaction fee payment
pub trait TransactionFeeHandler {
	/// Ubiquitous account type
	type AccountId;
	/// Runtime call type
	type Call: Dispatchable + Encode + GetDispatchInfo;
	/// pay fee for `call_info` from `account`
	fn pay_fee(
		len: u32,
		call: &Self::Call,
		info: &<Self::Call as Dispatchable>::Info,
		account: &Self::AccountId,
	) -> Result<(), ()>;
}

/// An interface to count total registrations of an account
pub trait RegistrationInfo {
	type AccountId;
	/// Registration information for an identity
	fn registered_identity_count(who: &Self::AccountId) -> u32;
}

/// An abstraction over the accounting behaviour of a fungible, multi-currency system
/// Currencies in the system are identifiable by a unique `CurrencyId`
pub trait MultiCurrency {
	/// The ID type for an account in the system
	type AccountId: Debug + Default;
	/// The balance of an account for a particular currency
	type Balance: AtLeast32BitUnsigned + Copy + Debug + Default + Saturating + MaybeSerializeDeserialize;
	/// The ID type of a currency in the system
	type CurrencyId: Debug + Default;
	/// The opaque token type for an imbalance of a particular currency. This is returned by unbalanced operations
	/// and must be dealt with. It may be dropped but cannot be cloned.
	type NegativeImbalance: Imbalance<Self::Balance, Opposite = Self::PositiveImbalance>;
	/// The opaque token type for an imbalance of a particular currency. This is returned by unbalanced operations
	/// and must be dealt with. It may be dropped but cannot be cloned.
	type PositiveImbalance: Imbalance<Self::Balance, Opposite = Self::NegativeImbalance>;

	// PUBLIC IMMUTABLES

	/// The minimum balance any single account may have. This is equivalent to the `Balances` module's
	/// `ExistentialDeposit`.
	fn minimum_balance(currency: Self::CurrencyId) -> Self::Balance;

	/// Return the currency Id of the system fee currency
	fn fee_currency() -> Self::CurrencyId;

	/// Return the currency Id of the system staking currency
	fn staking_currency() -> Self::CurrencyId;

	/// The combined balance (free + reserved) of `who` for the given `currency`.
	fn total_balance(who: &Self::AccountId, currency: Self::CurrencyId) -> Self::Balance;

	/// The 'free' balance of a given account.
	///
	/// This is the only balance that matters in terms of most operations on tokens. It alone
	/// is used to determine the balance when in the contract execution environment. When this
	/// balance falls below the value of `ExistentialDeposit`, then the 'current account' is
	/// deleted: specifically `FreeBalance`. Further, the `OnFreeBalanceZero` callback
	/// is invoked, giving a chance to external modules to clean up data associated with
	/// the deleted account.
	///
	/// `system::AccountNonce` is also deleted if `ReservedBalance` is also zero (it also gets
	/// collapsed to zero if it ever becomes less than `ExistentialDeposit`.
	fn free_balance(who: &Self::AccountId, currency: Self::CurrencyId) -> Self::Balance;

	/// Returns `Ok` iff the account is able to make a withdrawal of the given amount
	/// for the given reason. Basically, it's just a dry-run of `withdraw`.
	///
	/// `Err(...)` with the reason why not otherwise.
	fn ensure_can_withdraw(
		who: &Self::AccountId,
		currency: Self::CurrencyId,
		_amount: Self::Balance,
		reasons: WithdrawReasons,
		new_balance: Self::Balance,
	) -> DispatchResult;

	// PUBLIC MUTABLES (DANGEROUS)

	/// Adds up to `value` to the free balance of `who`. If `who` doesn't exist, it is created.
	///
	/// Infallible.
	fn deposit_creating(
		who: &Self::AccountId,
		currency: Self::CurrencyId,
		value: Self::Balance,
	) -> Self::PositiveImbalance;

	/// Mints `value` to the free balance of `who`.
	///
	/// If `who` doesn't exist, nothing is done and an Err returned.
	fn deposit_into_existing(
		who: &Self::AccountId,
		currency: Self::CurrencyId,
		value: Self::Balance,
	) -> result::Result<Self::PositiveImbalance, DispatchError>;

	/// Ensure an account's free balance equals some value; this will create the account
	/// if needed.
	///
	/// Returns a signed imbalance and status to indicate if the account was successfully updated or update
	/// has led to killing of the account.
	fn make_free_balance_be(
		who: &Self::AccountId,
		currency: Self::CurrencyId,
		balance: Self::Balance,
	) -> SignedImbalance<Self::Balance, Self::PositiveImbalance>;

	/// Transfer some liquid free balance to another staker.
	///
	/// This is a very high-level function. It will ensure all appropriate fees are paid
	/// and no imbalance in the system remains.
	fn transfer(
		source: &Self::AccountId,
		dest: &Self::AccountId,
		currency: Self::CurrencyId,
		value: Self::Balance,
		existence_requirement: ExistenceRequirement,
	) -> DispatchResult;

	/// Removes some free balance from `who` account for `reason` if possible. If `liveness` is
	/// `KeepAlive`, then no less than `ExistentialDeposit` must be left remaining.
	///
	/// This checks any locks, vesting, and liquidity requirements. If the removal is not possible,
	/// then it returns `Err`.
	///
	/// If the operation is successful, this will return `Ok` with a `NegativeImbalance` whose value
	/// is `value`.
	fn withdraw(
		who: &Self::AccountId,
		currency: Self::CurrencyId,
		value: Self::Balance,
		reasons: WithdrawReasons,
		liveness: ExistenceRequirement,
	) -> result::Result<Self::NegativeImbalance, DispatchError>;

	/// Move `amount` from free balance to reserved balance.
	///
	/// If the free balance is lower than `amount`, then no funds will be moved and an `Err` will
	/// be returned. This is different behavior than `unreserve`.
	fn reserve(who: &Self::AccountId, currency: Self::CurrencyId, amount: Self::Balance) -> DispatchResult;

	/// Move upto `amount` of reserved balance from `who` to the free balance of `beneficiary`.
	fn repatriate_reserved(
		who: &Self::AccountId,
		currency: Self::CurrencyId,
		beneficiary: &Self::AccountId,
		amount: Self::Balance,
	) -> result::Result<Self::Balance, DispatchError>;

	/// Moves up to `amount` from reserved balance to free balance. This function cannot fail.
	///
	/// As many assets up to `amount` will be moved as possible. If the reserve balance of `who`
	/// is less than `amount`, then the remaining amount will be returned.
	fn unreserve(who: &Self::AccountId, currency: Self::CurrencyId, amount: Self::Balance) -> Self::Balance;

	/// Bring a new currency into existence
	/// Returns the new currency Id
	/// `owner` - the asset owner address
	/// `total_supply` - number of whole tokens to mint to `owner`
	/// `decimal_places` - metadata denoting the decimal places for balances of the asset
	/// `minimum_balance` - a minimum balance for an account to exist
	/// `symbol` - ticker for the asset
	fn create(
		owner: &Self::AccountId,
		initial_supply: Self::Balance,
		decimal_places: u8,
		minimum_balance: u64,
		symbol: Vec<u8>,
	) -> Result<Self::CurrencyId, DispatchError>;
}

/// The interface to provide amount staked by a controller account
pub trait StakingAmount {
	type AccountId;
	type Balance;
	/// Gets the active balance of a controller accounts staked amount
	fn active_balance(controller: &Self::AccountId) -> Self::Balance;
	/// Gets the total amount staked by all accounts
	fn total_staked() -> Self::Balance;
}

/// The interface that states whether an account owns a token
pub trait IsTokenOwner {
	type AccountId;

	/// Gets whether account owns NFT of TokenId
	fn check_ownership(account: &Self::AccountId, token_id: &TokenId) -> bool;
}

/// The nft with the given token_id was transferred.
pub trait OnTransferSubscriber {
	/// The nft with the given token_id was transferred.
	fn on_nft_transfer(token_id: &TokenId);
}

/// Provides an oracle for ethereum chain state
pub trait EthereumStateOracle {
	/// EVM address type
	type Address;
	/// An Unsigned int uniquely identifying remote call requests
	type RequestId;
	/// Issues a request to the oracle to perform a remote 'eth_call'
	/// on the connected Ethereum chain.
	///
	/// `caller` - The calling address on CENNZnet*
	/// `destination` - The remote contract address on Ethereum
	/// `input_data` - evm 'input' data for the remote 'eth_call'
	/// `callback_signature` -Function selector for callback execution
	/// `callback_gas_limit` - Gas limit for callback execution
	/// `fee_preferences` - Options for paying callback fees in non-default currency
	/// `bounty` - A bounty for fulfillment of the request
	///
	/// *The caller must implement the callback ABI `function remoteCallReceiver(uint256 reqId, bytes returnData)`
	///
	/// Returns a unique request Id
	fn new_request(
		_caller: &Self::Address,
		_destination: &Self::Address,
		_input_data: &[u8],
		_callback_signature: &[u8; 4],
		_callback_gas_limit: u64,
		fee_preferences: Option<FeePreferences>,
		bounty: Balance,
	) -> Self::RequestId;
}

/// Provides an interface to invoke a contract execution
pub trait ContractExecutor {
	/// EVM address type
	type Address;
	/// Execute `target` contract with given input
	///
	/// `caller` - address that will invoke the callback
	/// `target` - contract address to receive the callback
	/// `input_data` - passed to evm as 'input'. it should encode the callback function selector
	/// `gas_limit` - gas limit for callback execution
	/// `max_fee_per_gas` - according to EIP-1559,
	/// `max_priority_fee_per_gas` - according to EIP -1559,
	///
	/// Returns consumed weight & result of execution
	fn execute(
		_caller: &Self::Address,
		_target: &Self::Address,
		_input_data: &[u8],
		_gas_limit: u64,
		_max_fee_per_gas: U256,
		_max_priority_fee_per_gas: U256,
	) -> DispatchResultWithPostInfo;
}

/// Verifies correctness of state on Ethereum i.e. by issuing `eth_call`s
pub trait EthCallOracle {
	/// EVM address type
	type Address;
	/// Identifies call requests
	type CallId;
	/// Performs an `eth_call` nearest to `timestamp` on contract `target` with `input`
	///
	/// Returns a call Id for subscribers
	fn call_at(target: Self::Address, input: &[u8], timestamp: u64) -> Self::CallId;
}

impl EthCallOracle for () {
	type Address = H160;
	type CallId = u64;
	fn call_at(_target: Self::Address, _input: &[u8], _timestamp: u64) -> Self::CallId {
		0_u64
	}
}

/// Subscribes to verified ethereum state
pub trait EthCallOracleSubscriber {
	/// Identifies requests
	type CallId;
	/// Receives verified details about prior `EthCallVerifier::call_at` requests upon their completion
	fn on_call_at_complete(call_id: Self::CallId, return_data: &[u8; 32], block_number: u64, block_timestamp: u64);
}

#[cfg(test)]
mod test {
	use super::{PrefixedAddressMapping, H160};
	use hex_literal::hex;
	use pallet_evm::AddressMapping;
	use precompile_utils::AddressMappingReversibleExt;
	use sp_runtime::AccountId32;

	#[test]
	fn address_mapping() {
		let address: AccountId32 = PrefixedAddressMapping::into_account_id(H160::from_slice(&hex!(
			"a86e122EdbDcBA4bF24a2Abf89F5C230b37DF49d"
		)));
		assert_eq!(
			AsRef::<[u8; 32]>::as_ref(&address),
			&hex!("63766d3a00000000000000a86e122edbdcba4bf24a2abf89f5c230b37df49d4a")
		);
	}

	#[test]
	fn reverse_address_mapping_from_eth() {
		let address: H160 = PrefixedAddressMapping::from_account_id(AccountId32::from(hex!(
			"63766d3a00000000000000a86e122edbdcba4bf24a2abf89f5c230b37df49d4a"
		)));
		assert_eq!(
			address,
			H160::from_slice(&hex!("a86e122EdbDcBA4bF24a2Abf89F5C230b37DF49d"))
		);
	}

	#[test]
	fn reverse_address_mapping_from_raw() {
		let address: H160 = PrefixedAddressMapping::from_account_id(AccountId32::from([0u8; 32]));
		assert_eq!(
			address,
			H160::from_slice(&hex!("0000000000000000000000000000000000000000"))
		);
	}

	#[test]
	fn reverse_address_mapping_from_cennz() {
		let address: H160 = PrefixedAddressMapping::from_account_id(AccountId32::from(hex!(
			"63766d3af24a2abf89f5c2a86e122edbdcba4bf24a2abf89f5c230b37df49d4a"
		)));
		assert_eq!(
			address,
			H160::from_slice(&hex!("6372743a00000000000000000000000000000000"))
		);
	}
}
