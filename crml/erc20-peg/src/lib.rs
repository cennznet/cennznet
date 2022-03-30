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

#![cfg_attr(not(feature = "std"), no_std)]

use cennznet_primitives::{
	eth::EventId,
	types::{AssetId, Balance},
};
use codec::{Decode, Encode};
use crml_support::{EventClaimSubscriber, EventClaimVerifier, MultiCurrency};
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage, ensure, log,
	pallet_prelude::*,
	traits::{ExistenceRequirement, Get, IsType, WithdrawReasons},
	transactional, PalletId,
};
use frame_system::{ensure_root, ensure_signed};
use sp_runtime::{
	traits::{AccountIdConversion, Saturating, Zero},
	DispatchError,
};
use sp_runtime::{
	traits::{Hash, One},
	SaturatedConversion,
};
use sp_std::prelude::*;

mod types;
use types::*;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub trait Config: frame_system::Config {
	/// An onchain address for this pallet
	type PegPalletId: Get<PalletId>;
	/// The EVM event signature of a deposit
	type DepositEventSignature: Get<[u8; 32]>;
	/// Failed claim delay length
	type FailedClaimDelay: Get<u32>;
	/// Submits event claims for Ethereum
	type EthBridge: EventClaimVerifier;
	/// Currency functions
	type MultiCurrency: MultiCurrency<AccountId = Self::AccountId, Balance = Balance, CurrencyId = AssetId>;
	/// The overarching event type.
	type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
}

decl_storage! {
	trait Store for Module<T: Config> as Erc20Peg {
		/// Whether deposit are active
		DepositsActive get(fn deposits_active): bool;
		/// Whether withdrawals are active
		WithdrawalsActive get(fn withdrawals_active): bool;
		/// Map ERC20 address to GA asset Id
		Erc20ToAssetId get(fn erc20_to_asset): map hasher(twox_64_concat) EthAddress => Option<AssetId>;
		/// Map GA asset Id to ERC20 address
		AssetIdToErc20 get(fn asset_to_erc20): map hasher(twox_64_concat) AssetId => Option<EthAddress>;
		/// Metadata for well-known erc20 tokens (symbol, decimals)
		Erc20Meta get(fn erc20_meta): map hasher(twox_64_concat) EthAddress => Option<(Vec<u8>, u8)>;
		/// Map from asset_id to minimum amount and delay
		ClaimDelay get(fn claim_delay): map hasher(twox_64_concat) AssetId => Option<(Balance, T::BlockNumber)>;
		/// Map from claim id to claim
		PendingClaims get(fn pending_claims): map hasher(twox_64_concat) ClaimId => Option<PendingClaim>;
		/// Map from block number to claims scheduled for that block
		ClaimSchedule get(fn claim_schedule): map hasher(twox_64_concat) T::BlockNumber => Vec<ClaimId>;
		/// The blocks with claims that are ready to be processed
		ReadyBlocks get(fn ready_blocks): Vec<T::BlockNumber>;
		/// The next available claim id for withdrawals and deposit claims
		NextClaimId get(fn next_claim_id): ClaimId;
		/// Hash of withdrawal information
		WithdrawalDigests get(fn withdrawal_digests): map hasher(twox_64_concat) EventId => T::Hash;
		/// The peg contract address on Ethereum
		ContractAddress get(fn contract_address): EthAddress;
		/// Whether CENNZ deposits are active
		CENNZDepositsActive get(fn cennz_deposit_active): bool;
	}
	add_extra_genesis {
		config(erc20s): Vec<(EthAddress, Vec<u8>, u8)>;
		build(|config: &GenesisConfig| {
			for (address, symbol, decimals) in config.erc20s.iter() {
				if symbol == b"CENNZ" {
					Erc20ToAssetId::insert::<EthAddress, AssetId>(*address, T::MultiCurrency::staking_currency());
				}
				Erc20Meta::insert(address, (symbol, decimals));
			}
		});
	}
}

decl_event! {
	pub enum Event<T> where
		AccountId = <T as frame_system::Config>::AccountId,
		BlockNumber = <T as frame_system::Config>::BlockNumber,
	{
		/// An erc20 deposit claim has started. (deposit Id, sender)
		Erc20Claim(u64, AccountId),
		/// An erc20 claim has been delayed.(claim_id, scheduled block, amount, beneficiary)
		Erc20DepositDelayed(ClaimId, BlockNumber, Balance, AccountId),
		/// A withdrawal has been delayed.(claim_id, scheduled block, amount, beneficiary)
		Erc20WithdrawalDelayed(ClaimId, BlockNumber, Balance, EthAddress),
		/// A bridged erc20 deposit succeeded.(deposit Id, asset, amount, beneficiary)
		Erc20Deposit(u64, AssetId, Balance, AccountId),
		/// Tokens were burnt for withdrawal on Ethereum as ERC20s (withdrawal Id, asset, amount, beneficiary)
		Erc20Withdraw(u64, AssetId, Balance, EthAddress),
		/// A bridged erc20 deposit failed.(deposit Id)
		Erc20DepositFail(u64),
		/// The peg contract address has been set
		SetContractAddress(EthAddress),
		/// ERC20 CENNZ deposits activated
		CENNZDepositsActive,
		/// A delay was added for an asset_id (asset_id, min_balance, delay)
		ClaimDelaySet(AssetId, Balance, BlockNumber),
		/// There are no more claim ids available, they've been exhausted
		NoAvailableClaimIds,
	}
}

decl_error! {
	pub enum Error for Module<T: Config> {
		/// Could not create the bridged asset
		CreateAssetFailed,
		/// Claim has bad account
		InvalidAddress,
		/// Claim has bad amount
		InvalidAmount,
		/// Deposits are inactive
		DepositsPaused,
		/// Withdrawals are inactive
		WithdrawalsPaused,
		/// Withdrawals of this asset are not supported
		UnsupportedAsset,
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {

		fn deposit_event() = default;

		/// Check and process outstanding claims
		fn on_initialize(now: T::BlockNumber) -> Weight {
			let mut weight: u64 = 10_000_000;
			if ClaimSchedule::<T>::contains_key(now) {
				ReadyBlocks::<T>::append(now);
				weight += 10_000_000;
			}
			weight as Weight
		}

		/// Check and process outstanding claims
		fn on_idle(_now: T::BlockNumber, remaining_weight: Weight) -> Weight {
			let initial_read_cost = 10_000_000;
			// Ensure we have enough weight to perform the initial read
			if remaining_weight <= initial_read_cost {
				return 0;
			}
			// Check that there are blocks in ready_blocks
			if ReadyBlocks::<T>::decode_len().is_none() {
				return 0;
			}

			// Process as many claims as we can
			let weight_each: Weight = 50_000_000;
			let max_claims = ((remaining_weight - initial_read_cost) / weight_each).saturated_into::<u8>();
			let mut ready_blocks: Vec<T::BlockNumber> = Self::ready_blocks();
			// Total claims processed in this block
			let mut processed_claim_count: u8 = 0;
			// Count of blocks where all claims have been processed
			let mut processed_block_count: u8 = 0;

			for block in ready_blocks.iter() {
				let mut claim_ids = ClaimSchedule::<T>::take(block);
				let remaining_claims = (max_claims - processed_claim_count) as usize;
				if claim_ids.len() > remaining_claims {
					// Update storage with unprocessed claims
					ClaimSchedule::<T>::insert(block, claim_ids.split_off(remaining_claims));
				} else {
					processed_block_count += 1;
				}
				processed_claim_count += claim_ids.len() as u8;
				// Process remaining claims from block
				for claim_id in claim_ids {
					Self::process_claim(claim_id);
				}
				if processed_claim_count >= max_claims {
					break;
				}
			}

			ReadyBlocks::<T>::put(ready_blocks.split_off(processed_block_count.into()));
			initial_read_cost + weight_each * processed_claim_count as Weight
		}

		/// Activate/deactivate deposits (root only)
		#[weight = 10_000_000]
		pub fn activate_deposits(origin, activate: bool) {
			ensure_root(origin)?;
			DepositsActive::put(activate);
		}

		/// Activate/deactivate withdrawals (root only)
		#[weight = 10_000_000]
		pub fn activate_withdrawals(origin, activate: bool) {
			ensure_root(origin)?;
			WithdrawalsActive::put(activate);
		}

		#[weight = 60_000_000]
		/// Submit deposit claim for an ethereum tx hash
		/// The deposit details must be provided for cross-checking by notaries
		/// Any caller may initiate a claim while only the intended beneficiary will be paid.
		#[transactional]
		pub fn deposit_claim(origin, tx_hash: H256, claim: Erc20DepositEvent) {
			// Note: require caller to provide the `claim` so we don't need to handle the-
			// complexities of notaries reporting differing deposit events
			let _origin = ensure_signed(origin)?;
			ensure!(Self::deposits_active(), Error::<T>::DepositsPaused);
			// fail a claim early for an amount that is too large
			ensure!(claim.amount < U256::from(u128::max_value()), Error::<T>::InvalidAmount);
			// fail a claim if beneficiary is not a valid CENNZnet address
			ensure!(T::AccountId::decode(&mut &claim.beneficiary.0[..]).is_ok(), Error::<T>::InvalidAddress);

			let asset_id = Self::erc20_to_asset(claim.token_address);
			if asset_id.is_some() {
				let claim_delay: Option<(Balance, T::BlockNumber)> = Self::claim_delay(asset_id.unwrap());
				if let Some((min_amount, delay)) = claim_delay {
					if U256::from(min_amount) <= claim.amount {
						Self::delay_claim(delay, PendingClaim::Deposit((claim.clone(), tx_hash)));
						return Ok(());
					}
				};
			}
			// process deposit immediately
			Self::process_deposit_claim(claim, tx_hash);
		}

		#[weight = 60_000_000]
		/// Withdraw generic assets from CENNZnet in exchange for ERC20s
		/// Tokens will be transferred to peg account and a proof generated to allow redemption of tokens on Ethereum
		#[transactional]
		pub fn withdraw(origin, asset_id: AssetId, amount: Balance, beneficiary: EthAddress) {
			let origin = ensure_signed(origin)?;
			ensure!(Self::withdrawals_active(), Error::<T>::WithdrawalsPaused);

			// there should be a known ERC20 address mapped for this asset
			// otherwise there may be no liquidity on the Ethereum side of the peg
			let token_address = Self::asset_to_erc20(asset_id);
			ensure!(token_address.is_some(), Error::<T>::UnsupportedAsset);
			let token_address = token_address.unwrap();

			if asset_id == T::MultiCurrency::staking_currency() {
				let _result = T::MultiCurrency::transfer(
					&origin,
					&T::PegPalletId::get().into_account(),
					asset_id,
					amount, // checked amount < u128 in `deposit_claim` qed.
					ExistenceRequirement::KeepAlive,
				)?;
			} else {
				let _imbalance = T::MultiCurrency::withdraw(
					&origin,
					asset_id,
					amount,
					WithdrawReasons::TRANSFER,
					frame_support::traits::ExistenceRequirement::KeepAlive,
				)?;
			}

			let message = WithdrawMessage {
				token_address,
				amount: amount.into(),
				beneficiary,
			};

			let claim_delay: Option<(Balance, T::BlockNumber)> = Self::claim_delay(asset_id);
			if let Some((min_amount, delay)) = claim_delay {
				if min_amount <= amount {
					Self::delay_claim(delay, PendingClaim::Withdrawal(message));
					return Ok(());
				}
			};
			// process withdrawal immediately
			Self::process_withdrawal(message, asset_id);
		}

		#[weight = 1_000_000]
		#[transactional]
		/// Set the peg contract address on Ethereum (requires governance)
		pub fn set_contract_address(origin, eth_address: EthAddress) {
			ensure_root(origin)?;
			ContractAddress::put(eth_address);
			Self::deposit_event(<Event<T>>::SetContractAddress(eth_address));
		}

		#[weight = 1_000_000]
		#[transactional]
		/// Activate ERC20 CENNZ deposits (requires governance)
		pub fn activate_cennz_deposits(origin) {
			ensure_root(origin)?;
			CENNZDepositsActive::put(true);
			Self::deposit_event(<Event<T>>::CENNZDepositsActive);
		}

		#[weight = {
			1_000_000 * details.len() as u64
		}]
		/// Set the metadata details for a given ERC20 address (requires governance)
		/// details: `[(contract address, symbol, decimals)]`
		pub fn set_erc20_meta(origin, details: Vec<(EthAddress, Vec<u8>, u8)>) {
			ensure_root(origin)?;
			for (address, symbol, decimals) in details {
				Erc20Meta::insert(address, (symbol, decimals));
			}
		}

		#[weight = 1_000_000]
		/// Sets the claim delay for a given AssetId
		pub fn set_claim_delay(origin, asset_id: AssetId, min_balance: Balance, delay: T::BlockNumber) {
			ensure_root(origin)?;
			ClaimDelay::<T>::insert(asset_id, (min_balance, delay));
			Self::deposit_event(<Event<T>>::ClaimDelaySet(asset_id, min_balance, delay));
		}
	}
}

impl<T: Config> Module<T> {
	/// Process claims at a block after a delay
	fn process_claim(claim_id: ClaimId) {
		if let Some(pending_claim) = PendingClaims::take(claim_id) {
			match pending_claim {
				PendingClaim::Deposit((deposit_claim, tx_hash)) => {
					Self::process_deposit_claim(deposit_claim, tx_hash);
				}
				PendingClaim::Withdrawal(withdrawal_message) => {
					// At this stage it is assumed that a mapping between erc20 to asset id exists for this token
					let asset_id = Self::erc20_to_asset(withdrawal_message.token_address);
					if asset_id.is_some() {
						Self::process_withdrawal(withdrawal_message, asset_id.unwrap());
					} else {
						log::error!(
							"📌 ERC20 withdrawal claim failed unexpectedly: {:?}",
							withdrawal_message
						);
					}
				}
			}
		}
	}

	fn process_deposit_claim(claim: Erc20DepositEvent, tx_hash: H256) {
		let event_claim_id = T::EthBridge::submit_event_claim(
			&Self::contract_address().into(),
			&T::DepositEventSignature::get().into(),
			&tx_hash,
			&EthAbiCodec::encode(&claim),
		);
		let beneficiary: T::AccountId = T::AccountId::decode(&mut &claim.beneficiary.0[..]).unwrap();
		match event_claim_id {
			Ok(claim_id) => Self::deposit_event(<Event<T>>::Erc20Claim(claim_id, beneficiary)),
			Err(_) => {
				// There was an error submitting an event claim. Could be that the bridge is down
				// In this case, delay the deposit claim by 120 blocks
				Self::delay_claim(
					T::BlockNumber::from(T::FailedClaimDelay::get()),
					PendingClaim::Deposit((claim.clone(), tx_hash)),
				);
			}
		}
	}

	fn process_withdrawal(message: WithdrawMessage, asset_id: AssetId) {
		let amount: Balance = message.amount.as_u128();
		let event_proof_id = T::EthBridge::generate_event_proof(&message);

		match event_proof_id {
			Ok(proof_id) => {
				// Create a hash of withdrawAmount, tokenAddress, receiver, eventId
				let proof_id: EventId = proof_id;
				let withdrawal_hash: T::Hash = T::Hashing::hash(&mut (message.clone(), proof_id).encode());
				WithdrawalDigests::<T>::insert(proof_id, withdrawal_hash);
				Self::deposit_event(<Event<T>>::Erc20Withdraw(
					proof_id,
					asset_id,
					amount,
					message.beneficiary,
				));
			}
			Err(_) => {
				// There was an error generating an event proof. Could be that the bridge is down
				// In this case, delay the withdrawal by 120 blocks
				Self::delay_claim(
					T::BlockNumber::from(T::FailedClaimDelay::get()),
					PendingClaim::Withdrawal(message),
				);
			}
		}
	}

	/// Delay a withdrawal or deposit claim until a later block
	pub fn delay_claim(delay: T::BlockNumber, pending_claim: PendingClaim) {
		let claim_id = NextClaimId::get();
		if !claim_id.checked_add(One::one()).is_some() {
			Self::deposit_event(<Event<T>>::NoAvailableClaimIds);
			return;
		}
		let claim_block = <frame_system::Pallet<T>>::block_number().saturating_add(delay);
		PendingClaims::insert(claim_id, &pending_claim);
		// Modify ClaimSchedule with new claim_id
		ClaimSchedule::<T>::append(claim_block, claim_id);
		NextClaimId::put(claim_id + 1);

		// Throw event for delayed claim
		match pending_claim {
			PendingClaim::Withdrawal(withdrawal) => {
				Self::deposit_event(<Event<T>>::Erc20WithdrawalDelayed(
					claim_id,
					claim_block,
					withdrawal.amount.as_u128(),
					withdrawal.beneficiary,
				));
			}
			PendingClaim::Deposit(deposit) => {
				let beneficiary: T::AccountId = T::AccountId::decode(&mut &deposit.0.beneficiary.0[..]).unwrap();
				Self::deposit_event(<Event<T>>::Erc20DepositDelayed(
					claim_id,
					claim_block,
					deposit.0.amount.as_u128(),
					beneficiary,
				));
			}
		}
	}

	/// fulfil a deposit claim for the given event
	pub fn do_deposit(verified_event: Erc20DepositEvent) -> Result<(AssetId, Balance, T::AccountId), DispatchError> {
		let asset_id = match Self::erc20_to_asset(verified_event.token_address) {
			None => {
				// create asset with known values from `Erc20Meta`
				// asset will be created with `18` decimal places and "" for symbol if the asset is unknown
				// dapps can also use `AssetToERC20` to retrieve the appropriate decimal places from ethereum
				let (symbol, decimals) =
					Erc20Meta::get(verified_event.token_address).unwrap_or((Default::default(), 18));
				let asset_id = T::MultiCurrency::create(
					&T::PegPalletId::get().into_account(),
					Zero::zero(), // 0 initial supply
					decimals,
					1, // minimum balance
					symbol,
				)
				.map_err(|_| Error::<T>::CreateAssetFailed)?;
				Erc20ToAssetId::insert(verified_event.token_address, asset_id);
				AssetIdToErc20::insert(asset_id, verified_event.token_address);

				asset_id
			}
			Some(asset_id) => asset_id,
		};

		// checked at the time of initiating the verified_event that beneficiary value is valid and this op will not fail qed.
		let beneficiary: T::AccountId = T::AccountId::decode(&mut &verified_event.beneficiary.0[..]).unwrap();

		// (Governance): CENNZ is a special case since the supply is already 100% minted
		// it must be transferred from the unclaimed wallet
		let amount = verified_event.amount.as_u128();
		if asset_id == T::MultiCurrency::staking_currency() && Self::cennz_deposit_active() {
			let _result = T::MultiCurrency::transfer(
				// TODO: decide upon: Treasury / Sudo::key() / Bridge,
				&T::PegPalletId::get().into_account(),
				&beneficiary,
				asset_id,
				amount, // checked amount < u128 in `deposit_claim` qed.
				ExistenceRequirement::KeepAlive,
			);
		} else {
			// checked amount < u128 on `deposit_claim` qed.
			let _imbalance = T::MultiCurrency::deposit_creating(
				&beneficiary,
				asset_id,
				amount, // checked amount < u128 in `deposit_claim` qed.
			);
		}

		Ok((asset_id, amount, beneficiary))
	}
}

impl<T: Config> EventClaimSubscriber for Module<T> {
	fn on_success(event_claim_id: u64, contract_address: &EthAddress, event_type: &H256, event_data: &[u8]) {
		if *contract_address == EthAddress::from(Self::contract_address())
			&& *event_type == H256::from(T::DepositEventSignature::get())
		{
			if let Some(deposit_event) = EthAbiCodec::decode(event_data) {
				match Self::do_deposit(deposit_event) {
					Ok((asset_id, amount, beneficiary)) => {
						Self::deposit_event(<Event<T>>::Erc20Deposit(event_claim_id, asset_id, amount, beneficiary))
					}
					Err(_err) => Self::deposit_event(<Event<T>>::Erc20DepositFail(event_claim_id)),
				}
			} else {
				// input data should be valid, we do not expect to fail here
				log::error!("📌 ERC20 deposit claim failed unexpectedly: {:?}", event_data);
			}
		}
	}
	fn on_failure(event_claim_id: u64, contract_address: &H160, event_type: &H256, _event_data: &[u8]) {
		if *contract_address == EthAddress::from(Self::contract_address())
			&& *event_type == H256::from(T::DepositEventSignature::get())
		{
			Self::deposit_event(<Event<T>>::Erc20DepositFail(event_claim_id));
		}
	}
}
