//!
//! Extra CENNZ-X trait + implementations
//!
use super::{Module, Trait};
use rstd::{marker::PhantomData, mem, prelude::*};
use runtime_primitives::traits::{As, Hash};
use support::{
	dispatch::Result,
	traits::{ArithmeticType, TransferAsset, WithdrawReason},
};

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

impl<T: Trait> ArithmeticType for Module<T> {
	type Type = T::Balance;
}

impl<T: Trait> TransferAsset<T::AccountId> for Module<T> {
	type Amount = T::Balance;

	/// The typical scenario is: user needs to pay X amount of fee asset to some account Y (i.e output only)
	fn transfer(from: &T::AccountId, to: &T::AccountId, fee_amount: T::Balance) -> Result {
		// TODO: Hard coded to use spending asset ID
		let fee_asset_id: T::AssetId = <generic_asset::Module<T>>::spending_asset_id();
		let xt = Self::current_extrinsic()?;

		if xt.fee_exchange.is_none() {
			// User is expected to have some fee asset balance so try a GA transfer directly.
			return <generic_asset::Module<T>>::make_transfer(&fee_asset_id, from, to, fee_amount);
		}

		let fee_exchange = xt.fee_exchange.unwrap();
		Self::make_asset_to_asset_output(
			from,
			to,
			&T::AssetId::sa(u64::from(fee_exchange.asset_id)), // TODO: hack `T::AssetID` missing `As<u32>` impl
			&fee_asset_id,
			fee_amount,
			T::Balance::sa(fee_exchange.max_payment),
			Self::fee_rate(),
		)
		.map(|_| ())
	}

	fn withdraw(who: &T::AccountId, value: T::Balance, _reason: WithdrawReason) -> Result {
		// TODO: pipe through to a real implementation
		Ok(())
	}

	fn deposit(who: &T::AccountId, value: T::Balance) -> Result {
		// TODO: pipe through to a real implementation
		Ok(())
	}
}

#[cfg(test)]
pub(crate) mod impl_tests {
	use super::*;
	use crate::tests::{Call, CennzXSpot, ExtBuilder, Origin, Test};
	use cennznet_primitives::{CennznetExtrinsic, FeeExchange, Index, Signature};
	use parity_codec::Encode;
	use runtime_io::with_externalities;
	use substrate_primitives::H256;

	/// A mock runtime module that does nothing
	/// Used for testing CennzExtrinsic with TransferAsset
	pub type MockModule = mock_module::Module<Test>;
	pub mod mock_module {
		use support::dispatch::Result;
		pub trait Trait {
			type Origin;
			type BlockNumber;
		}
		decl_event!(
			pub enum Event {
				Test,
			}
		);
		decl_module! {
			pub struct Module<T: Trait> for enum Call where origin: T::Origin {
				fn do_nothing(_origin) -> Result { unreachable!() }
			}
		}
	}
	type MockExtrinsic<T> = CennznetExtrinsic<
		<T as system::Trait>::AccountId,
		<T as system::Trait>::AccountId,
		Index,
		<T as Trait>::Call,
		Signature,
		u128,
	>;

	impl mock_module::Trait for Test {
		type Origin = Origin;
		type BlockNumber = u64;
	}

	const CORE_ASSET: u32 = 0;
	const OTHER_ASSET: u32 = 1;
	// TODO: Hard coded fee asset ID to match `TransferAsset::transfer` implementation
	const FEE_ASSET: u32 = 10;

	#[test]
	fn transfer_asset_with_fee_v1() {
		with_externalities(&mut ExtBuilder::default().build(), || {
			with_exchange!(CORE_ASSET => 1000, OTHER_ASSET => 1000);
			with_exchange!(CORE_ASSET => 1000, FEE_ASSET => 1000);

			let mut extrinsic = MockExtrinsic::<Test>::new_unsigned(Call::MockModule(mock_module::Call::do_nothing()));
			extrinsic.fee_exchange = Some(FeeExchange::new(OTHER_ASSET, 1_000_000));
			// Seed the test extrinsic
			<system::Module<Test>>::set_extrinsic_index(0);
			<system::Module<Test>>::note_extrinsic(Encode::encode(&extrinsic));

			let user = with_account!(CORE_ASSET => 0, OTHER_ASSET => 100);
			let fee_collector = with_account!("bob", CORE_ASSET => 0, FEE_ASSET => 0);

			assert_ok!(<CennzXSpot as TransferAsset<H256>>::transfer(&user, &fee_collector, 51));

			assert_exchange_balance_eq!(CORE_ASSET => 946, OTHER_ASSET => 1058);
			assert_exchange_balance_eq!(CORE_ASSET => 1054, FEE_ASSET => 949);

			assert_balance_eq!(user, CORE_ASSET => 0);
			assert_balance_eq!(user, OTHER_ASSET => 42);

			assert_balance_eq!(fee_collector, CORE_ASSET => 0);
			assert_balance_eq!(fee_collector, FEE_ASSET => 51);
		});
	}

	#[test]
	fn transfer_asset_with_ga() {
		with_externalities(&mut ExtBuilder::default().build(), || {
			let extrinsic = MockExtrinsic::<Test>::new_unsigned(Call::MockModule(mock_module::Call::do_nothing()));
			// Seed the test extrinsic
			<system::Module<Test>>::set_extrinsic_index(0);
			<system::Module<Test>>::note_extrinsic(Encode::encode(&extrinsic));

			let user = with_account!(CORE_ASSET => 0, FEE_ASSET => 100);
			let fee_collector = with_account!("bob", CORE_ASSET => 0, FEE_ASSET => 0);

			assert_ok!(<CennzXSpot as TransferAsset<H256>>::transfer(&user, &fee_collector, 51));

			assert_balance_eq!(user, CORE_ASSET => 0);
			assert_balance_eq!(user, FEE_ASSET => 49);

			assert_balance_eq!(fee_collector, CORE_ASSET => 0);
			assert_balance_eq!(fee_collector, FEE_ASSET => 51);
		});
	}

	#[test]
	fn u64_to_bytes_works() {
		assert_eq!(
			<ExchangeAddressGenerator<Test>>::u64_to_bytes(80_000),
			[128, 56, 1, 0, 0, 0, 0, 0]
		);
	}

	#[test]
	fn current_extrinsic_bad_encoding() {
		with_externalities(&mut ExtBuilder::default().build(), || {
			<system::Module<Test>>::set_extrinsic_index(0);
			<system::Module<Test>>::note_extrinsic(vec![1, 2, 3]);
			assert_err!(CennzXSpot::current_extrinsic(), "Got extrinsic with bad encoding");
		});
	}

	#[test]
	fn current_extrinsic() {
		with_externalities(&mut ExtBuilder::default().build(), || {
			let extrinsic = MockExtrinsic::<Test>::new_unsigned(Call::MockModule(mock_module::Call::do_nothing()));
			<system::Module<Test>>::set_extrinsic_index(0);
			<system::Module<Test>>::note_extrinsic(Encode::encode(&extrinsic));

			assert_ok!(CennzXSpot::current_extrinsic(), extrinsic);
		});
	}

}
