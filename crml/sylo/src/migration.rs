/* Copyright 2020 Centrality Investments Limited
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

//! Manage the authorized account set for the Sylo data migration

use frame_support::{decl_module, decl_storage, ensure, weights::SimpleDispatchInfo, IterableStorageMap};
use frame_system::{ensure_root, ensure_signed};
use sp_runtime::DispatchResult;
use sp_std::prelude::*;

pub trait Trait: frame_system::Trait {}

decl_storage! {
	trait Store for Module<T: Trait> as SyloMigration {
		/// Accounts which have authority to make Sylo data migration calls
		AuthorisedMigrators: map hasher(twox_64_concat) T::AccountId => bool;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin, system = frame_system {

		/// Add `account_id` as a authorized Sylo data migrator
		#[weight = SimpleDispatchInfo::FixedOperational(0)]
		pub fn set_migrator_account(origin, account_id: T::AccountId) -> DispatchResult {
			ensure_root(origin)?;
			AuthorisedMigrators::<T>::insert(account_id, true);
			Ok(())
		}

		/// Remove all Sylo migrator accounts from storage, thereby revoking all permissions.
		/// Any authorized migrator may call this.
		#[weight = SimpleDispatchInfo::FixedOperational(0)]
		pub fn revoke_migrators(origin) -> DispatchResult {
			Self::ensure_sylo_migrator(origin)?;
			let _ = AuthorisedMigrators::<T>::drain().collect::<Vec<(T::AccountId, bool)>>();
			Ok(())
		}
	}
}

impl<T: Trait> Module<T> {
	// Ensure `origin` is an authorized Sylo data migrator
	pub fn ensure_sylo_migrator(origin: T::Origin) -> DispatchResult {
		let account_id = ensure_signed(origin)?;
		ensure!(AuthorisedMigrators::<T>::get(account_id), "NotASyloMigrator");
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{ExtBuilder, Origin, Test};
	use frame_support::assert_ok;
	use sp_core::H256;
	use sp_runtime::DispatchError::Other;

	type Migration = Module<Test>;

	impl Trait for Test {}

	#[test]
	fn set_migration_account_works() {
		ExtBuilder::default().build().execute_with(|| {
			let migrator_a = H256::from_low_u64_be(2);
			let migrator_b = H256::from_low_u64_be(3);

			assert_ok!(Migration::set_migrator_account(Origin::ROOT, migrator_a));
			assert_ok!(Migration::set_migrator_account(Origin::ROOT, migrator_b));

			assert_ok!(Migration::ensure_sylo_migrator(Origin::signed(migrator_a)));
			assert_ok!(Migration::ensure_sylo_migrator(Origin::signed(migrator_b)));
		});
	}

	#[test]
	fn wrong_migration_account_fails_ensure() {
		ExtBuilder::default().build().execute_with(|| {
			let migration_account = H256::from_low_u64_be(2);
			let invalid_account = H256::from_low_u64_be(3);

			assert_ok!(Migration::set_migrator_account(Origin::ROOT, migration_account));

			assert_eq!(
				Migration::ensure_sylo_migrator(Origin::signed(invalid_account)),
				Err(Other("NotASyloMigrator"))
			);
		});
	}

	#[test]
	fn no_migration_account_fails_ensure() {
		ExtBuilder::default().build().execute_with(|| {
			let migration_account = H256::from_low_u64_be(2);

			assert_eq!(
				Migration::ensure_sylo_migrator(Origin::signed(migration_account)),
				Err(Other("NotASyloMigrator"))
			);
		});
	}

	#[test]
	fn revoke_migrators_works() {
		ExtBuilder::default().build().execute_with(|| {
			let migrator_a = H256::from_low_u64_be(2);
			let migrator_b = H256::from_low_u64_be(3);

			assert_ok!(Migration::set_migrator_account(Origin::ROOT, migrator_a));
			assert_ok!(Migration::set_migrator_account(Origin::ROOT, migrator_b));

			assert_ok!(Migration::revoke_migrators(Origin::signed(migrator_a)));

			assert_eq!(
				Migration::ensure_sylo_migrator(Origin::signed(migrator_a)),
				Err(Other("NotASyloMigrator"))
			);
			assert_eq!(
				Migration::ensure_sylo_migrator(Origin::signed(migrator_b)),
				Err(Other("NotASyloMigrator"))
			);

			// Migrator storage has emptied
			assert!(AuthorisedMigrators::<Test>::iter().collect::<Vec<(_, bool)>>().is_empty());
		});
	}

	#[test]
	fn revoke_migrators_with_invalid_account_fails() {
		ExtBuilder::default().build().execute_with(|| {
			let migration_account = H256::from_low_u64_be(2);
			let invalid_account = H256::from_low_u64_be(3);

			assert_ok!(Migration::set_migrator_account(Origin::ROOT, migration_account));

			assert_eq!(
				Migration::revoke_migrators(Origin::signed(invalid_account)),
				Err(Other("NotASyloMigrator"))
			);

			assert_ok!(Migration::ensure_sylo_migrator(Origin::signed(migration_account)));
		});
	}
}
