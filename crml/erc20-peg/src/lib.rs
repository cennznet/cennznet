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

use cennznet_primitives::types::{AssetId, Balance};
use codec::Decode;
use crml_support::{EventClaimSubscriber, EventClaimVerifier, MultiCurrency};
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage, ensure, log,
	pallet_prelude::*,
	traits::{ExistenceRequirement, Get, WithdrawReasons},
	transactional, PalletId,
};
use frame_system::{ensure_root, ensure_signed};
use sp_runtime::{
	traits::{AccountIdConversion, Saturating, Zero},
	DispatchError,
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
	/// Submits event claims for Ethereum
	type EthBridge: EventClaimVerifier;
	/// Currency functions
	type MultiCurrency: MultiCurrency<AccountId = Self::AccountId, Balance = Balance, CurrencyId = AssetId>;
	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
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
		/// Block numbers where delayed withdrawal or deposit claims will be made
		ClaimSchedule get(fn claim_schedule): double_map hasher(twox_64_concat) T::BlockNumber, hasher(twox_64_concat) ClaimId => Option<PendingClaim>;
		/// The next available claim id for withdrawals and deposit claims
		NextClaimId get(fn next_claim_id): ClaimId;
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
		UnsupportedAsset
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {

		fn deposit_event() = default;

		// /// Check and process outstanding claims
		// fn on_initialize(now: T::BlockNumber) -> Weight {
		// 	let removed_count = Self::process_claims_at(now);
		// 	// TODO Better weight estimates
		// 	1_000_000 * removed_count as Weight
		// }

		fn on_idle(now: T::BlockNumber, remaining_weight: Weight) -> Weight {
			let weight_each: Weight = 2_000_000;
			let max_claims = (remaining_weight / weight_each).saturated_into::<u8>();
			let removed_count = Self::process_claims_at(now, max_claims);
			weight_each * removed_count as Weight
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
			let origin = ensure_signed(origin)?;
			ensure!(Self::deposits_active(), Error::<T>::DepositsPaused);
			// fail a claim early for an amount that is too large
			ensure!(claim.amount < U256::from(u128::max_value()), Error::<T>::InvalidAmount);
			// fail a claim if beneficiary is not a valid CENNZnet address
			ensure!(T::AccountId::decode(&mut &claim.beneficiary.0[..]).is_ok(), Error::<T>::InvalidAddress);

			let asset_id = Self::erc20_to_asset(claim.token_address);
			if asset_id.is_some() {
				let claim_delay: Option<(Balance, T::BlockNumber)> = Self::claim_delay(asset_id.unwrap());
				match claim_delay {
					Some((min_amount, delay)) => {
						if U256::from(min_amount) > claim.amount {
							Self::process_deposit_claim(claim, tx_hash);
						} else {
							// Store deposit to be claimed later
							let claim_id = NextClaimId::get();
							let claim_block = <frame_system::Module<T>>::block_number().saturating_add(delay);
							ClaimSchedule::<T>::insert(claim_block, claim_id, PendingClaim::Deposit((claim.clone(), tx_hash)));
							NextClaimId::mutate(|c| *c += 1);
							Self::deposit_event(<Event<T>>::Erc20DepositDelayed(claim_id, claim_block, claim.amount.as_u128(), origin));
						}
					},
					None => Self::process_deposit_claim(claim, tx_hash),
				}
			} else {
				// Claim amount is below threshold or delay doesn't exist
				Self::process_deposit_claim(claim, tx_hash);
			}
		}

		#[weight = 60_000_000]
		/// Withdraw generic assets from CENNZnet in exchange for ERC20s
		/// Tokens will be burnt and a proof generated to allow redemption of tokens on Ethereum
		#[transactional]
		pub fn withdraw(origin, asset_id: AssetId, amount: Balance, beneficiary: EthAddress) {
			let origin = ensure_signed(origin)?;
			ensure!(Self::withdrawals_active(), Error::<T>::WithdrawalsPaused);

			// there should be a known ERC20 address mapped for this asset
			// otherwise there may be no liquidity on the Ethereum side of the peg
			let token_address = Self::asset_to_erc20(asset_id);
			ensure!(token_address.is_some(), Error::<T>::UnsupportedAsset);

			let _imbalance = T::MultiCurrency::withdraw(
				&origin,
				asset_id,
				amount,
				WithdrawReasons::empty(),
				frame_support::traits::ExistenceRequirement::KeepAlive,
			)?;

			let message = WithdrawMessage {
				token_address: token_address.unwrap(),
				amount: amount.into(),
				beneficiary,
			};
			let claim_delay: Option<(Balance, T::BlockNumber)> = Self::claim_delay(asset_id);
			match claim_delay {
				Some((min_amount, delay)) => {
					if min_amount > amount {
						Self::process_withdrawal(message, asset_id);
					} else {
						// Store withdrawal to be claimed later
						let claim_id = NextClaimId::get();
						let claim_block = <frame_system::Module<T>>::block_number().saturating_add(delay);
						ClaimSchedule::<T>::insert(claim_block, claim_id, PendingClaim::Withdrawal(message));
						NextClaimId::mutate(|c| *c += 1);
						Self::deposit_event(<Event<T>>::Erc20WithdrawalDelayed(claim_id, claim_block, amount, beneficiary));
					}
				},
				None => Self::process_withdrawal(message, asset_id),
			}
		}

		#[weight = 1_000_000]
		#[transactional]
		pub fn uwu(origin, asset_id: AssetId, amount: Balance, beneficiary: EthAddress) {
			let _ = ensure_root(origin)?;

			// there should be a known ERC20 address mapped for this asset
			// otherwise there may be no liquidity on the Ethereum side of the peg
			let token_address = Self::asset_to_erc20(asset_id);
			ensure!(token_address.is_some(), Error::<T>::UnsupportedAsset);

			let message = WithdrawMessage {
				token_address: token_address.unwrap(),
				amount: amount.into(),
				beneficiary
			};
			let event_proof_id = T::EthBridge::generate_event_proof(&message)?;

			Self::deposit_event(<Event<T>>::Erc20Withdraw(event_proof_id, asset_id, amount, beneficiary));
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
	fn process_claims_at(now: T::BlockNumber, max_claims: u8) -> u32 {
		let mut removed = 0_u32;
		while removed < max_claims {
			let (_claim_id, pending_claim) = ClaimSchedule::<T>::take(now)
			removed += 1;
			match pending_claim {
				PendingClaim::Deposit((deposit_claim, tx_hash)) => {
					Self::process_deposit_claim(deposit_claim, tx_hash);
				}
				PendingClaim::Withdrawal(withdrawal_message) => {
					// At this stage it is assumed that a mapping between erc20 to asset id exists for this token
					let asset_id = Self::erc20_to_asset(withdrawal_message.token_address).unwrap_or_default();
					Self::process_withdrawal(withdrawal_message, asset_id);
				}
			}
		}

		// for (_claim_id, pending_claim) in ClaimSchedule::<T>::drain_prefix(now).into_iter() {
		// 	removed += 1;
		// 	match pending_claim {
		// 		PendingClaim::Deposit((deposit_claim, tx_hash)) => {
		// 			Self::process_deposit_claim(deposit_claim, tx_hash);
		// 		}
		// 		PendingClaim::Withdrawal(withdrawal_message) => {
		// 			// At this stage it is assumed that a mapping between erc20 to asset id exists for this token
		// 			let asset_id = Self::erc20_to_asset(withdrawal_message.token_address).unwrap_or_default();
		// 			Self::process_withdrawal(withdrawal_message, asset_id);
		// 		}
		// 	}
		// }
		removed
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
			_ => {
				// There was an error submitting an event claim. Could be that the bridge is down
				// In this case, delay the deposit claim by 120 blocks
				let claim_id = NextClaimId::get();
				let claim_block =
					<frame_system::Module<T>>::block_number().saturating_add(T::BlockNumber::from(FAILED_CLAIM_DELAY));
				ClaimSchedule::<T>::insert(claim_block, claim_id, PendingClaim::Deposit((claim.clone(), tx_hash)));
				NextClaimId::mutate(|c| *c += 1);
				Self::deposit_event(<Event<T>>::Erc20DepositDelayed(
					claim_id,
					claim_block,
					claim.amount.as_u128(),
					beneficiary,
				));
			}
		}
	}

	fn process_withdrawal(message: WithdrawMessage, asset_id: AssetId) {
		let amount: Balance = message.amount.as_u128();
		let event_proof_id = T::EthBridge::generate_event_proof(&message);
		match event_proof_id {
			Ok(proof_id) => {
				Self::deposit_event(<Event<T>>::Erc20Withdraw(
					proof_id,
					asset_id,
					amount,
					message.beneficiary,
				));
			}
			_ => {
				// There was an error generating an event proof. Could be that the bridge is down
				// In this case, delay the withdrawal by 120 blocks
				let claim_id = NextClaimId::get();
				let claim_block =
					<frame_system::Module<T>>::block_number().saturating_add(T::BlockNumber::from(FAILED_CLAIM_DELAY));
				ClaimSchedule::<T>::insert(claim_block, claim_id, PendingClaim::Withdrawal(message.clone()));
				NextClaimId::mutate(|c| *c += 1);
				Self::deposit_event(<Event<T>>::Erc20WithdrawalDelayed(
					claim_id,
					claim_block,
					amount,
					message.beneficiary,
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
