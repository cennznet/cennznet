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

//! Manage the authorized accounts set for the Sylo fee payment

use super::{Trait as SyloTrait, WeightInfo};
use frame_support::{decl_module, decl_storage, ensure};
use frame_system::{ensure_root, ensure_signed};
use sp_runtime::DispatchResult;
use sp_std::prelude::*;

pub trait Trait: SyloTrait {}

const NOT_SYLO_PAYER: &str = "You are not a Sylo payer!";

decl_storage! {
	trait Store for Module<T: Trait> as SyloFeePayment {
		/// Accounts which have authority to pay for Sylo fees on behalf of the users
		AuthorisedPayers get(fn authorised_payers): Vec<T::AccountId>;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin, system = frame_system {

		/// Add `account_id` as an authorized Sylo fee payer. Only Sudo can set a payment account.
		#[weight = T::WeightInfo::set_payment_account()]
		pub fn set_payment_account(origin, account_id: T::AccountId) -> DispatchResult {
			ensure_root(origin)?;
			<AuthorisedPayers<T>>::mutate(|v|{if !v.contains(&account_id) {v.push(account_id)}});
			Ok(())
		}

		/// If the origin of the call is an authorised payer, revoke its authorisation.
		/// NOTE: This may halt all Sylo operations if there are no other payers.
		#[weight = T::WeightInfo::revoke_payment_account_self()]
		pub fn revoke_payment_account_self(origin) -> DispatchResult {
			let account_id = ensure_signed(origin)?;
			ensure!(Self::authorised_payers().contains(&account_id), NOT_SYLO_PAYER);
			<AuthorisedPayers<T>>::mutate(|v|v.retain(|x| *x != account_id));
			Ok(())
		}
	}
}

impl<T: Trait> Module<T> {
	/// Return an account that is set for payment, or `None` when nothing is set.
	/// In the future, we can make this function smart so it returns the account with enough money in it.
	pub fn payment_account() -> Option<T::AccountId> {
		Self::authorised_payers().first().cloned()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{ExtBuilder, Origin, Test};
	use frame_support::assert_ok;
	use frame_system::RawOrigin;
	use sp_runtime::DispatchError::Other;

	type SyloModule = Module<Test>;

	impl Trait for Test {}

	#[test]
	fn set_payment_account() {
		ExtBuilder::default().build().execute_with(|| {
			let payer_a = 2;
			let payer_b = 3;

			assert!(!SyloModule::authorised_payers().contains(&payer_a));
			assert!(!SyloModule::authorised_payers().contains(&payer_b));

			assert_ok!(SyloModule::set_payment_account(Origin::root(), payer_a));
			assert_ok!(SyloModule::set_payment_account(Origin::root(), payer_b));

			assert!(SyloModule::authorised_payers().contains(&payer_a));
			assert!(SyloModule::authorised_payers().contains(&payer_b));
		});
	}

	#[test]
	fn get_payment_account() {
		ExtBuilder::default().build().execute_with(|| {
			let payer_a = 2;
			let payer_b = 3;

			assert_ok!(SyloModule::set_payment_account(Origin::root(), payer_a.clone()));
			assert_ok!(SyloModule::set_payment_account(Origin::root(), payer_b));

			assert_eq!(SyloModule::payment_account().unwrap(), payer_a);
		});
	}

	#[test]
	fn get_payment_account_when_no_account_is_set() {
		ExtBuilder::default().build().execute_with(|| {
			assert_eq!(SyloModule::payment_account(), None);
		});
	}

	#[test]
	fn revoke_payment_account_self_a_payer() {
		ExtBuilder::default().build().execute_with(|| {
			let payer_a = 2;

			assert_ok!(SyloModule::set_payment_account(Origin::root(), payer_a));

			assert_ok!(SyloModule::revoke_payment_account_self(Origin::from(
				RawOrigin::Signed(payer_a.clone())
			)));

			assert!(!SyloModule::authorised_payers().contains(&payer_a));
		});
	}

	#[test]
	fn revoke_payment_account_self_a_non_payer() {
		ExtBuilder::default().build().execute_with(|| {
			let payer_a = 2;

			assert_eq!(
				SyloModule::revoke_payment_account_self(Origin::from(RawOrigin::Signed(payer_a.clone()))),
				Err(Other(NOT_SYLO_PAYER))
			);

			assert!(!SyloModule::authorised_payers().contains(&payer_a));
		});
	}
}
