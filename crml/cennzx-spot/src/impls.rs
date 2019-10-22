// Copyright 2019 Centrality Investments Limited
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
//!
//! Extra CENNZX-Spot traits + implementations
//!
use super::{Module, Trait};
use cennznet_primitives::FeeExchange;
use fees::BuyFeeAsset;
use primitives::crypto::{UncheckedFrom, UncheckedInto};
use rstd::{marker::PhantomData, mem, prelude::*};
use runtime_primitives::traits::Hash;
use support::dispatch::Result;

/// A function that generates an `AccountId` for a CENNZX-SPOT exchange / (core, asset) pair
pub trait ExchangeAddressFor<AssetId: Sized, AccountId: Sized> {
	fn exchange_address_for(core_asset_id: AssetId, asset_id: AssetId) -> AccountId;
}

// A CENNZX-Spot exchange address generator implementation
pub struct ExchangeAddressGenerator<T: Trait>(PhantomData<T>);

impl<T: Trait> ExchangeAddressFor<T::AssetId, T::AccountId> for ExchangeAddressGenerator<T>
where
	T::AccountId: UncheckedFrom<T::Hash> + AsRef<[u8]>,
{
	/// Generates an exchange address for the given core / asset pair
	fn exchange_address_for(core_asset_id: T::AssetId, asset_id: T::AssetId) -> T::AccountId {
		let mut buf = Vec::new();
		buf.extend_from_slice(b"cennz-x-spot:");
		buf.extend_from_slice(&u64_to_bytes(core_asset_id.into()));
		buf.extend_from_slice(&u64_to_bytes(asset_id.into()));

		T::Hashing::hash(&buf[..]).unchecked_into()
	}
}

fn u64_to_bytes(x: u64) -> [u8; 8] {
	unsafe { mem::transmute(x.to_le()) }
}

impl<T: Trait> BuyFeeAsset<T::AccountId, T::Balance> for Module<T> {
	type FeeExchange = FeeExchange<T::Balance>;
	/// Use the CENNZX-Spot exchange to seamlessly buy fee asset
	fn buy_fee_asset(who: &T::AccountId, amount: T::Balance, exchange_op: &FeeExchange<T::Balance>) -> Result {
		// TODO: Hard coded to use spending asset ID
		let fee_asset_id: T::AssetId = <generic_asset::Module<T>>::spending_asset_id();
		Self::make_asset_swap_output(
			&who,
			&who,
			&T::AssetId::from(exchange_op.asset_id),
			&fee_asset_id,
			amount,
			exchange_op.max_payment,
			Self::fee_rate(),
		)
		.map(|_| ())
		.map_err(|_| "Failed to charge transaction fees during conversion")
	}
}

#[cfg(test)]
pub(crate) mod impl_tests {
	use super::*;
	use crate::tests::{CennzXSpot, ExtBuilder, Test};
	use cennznet_primitives::FeeExchange;
	use primitives::H256;
	use runtime_io::with_externalities;

	const CORE_ASSET: u32 = 0;
	const OTHER_ASSET: u32 = 1;
	// TODO: Hard coded fee asset ID to match `TransferAsset::transfer` implementation
	const FEE_ASSET: u32 = 10;

	#[test]
	fn buy_fee_asset() {
		with_externalities(&mut ExtBuilder::default().build(), || {
			with_exchange!(CORE_ASSET => 1000, OTHER_ASSET => 1000);
			with_exchange!(CORE_ASSET => 1000, FEE_ASSET => 1000);

			let user = with_account!(CORE_ASSET => 0, OTHER_ASSET => 100);

			assert_ok!(<CennzXSpot as BuyFeeAsset<_, _>>::buy_fee_asset(
				&user,
				51,
				&FeeExchange::new(OTHER_ASSET, 1_000_000),
			));

			assert_exchange_balance_eq!(CORE_ASSET => 946, OTHER_ASSET => 1058);
			assert_exchange_balance_eq!(CORE_ASSET => 1054, FEE_ASSET => 949);

			assert_balance_eq!(user, CORE_ASSET => 0);
			assert_balance_eq!(user, OTHER_ASSET => 42);
		});
	}

	#[test]
	fn buy_fee_asset_insufficient_trade_asset() {
		with_externalities(&mut ExtBuilder::default().build(), || {
			let user = with_account!(CORE_ASSET => 0, OTHER_ASSET => 10);

			assert_err!(
				<CennzXSpot as BuyFeeAsset<_, _>>::buy_fee_asset(&user, 51, &FeeExchange::new(OTHER_ASSET, 1_000_000)),
				"Failed to charge transaction fees during conversion"
			);

			assert_balance_eq!(user, CORE_ASSET => 0);
			assert_balance_eq!(user, OTHER_ASSET => 10);
		});
	}

	#[test]
	fn u64_to_bytes_works() {
		assert_eq!(u64_to_bytes(80_000), [128, 56, 1, 0, 0, 0, 0, 0]);
	}
}
