// Copyright 2018-2020 Parity Technologies (UK) Ltd. and Centrality Investments Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

//! # Balance Scaling Module
//!
//! This module provides the basic logic needed to scale down CENNZ and CPAY balances to make them
//! a better experience for users to handle.

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_module, traits::Get, weights::Weight, IterableStorageDoubleMap};
use pallet_generic_asset::{FreeBalance, Module as GenericAsset};
use sp_runtime::traits::{CheckedDiv, CheckedSub, Zero};

pub trait Trait: pallet_generic_asset::Trait + pallet_sudo::Trait {
	type ScaleDownFactor: Get<Self::Balance>;
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn on_runtime_upgrade() -> Weight {
			let scale_down = |asset_id| {
				let balances_iter =
					<FreeBalance<T> as IterableStorageDoubleMap<T::AssetId, T::AccountId, T::Balance>>::iter(asset_id);
				balances_iter.for_each(|(who, balance)| {
					let scaled_balance = balance.checked_div(&<T as Trait>::ScaleDownFactor::get()).unwrap_or(balance);
					let burn_amount = balance.checked_sub(&scaled_balance).unwrap_or(Zero::zero());
					let _ = GenericAsset::<T>::burn_free(&asset_id, &pallet_sudo::Module::<T>::key(), &who, &burn_amount);
				});
			};

			scale_down(GenericAsset::<T>::spending_asset_id());
			scale_down(GenericAsset::<T>::staking_asset_id());

			Zero::zero() // No weight
		}
	}
}
