//! Handles all transaction fee related operations

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

use parity_codec::Codec;
use runtime_primitives::traits::{As, CheckedAdd, CheckedSub, MaybeDebug, Zero};
use support::{
	additional_traits::ChargeFee,
	decl_event, decl_fee, decl_module, decl_storage,
	dispatch::Result,
	for_each_tuple,
	traits::{Currency, ExistenceRequirement, WithdrawReason},
	StorageMap,
};
use system;

mod mock;
mod tests;

pub type AssetOf<T> = <<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;

pub trait OnFeeCharged<Amount> {
	fn on_fee_charged(fee: &Amount);
}

pub trait CheckCallFee<Amount, Call> {
	/// Return the associated fee amount for the given runtime module `call`
	fn check_call_fee(call: &Call) -> Amount;
}

decl_fee!(
	pub enum Fee {
		/// The extrinsic base fee
		Base,
		/// The extrinsic per byte fee
		Bytes,
	}
);

macro_rules! impl_fee_charged {
	() => (
		impl<T> OnFeeCharged<T> for () {
			fn on_fee_charged(_: &T) {}
		}
	);

	( $($t:ident)* ) => {
		impl<T, $($t: OnFeeCharged<T>),*> OnFeeCharged<T> for ($($t,)*) {
			fn on_fee_charged(fee: &T) {
				$($t::on_fee_charged(fee);)*
			}
		}
	}
}

for_each_tuple!(impl_fee_charged);

pub type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;

/// A trait which enables buying some fee asset using another asset.
/// It is targeted at the CENNZX Spot exchange and the CennznetExtrinsic format.
pub trait BuyFeeAsset<AccountId, Balance> {
	type FeeExchange;
	/// Buy `amount` of fee asset for `who` using asset info from `fee_exchange.
	/// Note: It does not charge the fee asset, that is left to a `ChargeFee` implementation
	fn buy_fee_asset(who: &AccountId, amount: Balance, fee_exchange: &Self::FeeExchange) -> Result;
}

impl<AccountId, Balance> BuyFeeAsset<AccountId, Balance> for () {
	type FeeExchange = ();
	fn buy_fee_asset(_: &AccountId, _: Balance, _: &Self::FeeExchange) -> Result {
		Ok(())
	}
}

pub trait Trait: system::Trait {
	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;

	/// The Currency used for assets transfers.
	type Currency: Currency<Self::AccountId>;

	/// A function which buys fee asset if signalled by the extrinsic
	type BuyFeeAsset: BuyFeeAsset<Self::AccountId, BalanceOf<Self>>;

	/// A function invoked in `on_finalise`, with accumulated fees of the whole block.
	type OnFeeCharged: OnFeeCharged<AssetOf<Self>>;

	/// An extrinsic fee type. It is also a key for the fee registry
	type Fee: Codec + PartialEq + Clone + MaybeDebug; // Like `Parameter` but we don't support `Eq`
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event<T>() = default;

		fn on_finalize() {
			let extrinsic_count = <system::Module<T>>::extrinsic_count();
			// the accumulated fee of the whole block
			let mut block_fee = <AssetOf<T>>::sa(0);
			(0..extrinsic_count).for_each(|index| {
				// Deposit `Charged` event if some amount of fee charged.
				let fee = <CurrentTransactionFee<T>>::take(index);
				if !fee.is_zero() {
					block_fee += fee;
					Self::deposit_event(RawEvent::Charged(index, fee));
				}
			});
			T::OnFeeCharged::on_fee_charged(&block_fee);
		}

		/// Set a new associated cost for the given fee type
		fn set_fee(fee: T::Fee, new_amount: AssetOf<T>) {
			FeeRegistry::<T>::mutate(fee, |amount| *amount = new_amount);
	}
}
}

decl_event!(
	pub enum Event<T>
	where
		Amount = AssetOf<T>,
	{
		/// Fee charged (extrinsic_index, fee_amount)
		Charged(u32, Amount),
	}
);

decl_storage! {
	trait Store for Module<T: Trait> as Fees {
		/// The central register of extrinsic fees. It maps fee types to
		/// their associated cost.
		FeeRegistry get(fee_registry) config(): map T::Fee => AssetOf<T>;

		/// The `extrinsic_index => accumulated_fees` map, containing records to
		/// track the overall charged fees for each transaction.
		///
		/// All records should be removed at finalise stage.
		CurrentTransactionFee get(current_transaction_fee): map u32 => AssetOf<T>;
	}
}

impl<T: Trait> ChargeFee<T::AccountId> for Module<T> {
	type Amount = AssetOf<T>;

	fn charge_fee(transactor: &T::AccountId, amount: AssetOf<T>) -> Result {
		let extrinsic_index = <system::Module<T>>::extrinsic_index().ok_or_else(|| "no extrinsic index found")?;
		let current_fee = Self::current_transaction_fee(extrinsic_index);
		let new_fee = current_fee
			.checked_add(&amount)
			.ok_or_else(|| "fee got overflow after charge")?;

		T::Currency::withdraw(
			transactor,
			amount,
			WithdrawReason::TransactionPayment,
			ExistenceRequirement::KeepAlive,
		)?;

		<CurrentTransactionFee<T>>::insert(extrinsic_index, new_fee);
		Ok(())
	}

	fn refund_fee(transactor: &T::AccountId, amount: AssetOf<T>) -> Result {
		let extrinsic_index = <system::Module<T>>::extrinsic_index().ok_or_else(|| "no extrinsic index found")?;
		let current_fee = Self::current_transaction_fee(extrinsic_index);
		let new_fee = current_fee
			.checked_sub(&amount)
			.ok_or_else(|| "fee got underflow after refund")?;

		T::Currency::deposit_into_existing(transactor, amount)?;

		<CurrentTransactionFee<T>>::insert(extrinsic_index, new_fee);
		Ok(())
	}
}
