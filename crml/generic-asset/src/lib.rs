// Copyright 2019-2021 by Centrality Investments Ltd.
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

//! # Generic Asset Module
//!
//! The Generic Asset module provides functionality for handling accounts and asset balances.
//!
//! ## Overview
//!
//! The Generic Asset module provides functions for:
//!
//! - Creating a new kind of asset.
//! - Setting permissions of an asset.
//! - Getting and setting free balances.
//! - Retrieving total, reserved and unreserved balances.
//! - Repatriating a reserved balance to a beneficiary account.
//! - Transferring a balance between accounts (when not reserved).
//! - Slashing an account balance.
//! - Managing total issuance.
//! - Setting and managing locks.
//!
//! ### Terminology
//!
//! - **Staking Asset:** The asset for staking, to participate as Validators in the network.
//! - **Spending Asset:** The asset for payment, such as paying transfer fees, gas fees, etc.
//! - **Permissions:** A set of rules for a kind of asset, defining the allowed operations to the asset, and which
//! accounts are allowed to possess it.
//! - **Total Issuance:** The total number of units in existence in a system.
//! - **Free Balance:** The portion of a balance that is not reserved. The free balance is the only balance that matters
//! for most operations. When this balance falls below the existential deposit, most functionality of the account is
//! removed. When both it and the reserved balance are deleted, then the account is said to be dead.
//! - **Reserved Balance:** Reserved balance still belongs to the account holder, but is suspended. Reserved balance
//! can still be slashed, but only after all the free balance has been slashed. If the reserved balance falls below the
//! existential deposit then it and any related functionality will be deleted. When both it and the free balance are
//! deleted, then the account is said to be dead.
//! - **Imbalance:** A condition when some assets were credited or debited without equal and opposite accounting
//! (i.e. a difference between total issuance and account balances). Functions that result in an imbalance will
//! return an object of the `Imbalance` trait that can be managed within your runtime logic. (If an imbalance is
//! simply dropped, it should automatically maintain any book-keeping such as total issuance.)
//! - **Lock:** A freeze on a specified amount of an account's free balance until a specified block number. Multiple
//! locks always operate over the same funds, so they "overlay" rather than "stack".
//!
//! ### Implementations
//!
//! The Generic Asset module provides `AssetCurrency`, which implements the following traits. If these traits provide
//! the functionality that you need, you can avoid coupling with the Generic Asset module.
//!
//! - `Currency`: Functions for dealing with a fungible assets system.
//! - `ReservableCurrency`: Functions for dealing with assets that can be reserved from an account.
//! - `LockableCurrency`: Functions for dealing with accounts that allow liquidity restrictions.
//! - `Imbalance`: Functions for handling imbalances between total issuance in the system and account balances.
//! Must be used when a function creates new assets (e.g. a reward) or destroys some assets (e.g. a system fee).
//!
//! The Generic Asset module provides two types of `AssetCurrency` as follows.
//!
//! - `StakingAssetCurrency`: Currency for staking.
//! - `SpendingAssetCurrency`: Currency for payments such as transfer fee, gas fee.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! - `create`: Create a new kind of asset and nominates the owner of this asset. The origin of this call must
//! be root.
//! - `transfer`: Transfer some liquid free balance to another account.
//! - `update_permission`: Updates permission for a given `asset_id` and an account. The origin of this call
//! must have update permissions.
//! - `mint`: Mint an asset, increases its total issuance. The origin of this call must have mint permissions.
//! - `burn`: Burn an asset, decreases its total issuance. The origin of this call must have burn permissions.
//! - `create_reserved`: Create a new kind of reserved asset. The origin of this call must be root.
//!
//! ### Public Functions
//!
//! - `total_balance`: Get an account's total balance of an asset kind.
//! - `free_balance`: Get an account's free balance of an asset kind.
//! - `reserved_balance`: Get an account's reserved balance of an asset kind.
//! - `create_asset`: Creates an asset.
//! - `make_transfer`: Transfer some liquid free balance from one account to another.
//! This will not emit the `Transferred` event.
//! - `make_transfer_with_event`: Transfer some liquid free balance from one account to another.
//! This will emit the `Transferred` event.
//! - `reserve`: Moves an amount from free balance to reserved balance.
//! - `unreserve`: Move up to an amount from reserved balance to free balance. This function cannot fail.
//! - `mint_free`: Mint to an account's free balance.
//! - `burn_free`: Burn an account's free balance.
//! - `slash`: Deduct up to an amount from the combined balance of `who`, preferring to deduct from the
//!	free balance. This function cannot fail.
//! - `slash_reserved`: Deduct up to an amount from reserved balance of an account. This function cannot fail.
//! - `repatriate_reserved`: Move up to an amount from reserved balance of an account to free balance of another
//! account.
//! - `check_permission`: Check permission to perform burn, mint or update.
//! - `ensure_can_withdraw`: Check if the account is able to make a withdrawal of the given amount
//!	for the given reason.
//!
//! ### Usage
//!
//! The following examples show how to use the Generic Asset Pallet in your custom pallet.
//!
//! ### Examples from the FRAME pallet
//!
//! The Fees Pallet uses the `Currency` trait to handle fee charge/refund, and its types inherit from `Currency`:
//!
//! ```
//! use frame_support::{
//! 	dispatch,
//! 	traits::{Currency, ExistenceRequirement, WithdrawReasons},
//! };
//! # pub trait Config: frame_system::Config {
//! # 	type Currency: Currency<Self::AccountId>;
//! # }
//! type AssetOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
//!
//! fn charge_fee<T: Config>(transactor: &T::AccountId, amount: AssetOf<T>) -> dispatch::DispatchResult {
//! 	// ...
//! 	T::Currency::withdraw(
//! 		transactor,
//! 		amount,
//! 		WithdrawReasons::TRANSACTION_PAYMENT,
//! 		ExistenceRequirement::KeepAlive,
//! 	)?;
//! 	// ...
//! 	Ok(())
//! }
//!
//! fn refund_fee<T: Config>(transactor: &T::AccountId, amount: AssetOf<T>) -> dispatch::DispatchResult {
//! 	// ...
//! 	T::Currency::deposit_into_existing(transactor, amount)?;
//! 	// ...
//! 	Ok(())
//! }
//!
//! # fn main() {}
//! ```
//!
//! ## Genesis config
//!
//! The Generic Asset Pallet depends on the [`GenesisConfig`](./struct.GenesisConfig.html).

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Codec, Decode, Encode, FullCodec};
use crml_support::AssetIdAuthority;
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage, ensure,
	traits::{
		BalanceStatus, Currency, ExistenceRequirement, Get, Imbalance, LockIdentifier, LockableCurrency, OnUnbalanced,
		ReservableCurrency, SignedImbalance, WithdrawReasons,
	},
	IterableStorageDoubleMap, IterableStorageMap, Parameter, StorageMap,
};
use frame_system::{ensure_root, ensure_signed};
use sp_runtime::{
	traits::{
		AtLeast32BitUnsigned, Bounded, CheckedAdd, CheckedMul, CheckedSub, MaybeSerializeDeserialize, Member, One,
		UniqueSaturatedInto, Zero,
	},
	DispatchError, DispatchResult, RuntimeDebug, SaturatedConversion,
};
use sp_std::{cmp, fmt::Debug, iter::Sum, prelude::*, result};

mod benchmarking;
mod imbalances;
pub mod impls;
mod mock;
mod tests;
mod types;
mod weights;

// Export GA types/traits
pub use self::imbalances::{CheckedImbalance, NegativeImbalance, OffsetResult, PositiveImbalance};
pub use types::*;
use weights::WeightInfo;

pub trait Config: frame_system::Config {
	/// The type for asset IDs
	type AssetId: Parameter + Member + AtLeast32BitUnsigned + Default + Copy + MaybeSerializeDeserialize + Codec;
	/// The type for asset amounts
	type Balance: Parameter
		+ Member
		+ AtLeast32BitUnsigned
		+ Default
		+ Copy
		+ MaybeSerializeDeserialize
		+ Debug
		+ FullCodec;
	/// The system event type
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;

	/// The type that handles the imbalance of dust cleaning.
	type OnDustImbalance: OnUnbalanced<NegativeImbalance<Self>>;

	/// Weight information for extrinsics in this module.
	type WeightInfo: WeightInfo;
}

decl_error! {
	/// Error for the generic-asset module.
	pub enum Error for Module<T: Config> {
		/// No new assets id available.
		AssetIdExhausted,
		/// Cannot transfer zero amount.
		ZeroAmount,
		/// The origin does not have enough permission to update permissions.
		NoUpdatePermission,
		/// The origin does not have permission to mint an asset.
		NoMintPermission,
		/// The origin does not have permission to burn an asset.
		NoBurnPermission,
		/// Total issuance got overflowed after minting.
		TotalMintingOverflow,
		/// Free balance got overflowed after minting.
		FreeMintingOverflow,
		/// Total issuance got underflowed after burning.
		TotalBurningUnderflow,
		/// Free balance got underflowed after burning.
		FreeBurningUnderflow,
		/// Asset id is already taken.
		AssetIdExists,
		/// Failure due to asset id not existing on chain
		AssetIdNotExist,
		/// The balance is too low to send amount.
		InsufficientBalance,
		/// The transfer will cause the account to overflow
		TransferOverflow,
		/// The account liquidity restrictions prevent withdrawal.
		LiquidityRestrictions,
		/// Existential deposit for assets should always be greater than zero.
		ZeroExistentialDeposit,
		/// There is no such account id in the storage.
		AccountIdNotExist,
		/// The integer for decimal places is too large for conversion into u128.
		DecimalTooLarge,
		/// The integer for initial issuance is too large for conversion into u128.
		InitialIssuanceTooLarge,
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		/// Create a new kind of asset and nominates the owner of this asset.
		/// The asset_id will be the next unoccupied asset_id
		/// Accounts who will have the permissions to mint/burn/change permission are passed in via 'options'
		/// origin of this call must be root.
		///
		/// Weights:
		/// O(1) Limited number of read and writes.
		/// Should not be called often.
		#[weight = T::WeightInfo::create()]
		fn create(
			origin,
			owner: T::AccountId,
			options: AssetOptions<T::Balance, T::AccountId>,
			info: AssetInfo,
		) -> DispatchResult {
			if let Err(_) = ensure_signed(origin.clone()) {
				ensure_root(origin)?;
			}
			Self::create_asset(None, Some(owner), options, info)
		}

		/// Transfer some liquid free balance to another account.
		///
		/// `transfer` will set the `FreeBalance` of the sender and receiver.
		/// It will decrease the total issuance of the system by the `TransferFee`.
		/// If the sender's account is below the existential deposit as a result
		/// of the transfer, the account will be reaped.
		///
		/// The dispatch origin for this call must be `Signed` by the transactor.
		///
		/// # <weight>
		/// - Dependent on arguments but not critical, given proper implementations for
		///   input config types. See related functions below.
		/// - It contains a limited number of reads and writes internally and no complex computation.
		///
		/// # </weight>
		#[weight = T::WeightInfo::transfer()]
		pub fn transfer(origin, #[compact] asset_id: T::AssetId, to: T::AccountId, #[compact] amount: T::Balance) {
			let origin = ensure_signed(origin)?;
			ensure!(!amount.is_zero(), Error::<T>::ZeroAmount);
			Self::make_transfer_with_event(asset_id, &origin, &to, amount, ExistenceRequirement::AllowDeath)?;
		}

		/// Transfer all of the free balance of `asset_id` to another account.
		#[weight = T::WeightInfo::transfer()]
		pub fn transfer_all(origin, #[compact] asset_id: T::AssetId, to: T::AccountId) {
			let origin = ensure_signed(origin)?;
			Self::make_transfer_with_event(asset_id, &origin, &to, Self::free_balance(asset_id, &origin), ExistenceRequirement::AllowDeath)?;
		}

		/// Updates permissions(mint/burn/change permission) for a given `asset_id` and an account.
		///
		/// The `origin` must have `update` permission.
		///
		/// weights:
		/// O(1) limited number of read and writes
		/// Expected to not be called frequently
		#[weight = T::WeightInfo::update_permission()]
		fn update_permission(
			origin,
			#[compact] asset_id: T::AssetId,
			new_permission: PermissionLatest<T::AccountId>
		) -> DispatchResult {
			let origin = ensure_signed(origin)?;

			let permissions: PermissionVersions<T::AccountId> = new_permission.into();

			if Self::check_permission(asset_id, &origin, &PermissionType::Update) {
				<Permissions<T>>::insert(asset_id, &permissions);

				Self::deposit_event(Event::<T>::PermissionUpdated(asset_id, permissions.into()));

				Ok(())
			} else {
				Err(Error::<T>::NoUpdatePermission)?
			}
		}

		/// Updates asset info for a given `asset_id`.
		///
		/// The `origin` must have `update` permission.
		///
		/// weights:
		/// O(1) limited number of read and writes
		/// Expected to not be called frequently
		#[weight = T::WeightInfo::update_asset_info()]
		fn update_asset_info(origin, #[compact] asset_id: T::AssetId, info: AssetInfo) -> DispatchResult {
			let origin = ensure_signed(origin)?;

			if !<TotalIssuance<T>>::contains_key(asset_id) {
				Err(Error::<T>::AssetIdNotExist)?
			}

			if !Self::check_permission(asset_id, &origin, &PermissionType::Update) {
				Err(Error::<T>::NoUpdatePermission)?
			}

			<AssetMeta<T>>::insert(asset_id, info.clone());

			Self::deposit_event(Event::<T>::AssetInfoUpdated(asset_id, info));

			Ok(())
		}

		/// Mints an asset, increases its total issuance. Deposits the newly minted currency into target account
		/// The origin must have `mint` permissions.
		///
		/// Weights:
		/// O(1) limited number of read/writes
		#[weight = T::WeightInfo::mint()]
		fn mint(origin, #[compact] asset_id: T::AssetId, to: T::AccountId, amount: T::Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::mint_free(asset_id, &who, &to, &amount)?;
			Self::deposit_event(Event::<T>::Minted(asset_id, to, amount));
			Ok(())
		}

		/// Burns an asset, decreases its total issuance. Deduct the money from target account
		/// The `origin` must have `burn` permissions.
		///
		/// Weights:
		/// O(1) Limited number of reads/writes.
		#[weight = T::WeightInfo::burn()]
		fn burn(origin, #[compact] asset_id: T::AssetId, target: T::AccountId, amount: T::Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::burn_free(asset_id, &who, &target, &amount)?;
			Self::deposit_event(Event::<T>::Burned(asset_id, target, amount));
			Ok(())
		}

		/// Create a new asset with reserved asset_id.
		/// Internally calls create_asset with an asset_id
		/// Requires Root call.
		///
		/// Weights:
		/// O(1) Limited read/writes
		#[weight = T::WeightInfo::create_reserved()]
		fn create_reserved(
			origin,
			asset_id: T::AssetId,
			options: AssetOptions<T::Balance, T::AccountId>,
			info: AssetInfo,
		) -> DispatchResult {
			ensure_root(origin)?;
			Self::create_asset(Some(asset_id), None, options, info)
		}

		/// On runtime upgrade, update account data for existing accounts and remove dust balances
		fn on_runtime_upgrade() -> frame_support::weights::Weight {
			if StorageVersion::get() == Releases::V0 as u32 {
				StorageVersion::put(Releases::V1 as u32);

				migrate_locks::<T>();

				// For each (asset, account)
				// 1) release reserved balance storage if 0
				// 2) release free balance storage if < ED
				AssetMeta::<T>::iter().for_each(|(asset_id, asset_meta)| {
					let mut total_dust_imbalance = NegativeImbalance::new(Zero::zero(), asset_id);
					<FreeBalance<T>>::iter_prefix(asset_id)
						.filter_map(|(account_id, balance)| {
							if balance < asset_meta.existential_deposit().saturated_into() {
								Some(account_id)
							} else {
								// set provider = 1
								if <frame_system::Module<T>>::providers(&account_id).is_zero() {
									<frame_system::Module<T>>::inc_providers(&account_id);
								}
								None
							}
						})
						.for_each(|account_id| {
							let amount = <FreeBalance<T>>::take(asset_id, account_id);
							if amount > Zero::zero() {
								total_dust_imbalance.subsume(NegativeImbalance::new(amount, asset_id));
							}
						});

					if total_dust_imbalance.peek() > Zero::zero() {
						T::OnDustImbalance::on_nonzero_unbalanced(total_dust_imbalance);
					}
				});

				T::BlockWeights::get().max_block
			} else {
				Zero::zero()
			}
		}
	}
}

// A value placed in storage that represents the current version of the storage. This value is used
// by the `on_runtime_upgrade` logic to determine whether we run storage migration logic.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug)]
enum Releases {
	/// Storage version runtime version <= 40
	V0 = 0,
	/// Storage version as of runtime version 41
	V1 = 1,
}

impl Default for Releases {
	fn default() -> Self {
		Releases::V1
	}
}

decl_storage! {
	trait Store for Module<T: Config> as GenericAsset {
		/// Total issuance of a given asset.
		///
		/// TWOX-NOTE: `AssetId` is trusted.
		pub TotalIssuance get(fn total_issuance) build(|config: &GenesisConfig<T>| {
			let issuance = config.initial_balance * (config.endowed_accounts.len() as u32).into();
			config.assets.iter().map(|id| (id.clone(), issuance)).collect::<Vec<_>>()
		}): map hasher(twox_64_concat) T::AssetId => T::Balance;

		/// The free balance of a given asset under an account.
		///
		/// TWOX-NOTE: `AssetId` is trusted.
		pub FreeBalance:
			double_map hasher(twox_64_concat) T::AssetId, hasher(blake2_128_concat) T::AccountId => T::Balance;

		/// The reserved balance of a given asset under an account.
		///
		/// TWOX-NOTE: `AssetId` is trusted.
		pub ReservedBalance:
			double_map hasher(twox_64_concat) T::AssetId, hasher(blake2_128_concat) T::AccountId => T::Balance;

		/// Next available ID for user-created asset.
		pub NextAssetId get(fn next_asset_id) config(): T::AssetId;

		/// Permission options for a given asset.
		///
		/// TWOX-NOTE: `AssetId` is trusted.
		pub Permissions get(fn get_permission) build(|config: &GenesisConfig<T>| {
			config.permissions
				.iter()
				.map(|(asset, owner)| (*asset, PermissionsV1::new(owner.clone()).into())).collect::<Vec<_>>()
		}): map hasher(twox_64_concat) T::AssetId => PermissionVersions<T::AccountId>;

		/// Any liquidity locks on some account balances.
		pub Locks get(fn locks):
			double_map hasher(twox_64_concat) T::AssetId, hasher(blake2_128_concat) T::AccountId => Vec<BalanceLock<T::Balance>>;

		/// The identity of the asset which is the one that is designated for the chain's staking system.
		pub StakingAssetId get(fn staking_asset_id) config(): T::AssetId;

		/// The identity of the asset which is the one that is designated for paying the chain's transaction fee.
		pub SpendingAssetId get(fn spending_asset_id) config(): T::AssetId;

		/// The info for assets
		pub AssetMeta get(fn asset_meta) config(): map hasher(twox_64_concat) T::AssetId => AssetInfo;

		/// Storage version of the pallet.
		///
		/// This is set to v1 for new networks.
		StorageVersion build(|_: &GenesisConfig<T>| Releases::V0 as u32): u32;
	}
	add_extra_genesis {
		config(assets): Vec<T::AssetId>;
		config(initial_balance): T::Balance;
		config(endowed_accounts): Vec<T::AccountId>;
		config(permissions): Vec<(T::AssetId, T::AccountId)>;

		build(|config: &GenesisConfig<T>| {
			config.assets.iter().for_each(|asset_id| {
				config.endowed_accounts.iter().for_each(|account_id| {
					Module::<T>::set_free_balance(*asset_id, account_id, config.initial_balance);
				});
			});
			config.asset_meta.iter().for_each(|(id, info)| {
				<AssetMeta<T>>::insert(id, info);
			});
		});
	}
}

fn migrate_locks<T: Config>() {
	#[allow(dead_code)]
	mod old_storage {
		use super::Config;
		use crate::types::BalanceLock;
		use sp_std::vec::Vec;

		pub struct Module<T>(sp_std::marker::PhantomData<T>);
		frame_support::decl_storage! {
			trait Store for Module<T: Config> as GenericAsset {
				pub Locks get(fn locks):
					map hasher(blake2_128_concat) T::AccountId => Vec<BalanceLock<T::Balance>>;
			}
		}
	}

	let staking_asset_id = <Module<T>>::staking_asset_id();
	let all_locks = <old_storage::Locks<T>>::drain().collect::<Vec<(T::AccountId, Vec<BalanceLock<T::Balance>>)>>();
	all_locks.iter().for_each(|(account_id, locks)| {
		if !locks.is_empty() {
			<Locks<T>>::insert(staking_asset_id, &account_id, locks);
		}
	});
}

decl_event! {
	pub enum Event<T> where
		<T as frame_system::Config>::AccountId,
		<T as Config>::AssetId,
		<T as Config>::Balance,
		AssetOptions = AssetOptions<<T as Config>::Balance, <T as frame_system::Config>::AccountId>
	{
		/// Asset created (asset_id, creator, asset_options).
		Created(AssetId, AccountId, AssetOptions),
		/// Asset transfer succeeded (asset_id, from, to, amount).
		Transferred(AssetId, AccountId, AccountId, Balance),
		/// Asset permission updated (asset_id, new_permissions).
		PermissionUpdated(AssetId, PermissionLatest<AccountId>),
		/// Asset info updated (asset_id, asset_info).
		AssetInfoUpdated(AssetId, AssetInfo),
		/// New asset minted (asset_id, account, amount).
		Minted(AssetId, AccountId, Balance),
		/// Asset burned (asset_id, account, amount).
		Burned(AssetId, AccountId, Balance),
		/// Asset balance storage has been reclaimed due to falling below the existential deposit
		DustReclaimed(AssetId, AccountId, Balance),
	}
}

impl<T: Config> Module<T> {
	/// Get an account's total balance of an asset kind.
	pub fn total_balance(asset_id: T::AssetId, who: &T::AccountId) -> T::Balance {
		Self::free_balance(asset_id, who) + Self::reserved_balance(asset_id, who)
	}

	/// Get an account's free balance of an asset kind.
	pub fn free_balance(asset_id: T::AssetId, who: &T::AccountId) -> T::Balance {
		<FreeBalance<T>>::get(asset_id, who)
	}

	/// Get an account's reserved balance of an asset kind.
	pub fn reserved_balance(asset_id: T::AssetId, who: &T::AccountId) -> T::Balance {
		<ReservedBalance<T>>::get(asset_id, who)
	}

	/// Mint to an account's free balance, without event
	pub fn mint_free(
		asset_id: T::AssetId,
		who: &T::AccountId,
		to: &T::AccountId,
		amount: &T::Balance,
	) -> DispatchResult {
		if Self::check_permission(asset_id, who, &PermissionType::Mint) {
			let original_free_balance = Self::free_balance(asset_id, &to);
			let current_total_issuance = <TotalIssuance<T>>::get(asset_id);
			let new_total_issuance = current_total_issuance
				.checked_add(&amount)
				.ok_or(Error::<T>::TotalMintingOverflow)?;
			let value = original_free_balance
				.checked_add(&amount)
				.ok_or(Error::<T>::FreeMintingOverflow)?;

			<TotalIssuance<T>>::insert(asset_id, new_total_issuance);
			Self::set_free_balance(asset_id, &to, value);
			Ok(())
		} else {
			Err(Error::<T>::NoMintPermission)?
		}
	}

	/// Burn an account's free balance, without event
	pub fn burn_free(
		asset_id: T::AssetId,
		who: &T::AccountId,
		to: &T::AccountId,
		amount: &T::Balance,
	) -> DispatchResult {
		if Self::check_permission(asset_id, who, &PermissionType::Burn) {
			let original_free_balance = Self::free_balance(asset_id, to);

			let current_total_issuance = <TotalIssuance<T>>::get(asset_id);
			let new_total_issuance = current_total_issuance
				.checked_sub(amount)
				.ok_or(Error::<T>::TotalBurningUnderflow)?;
			let value = original_free_balance
				.checked_sub(amount)
				.ok_or(Error::<T>::FreeBurningUnderflow)?;

			<TotalIssuance<T>>::insert(asset_id, new_total_issuance);
			Self::set_free_balance(asset_id, &to, value);
			Ok(())
		} else {
			Err(Error::<T>::NoBurnPermission)?
		}
	}

	/// Creates an asset.
	///
	/// # Arguments
	/// * `asset_id`: An ID of a reserved asset.
	/// If not provided, a user-generated asset will be created with the next available ID.
	/// * `from_account`: The initiator account of this call
	/// * `asset_options`: Asset creation options.
	///
	pub fn create_asset(
		asset_id: Option<T::AssetId>,
		from_account: Option<T::AccountId>,
		options: AssetOptions<T::Balance, T::AccountId>,
		info: AssetInfo,
	) -> DispatchResult {
		ensure!(
			!info.existential_deposit().is_zero(),
			Error::<T>::ZeroExistentialDeposit
		);

		let decimal_factor: T::Balance = 10u128
			.checked_pow(info.decimal_places().into())
			.ok_or(Error::<T>::DecimalTooLarge)?
			.unique_saturated_into();
		// Assuming that Balance is u128 or less. Implemented in this way for practicality
		let total_issuance: T::Balance = decimal_factor
			.checked_mul(&options.initial_issuance)
			.ok_or(Error::<T>::InitialIssuanceTooLarge)?;

		let asset_id = if let Some(asset_id) = asset_id {
			ensure!(!asset_id.is_zero(), Error::<T>::AssetIdExists);
			ensure!(!<TotalIssuance<T>>::contains_key(asset_id), Error::<T>::AssetIdExists);
			ensure!(asset_id < Self::next_asset_id(), Error::<T>::AssetIdExists);
			asset_id
		} else {
			let asset_id = Self::next_asset_id();
			let next_id = asset_id.checked_add(&One::one()).ok_or(Error::<T>::AssetIdExhausted)?;
			<NextAssetId<T>>::put(next_id);
			asset_id
		};

		let account_id = from_account.unwrap_or_default();
		let permissions: PermissionVersions<T::AccountId> = options.permissions.clone().into();

		<TotalIssuance<T>>::insert(asset_id, &total_issuance);
		Self::set_free_balance(asset_id, &account_id, total_issuance);
		<Permissions<T>>::insert(asset_id, permissions);
		<AssetMeta<T>>::insert(asset_id, info);

		Self::deposit_event(Event::<T>::Created(asset_id, account_id, options));

		Ok(())
	}

	/// Transfer some liquid free balance from one account to another.
	/// This will not emit the `Transferred` event.
	pub fn make_transfer(
		asset_id: T::AssetId,
		from: &T::AccountId,
		to: &T::AccountId,
		amount: T::Balance,
		_req: ExistenceRequirement,
	) -> DispatchResult {
		let new_from_balance = Self::free_balance(asset_id, from)
			.checked_sub(&amount)
			.ok_or(Error::<T>::InsufficientBalance)?;
		let new_to_balance = Self::free_balance(asset_id, to)
			.checked_add(&amount)
			.ok_or(Error::<T>::TransferOverflow)?;

		Self::ensure_can_withdraw(asset_id, from, amount, WithdrawReasons::TRANSFER, new_from_balance)?;

		if from == to {
			return Ok(());
		}

		Self::set_free_balance(asset_id, to, new_to_balance);
		Self::set_free_balance(asset_id, from, new_from_balance);

		let existential_deposit = Self::asset_meta(asset_id).existential_deposit();
		if new_from_balance < existential_deposit.saturated_into() {
			Self::reclaim_free_balance(asset_id, from);
		}

		Ok(())
	}

	/// Transfer some liquid free balance from one account to another.
	/// This will emit the `Transferred` event.
	pub fn make_transfer_with_event(
		asset_id: T::AssetId,
		from: &T::AccountId,
		to: &T::AccountId,
		amount: T::Balance,
		req: ExistenceRequirement,
	) -> DispatchResult {
		Self::make_transfer(asset_id, from, to, amount, req)?;

		if from != to {
			Self::deposit_event(Event::<T>::Transferred(asset_id, from.clone(), to.clone(), amount));
		}

		Ok(())
	}

	/// Move `amount` from free balance to reserved balance.
	///
	/// If the free balance is lower than `amount`, then no funds will be moved and an `Err` will
	/// be returned. This is different behavior than `unreserve`.
	pub fn reserve(asset_id: T::AssetId, who: &T::AccountId, amount: T::Balance) -> DispatchResult {
		// Do we need to consider that this is an atomic transaction?
		let original_reserve_balance = Self::reserved_balance(asset_id, who);
		let original_free_balance = Self::free_balance(asset_id, who);
		if original_free_balance < amount {
			Err(Error::<T>::InsufficientBalance)?
		}

		let new_reserve_balance = original_reserve_balance + amount;
		let new_free_balance = original_free_balance - amount;

		// `free` balance should be freed if set to a dust amount
		let existential_deposit = Self::asset_meta(asset_id).existential_deposit();
		if new_free_balance < existential_deposit.saturated_into() {
			Self::reclaim_free_balance(asset_id, who);
		} else {
			<FreeBalance<T>>::insert(asset_id, who, &new_free_balance);
		}

		<ReservedBalance<T>>::insert(asset_id, who, &new_reserve_balance);

		Ok(())
	}

	/// Moves up to `amount` from reserved balance to free balance. This function cannot fail.
	///
	/// As many assets up to `amount` will be moved as possible. If the reserve balance of `who`
	/// is less than `amount`, then the remaining amount will be returned.
	/// NOTE: This is different behavior than `reserve`.
	pub fn unreserve(asset_id: T::AssetId, who: &T::AccountId, amount: T::Balance) -> T::Balance {
		let b = Self::reserved_balance(asset_id, who);
		let actual = cmp::min(b, amount);
		let original_free_balance = Self::free_balance(asset_id, who);
		let new_free_balance = original_free_balance + actual;
		let new_reserved_balance = b - actual;

		Self::set_reserved_balance(asset_id, who, new_reserved_balance);
		<FreeBalance<T>>::insert(asset_id, &who, new_free_balance);

		amount - actual
	}

	/// Deduct up to `amount` from the combined balance of `who`, preferring to deduct from the
	/// free balance. This function cannot fail.
	///
	/// As much funds up to `amount` will be deducted as possible. If this is less than `amount`
	/// then `Some(remaining)` will be returned. Full completion is given by `None`.
	/// NOTE: LOW-LEVEL: This will not attempt to maintain total issuance. It is expected that
	/// the caller will do this.
	pub fn slash(asset_id: T::AssetId, who: &T::AccountId, amount: T::Balance) -> Option<T::Balance> {
		let free_balance = Self::free_balance(asset_id, who);
		let free_slash = cmp::min(free_balance, amount);
		let new_free_balance = free_balance - free_slash;
		Self::set_free_balance(asset_id, who, new_free_balance);
		if free_slash < amount {
			Self::slash_reserved(asset_id, who, amount - free_slash)
		} else {
			None
		}
	}

	/// Deducts up to `amount` from reserved balance of `who`. This function cannot fail.
	///
	/// As much funds up to `amount` will be deducted as possible. If the reserve balance of `who`
	/// is less than `amount`, then a non-zero second item will be returned.
	/// NOTE: LOW-LEVEL: This will not attempt to maintain total issuance. It is expected that
	/// the caller will do this.
	pub fn slash_reserved(asset_id: T::AssetId, who: &T::AccountId, amount: T::Balance) -> Option<T::Balance> {
		let original_reserve_balance = Self::reserved_balance(asset_id, who);
		let slash = cmp::min(original_reserve_balance, amount);
		let new_reserve_balance = original_reserve_balance - slash;
		Self::set_reserved_balance(asset_id, who, new_reserve_balance);
		if amount == slash {
			None
		} else {
			Some(amount - slash)
		}
	}

	/// Move up to `amount` from the reserved balance of one account into the free balance of another.
	/// The entire reserve balance will be transferred if it is less than `amount`.
	pub fn repatriate_reserved(
		asset_id: T::AssetId,
		payee: &T::AccountId,
		beneficiary: &T::AccountId,
		amount: T::Balance,
	) -> Result<T::Balance, DispatchError> {
		if amount.is_zero() {
			return Ok(Zero::zero());
		}
		let payee_reserve_balance = Self::reserved_balance(asset_id, payee);
		let repatriated_amount = sp_std::cmp::min(payee_reserve_balance, amount);

		let beneficiary_free_balance = Self::free_balance(asset_id, beneficiary);
		let new_beneficiary_free_balance = beneficiary_free_balance + repatriated_amount;
		let new_payee_reserve_balance = payee_reserve_balance - repatriated_amount;

		// Intentionally allowing `beneficiary` to receive dust amounts
		// `repatriate_reserved` is an internal function likely called by protocol operations
		// this will allow an account to accumulate without being reaped too early
		<FreeBalance<T>>::insert(asset_id, &beneficiary, &new_beneficiary_free_balance);
		Self::set_reserved_balance(asset_id, payee, new_payee_reserve_balance);

		Ok(amount - repatriated_amount)
	}

	/// Check permission to perform burn, mint or update.
	///
	/// # Arguments
	/// * `asset_id`:  A `T::AssetId` type that contains the `asset_id`, which has the permission embedded.
	/// * `who`: A `T::AccountId` type that contains the `account_id` for which to check permissions.
	/// * `what`: The permission to check.
	///
	pub fn check_permission(asset_id: T::AssetId, who: &T::AccountId, what: &PermissionType) -> bool {
		let permission_versions: PermissionVersions<T::AccountId> = Self::get_permission(asset_id);
		let permission = permission_versions.into();

		match (what, permission) {
			(
				PermissionType::Burn,
				PermissionLatest {
					burn: Owner::Address(account),
					..
				},
			) => account == *who,
			(
				PermissionType::Mint,
				PermissionLatest {
					mint: Owner::Address(account),
					..
				},
			) => account == *who,
			(
				PermissionType::Update,
				PermissionLatest {
					update: Owner::Address(account),
					..
				},
			) => account == *who,
			_ => false,
		}
	}

	/// Return `Ok` if the account is able to make a withdrawal of the given amount
	/// for the given reason.
	///
	/// `Err(...)` with the reason why not otherwise.
	pub fn ensure_can_withdraw(
		asset_id: T::AssetId,
		who: &T::AccountId,
		amount: T::Balance,
		reasons: WithdrawReasons,
		new_balance: T::Balance,
	) -> DispatchResult {
		if amount.is_zero() {
			return Ok(());
		}
		let locks = Self::locks(asset_id, who);
		if locks.is_empty() {
			return Ok(());
		}
		if locks
			.into_iter()
			.all(|l| new_balance >= l.amount || !l.reasons.intersects(reasons))
		{
			Ok(())
		} else {
			Err(Error::<T>::LiquidityRestrictions)?
		}
	}

	/// Return registered asset metadata
	pub fn registered_assets() -> Vec<(T::AssetId, AssetInfo)> {
		AssetMeta::<T>::iter().collect()
	}

	/// Return all types of balance stored under an account
	/// Include reserved, locked and free balance
	pub fn get_all_balances(account_id: &T::AccountId, asset_id: T::AssetId) -> AllBalances<T::Balance>
	where
		T::Balance: Sum,
	{
		let reserved = <ReservedBalance<T>>::get(&asset_id, &account_id);
		let locks = Self::locks(&asset_id, &account_id);
		let staked: T::Balance = locks.iter().map(|l| l.amount).sum();
		let available = <FreeBalance<T>>::get(&asset_id, &account_id) - staked;

		AllBalances {
			reserved,
			staked,
			available,
		}
	}

	/// Reclaim asset storage items for an account
	/// Any dust imbalance from free balance is passed to a dust imbalance handler.
	fn reclaim_free_balance(asset_id: T::AssetId, who: &T::AccountId) {
		let amount = <FreeBalance<T>>::take(asset_id, who);
		if amount > Zero::zero() {
			T::OnDustImbalance::on_nonzero_unbalanced(NegativeImbalance::new(amount, asset_id));
		}

		Self::deposit_event(Event::<T>::DustReclaimed(asset_id, who.clone(), amount));
	}

	/// NOTE: LOW-LEVEL: This will not attempt to maintain total issuance. It is expected that
	/// the caller will do this.
	fn set_reserved_balance(asset_id: T::AssetId, who: &T::AccountId, reserved: T::Balance) {
		if reserved.is_zero() {
			// `who` account's reserve balance storage should be freed
			<ReservedBalance<T>>::remove(asset_id, who);
		} else {
			<ReservedBalance<T>>::insert(asset_id, who, &reserved);
		}
	}

	/// NOTE: LOW-LEVEL: This will not attempt to maintain total issuance. It is expected that
	/// the caller will do this.
	fn set_free_balance(asset_id: T::AssetId, who: &T::AccountId, free: T::Balance) {
		<FreeBalance<T>>::mutate(asset_id, who, |balance| {
			// Tell the system module we are "providing" the account
			// This is only done so that FRAME pallets from substrate think
			// this accounts "exists"
			if <frame_system::Module<T>>::providers(&who).is_zero() {
				<frame_system::Module<T>>::inc_providers(&who);
			}
			*balance = free
		});
	}

	fn set_lock(
		id: LockIdentifier,
		asset_id: T::AssetId,
		who: &T::AccountId,
		amount: T::Balance,
		reasons: WithdrawReasons,
	) {
		let mut new_lock = Some(BalanceLock { id, amount, reasons });
		let mut locks = <Module<T>>::locks(asset_id, who)
			.into_iter()
			.filter_map(|l| if l.id == id { new_lock.take() } else { Some(l) })
			.collect::<Vec<_>>();
		if let Some(lock) = new_lock {
			locks.push(lock)
		}
		<Locks<T>>::insert(asset_id, who, locks);
	}

	fn extend_lock(
		id: LockIdentifier,
		asset_id: T::AssetId,
		who: &T::AccountId,
		amount: T::Balance,
		reasons: WithdrawReasons,
	) {
		let mut new_lock = Some(BalanceLock { id, amount, reasons });
		let mut locks = <Module<T>>::locks(asset_id, who)
			.into_iter()
			.filter_map(|l| {
				if l.id == id {
					new_lock.take().map(|nl| BalanceLock {
						id: l.id,
						amount: l.amount.max(nl.amount),
						reasons: l.reasons | nl.reasons,
					})
				} else {
					Some(l)
				}
			})
			.collect::<Vec<_>>();
		if let Some(lock) = new_lock {
			locks.push(lock)
		}
		<Locks<T>>::insert(asset_id, who, locks);
	}

	fn remove_lock(id: LockIdentifier, asset_id: T::AssetId, who: &T::AccountId) {
		let mut locks = <Module<T>>::locks(asset_id, who);
		locks.retain(|l| l.id != id);
		if locks.is_empty() {
			<Locks<T>>::remove(asset_id, who);
		} else {
			<Locks<T>>::insert(asset_id, who, locks);
		}
	}
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct AssetCurrency<T, U>(sp_std::marker::PhantomData<T>, sp_std::marker::PhantomData<U>);

impl<T, U> Currency<T::AccountId> for AssetCurrency<T, U>
where
	T: Config,
	U: AssetIdAuthority<AssetId = T::AssetId>,
{
	type Balance = T::Balance;
	type PositiveImbalance = PositiveImbalance<T>;
	type NegativeImbalance = NegativeImbalance<T>;

	fn total_balance(who: &T::AccountId) -> Self::Balance {
		Self::free_balance(&who) + Self::reserved_balance(&who)
	}

	fn free_balance(who: &T::AccountId) -> Self::Balance {
		<Module<T>>::free_balance(U::asset_id(), &who)
	}

	/// Returns the total staking asset issuance
	fn total_issuance() -> Self::Balance {
		<Module<T>>::total_issuance(U::asset_id())
	}

	fn minimum_balance() -> Self::Balance {
		AssetMeta::<T>::get(U::asset_id())
			.existential_deposit()
			.unique_saturated_into()
	}

	fn transfer(
		transactor: &T::AccountId,
		dest: &T::AccountId,
		value: Self::Balance,
		req: ExistenceRequirement,
	) -> DispatchResult {
		<Module<T>>::make_transfer(U::asset_id(), transactor, dest, value, req)
	}

	fn ensure_can_withdraw(
		who: &T::AccountId,
		amount: Self::Balance,
		reasons: WithdrawReasons,
		new_balance: Self::Balance,
	) -> DispatchResult {
		<Module<T>>::ensure_can_withdraw(U::asset_id(), who, amount, reasons, new_balance)
	}

	fn withdraw(
		who: &T::AccountId,
		value: Self::Balance,
		reasons: WithdrawReasons,
		_req: ExistenceRequirement,
	) -> result::Result<Self::NegativeImbalance, DispatchError> {
		let new_balance = Self::free_balance(who)
			.checked_sub(&value)
			.ok_or(Error::<T>::InsufficientBalance)?;
		Self::ensure_can_withdraw(who, value, reasons, new_balance)?;

		// `free` balance should be freed if set to a dust amount
		if new_balance < Self::minimum_balance() {
			let amount = <FreeBalance<T>>::take(U::asset_id(), who);
			if amount > Zero::zero() {
				T::OnDustImbalance::on_nonzero_unbalanced(NegativeImbalance::new(amount, U::asset_id()));
			}
			<Module<T>>::deposit_event(Event::<T>::DustReclaimed(U::asset_id(), who.clone(), amount));
		} else {
			<Module<T>>::set_free_balance(U::asset_id(), who, new_balance);
		}

		Ok(NegativeImbalance::new(value, U::asset_id()))
	}

	fn deposit_into_existing(
		who: &T::AccountId,
		value: Self::Balance,
	) -> result::Result<Self::PositiveImbalance, DispatchError> {
		if <FreeBalance<T>>::contains_key(U::asset_id(), who) {
			Ok(Self::deposit_creating(who, value))
		} else {
			Err(Error::<T>::AccountIdNotExist)?
		}
	}

	fn deposit_creating(who: &T::AccountId, value: Self::Balance) -> Self::PositiveImbalance {
		let imbalance = Self::make_free_balance_be(who, Self::free_balance(who) + value);
		if let SignedImbalance::Positive(p) = imbalance {
			p
		} else {
			// Impossible, but be defensive.
			Self::PositiveImbalance::zero()
		}
	}

	fn make_free_balance_be(
		who: &T::AccountId,
		balance: Self::Balance,
	) -> SignedImbalance<Self::Balance, Self::PositiveImbalance> {
		let original = <Module<T>>::free_balance(U::asset_id(), who);
		let imbalance = if original <= balance {
			SignedImbalance::Positive(PositiveImbalance::new(balance - original, U::asset_id()))
		} else {
			SignedImbalance::Negative(NegativeImbalance::new(original - balance, U::asset_id()))
		};
		<Module<T>>::set_free_balance(U::asset_id(), who, balance);
		imbalance
	}

	fn can_slash(who: &T::AccountId, value: Self::Balance) -> bool {
		<Module<T>>::free_balance(U::asset_id(), &who) >= value
	}

	fn slash(who: &T::AccountId, value: Self::Balance) -> (Self::NegativeImbalance, Self::Balance) {
		let remaining = <Module<T>>::slash(U::asset_id(), who, value);
		if let Some(r) = remaining {
			(NegativeImbalance::new(value - r, U::asset_id()), r)
		} else {
			(NegativeImbalance::new(value, U::asset_id()), Zero::zero())
		}
	}

	fn burn(mut amount: Self::Balance) -> Self::PositiveImbalance {
		<TotalIssuance<T>>::mutate(U::asset_id(), |issued| {
			issued.checked_sub(&amount).unwrap_or_else(|| {
				amount = *issued;
				Zero::zero()
			})
		});
		PositiveImbalance::new(amount, U::asset_id())
	}

	fn issue(mut amount: Self::Balance) -> Self::NegativeImbalance {
		<TotalIssuance<T>>::mutate(U::asset_id(), |issued| {
			*issued = issued.checked_add(&amount).unwrap_or_else(|| {
				amount = Self::Balance::max_value() - *issued;
				Self::Balance::max_value()
			})
		});
		NegativeImbalance::new(amount, U::asset_id())
	}
}

impl<T, U> ReservableCurrency<T::AccountId> for AssetCurrency<T, U>
where
	T: Config,
	U: AssetIdAuthority<AssetId = T::AssetId>,
{
	fn can_reserve(who: &T::AccountId, value: Self::Balance) -> bool {
		Self::free_balance(who)
			.checked_sub(&value)
			.map_or(false, |new_balance| {
				<Module<T>>::ensure_can_withdraw(U::asset_id(), who, value, WithdrawReasons::RESERVE, new_balance)
					.is_ok()
			})
	}

	fn reserved_balance(who: &T::AccountId) -> Self::Balance {
		<Module<T>>::reserved_balance(U::asset_id(), &who)
	}

	fn reserve(who: &T::AccountId, value: Self::Balance) -> DispatchResult {
		<Module<T>>::reserve(U::asset_id(), who, value)
	}

	fn unreserve(who: &T::AccountId, value: Self::Balance) -> Self::Balance {
		<Module<T>>::unreserve(U::asset_id(), who, value)
	}

	fn slash_reserved(who: &T::AccountId, value: Self::Balance) -> (Self::NegativeImbalance, Self::Balance) {
		let leftover = <Module<T>>::slash_reserved(U::asset_id(), who, value).unwrap_or(Zero::zero());
		(NegativeImbalance::new(value - leftover, U::asset_id()), leftover)
	}

	fn repatriate_reserved(
		slashed: &T::AccountId,
		beneficiary: &T::AccountId,
		value: Self::Balance,
		_status: BalanceStatus,
	) -> result::Result<Self::Balance, DispatchError> {
		<Module<T>>::repatriate_reserved(U::asset_id(), slashed, beneficiary, value)
	}
}

pub struct StakingAssetIdAuthority<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> AssetIdAuthority for StakingAssetIdAuthority<T> {
	type AssetId = T::AssetId;
	fn asset_id() -> Self::AssetId {
		<Module<T>>::staking_asset_id()
	}
}

pub struct SpendingAssetIdAuthority<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> AssetIdAuthority for SpendingAssetIdAuthority<T> {
	type AssetId = T::AssetId;
	fn asset_id() -> Self::AssetId {
		<Module<T>>::spending_asset_id()
	}
}

impl<T, U> LockableCurrency<T::AccountId> for AssetCurrency<T, U>
where
	T: Config,
	T::Balance: MaybeSerializeDeserialize + Debug,
	U: AssetIdAuthority<AssetId = T::AssetId>,
{
	type Moment = T::BlockNumber;
	type MaxLocks = ();

	fn set_lock(id: LockIdentifier, who: &T::AccountId, amount: T::Balance, reasons: WithdrawReasons) {
		<Module<T>>::set_lock(id, U::asset_id(), who, amount, reasons)
	}

	fn extend_lock(id: LockIdentifier, who: &T::AccountId, amount: T::Balance, reasons: WithdrawReasons) {
		<Module<T>>::extend_lock(id, U::asset_id(), who, amount, reasons)
	}

	fn remove_lock(id: LockIdentifier, who: &T::AccountId) {
		<Module<T>>::remove_lock(id, U::asset_id(), who)
	}
}

pub type StakingAssetCurrency<T> = AssetCurrency<T, StakingAssetIdAuthority<T>>;
pub type SpendingAssetCurrency<T> = AssetCurrency<T, SpendingAssetIdAuthority<T>>;
