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

//! CENNZnet token approvals
//!
//! Module for handling approvals on CENNZnet to allow for ERC-721 and ERC-20 crossover
//!
//! Ethereum standards allow for token transfers of accounts on behalf of the token owner
//! to allow for easier precompiling of ERC-721 and ERC-20 tokens, this module handles approvals on CENNZnet
//! for token transfers.

use cennznet_primitives::types::{AccountId, AssetId, Balance, CollectionId, SerialNumber, SeriesId, TokenId};
use crml_support::{IsTokenOwner, MultiCurrency, OnTransferSubscriber, PrefixedAddressMapping};
use frame_support::{decl_error, decl_module, decl_storage, ensure};
use frame_system::pallet_prelude::*;
use pallet_evm::AddressMapping;
use sp_core::H160;
use sp_runtime::DispatchResult;
use sp_std::prelude::*;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

/// The module's configuration trait.
pub trait Config: frame_system::Config {
	/// Handles a multi-currency fungible asset system
	type MultiCurrency: MultiCurrency<AccountId = AccountId, CurrencyId = AssetId, Balance = Balance>;
	/// NFT ownership interface
	type IsTokenOwner: IsTokenOwner<AccountId = AccountId>;
}

impl<T: Config> OnTransferSubscriber for Module<T> {
	/// Do anything that needs to be done after an NFT has been transferred
	fn on_nft_transfer(token_id: &TokenId) {
		// Set approval to none
		Self::remove_erc721_approval(token_id);
	}
}

decl_error! {
	/// Error for the token-approvals module.
	pub enum Error for Module<T: Config> {
		/// The account is not the owner of the token
		NotTokenOwner,
		/// The caller account can't be the same as the operator account
		CallerNotOperator,
	}
}

// This module's storage items.
decl_storage! {
	trait Store for Module<T: Config> as TokenApprovals {
		// Account with transfer approval for a single NFT
		pub ERC721Approvals get(fn erc721_approvals): map hasher(twox_64_concat) (CollectionId, SeriesId, SerialNumber) => H160;
		// Account with transfer approval for an NFT series of another account
		pub ERC721ApprovalsForAll get(fn erc721_approvals_for_all): double_map hasher(twox_64_concat) H160, hasher(twox_64_concat) (CollectionId, SeriesId) => Vec<H160>;
		// Mapping from account/ asset_id to an approved balance of another account
		pub ERC20Approvals get(fn erc20_approvals): double_map hasher(twox_64_concat) (H160, AssetId), hasher(twox_64_concat) H160 => Balance;
	}
}

// The module's dispatchable functions.
decl_module! {
	/// The module declaration.
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		type Error = Error<T>;
		/// Set approval for a single NFT
		/// Mapping from token_id to operator
		/// clears approval on transfer
		#[weight = 125_000_000]
		pub fn erc721_approval(
			origin,
			caller: H160,
			operator_account: H160,
			token_id: TokenId,
		) -> DispatchResult {
			// mapping(uint256 => address) private _tokenApprovals;

			let _ = ensure_none(origin)?;
			ensure!(caller != operator_account, Error::<T>::CallerNotOperator);
			// Check that origin owns NFT
			let owner = PrefixedAddressMapping::into_account_id(caller);
			ensure!(T::IsTokenOwner::check_ownership(&owner, &token_id), Error::<T>::NotTokenOwner);
			ERC721Approvals::insert(token_id, operator_account);
			Ok(())
		}

		/// Set approval for an account to transfer an amount of tokens on behalf of the caller
		/// Mapping from caller to spender and amount
		/// mapping(address => mapping(address => uint256)) private _allowances;
		#[weight = 100_000_000]
		pub fn erc20_approval(
			origin,
			caller: H160,
			spender: H160,
			asset_id: AssetId,
			amount: Balance,
		) -> DispatchResult {
			// mapping(address => mapping(address => uint256)) private _allowances;
			let _ = ensure_none(origin)?;
			ensure!(caller != spender, Error::<T>::CallerNotOperator);
			ERC20Approvals::insert((caller, asset_id), spender, amount);
			Ok(())
		}

		/// Removes an approval over an account and asset_id
		#[weight = 100_000_000]
		pub fn erc20_remove_approval(
			origin,
			caller: H160,
			spender: H160,
			asset_id: AssetId,
		) -> DispatchResult {
			// mapping(address => mapping(address => uint256)) private _allowances;
			let _ = ensure_none(origin)?;
			ERC20Approvals::remove((caller, asset_id), spender);
			Ok(())
		}

		// #[weight = 175_000_000]
		// pub fn erc721_approval_for_all(
		// 	origin,
		// 	caller: H160,
		// 	operator_account: H160,
		// 	collection_id: CollectionId,
		// 	series_id: SeriesId,
		// ) -> DispatchResult {
		// 	let _ = ensure_none(origin)?;
		// 	// mapping(address => mapping(address => bool)) private _operatorApprovals;
		// 	ensure!(caller != operator_account, Error::<T>::CallerNotOperator);
		// 	let approvals = Self::erc721_approvals_for_all(&caller, (collection_id, series_id));
		// 	ensure!(!approvals.contains(&operator_account), Error::<T>::AlreadyApproved);
		//
		// 	ERC721ApprovalsForAll::<T>::append(caller, (collection_id, series_id), operator_account.clone());
		//
		// 	Ok(())
		// }
	}
}

impl<T: Config> Module<T> {
	/// Removes the approval of a single NFT
	/// Triggered by transferring the token
	pub fn remove_erc721_approval(token_id: &TokenId) {
		// Check that origin owns NFT
		ERC721Approvals::remove(token_id);
	}
}
