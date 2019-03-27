//! Handles all transaction fee related operations

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

use runtime_primitives::traits::{As, CheckedAdd, CheckedMul, CheckedSub, Zero};
use support::{
	additional_traits::FeeAmounts,
	decl_event, decl_module, decl_storage,
	dispatch::Result,
	for_each_tuple,
	traits::{ArithmeticType, ChargeBytesFee, ChargeFee, TransferAsset, WithdrawReason},
	StorageMap,
};
use system;

mod mock;
mod tests;

type AssetOf<T> = <<T as Trait>::TransferAsset as ArithmeticType>::Type;

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

pub trait Trait: system::Trait {
	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;

	/// A function does the asset transfer between accounts
	type TransferAsset: ArithmeticType + TransferAsset<Self::AccountId, Amount = AssetOf<Self>>;

	/// A function invoked in `on_finalise`, with accumulated fees of the whole block.
	type OnFeeCharged: OnFeeCharged<AssetOf<Self>>;
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event<T>() = default;

		fn on_finalise() {
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
		Amount = AssetOf<T>
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

impl<T: Trait> ChargeBytesFee<T::AccountId> for Module<T> {
	fn charge_base_bytes_fee(transactor: &T::AccountId, encoded_len: usize) -> Result {
		let bytes_fee = Self::transaction_byte_fee()
			.checked_mul(&<AssetOf<T> as As<u64>>::sa(encoded_len as u64))
			.ok_or_else(|| "bytes fee overflow")?;
		let overall = Self::transaction_base_fee()
			.checked_add(&bytes_fee)
			.ok_or_else(|| "bytes fee overflow")?;
		Self::charge_fee(transactor, overall)
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

		T::TransferAsset::withdraw(transactor, amount, WithdrawReason::TransactionPayment)?;

		<CurrentTransactionFee<T>>::insert(extrinsic_index, new_fee);
		Ok(())
	}

	fn refund_fee(transactor: &T::AccountId, amount: AssetOf<T>) -> Result {
		let extrinsic_index = <system::Module<T>>::extrinsic_index().ok_or_else(|| "no extrinsic index found")?;
		let current_fee = Self::current_transaction_fee(extrinsic_index);
		let new_fee = current_fee
			.checked_sub(&amount)
			.ok_or_else(|| "fee got underflow after refund")?;

		T::TransferAsset::deposit(transactor, amount)?;

		<CurrentTransactionFee<T>>::insert(extrinsic_index, new_fee);
		Ok(())
	}
}

impl<T: Trait> FeeAmounts for Module<T> {
	type Amount = AssetOf<T>;

	fn base_fee() -> Self::Amount {
		Self::transaction_base_fee()
	}

	fn byte_fee() -> Self::Amount {
		Self::transaction_byte_fee()
	}
}
