//! Handles all transaction fee related operations

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

use cennznet_primitives::{Balance, CheckedCennznetExtrinsic, FeeExchange, Index};
use parity_codec::HasCompact;
use runtime_primitives::traits::{As, CheckedAdd, CheckedMul, CheckedSub, Zero};
use support::{
	additional_traits::ChargeFee,
	decl_event, decl_module, decl_storage,
	dispatch::{Dispatchable, Result},
	for_each_tuple,
	traits::{Currency, ExistenceRequirement, MakePayment, WithdrawReason},
	Parameter, StorageMap,
};
use system;

mod mock;
mod tests;

type AssetOf<T> = <<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;
pub type CheckedExtrinsicOf<T> =
	CheckedCennznetExtrinsic<<T as system::Trait>::AccountId, Index, <T as Trait>::Call, Balance>;

pub trait OnFeeCharged<Amount> {
	fn on_fee_charged(fee: &Amount);
}

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

/// A trait which enables buying some fee asset using another asset.
/// It is targeted at the CENNZX Spot exchange and the CennznetExtrinsic format.
pub trait BuyFeeAsset<AccountId, Balance: HasCompact> {
	/// Buy `amount` of fee asset for `who` using asset info from `fee_exchange.
	/// Note: It does not charge the fee asset, that is left to a `ChargeFee` implementation
	fn buy_fee_asset(who: &AccountId, amount: Balance, fee_exchange: &FeeExchange<Balance>) -> Result;
}

pub trait Trait: system::Trait {
	/// Needed for to give `<T as Trait>::Call` for `CheckedExtrinsicOf`
	type Call: Parameter + Dispatchable<Origin = <Self as system::Trait>::Origin>;

	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;

	/// The Currency used for assets transfers.
	type Currency: Currency<Self::AccountId>;

	/// A function which buys fee asset if signalled by the extrinsic
	type BuyFeeAsset: BuyFeeAsset<Self::AccountId, Balance>;

	/// A function invoked in `on_finalise`, with accumulated fees of the whole block.
	type OnFeeCharged: OnFeeCharged<AssetOf<Self>>;
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
		/// The fee to be paid for making a transaction; the base.
		pub TransactionBaseFee get(transaction_base_fee) config(): AssetOf<T>;
		/// The fee to be paid for making a transaction; the per-byte portion.
		pub TransactionByteFee get(transaction_byte_fee) config(): AssetOf<T>;

		/// The `extrinsic_index => accumulated_fees` map, containing records to
		/// track the overall charged fees for each transaction.
		///
		/// All records should be removed at finalise stage.
		CurrentTransactionFee get(current_transaction_fee): map u32 => AssetOf<T>;
	}
}

impl<T: Trait> MakePayment<T::AccountId, CheckedExtrinsicOf<T>> for Module<T> {
	fn make_payment(transactor: &T::AccountId, encoded_len: usize, extrinsic: &CheckedExtrinsicOf<T>) -> Result {
		let bytes_fee = Self::transaction_byte_fee()
			.checked_mul(&<AssetOf<T> as As<u64>>::sa(encoded_len as u64))
			.ok_or_else(|| "bytes fee overflow")?;
		let total_fee = Self::transaction_base_fee()
			.checked_add(&bytes_fee)
			.ok_or_else(|| "bytes fee overflow")?;

		if let Some(ref op) = &extrinsic.fee_exchange {
			let _ = T::BuyFeeAsset::buy_fee_asset(transactor, total_fee.as_() as u128, op)?;
		}

		Self::charge_fee(transactor, total_fee)
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
