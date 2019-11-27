// Copyright (C) 2019 Centrality Investments Limited
// This file is part of CENNZnet.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.
//!
//! Extra CENNZX-Spot traits + implementations
//!
use super::Trait;
use primitives::crypto::{UncheckedFrom, UncheckedInto};
use rstd::{marker::PhantomData, mem, prelude::*};
use runtime_primitives::traits::Hash;

/// A function that generates an `AccountId` for a CENNZX-SPOT exchange / (core, asset) pair
pub trait ExchangeAddressFor<AssetId: Sized, AccountId: Sized> {
	fn exchange_address_for(core_asset_id: AssetId, asset_id: AssetId) -> AccountId;
}

// A CENNZX-Spot exchange address generator implementation
pub struct ExchangeAddressGenerator<T: Trait>(PhantomData<T>);

impl<T: Trait> ExchangeAddressFor<T::AssetId, T::AccountId> for ExchangeAddressGenerator<T>
where
	T::AccountId: UncheckedFrom<T::Hash> + AsRef<[u8]>,
	T::AssetId: Into<u64>,
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

#[cfg(test)]
pub(crate) mod impl_tests {
	use super::*;

	#[test]
	fn u64_to_bytes_works() {
		assert_eq!(u64_to_bytes(80_000), [128, 56, 1, 0, 0, 0, 0, 0]);
	}
}
