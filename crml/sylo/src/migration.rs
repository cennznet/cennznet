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

use frame_support::{decl_module, decl_storage, ensure, weights::SimpleDispatchInfo};
use frame_system::{ensure_root, ensure_signed};
use sp_runtime::{DispatchError::BadOrigin, DispatchResult};

pub trait Trait: frame_system::Trait {}

decl_storage! {
	trait Store for Module<T: Trait> as SyloMigration {
		MigrationAccount: T::AccountId;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin, system = frame_system {

		#[weight = SimpleDispatchInfo::FixedOperational(0)]
		pub fn set_migrator_account(origin, account_id: T::AccountId) -> DispatchResult {
			ensure_root(origin)?;
			MigrationAccount::<T>::put(account_id);
			Ok(())
		}

		#[weight = SimpleDispatchInfo::FixedOperational(0)]
		pub fn self_destruct(origin) -> DispatchResult {
			Self::ensure_sylo_migrator(origin)?;
			MigrationAccount::<T>::kill();
			Ok(())
		}
	}
}

impl<T: Trait> Module<T> {
	pub fn ensure_sylo_migrator(origin: T::Origin) -> DispatchResult {
		let account_id = ensure_signed(origin)?;
		ensure!(MigrationAccount::<T>::get() == account_id, BadOrigin);
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{ExtBuilder, Origin, Test};
	use frame_support::assert_ok;
	use sp_core::H256;
	use sp_runtime::DispatchError::BadOrigin;

	type Migration = Module<Test>;

	impl Trait for Test {}

	#[test]
	fn set_migration_account_works() {
		ExtBuilder::default().build().execute_with(|| {
			let migration_account = H256::from_low_u64_be(2);

			assert_ok!(Migration::set_migrator_account(Origin::ROOT, migration_account));

			assert_ok!(Migration::ensure_sylo_migrator(Origin::signed(migration_account)));
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
				Err(BadOrigin)
			);
		});
	}

	#[test]
	fn no_migration_account_fails_ensure() {
		ExtBuilder::default().build().execute_with(|| {
			let migration_account = H256::from_low_u64_be(2);

			assert_eq!(
				Migration::ensure_sylo_migrator(Origin::signed(migration_account)),
				Err(BadOrigin)
			);
		});
	}

	#[test]
	fn remove_migration_account_works() {
		ExtBuilder::default().build().execute_with(|| {
			let migration_account = H256::from_low_u64_be(2);

			assert_ok!(Migration::set_migrator_account(Origin::ROOT, migration_account));

			assert_ok!(Migration::self_destruct(Origin::signed(migration_account)));

			assert_eq!(
				Migration::ensure_sylo_migrator(Origin::signed(migration_account)),
				Err(BadOrigin)
			);
		});
	}

	#[test]
	fn remove_migration_account_with_invalid_account_fails() {
		ExtBuilder::default().build().execute_with(|| {
			let migration_account = H256::from_low_u64_be(2);
			let invalid_account = H256::from_low_u64_be(3);

			assert_ok!(Migration::set_migrator_account(Origin::ROOT, migration_account));

			assert_eq!(
				Migration::self_destruct(Origin::signed(invalid_account)),
				Err(BadOrigin)
			);

			assert_ok!(Migration::ensure_sylo_migrator(Origin::signed(migration_account)));
		});
	}
}
