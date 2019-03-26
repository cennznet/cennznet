//!
//! Extra CENNZ-X trait + implementations
//!
use super::Trait;
use rstd::{marker::PhantomData, mem, prelude::*};
use runtime_primitives::traits::{As, Hash};

/// A function that generates an `AccountId` for a CENNZ-X exchange / (core, asset) pair
pub trait ExchangeAddressFor<AssetId: Sized, AccountId: Sized> {
	fn exchange_address_for(core_asset_id: AssetId, asset_id: AssetId) -> AccountId;
}

// A CENNZ-X exchange address generator implementation
pub struct ExchangeAddressGenerator<T: Trait>(PhantomData<T>);

impl<T: Trait> ExchangeAddressFor<T::AssetId, T::AccountId> for ExchangeAddressGenerator<T>
where
	T::AccountId: From<T::Hash> + AsRef<[u8]>,
{
	/// Generates an exchange address for the given core / asset pair
	fn exchange_address_for(core_asset_id: T::AssetId, asset_id: T::AssetId) -> T::AccountId {
		let mut buf = Vec::new();
		buf.extend_from_slice(b"cennz-x-spot:");
		buf.extend_from_slice(&Self::u64_to_bytes(As::as_(core_asset_id)));
		buf.extend_from_slice(&Self::u64_to_bytes(As::as_(asset_id)));

		T::Hashing::hash(&buf[..]).into()
	}
}

impl<T: Trait> ExchangeAddressGenerator<T> {
	/// Convert a `u64` into its byte array representation
	fn u64_to_bytes(x: u64) -> [u8; 8] {
		unsafe { mem::transmute(x.to_le()) }
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tests::Test;

	#[test]
	fn u64_to_bytes_works() {
		assert_eq!(
			<ExchangeAddressGenerator<Test>>::u64_to_bytes(80_000),
			[128, 56, 1, 0, 0, 0, 0, 0]
		);
	}

}
