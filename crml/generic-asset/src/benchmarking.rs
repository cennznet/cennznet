// Copyright 2019-2020 Plug New Zealand Limited
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Generic assets benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use crate::Module as GenericAsset;
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite, whitelisted_caller};
use frame_system::RawOrigin;
use sp_std::ops::{Add, Mul};

const SEED: u32 = 0;

benchmarks! {
	// Benchmark `transfer` extrinsic with the worst possible conditions:
	// Transfer will kill the sender account.
	// Transfer will create the recipient account.
	transfer {
		let caller: T::AccountId = whitelisted_caller();

		// spending asset id
		let asset_id = GenericAsset::<T>::spending_asset_id();
		let initial_balance = T::Balance::from(5_000_000u32);
		GenericAsset::<T>::set_free_balance(asset_id, &caller, initial_balance);

		let recipient: T::AccountId = account("recipient", 0, SEED);
		let transfer_amount = T::Balance::from(5_000_000u32);
	}: transfer(RawOrigin::Signed(caller.clone()), asset_id, recipient.clone(), transfer_amount)
	verify {
		assert_eq!(GenericAsset::<T>::free_balance(asset_id, &caller), Zero::zero());
		assert_eq!(GenericAsset::<T>::free_balance(asset_id, &recipient), transfer_amount);
	}

	transfer_keep_alive {
		let caller: T::AccountId = whitelisted_caller();

		// spending asset id
		let asset_id = GenericAsset::<T>::spending_asset_id();
		let initial_balance = T::Balance::from(5_000_000u32);
		GenericAsset::<T>::set_free_balance(asset_id, &caller, initial_balance);

		let recipient: T::AccountId = account("recipient", 0, SEED);
		let transfer_amount = T::Balance::from(5_000_000u32);
	}: transfer_keep_alive(RawOrigin::Signed(caller.clone()), asset_id, recipient.clone(), transfer_amount)
	verify {
		assert_eq!(GenericAsset::<T>::free_balance(asset_id, &caller), Zero::zero());
		assert_eq!(GenericAsset::<T>::free_balance(asset_id, &recipient), transfer_amount);
	}

	// Benchmark `burn`, GA's create comes from ROOT account. This always creates an asset.
	// Mint some amount of new asset to an account and burn the asset from it.
	burn {
		let caller: T::AccountId = whitelisted_caller();
		let initial_balance = T::Balance::from(5_000_000u32);
		let asset_id = GenericAsset::<T>::next_asset_id();
		let permissions = PermissionLatest::<T::AccountId>::new(caller.clone());
		let asset_options :AssetOptions<T::Balance, T::AccountId> = AssetOptions {
			initial_issuance: initial_balance,
			permissions,
		};
		let asset_info = AssetInfo::default();
		let decimal_factor: T::Balance = 10u32.pow(asset_info.decimal_places().into()).into();

		let _ = GenericAsset::<T>::create(
			RawOrigin::Root.into(),
			caller.clone(),
			asset_options,
			asset_info,
		);

		let account: T::AccountId = account("bob", 0, SEED);

		// Mint some asset to the account 'bob' so that 'bob' can burn those
		let mint_amount = T::Balance::from(5_000_000u32);
		let _ = GenericAsset::<T>::mint(RawOrigin::Signed(caller.clone()).into(), asset_id, account.clone(), mint_amount);

		let burn_amount = T::Balance::from(5_000_000u32);
	}: burn(RawOrigin::Signed(caller.clone()), asset_id, account.clone(), burn_amount)
	verify {
		assert_eq!(GenericAsset::<T>::free_balance(asset_id, &account), Zero::zero());
		assert_eq!(GenericAsset::<T>::total_issuance(asset_id), initial_balance.mul(decimal_factor));
	}

	// Benchmark `burn`, GA's create comes from ROOT account.
	create {
		let caller: T::AccountId = whitelisted_caller();
		let initial_balance = T::Balance::from(5_000_000u32);
		let permissions = PermissionLatest::<T::AccountId>::new(caller.clone());
		let asset_id = GenericAsset::<T>::next_asset_id();
		let asset_options :AssetOptions<T::Balance, T::AccountId> = AssetOptions {
			initial_issuance: initial_balance,
			permissions,
		};
		let asset_info = AssetInfo::default();
		let decimal_factor: T::Balance = 10u32.pow(asset_info.decimal_places().into()).into();

	}: create(RawOrigin::Root, caller.clone(), asset_options, asset_info)
	verify {
		let total_issuance = initial_balance.mul(decimal_factor);
		assert_eq!(GenericAsset::<T>::total_issuance(&asset_id), total_issuance);
		assert_eq!(GenericAsset::<T>::free_balance(asset_id, &caller.clone()), total_issuance);
	}

	// Benchmark `mint`, create asset from ROOT account.
	// Owner of the asset can then mint amount to 'recipient' account
	mint {
		let caller: T::AccountId = whitelisted_caller();
		let mint_to: T::AccountId = account("recipient", 0, SEED);
		let initial_balance = T::Balance::from(5_000_000u32);
		let asset_id = GenericAsset::<T>::next_asset_id();
		let permissions = PermissionLatest::<T::AccountId>::new(caller.clone());
		let asset_options :AssetOptions<T::Balance, T::AccountId> = AssetOptions {
			initial_issuance: initial_balance,
			permissions,
		};
		let asset_info = AssetInfo::default();
		let decimal_factor: T::Balance = 10u32.pow(asset_info.decimal_places().into()).into();

		let _ = GenericAsset::<T>::create(
			RawOrigin::Root.into(),
			caller.clone(),
			asset_options,
			asset_info,
		);

		let mint_amount = T::Balance::from(1_000_000u32);
	}: mint(RawOrigin::Signed(caller.clone()), asset_id, mint_to.clone(), mint_amount )
	verify {
		let total_issuance = initial_balance.mul(decimal_factor).add(mint_amount);
		assert_eq!(GenericAsset::<T>::total_issuance(&asset_id), total_issuance);
		assert_eq!(GenericAsset::<T>::free_balance(asset_id, &mint_to.clone()), mint_amount);
	}

	// Benchmark `update_asset_info`, create asset from ROOT account.
	// Update the asset info
	update_asset_info {
		let caller: T::AccountId = whitelisted_caller();
		let web3_asset_info = AssetInfo::new(b"WEB3.0".to_vec(), 3, T::Balance::from(5u32));
		let initial_balance = T::Balance::from(5_000_000u32);
		let asset_id = GenericAsset::<T>::next_asset_id();
		let permissions = PermissionLatest::<T::AccountId>::new(caller.clone());
		let burn_amount = T::Balance::from(5_000u32);
		let asset_options :AssetOptions<T::Balance, T::AccountId> = AssetOptions {
			initial_issuance: initial_balance,
			permissions,
		};
		let _ = GenericAsset::<T>::create(
			RawOrigin::Root.into(),
			caller.clone(),
			asset_options,
			web3_asset_info.clone()
		);

		let web3_asset_info = AssetInfo::new(b"WEB3.1".to_vec(), 5, T::Balance::from(7u32));
	}: update_asset_info(RawOrigin::Signed(caller.clone()), asset_id, web3_asset_info.clone())
	verify {
		assert_eq!(GenericAsset::<T>::asset_meta(asset_id), web3_asset_info);
	}

	// Benchmark `update_permission`, create asset from ROOT account with 'update' permission.
	// Update permission to include update and mint
	update_permission {
		let caller: T::AccountId = whitelisted_caller();
		let initial_balance = T::Balance::from(5_000_000u32);
		let permissions = PermissionLatest {
			update: Owner::Address(caller.clone()),
			mint: Owner::None,
			burn: Owner::None,
		};

		let new_permission = PermissionLatest {
			update: Owner::Address(caller.clone()),
			mint: Owner::Address(caller.clone()),
			burn: Owner::None,
		};
		let asset_id = GenericAsset::<T>::next_asset_id();
		let asset_options :AssetOptions<T::Balance, T::AccountId> = AssetOptions {
			initial_issuance: initial_balance,
			permissions,
		};
		let _ = GenericAsset::<T>::create(
			RawOrigin::Root.into(),
			caller.clone(),
			asset_options,
			AssetInfo::default()
		);
	}: update_permission(RawOrigin::Signed(caller.clone()), asset_id, new_permission)
	verify {
		assert!(GenericAsset::<T>::check_permission(asset_id, &caller.clone(), &PermissionType::Mint));
		assert!(!GenericAsset::<T>::check_permission(asset_id, &caller, &PermissionType::Burn));
	}

	// Benchmark `create_reserved`, create reserved asset from ROOT account.
	create_reserved {
		let caller: T::AccountId = whitelisted_caller();
		let initial_balance = T::Balance::from(5_000_000u32);
		let permissions = PermissionLatest::<T::AccountId>::new(caller.clone());
		// create reserved asset with asset_id >= next_asset_id should fail so set the next asset id to some value
		<NextAssetId<T>>::put(T::AssetId::from(10001u32));
		let asset_id = T::AssetId::from(1000u32);
		let asset_options :AssetOptions<T::Balance, T::AccountId> = AssetOptions {
			initial_issuance: initial_balance,
			permissions,
		};
		let asset_info = AssetInfo::default();
		let decimal_factor: T::Balance = 10u32.pow(asset_info.decimal_places().into()).into();

	}: create_reserved(RawOrigin::Root, asset_id, asset_options, asset_info)
	verify {
		let total_issuance = initial_balance.mul(decimal_factor);
		assert_eq!(GenericAsset::<T>::total_issuance(&asset_id), total_issuance);
		assert_eq!(GenericAsset::<T>::free_balance(asset_id, &T::AccountId::default()), total_issuance);
		assert_eq!(asset_id, T::AssetId::from(1000u32));
	}
}

impl_benchmark_test_suite!(
	GenericAsset,
	crate::mock::new_test_ext_with_default(),
	crate::mock::Test,
);
