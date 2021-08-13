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
use frame_support::{decl_error, decl_event, decl_module, decl_storage, ensure, log, traits::Get, weights::Weight};
use frame_system::{ensure_root, ensure_signed};
use sp_runtime::{
	traits::{AccountIdConversion, Zero},
	DispatchError, ModuleId,
};
use sp_std::prelude::*;

mod types;
use types::*;

pub trait Config: frame_system::Config {
	/// An onchain address for this pallet
	type PegModuleId: Get<ModuleId>;
	/// The deposit contract address on Ethereum
	type DepositContractAddress: Get<[u8; 20]>;
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
		/// Wether deposit are active
		DepositsActive get(fn deposits_active): bool;
		/// Map ERC20 address to GA asset Id
		Erc20ToAssetId get(fn erc20_to_asset): map hasher(twox_64_concat) EthAddress => Option<AssetId>;
		/// Map GA asset Id to ERC20 address
		AssetIdToErc20 get(fn asset_to_erc20): map hasher(twox_64_concat) AssetId => Option<EthAddress>;
		/// Metadata for well-known erc20 tokens
		Erc20Meta get(fn erc20_meta): map hasher(twox_64_concat) EthAddress => Option<(Vec<u8>, u8)>;
	}
	add_extra_genesis {
		config(erc20s): Vec<(EthAddress, Vec<u8>, u8)>;
		build(|config: &GenesisConfig| {
			for (address, symbol, decimals) in config.erc20s.iter() {
				if symbol == b"CENNZ" {
					// hack: cpay asset Id is always CENNZ + 1
					Erc20ToAssetId::insert::<EthAddress, AssetId>(*address, T::MultiCurrency::fee_currency() - 1);
				}
				Erc20Meta::insert(address, (symbol, decimals));
			}
		});
	}
}

decl_event! {
	pub enum Event<T> where AccountId = <T as frame_system::Config>::AccountId {
		/// An erc20 deposit claim has started. (deposit Id, sender)
		Erc20Claim(u64, AccountId),
		/// A bridged erc20 deposit succeeded.(deposit Id, asset, amount, beneficiary)
		Erc20Deposit(u64, AssetId, Balance, AccountId),
		/// A bridged erc20 deposit failed.(deposit Id)
		Erc20DepositFail(u64),
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
		DepositsPaused
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {

		fn deposit_event() = default;

		/// Activate/deactivate deposits (root only)
		#[weight = 10_000_000]
		pub fn activate_deposits(origin, activate: bool) {
			ensure_root(origin)?;
			DepositsActive::put(activate);
		}

		#[weight = 50_000_000]
		/// Submit deposit claim for an ethereum tx hash
		/// The deposit details must be provided for cross-checking by notaries
		/// Any caller may initiate a claim while only the intended beneficiary will be paid.
		pub fn deposit_claim(origin, tx_hash: H256, claim: Erc20DepositEvent) {
			// Note: require caller to provide the `claim` so we don't need to handle the-
			// complexities of notaries reporting differing deposit events
			let origin = ensure_signed(origin)?;
			ensure!(Self::deposits_active(), Error::<T>::DepositsPaused);
			// fail a claim early for an amount that is too large
			ensure!(claim.amount < U256::from(u128::max_value()), Error::<T>::InvalidAmount);
			// fail a claim if beneficiary is not a valid CENNZnet address
			ensure!(T::AccountId::decode(&mut &claim.beneficiary.0[..]).is_ok(), Error::<T>::InvalidAddress);

			let event_claim_id = T::EthBridge::submit_event_claim(
					&T::DepositContractAddress::get().into(),
					&T::DepositEventSignature::get().into(),
					&tx_hash,
					&EthAbiCodec::encode(&claim),
			)?;

			Self::deposit_event(<Event<T>>::Erc20Claim(event_claim_id, origin));
		}

	}
}

impl<T: Config> Module<T> {
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
					&T::PegModuleId::get().into_account(),
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
		// CENNZ is a special case since the supply is already 100% minted
		// it must be transferred from the unclaimed wallet
		// if symbol == b"CENNZ" {
		// 	let _result = T::MultiCurrency::transfer(
		// 		// TODO: decide upon: Treasury / Sudo::key() / Bridge,
		// 		&T::PegModuleId::get().into_account(),
		// 		&beneficiary,
		// 		asset_id,
		// 		verified_event.amount.as_u128(), // checked amount < u128 in `deposit_claim` qed.
		// 		ExistenceRequirement::KeepAlive,
		// 	);
		// } else {

		// checked amount < u128 on `deposit_claim` qed.
		let amount = verified_event.amount.as_u128();
		let _imbalance = T::MultiCurrency::deposit_creating(
			&beneficiary,
			asset_id,
			amount, // checked amount < u128 in `deposit_claim` qed.
		);

		Ok((asset_id, amount, beneficiary))
	}
}

impl<T: Config> EventClaimSubscriber for Module<T> {
	fn on_success(event_claim_id: u64, contract_address: &EthAddress, event_type: &H256, event_data: &[u8]) {
		if *contract_address == EthAddress::from(T::DepositContractAddress::get())
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
}
