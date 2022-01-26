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

use frame_support::{decl_error, decl_event, decl_module, decl_storage, ensure, pallet_prelude::*};
use frame_system::pallet_prelude::*;
use sp_std::prelude::*;
use codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_runtime::DispatchResult;
use crml_support::{IsTokenOwner, MultiCurrency};
use cennznet_primitives::types::{AssetId, Balance, CollectionId, SeriesId, SerialNumber};

// Shows the approved account and amount for a generic asset
// #[derive(Decode, Encode, Debug, Clone, PartialEq, TypeInfo)]
// pub struct ERC20ApprovalInfo<T: Config> {
// 	// The account which has approval
// 	pub approved_account: T::AccountId,
// 	// The amount this account is approved for
// 	pub approved_amount: Balance,
// }

/// The module's configuration trait.
pub trait Config: frame_system::Config {
    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
    /// Handles a multi-currency fungible asset system
    type MultiCurrency: MultiCurrency<AccountId = Self::AccountId, CurrencyId = AssetId, Balance = Balance>;
    /// NFT ownership interface
    type IsTokenOwner: IsTokenOwner<AccountId = Self::AccountId>;
}

decl_event!(
	pub enum Event<T>
	where
		<T as frame_system::Config>::AccountId,
		CollectionId = CollectionId,
		SeriesId = SeriesId,
		SerialNumber = SerialNumber,
	{
		// Approval has been set (account_id, collection_id, series_id, serial_number)
		NFTApprovalSet(AccountId, CollectionId, SeriesId, SerialNumber),
		// Approval has been set for series (account_id, collection_id, series_id)
		NFTApprovalSetForAll(AccountId, CollectionId, SeriesId),
		// Approval has been set for generic asset (account_id, asset_id, amount)
		// GenericAssetApprovalSetForAll(AccountId, AssetId, Balance),
	}
);

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
		pub ERC721Approvals get(fn erc721_approvals): map hasher(twox_64_concat) (CollectionId, SeriesId, SerialNumber) => Option<T::AccountId>;
		// Account with transfer approval for an NFT series of another account
		pub ERC721ApprovalsForAll get(fn erc721_approvals_for_all): double_map hasher(twox_64_concat) T::AccountId, hasher(twox_64_concat) (CollectionId, SeriesId) => Option<Vec<T::AccountId>>;
		// Account with transfer approval for an amount of Generic Asset tokens of another account
		// pub ERC20Approvals get(fn ERC20_approvals): double_map hasher(twox_64_concat) T::AccountId, hasher(twox_64_concat) AssetId => ERC20ApprovalInfo<T>;
	}
}

// The module's dispatchable functions.
decl_module! {
	/// The module declaration.
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		// Initializing events
		// this is needed only if you are using events in your module
		fn deposit_event() = default;


	}
}

impl<T: Config> Module<T> {
    // Set approval for a single NFT
    pub fn erc721_approval(
        caller: T::AccountId,
        operator_account: T::AccountId,
        collection_id: CollectionId,
        series_id: SeriesId,
        serial_number: SerialNumber
    ) -> DispatchResult {
        // TODO: Focus on Approval
        // mapping(uint256 => address) private _tokenApprovals;

        ensure!(caller != operator_account, Error::<T>::CallerNotOperator);
        // Check that origin owns NFT TODO: Check what happens if token doesn't exist
        ensure!(T::IsTokenOwner::check_ownership(&caller, &collection_id, &series_id, &serial_number), Error::<T>::NotTokenOwner);
        ERC721Approvals::<T>::insert((collection_id, series_id, serial_number), operator_account.clone());

        // Something like OnNewAccount() from runtime/lib which checks when an NFT is transferred

        Self::deposit_event(RawEvent::NFTApprovalSet(operator_account, collection_id, series_id, serial_number));
        Ok(())
    }

    // Set approval for an NFT series
    pub fn erc721_approval_for_all(
        caller: T::AccountId,
        operator_account: T::AccountId,
        collection_id: CollectionId,
        series_id: SeriesId,
    ) -> DispatchResult {
        // TODO:
        // Mapping from owner to operator approvals
        // mapping(address => mapping(address => bool)) private _operatorApprovals;

        // Check that the series actually exists
        // Mapping from one address to multiple addresses with approval (Can have more than one)
        // Doesn't clear approvals on transfer, but based on account.
        //  This means that if you sell all NFTs in a collection, then buy another one, you don't need
        //  to set approval again

        // Self::deposit_event(RawEvent::NFTApprovalSetForAll(approved_account, collection_id, series_id));
        Ok(())
    }

    // Set approval for a single NFT
    pub fn erc20_approval(
        caller: T::AccountId,
        operator_account: T::AccountId,
        asset_id: AssetId,
        amount: Balance,
    ) -> DispatchResult {
        // TODO:
        // Check that the account has balance > amount
        // ensure operator_account != origin
        // Multiple accounts can be approved


        // Self::deposit_event(RawEvent::GenericAssetApprovalSet(approved_account, asset_id, amount));
        Ok(())
    }
}