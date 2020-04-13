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

use crate::{device, e2ee, groups, inbox, response, vault};
use frame_support::{decl_module, decl_storage, ensure, weights::SimpleDispatchInfo};
use frame_system::{ensure_root, ensure_signed};
use sp_runtime::{DispatchError::BadOrigin, DispatchResult};

pub trait Trait: device::Trait + e2ee::Trait + groups::Trait + inbox::Trait + response::Trait + vault::Trait {}

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
		pub fn remove_migrator_account(origin) -> DispatchResult {
			ensure_root(origin)?;
			MigrationAccount::<T>::kill();
			Ok(())
		}
	}
}

impl<T: Trait> Module<T> {
	fn ensure_sylo_migrator(origin: T::Origin) -> DispatchResult {
		let account_id = ensure_signed(origin)?;
		ensure!(MigrationAccount::<T>::get() == account_id, BadOrigin);
		Ok(())
	}
}
