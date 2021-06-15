// Copyright 2019-2021 Plug New Zealand Ltd.
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

//! Extra trait implementations for the `GenericAsset` module

use crate::{Config, Error, Module, NegativeImbalance, PositiveImbalance, SpendingAssetIdAuthority};
use crml_support::{AssetIdAuthority, MultiCurrencyAccounting};
use frame_support::traits::{ExistenceRequirement, Imbalance, SignedImbalance, WithdrawReasons};
use sp_runtime::{
	traits::{CheckedSub, UniqueSaturatedInto, Zero},
	DispatchError, DispatchResult,
};
use sp_std::result;

impl<T: Config> MultiCurrencyAccounting for Module<T> {
	type AccountId = T::AccountId;
	type CurrencyId = T::AssetId;
	type Balance = T::Balance;
	type DefaultCurrencyId = SpendingAssetIdAuthority<T>;
	type PositiveImbalance = PositiveImbalance<T>;
	type NegativeImbalance = NegativeImbalance<T>;

	fn minimum_balance(currency: Option<Self::CurrencyId>) -> Self::Balance {
		<Module<T>>::asset_meta(currency.unwrap_or_else(|| Self::DefaultCurrencyId::asset_id()))
			.existential_deposit()
			.unique_saturated_into()
	}

	fn total_balance(who: &T::AccountId, currency: Option<Self::CurrencyId>) -> Self::Balance {
		<Module<T>>::total_balance(currency.unwrap_or_else(|| Self::DefaultCurrencyId::asset_id()), who)
	}

	fn free_balance(who: &T::AccountId, currency: Option<Self::CurrencyId>) -> Self::Balance {
		<Module<T>>::free_balance(currency.unwrap_or_else(|| Self::DefaultCurrencyId::asset_id()), who)
	}

	fn deposit_creating(
		who: &T::AccountId,
		currency: Option<Self::CurrencyId>,
		value: Self::Balance,
	) -> Self::PositiveImbalance {
		if value.is_zero() {
			return Self::PositiveImbalance::zero();
		}

		let asset_id = currency.unwrap_or_else(|| Self::DefaultCurrencyId::asset_id());
		let imbalance = Self::make_free_balance_be(who, currency, <Module<T>>::free_balance(asset_id, who) + value);
		if let SignedImbalance::Positive(p) = imbalance {
			p
		} else {
			// Impossible, but be defensive.
			Self::PositiveImbalance::zero()
		}
	}

	fn deposit_into_existing(
		who: &T::AccountId,
		currency: Option<Self::CurrencyId>,
		value: Self::Balance,
	) -> result::Result<Self::PositiveImbalance, DispatchError> {
		// No existential deposit rule and creation fee in GA. `deposit_into_existing` is same with `deposit_creating`.
		Ok(Self::deposit_creating(who, currency, value))
	}

	fn ensure_can_withdraw(
		who: &T::AccountId,
		currency: Option<Self::CurrencyId>,
		amount: Self::Balance,
		reasons: WithdrawReasons,
		new_balance: Self::Balance,
	) -> DispatchResult {
		<Module<T>>::ensure_can_withdraw(
			currency.unwrap_or_else(|| Self::DefaultCurrencyId::asset_id()),
			who,
			amount,
			reasons,
			new_balance,
		)
	}

	fn make_free_balance_be(
		who: &T::AccountId,
		currency: Option<Self::CurrencyId>,
		balance: Self::Balance,
	) -> SignedImbalance<Self::Balance, Self::PositiveImbalance> {
		let asset_id = currency.unwrap_or_else(|| Self::DefaultCurrencyId::asset_id());
		let original = <Module<T>>::free_balance(asset_id, who);
		let imbalance = if original <= balance {
			SignedImbalance::Positive(Self::PositiveImbalance::new(balance - original, asset_id))
		} else {
			SignedImbalance::Negative(Self::NegativeImbalance::new(original - balance, asset_id))
		};
		<Module<T>>::set_free_balance(asset_id, who, balance);
		imbalance
	}

	fn transfer(
		transactor: &T::AccountId,
		dest: &T::AccountId,
		currency: Option<Self::CurrencyId>,
		value: Self::Balance,
		req: ExistenceRequirement,
	) -> DispatchResult {
		if value.is_zero() {
			return Ok(());
		}
		<Module<T>>::make_transfer(
			currency.unwrap_or_else(|| Self::DefaultCurrencyId::asset_id()),
			transactor,
			dest,
			value,
			req,
		)
	}

	fn withdraw(
		who: &T::AccountId,
		currency: Option<Self::CurrencyId>,
		value: Self::Balance,
		reasons: WithdrawReasons,
		_ex: ExistenceRequirement, // no existential deposit policy for generic asset
	) -> result::Result<Self::NegativeImbalance, DispatchError> {
		if value.is_zero() {
			return Ok(Self::NegativeImbalance::zero());
		}

		let asset_id = currency.unwrap_or_else(|| Self::DefaultCurrencyId::asset_id());
		let new_balance = <Module<T>>::free_balance(asset_id, who)
			.checked_sub(&value)
			.ok_or(Error::<T>::InsufficientBalance)?;

		<Module<T>>::ensure_can_withdraw(asset_id, who, value, reasons, new_balance)?;
		<Module<T>>::set_free_balance(asset_id, who, new_balance);

		Ok(Self::NegativeImbalance::new(value, asset_id))
	}

	fn reserve(who: &Self::AccountId, currency: Option<Self::CurrencyId>, amount: Self::Balance) -> DispatchResult {
		if amount.is_zero() {
			return Ok(());
		}

		<Module<T>>::reserve(
			currency.unwrap_or_else(|| Self::DefaultCurrencyId::asset_id()),
			who,
			amount,
		)
	}

	fn repatriate_reserved(
		who: &Self::AccountId,
		currency: Option<Self::CurrencyId>,
		beneficiary: &Self::AccountId,
		amount: Self::Balance,
	) -> result::Result<Self::Balance, DispatchError> {
		<Module<T>>::repatriate_reserved(
			currency.unwrap_or_else(|| Self::DefaultCurrencyId::asset_id()),
			who,
			beneficiary,
			amount,
		)
	}

	fn unreserve(who: &Self::AccountId, currency: Option<Self::CurrencyId>, amount: Self::Balance) -> Self::Balance {
		if amount.is_zero() {
			return Zero::zero();
		}

		<Module<T>>::unreserve(
			currency.unwrap_or_else(|| Self::DefaultCurrencyId::asset_id()),
			who,
			amount,
		)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{
		new_test_ext_with_balance, new_test_ext_with_default, GenericAsset, Test, STAKING_ASSET_ID, TEST1_ASSET_ID,
		TEST2_ASSET_ID,
	};
	use frame_support::{assert_noop, assert_ok};
	use sp_runtime::traits::Zero;

	#[test]
	fn multi_accounting_minimum_balance() {
		new_test_ext_with_default().execute_with(|| {
			assert_eq!(
				<GenericAsset as MultiCurrencyAccounting>::minimum_balance(Some(TEST1_ASSET_ID)),
				3
			);
			assert_eq!(
				<GenericAsset as MultiCurrencyAccounting>::minimum_balance(Some(TEST2_ASSET_ID)),
				5
			);
			assert_eq!(
				<GenericAsset as MultiCurrencyAccounting>::minimum_balance(Some(STAKING_ASSET_ID)),
				1
			);
			// When currency is None, the minimum balance for the default currency will be returned.
			assert_eq!(<GenericAsset as MultiCurrencyAccounting>::minimum_balance(None), 1);
		});
	}

	#[test]
	fn multi_accounting_total_balance() {
		let (alice, asset_id, amount) = (&1, 16000, 100);
		new_test_ext_with_balance(asset_id, *alice, amount).execute_with(|| {
			assert_eq!(
				<GenericAsset as MultiCurrencyAccounting>::total_balance(alice, Some(asset_id)),
				amount
			);

			GenericAsset::reserve(asset_id, alice, amount / 2).ok();
			// total balance should include reserved balance
			assert_eq!(
				<GenericAsset as MultiCurrencyAccounting>::total_balance(alice, Some(asset_id)),
				amount
			);
		});
	}

	#[test]
	fn multi_accounting_free_balance() {
		let (alice, asset_id, amount) = (&1, 16000, 100);
		new_test_ext_with_balance(asset_id, *alice, amount).execute_with(|| {
			assert_eq!(
				<GenericAsset as MultiCurrencyAccounting>::free_balance(alice, Some(asset_id)),
				amount
			);

			GenericAsset::reserve(asset_id, alice, amount / 2).ok();
			// free balance should not include reserved balance
			assert_eq!(
				<GenericAsset as MultiCurrencyAccounting>::free_balance(alice, Some(asset_id)),
				amount / 2
			);
		});
	}

	#[test]
	fn multi_accounting_deposit_creating() {
		let (alice, asset_id, amount) = (&1, 16000, 100);
		new_test_ext_with_default().execute_with(|| {
			let imbalance = <GenericAsset as MultiCurrencyAccounting>::deposit_creating(alice, Some(asset_id), amount);
			// Check a positive imbalance of `amount` was created
			assert_eq!(imbalance.peek(), amount);
			// check free balance of asset has increased with `make_free_balance_be
			assert_eq!(GenericAsset::free_balance(asset_id, &alice), amount);
			// explitically drop `imbalance` so issuance is managed
			drop(imbalance);
			// check issuance of asset has increased with `make_free_balance_be`
			assert_eq!(GenericAsset::total_issuance(asset_id), amount);
		});
	}

	#[test]
	fn multi_accounting_deposit_into_existing() {
		let (alice, asset_id, amount) = (&1, 16000, 100);
		new_test_ext_with_default().execute_with(|| {
			let result =
				<GenericAsset as MultiCurrencyAccounting>::deposit_into_existing(alice, Some(asset_id), amount);
			// Check a positive imbalance of `amount` was created
			assert_eq!(result.unwrap().peek(), amount);
			// check free balance of asset has increased with `make_free_balance_be
			assert_eq!(GenericAsset::free_balance(asset_id, &alice), amount);
			// check issuance of asset has increased with `make_free_balance_be`
			assert_eq!(GenericAsset::total_issuance(asset_id), amount);
		});
	}

	#[test]
	fn multi_accounting_ensure_can_withdraw() {
		let (alice, asset_id, amount) = (1, 16000, 100);
		new_test_ext_with_balance(asset_id, alice, amount).execute_with(|| {
			assert_eq!(
				<GenericAsset as MultiCurrencyAccounting>::ensure_can_withdraw(
					&alice,
					Some(asset_id),
					amount / 2,
					WithdrawReasons::all(),
					amount / 2,
				),
				Ok(())
			);

			// check free balance has not decreased
			assert_eq!(GenericAsset::free_balance(asset_id, &alice), amount);
			// check issuance has not decreased
			assert_eq!(GenericAsset::total_issuance(asset_id), amount);
		});
	}

	#[test]
	fn multi_accounting_make_free_balance_be() {
		let (alice, asset_id, amount) = (1, 16000, 100);
		new_test_ext_with_default().execute_with(|| {
			// Issuance should be `0` initially
			assert!(GenericAsset::total_issuance(asset_id).is_zero());

			let result =
				<GenericAsset as MultiCurrencyAccounting>::make_free_balance_be(&alice, Some(asset_id), amount);
			// Check a positive imbalance of `amount` was created
			if let SignedImbalance::Positive(imb) = result {
				assert_eq!(imb.peek(), amount);
			} else {
				assert!(false);
			}
			// check free balance of asset has increased with `make_free_balance_be
			assert_eq!(GenericAsset::free_balance(asset_id, &alice), amount);
			// check issuance of asset has increased with `make_free_balance_be`
			assert_eq!(GenericAsset::total_issuance(asset_id), amount);
		});
	}

	#[test]
	fn multi_accounting_transfer() {
		let (alice, dest_id, asset_id, amount) = (1, 2, 16000, 100);

		new_test_ext_with_balance(asset_id, alice, amount).execute_with(|| {
			assert_eq!(
				<GenericAsset as MultiCurrencyAccounting>::transfer(
					&alice,
					&dest_id,
					Some(asset_id),
					amount,
					ExistenceRequirement::KeepAlive
				),
				Ok(())
			);
			assert_eq!(GenericAsset::free_balance(asset_id, &dest_id), amount);
		});
	}

	#[test]
	fn multi_accounting_withdraw() {
		let (alice, asset_id, amount) = (1, 16000, 100);
		new_test_ext_with_balance(asset_id, alice, amount).execute_with(|| {
			assert_eq!(GenericAsset::total_issuance(asset_id), amount);
			let result = <GenericAsset as MultiCurrencyAccounting>::withdraw(
				&alice,
				Some(asset_id),
				amount / 2,
				WithdrawReasons::all(),
				ExistenceRequirement::KeepAlive,
			);
			assert_eq!(result.unwrap().peek(), amount / 2);

			// check free balance of asset has decreased for the account
			assert_eq!(GenericAsset::free_balance(asset_id, &alice), amount / 2);
			// check global issuance has decreased for the asset
			assert_eq!(GenericAsset::total_issuance(asset_id), amount / 2);
		});
	}

	#[test]
	fn it_uses_default_currency_when_unspecified() {
		// Run through all the `MultiAccounting` functions checking that the default currency is
		// used when the Asset ID is left unspecified (`None`)
		let (alice, bob, amount) = (&1, &2, 100);
		new_test_ext_with_balance(16001, *alice, amount).execute_with(|| {
			assert_eq!(
				<GenericAsset as MultiCurrencyAccounting>::total_balance(alice, None),
				amount
			);

			assert_eq!(
				<GenericAsset as MultiCurrencyAccounting>::free_balance(alice, None),
				amount
			);

			// Mint `amount` of default currency into `alice`s account
			let _ = <GenericAsset as MultiCurrencyAccounting>::deposit_creating(alice, None, amount);
			// Check balance updated
			assert_eq!(
				<GenericAsset as MultiCurrencyAccounting>::total_balance(alice, None),
				amount + amount
			);
			assert_eq!(GenericAsset::total_issuance(16001), amount + amount);

			// Make free balance be equal to `amount` again
			let _ = <GenericAsset as MultiCurrencyAccounting>::make_free_balance_be(alice, None, amount);
			assert_eq!(
				<GenericAsset as MultiCurrencyAccounting>::free_balance(alice, None),
				amount
			);
			assert_eq!(GenericAsset::total_issuance(16001), amount);

			// Mint `amount` of the default currency into `alice`s account. Similar to `deposit_creating` above
			let _ = <GenericAsset as MultiCurrencyAccounting>::deposit_into_existing(alice, None, amount);
			// Check balance updated
			assert_eq!(
				<GenericAsset as MultiCurrencyAccounting>::total_balance(alice, None),
				amount + amount
			);
			assert_eq!(GenericAsset::total_issuance(16001), amount + amount);

			// transfer
			let _ = <GenericAsset as MultiCurrencyAccounting>::transfer(
				alice,
				bob,
				None,
				amount,
				ExistenceRequirement::KeepAlive,
			);
			assert_eq!(
				<GenericAsset as MultiCurrencyAccounting>::free_balance(alice, None),
				amount
			);
			assert_eq!(
				<GenericAsset as MultiCurrencyAccounting>::free_balance(bob, None),
				amount
			);
			assert_eq!(GenericAsset::total_issuance(16001), amount + amount);

			// ensure can withdraw
			assert!(<GenericAsset as MultiCurrencyAccounting>::ensure_can_withdraw(
				alice,
				None,
				amount,
				WithdrawReasons::all(),
				amount,
			)
			.is_ok());

			// withdraw
			let _ = <GenericAsset as MultiCurrencyAccounting>::withdraw(
				alice,
				None,
				amount / 2,
				WithdrawReasons::all(),
				ExistenceRequirement::KeepAlive,
			);
			assert_eq!(
				<GenericAsset as MultiCurrencyAccounting>::free_balance(alice, None),
				amount / 2
			);
		});
	}
	#[test]
	fn multi_accounting_transfer_more_than_free_balance_should_fail() {
		let (alice, dest_id, asset_id, amount) = (1, 2, 16000, 100);

		new_test_ext_with_balance(asset_id, alice, amount).execute_with(|| {
			assert_noop!(
				<GenericAsset as MultiCurrencyAccounting>::transfer(
					&alice,
					&dest_id,
					Some(asset_id),
					amount * 2,
					ExistenceRequirement::KeepAlive
				),
				Error::<Test>::InsufficientBalance,
			);
		});
	}

	#[test]
	fn multi_accounting_transfer_locked_funds_should_fail() {
		let (alice, dest_id, asset_id, amount) = (1, 2, 16000, 100);
		new_test_ext_with_balance(asset_id, alice, amount).execute_with(|| {
			// Lock alice's funds
			GenericAsset::set_lock(1u64.to_be_bytes(), asset_id, &alice, amount, WithdrawReasons::all());

			assert_noop!(
				<GenericAsset as MultiCurrencyAccounting>::transfer(
					&alice,
					&dest_id,
					Some(asset_id),
					amount,
					ExistenceRequirement::KeepAlive
				),
				Error::<Test>::LiquidityRestrictions,
			);
		});
	}

	#[test]
	fn multi_accounting_transfer_reserved_funds_should_fail() {
		let (alice, dest_id, asset_id, amount) = (1, 2, 16000, 100);
		new_test_ext_with_balance(asset_id, alice, amount).execute_with(|| {
			GenericAsset::reserve(asset_id, &alice, amount).ok();
			assert_noop!(
				<GenericAsset as MultiCurrencyAccounting>::transfer(
					&alice,
					&dest_id,
					Some(asset_id),
					amount,
					ExistenceRequirement::KeepAlive
				),
				Error::<Test>::InsufficientBalance,
			);
		});
	}

	#[test]
	fn multi_accounting_withdraw_more_than_free_balance_should_fail() {
		let (alice, asset_id, amount) = (1, 16000, 100);
		new_test_ext_with_balance(asset_id, alice, amount).execute_with(|| {
			assert_noop!(
				<GenericAsset as MultiCurrencyAccounting>::withdraw(
					&alice,
					Some(asset_id),
					amount * 2,
					WithdrawReasons::all(),
					ExistenceRequirement::KeepAlive
				),
				Error::<Test>::InsufficientBalance,
			);
		});
	}

	#[test]
	fn multi_accounting_withdraw_locked_funds_should_fail() {
		let (alice, asset_id, amount) = (1, 16000, 100);
		new_test_ext_with_balance(asset_id, alice, amount).execute_with(|| {
			// Lock alice's funds
			GenericAsset::set_lock(1u64.to_be_bytes(), asset_id, &alice, amount, WithdrawReasons::all());

			assert_noop!(
				<GenericAsset as MultiCurrencyAccounting>::withdraw(
					&alice,
					Some(asset_id),
					amount,
					WithdrawReasons::all(),
					ExistenceRequirement::KeepAlive
				),
				Error::<Test>::LiquidityRestrictions,
			);
		});
	}

	#[test]
	fn multi_accounting_withdraw_reserved_funds_should_fail() {
		let (alice, asset_id, amount) = (1, 16000, 100);
		new_test_ext_with_balance(asset_id, alice, amount).execute_with(|| {
			// Reserve alice's funds
			GenericAsset::reserve(asset_id, &alice, amount).ok();

			assert_noop!(
				<GenericAsset as MultiCurrencyAccounting>::withdraw(
					&alice,
					Some(asset_id),
					amount,
					WithdrawReasons::all(),
					ExistenceRequirement::KeepAlive
				),
				Error::<Test>::InsufficientBalance,
			);
		});
	}

	#[test]
	fn multi_accounting_make_free_balance_edge_cases() {
		let (alice, asset_id) = (&1, 16000);
		new_test_ext_with_default().execute_with(|| {
			let max_value = u64::max_value();
			let min_value = Zero::zero();

			let _ = <GenericAsset as MultiCurrencyAccounting>::make_free_balance_be(alice, Some(asset_id), max_value);
			// Check balance updated
			assert_eq!(GenericAsset::total_issuance(asset_id), max_value);
			assert_eq!(
				<GenericAsset as MultiCurrencyAccounting>::free_balance(alice, Some(asset_id)),
				max_value
			);

			let _ = <GenericAsset as MultiCurrencyAccounting>::make_free_balance_be(alice, Some(asset_id), min_value);
			// Check balance updated
			assert_eq!(GenericAsset::total_issuance(asset_id), min_value);
			assert_eq!(
				<GenericAsset as MultiCurrencyAccounting>::free_balance(alice, Some(asset_id)),
				min_value
			);
		})
	}

	#[test]
	fn reserve() {
		let (alice, asset_id) = (&1, 16000);
		new_test_ext_with_default().execute_with(|| {
			let _ = <GenericAsset as MultiCurrencyAccounting>::make_free_balance_be(alice, Some(asset_id), 100_000);
			assert_ok!(<GenericAsset as MultiCurrencyAccounting>::reserve(
				alice,
				Some(asset_id),
				50_000
			));
			assert_eq!(GenericAsset::free_balance(asset_id, alice), 50_000);
			assert_eq!(GenericAsset::reserved_balance(asset_id, alice), 50_000);
		})
	}

	#[test]
	fn repatriate_reserved() {
		let (alice, asset_id) = (&1, 16000);
		let beneficiary = &2;
		new_test_ext_with_default().execute_with(|| {
			let _ = <GenericAsset as MultiCurrencyAccounting>::make_free_balance_be(alice, Some(asset_id), 100_000);
			assert_ok!(<GenericAsset as MultiCurrencyAccounting>::reserve(
				alice,
				Some(asset_id),
				50_000
			));
			assert!(GenericAsset::free_balance(asset_id, beneficiary).is_zero());
			assert_ok!(<GenericAsset as MultiCurrencyAccounting>::repatriate_reserved(
				alice,
				Some(asset_id),
				beneficiary,
				50_000
			));

			assert_eq!(GenericAsset::free_balance(asset_id, alice), 50_000);
			assert!(GenericAsset::reserved_balance(asset_id, alice).is_zero());
			assert_eq!(GenericAsset::free_balance(asset_id, beneficiary), 50_000);
		})
	}

	#[test]
	fn unreserve() {
		let (alice, asset_id) = (&1, 16000);
		new_test_ext_with_default().execute_with(|| {
			let _ = <GenericAsset as MultiCurrencyAccounting>::make_free_balance_be(alice, Some(asset_id), 100_000);
			assert_ok!(<GenericAsset as MultiCurrencyAccounting>::reserve(
				alice,
				Some(asset_id),
				50_000
			));
			assert_eq!(
				<GenericAsset as MultiCurrencyAccounting>::unreserve(alice, Some(asset_id), 40_000),
				0
			);

			assert_eq!(GenericAsset::free_balance(asset_id, alice), 90_000);
			assert_eq!(GenericAsset::reserved_balance(asset_id, alice), 10_000);
		})
	}
}
