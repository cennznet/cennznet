// Migrations from v40 - v41 runtime
use crate::{AssetInfo, AssetMeta, BalanceLock, Config, Locks, Module};
use frame_support::{
	storage::{IterableStorageMap, StorageDoubleMap, StorageMap},
	weights::{constants::RocksDbWeight as DbWeight, Weight},
};
use sp_std::vec::Vec;

/// Locks are keyed by asset Id in v41
pub fn migrate_locks<T: Config>() -> Weight {
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

	DbWeight::get().writes(all_locks.len() as u64)
}

/// AssetInfo struct has added `existential_deposit` field in v41
pub fn migrate_asset_info<T: Config>() -> Weight {
	#[allow(dead_code)]
	mod old_storage {
		use super::{Config, StorageMap};
		use codec::{Decode, Encode};
		use sp_runtime::RuntimeDebug;
		use sp_std::vec::Vec;

		pub struct Module<T>(sp_std::marker::PhantomData<T>);
		#[derive(Encode, Decode, PartialEq, Eq, Default, Clone, RuntimeDebug)]
		pub struct AssetInfoOld {
			pub symbol: Vec<u8>,
			pub decimal_places: u8,
		}

		frame_support::decl_storage! {
			trait Store for Module<T: Config> as GenericAsset {
				pub AssetMeta get(fn asset_meta) config(): map hasher(twox_64_concat) T::AssetId => AssetInfoOld;
			}
		}
	}

	// Update asset meta
	let old_asset_meta: Vec<(T::AssetId, old_storage::AssetInfoOld)> = <old_storage::AssetMeta<T>>::drain().collect();
	let insert_count = old_asset_meta.len();
	old_asset_meta.into_iter().for_each(|(asset_id, asset_meta)| {
		AssetMeta::<T>::insert(
			asset_id,
			AssetInfo::new(asset_meta.symbol, asset_meta.decimal_places, 1),
		);
	});

	DbWeight::get().writes(insert_count as u64)
}
