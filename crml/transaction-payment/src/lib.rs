// Copyright 2018-2020 Parity Technologies (UK) Ltd. and Centrality Investments Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

//! # Transaction Payment Module
//!
//! This module provides the basic logic needed to pay the absolute minimum amount needed for a
//! transaction to be included. This includes:
//!   - _weight fee_: A fee proportional to amount of weight a transaction consumes.
//!   - _length fee_: A fee proportional to the encoded length of the transaction.
//!   - _tip_: An optional tip. Tip increases the priority of the transaction, giving it a higher
//!     chance to be included by the transaction queue.
//!
//! Additionally, this module allows one to configure:
//!   - The mapping between one unit of weight to one unit of fee via [`WeightToFee`].
//!   - A means of updating the fee for the next block, via defining a multiplier, based on the
//!     final state of the chain at the end of the previous block. This can be configured via
//!     [`FeeMultiplierUpdate`]

#![cfg_attr(not(feature = "std"), no_std)]

use cennznet_primitives::{
	traits::{BuyFeeAsset, IsGasMeteredCall},
	types::FeeExchange,
};
use codec::{Decode, Encode};
use frame_support::{
	decl_module, decl_storage, storage,
	traits::{Currency, ExistenceRequirement, Get, OnUnbalanced, WithdrawReason},
	weights::{DispatchInfo, GetDispatchInfo, Weight},
	Parameter,
};
use pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo;
use sp_runtime::{
	traits::{
		CheckedSub, Convert, MaybeSerializeDeserialize, Member, SaturatedConversion, Saturating, SignedExtension,
		SimpleArithmetic, Zero,
	},
	transaction_validity::{
		InvalidTransaction, TransactionPriority, TransactionValidity, TransactionValidityError, ValidTransaction,
	},
	Fixed64,
};
use sp_std::{fmt::Debug, prelude::*};

#[cfg(test)]
mod mock;

type Multiplier = Fixed64;
type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as frame_system::Trait>::AccountId>>::Balance;
type NegativeImbalanceOf<T> =
	<<T as Trait>::Currency as Currency<<T as frame_system::Trait>::AccountId>>::NegativeImbalance;

pub const GAS_FEE_EXCHANGE_KEY: &[u8] = b"gas-fee-exchange-key";

pub trait Trait: frame_system::Trait {
	/// The units in which we record balances.
	type Balance: Parameter + Member + SimpleArithmetic + Default + Copy + MaybeSerializeDeserialize + Debug;

	/// The arithmetic type of asset identifier.
	type AssetId: Parameter + Member + SimpleArithmetic + Default + Copy;

	/// The currency type in which fees will be paid.
	type Currency: Currency<Self::AccountId> + Send + Sync;

	/// Handler for the unbalanced reduction when taking transaction fees.
	type OnTransactionPayment: OnUnbalanced<NegativeImbalanceOf<Self>>;

	/// The fee to be paid for making a transaction; the base.
	type TransactionBaseFee: Get<BalanceOf<Self>>;

	/// The fee to be paid for making a transaction; the per-byte portion.
	type TransactionByteFee: Get<BalanceOf<Self>>;

	/// Convert a weight value into a deductible fee based on the currency type.
	type WeightToFee: Convert<Weight, BalanceOf<Self>>;

	/// Update the multiplier of the next block, based on the previous block's weight.
	type FeeMultiplierUpdate: Convert<(Weight, Multiplier), Multiplier>;

	/// A service which will buy fee assets if signalled by the extrinsic.
	type BuyFeeAsset: BuyFeeAsset<
		AccountId = Self::AccountId,
		Balance = BalanceOf<Self>,
		FeeExchange = FeeExchange<Self::AssetId, BalanceOf<Self>>,
	>;

	/// Something which can report whether a call is gas metered
	type GasMeteredCallResolver: IsGasMeteredCall<Call = <Self as frame_system::Trait>::Call>;
}

decl_storage! {
	trait Store for Module<T: Trait> as Balances {
		NextFeeMultiplier get(fn next_fee_multiplier): Multiplier = Multiplier::from_parts(0);
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		/// The fee to be paid for making a transaction; the base.
		const TransactionBaseFee: BalanceOf<T> = T::TransactionBaseFee::get();

		/// The fee to be paid for making a transaction; the per-byte portion.
		const TransactionByteFee: BalanceOf<T> = T::TransactionByteFee::get();

		fn on_finalize() {
			let current_weight = <frame_system::Module<T>>::all_extrinsics_weight();
			NextFeeMultiplier::mutate(|fm| {
				*fm = T::FeeMultiplierUpdate::convert((current_weight, *fm))
			});
		}
	}
}

impl<T: Trait> Module<T> {
	/// Query the data that we know about the fee of a given `call`.
	///
	/// As this module is not and cannot be aware of the internals of a signed extension, it only
	/// interprets them as some encoded value and takes their length into account.
	///
	/// All dispatchables must be annotated with weight and will have some fee info. This function
	/// always returns.
	// NOTE: we can actually make it understand `ChargeTransactionPayment`, but would be some hassle
	// for sure. We have to make it aware of the index of `ChargeTransactionPayment` in `Extra`.
	// Alternatively, we could actually execute the tx's per-dispatch and record the balance of the
	// sender before and after the pipeline.. but this is way too much hassle for a very very little
	// potential gain in the future.
	pub fn query_info<Extrinsic: GetDispatchInfo>(
		unchecked_extrinsic: Extrinsic,
		len: u32,
	) -> RuntimeDispatchInfo<BalanceOf<T>>
	where
		T: Send + Sync,
		BalanceOf<T>: Send + Sync,
	{
		let dispatch_info = <Extrinsic as GetDispatchInfo>::get_dispatch_info(&unchecked_extrinsic);

		let partial_fee = <ChargeTransactionPayment<T>>::compute_fee(len, dispatch_info, 0u32.into());
		let DispatchInfo { weight, class, .. } = dispatch_info;

		RuntimeDispatchInfo {
			weight,
			class,
			partial_fee,
		}
	}
}

/// Require the transactor pay for themselves and maybe include a tip to gain additional priority
/// in the queue.
#[derive(Encode, Decode, Clone, Eq, PartialEq)]
pub struct ChargeTransactionPayment<T: Trait + Send + Sync> {
	#[codec(compact)]
	tip: BalanceOf<T>,
	fee_exchange: Option<FeeExchange<T::AssetId, BalanceOf<T>>>,
}

impl<T: Trait + Send + Sync> ChargeTransactionPayment<T> {
	/// utility constructor. Used only in client/factory code.
	pub fn from(tip: BalanceOf<T>, fee_exchange: Option<FeeExchange<T::AssetId, BalanceOf<T>>>) -> Self {
		Self { tip, fee_exchange }
	}

	/// Compute the final fee value for a particular transaction.
	///
	/// The final fee is composed of:
	///   - _base_fee_: This is the minimum amount a user pays for a transaction.
	///   - _len_fee_: This is the amount paid merely to pay for size of the transaction.
	///   - _weight_fee_: This amount is computed based on the weight of the transaction. Unlike
	///      size-fee, this is not input dependent and reflects the _complexity_ of the execution
	///      and the time it consumes.
	///   - _targeted_fee_adjustment_: This is a multiplier that can tune the final fee based on
	///     the congestion of the network.
	///   - (optional) _tip_: if included in the transaction, it will be added on top. Only signed
	///      transactions can have a tip.
	///
	/// final_fee = base_fee + targeted_fee_adjustment(len_fee + weight_fee) + tip;
	pub fn compute_fee(len: u32, info: <Self as SignedExtension>::DispatchInfo, tip: BalanceOf<T>) -> BalanceOf<T>
	where
		BalanceOf<T>: Sync + Send,
	{
		if info.pays_fee {
			let len = <BalanceOf<T>>::from(len);
			let per_byte = T::TransactionByteFee::get();
			let len_fee = per_byte.saturating_mul(len);

			let weight_fee = {
				// cap the weight to the maximum defined in runtime, otherwise it will be the `Bounded`
				// maximum of its data type, which is not desired.
				let capped_weight = info.weight.min(<T as frame_system::Trait>::MaximumBlockWeight::get());
				T::WeightToFee::convert(capped_weight)
			};

			// the adjustable part of the fee
			let adjustable_fee = len_fee.saturating_add(weight_fee);
			let targeted_fee_adjustment = NextFeeMultiplier::get();
			// adjusted_fee = adjustable_fee + (adjustable_fee * targeted_fee_adjustment)
			let adjusted_fee = targeted_fee_adjustment.saturated_multiply_accumulate(adjustable_fee);

			let base_fee = T::TransactionBaseFee::get();
			let final_fee = base_fee.saturating_add(adjusted_fee).saturating_add(tip);

			final_fee
		} else {
			tip
		}
	}
}

impl<T: Trait + Send + Sync> sp_std::fmt::Debug for ChargeTransactionPayment<T> {
	#[cfg(feature = "std")]
	fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		write!(f, "ChargeTransactionPayment<{:?}>", self)
	}
	#[cfg(not(feature = "std"))]
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}

impl<T> SignedExtension for ChargeTransactionPayment<T>
where
	T: Trait + Send + Sync,
	BalanceOf<T>: Send + Sync,
{
	type AccountId = T::AccountId;
	type Call = T::Call;
	type AdditionalSigned = ();
	type DispatchInfo = DispatchInfo;
	type Pre = ();
	fn additional_signed(&self) -> sp_std::result::Result<(), TransactionValidityError> {
		Ok(())
	}

	fn validate(
		&self,
		who: &Self::AccountId,
		call: &Self::Call,
		info: Self::DispatchInfo,
		len: usize,
	) -> TransactionValidity {
		let fee = Self::compute_fee(len as u32, info, self.tip);

		// How much user nominated fee asset has been spent so far
		// used for accounting the 'max payment' preference
		let mut fee_asset_spent: BalanceOf<T> = Zero::zero();

		// Only mess with balances if the fee is not zero.
		if !fee.is_zero() {
			if let Some(exchange) = &self.fee_exchange {
				// Buy the CENNZnet fee currency paying with the user's nominated fee currency
				fee_asset_spent = T::BuyFeeAsset::buy_fee_asset(who, fee, &exchange)
					.map_err(|_| TransactionValidityError::Invalid(InvalidTransaction::Payment))?;
			}

			// Pay for the transaction `fee` in the native fee currency
			let imbalance = match T::Currency::withdraw(
				who,
				fee,
				if self.tip.is_zero() {
					WithdrawReason::TransactionPayment.into()
				} else {
					WithdrawReason::TransactionPayment | WithdrawReason::Tip
				},
				ExistenceRequirement::KeepAlive,
			) {
				Ok(imbalance) => imbalance,
				Err(_) => return Err(TransactionValidityError::Invalid(InvalidTransaction::Payment)),
			};

			T::OnTransactionPayment::on_unbalanced(imbalance);
		}

		// Certain contract module calls require gas metering and special handling for
		// multi-currency gas payment
		if T::GasMeteredCallResolver::is_gas_metered(call) {
			// Temporarily store the `FeeExchange` info so that it can be used later by the-
			// custom `GasHandler` impl see `runtime/src/impls.rs`. This is a hack!
			if let Some(exchange) = &self.fee_exchange {
				storage::unhashed::put(
					&GAS_FEE_EXCHANGE_KEY,
					&FeeExchange::new_v1(
						exchange.asset_id(),
						exchange.max_payment().checked_sub(&fee_asset_spent).unwrap_or(0.into()),
					),
				);
			} else {
				// Pre-caution to ensure `FeeExchange` data is cleared
				storage::unhashed::kill(&GAS_FEE_EXCHANGE_KEY)
			};
		}

		// The transaction is valid
		let mut r = ValidTransaction::default();
		// NOTE: we probably want to maximize the _fee (of any type) per weight unit_ here, which
		// will be a bit more than setting the priority to tip. For now, this is enough.
		r.priority = fee.saturated_into::<TransactionPriority>();
		Ok(r)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::*;
	use frame_support::weights::DispatchClass;
	use sp_runtime::{testing::TestXt, traits::Extrinsic};

	// A balance transfer
	const CALL: &<Runtime as frame_system::Trait>::Call = &mock::Call::Balances(pallet_balances::Call::transfer(2, 69));
	// A balance transfer, which will be considered 'gas metered' for testing purposes
	const METERED_CALL: &<Runtime as frame_system::Trait>::Call =
		&mock::Call::Balances(pallet_balances::Call::transfer_keep_alive(GAS_METERED_ACCOUNT_ID, 69));

	#[test]
	fn signed_extension_transaction_payment_work() {
		ExtBuilder::default()
			.balance_factor(10) // 100
			.fees(5, 1, 1) // 5 fixed, 1 per byte, 1 per weight
			.build()
			.execute_with(|| {
				let len = 10;
				assert!(ChargeTransactionPayment::<Runtime>::from(0, None)
					.pre_dispatch(&1, CALL, info_from_weight(5), len)
					.is_ok());
				assert_eq!(Balances::free_balance(&1), 100 - 5 - 5 - 10);

				assert!(ChargeTransactionPayment::<Runtime>::from(5, /* tipped */ None)
					.pre_dispatch(&2, CALL, info_from_weight(3), len)
					.is_ok());
				assert_eq!(Balances::free_balance(&2), 200 - 5 - 10 - 3 - 5);
			});
	}

	#[test]
	fn signed_extension_transaction_payment_is_bounded() {
		ExtBuilder::default()
			.balance_factor(1000)
			.fees(0, 0, 1)
			.build()
			.execute_with(|| {
				// maximum weight possible
				assert!(ChargeTransactionPayment::<Runtime>::from(0, None)
					.pre_dispatch(&1, CALL, info_from_weight(Weight::max_value()), 10)
					.is_ok());
				// fee will be proportional to what is the actual maximum weight in the runtime.
				assert_eq!(
					Balances::free_balance(&1),
					(10000 - <Runtime as frame_system::Trait>::MaximumBlockWeight::get()) as u64
				);
			});
	}

	#[test]
	fn signed_extension_allows_free_transactions() {
		ExtBuilder::default()
			.fees(100, 1, 1)
			.balance_factor(0)
			.build()
			.execute_with(|| {
				// 1 ain't have a penny.
				assert_eq!(Balances::free_balance(&1), 0);

				let len = 100;

				// like a FreeOperational
				let operational_transaction = DispatchInfo {
					weight: 0,
					class: DispatchClass::Operational,
					pays_fee: false,
				};
				assert!(ChargeTransactionPayment::<Runtime>::from(0, None)
					.validate(&1, CALL, operational_transaction, len)
					.is_ok());

				// like a FreeNormal
				let free_transaction = DispatchInfo {
					weight: 0,
					class: DispatchClass::Normal,
					pays_fee: true,
				};
				assert!(ChargeTransactionPayment::<Runtime>::from(0, None)
					.validate(&1, CALL, free_transaction, len)
					.is_err());
			});
	}

	#[test]
	fn signed_ext_length_fee_is_also_updated_per_congestion() {
		ExtBuilder::default()
			.fees(5, 1, 1)
			.balance_factor(10)
			.build()
			.execute_with(|| {
				// all fees should be x1.5
				NextFeeMultiplier::put(Fixed64::from_rational(1, 2));
				let len = 10;

				assert!(ChargeTransactionPayment::<Runtime>::from(10, None) // tipped
					.pre_dispatch(&1, CALL, info_from_weight(3), len)
					.is_ok());
				assert_eq!(Balances::free_balance(&1), 100 - 10 - 5 - (10 + 3) * 3 / 2);
			})
	}

	#[test]
	fn query_info_works() {
		let call = mock::Call::Balances(pallet_balances::Call::transfer(2, 69));
		let origin = 111111;
		let extra = ();
		let xt = TestXt::new(call, Some((origin, extra))).unwrap();
		let info = xt.get_dispatch_info();
		let ext = xt.encode();
		let len = ext.len() as u32;
		ExtBuilder::default().fees(5, 1, 2).build().execute_with(|| {
			// all fees should be x1.5
			NextFeeMultiplier::put(Fixed64::from_rational(1, 2));

			assert_eq!(
				TransactionPayment::query_info(xt, len),
				RuntimeDispatchInfo {
					weight: info.weight,
					class: info.class,
					partial_fee: 5 /* base */
						+ (
							len as u64 /* len * 1 */
							+ info.weight.min(MaximumBlockWeight::get()) as u64 * 2 /* weight * weight_to_fee */
						) * 3 / 2
				},
			);
		});
	}

	#[test]
	fn compute_fee_works_without_multiplier() {
		ExtBuilder::default()
			.fees(100, 10, 1)
			.balance_factor(0)
			.build()
			.execute_with(|| {
				// Next fee multiplier is zero
				assert_eq!(NextFeeMultiplier::get(), Fixed64::from_natural(0));

				// Tip only, no fees works
				let dispatch_info = DispatchInfo {
					weight: 0,
					class: DispatchClass::Operational,
					pays_fee: false,
				};
				assert_eq!(
					ChargeTransactionPayment::<Runtime>::compute_fee(0, dispatch_info, 10),
					10
				);
				// No tip, only base fee works
				let dispatch_info = DispatchInfo {
					weight: 0,
					class: DispatchClass::Operational,
					pays_fee: true,
				};
				assert_eq!(
					ChargeTransactionPayment::<Runtime>::compute_fee(0, dispatch_info, 0),
					100
				);
				// Tip + base fee works
				assert_eq!(
					ChargeTransactionPayment::<Runtime>::compute_fee(0, dispatch_info, 69),
					169
				);
				// Len (byte fee) + base fee works
				assert_eq!(
					ChargeTransactionPayment::<Runtime>::compute_fee(42, dispatch_info, 0),
					520
				);
				// Weight fee + base fee works
				let dispatch_info = DispatchInfo {
					weight: 1000,
					class: DispatchClass::Operational,
					pays_fee: true,
				};
				assert_eq!(
					ChargeTransactionPayment::<Runtime>::compute_fee(0, dispatch_info, 0),
					1100
				);
			});
	}

	#[test]
	fn compute_fee_works_with_multiplier() {
		ExtBuilder::default()
			.fees(100, 10, 1)
			.balance_factor(0)
			.build()
			.execute_with(|| {
				// Add a next fee multiplier
				NextFeeMultiplier::put(Fixed64::from_rational(1, 2)); // = 1/2 = .5
													  // Base fee is unaffected by multiplier
				let dispatch_info = DispatchInfo {
					weight: 0,
					class: DispatchClass::Operational,
					pays_fee: true,
				};
				assert_eq!(
					ChargeTransactionPayment::<Runtime>::compute_fee(0, dispatch_info, 0),
					100
				);

				// Everything works together :)
				let dispatch_info = DispatchInfo {
					weight: 123,
					class: DispatchClass::Operational,
					pays_fee: true,
				};
				// 123 weight, 456 length, 100 base
				// adjustable fee = (123 * 1) + (456 * 10) = 4683
				// adjusted fee = (4683 * .5) + 4683 = 7024.5 -> 7024
				// final fee = 100 + 7024 + 789 tip = 7913
				assert_eq!(
					ChargeTransactionPayment::<Runtime>::compute_fee(456, dispatch_info, 789),
					7913
				);
			});
	}

	#[test]
	fn compute_fee_does_not_overflow() {
		ExtBuilder::default()
			.fees(100, 10, 1)
			.balance_factor(0)
			.build()
			.execute_with(|| {
				// Overflow is handled
				let dispatch_info = DispatchInfo {
					weight: <u32>::max_value(),
					class: DispatchClass::Operational,
					pays_fee: true,
				};
				assert_eq!(
					ChargeTransactionPayment::<Runtime>::compute_fee(
						<u32>::max_value(),
						dispatch_info,
						<u64>::max_value()
					),
					<u64>::max_value()
				);
			});
	}

	#[test]
	fn uses_valid_currency_fee_exchange() {
		ExtBuilder::default()
			.fees(5, 1, 1)
			.balance_factor(1)
			.build()
			.execute_with(|| {
				let len = 10;
				let fee_exchange = FeeExchange::new_v1(VALID_ASSET_TO_BUY_FEE, 100_000);
				assert!(ChargeTransactionPayment::<Runtime>::from(10, Some(fee_exchange))
					.pre_dispatch(&1, CALL, info_from_weight(3), len)
					.is_ok());
			})
	}

	#[test]
	fn uses_invalid_currency_fee_exchange() {
		ExtBuilder::default()
			.fees(5, 1, 1)
			.balance_factor(1)
			.build()
			.execute_with(|| {
				let len = 10;
				let fee_exchange = FeeExchange::new_v1(INVALID_ASSET_TO_BUY_FEE, 100_000);
				assert!(ChargeTransactionPayment::<Runtime>::from(10, Some(fee_exchange))
					.pre_dispatch(&1, CALL, info_from_weight(3), len)
					.is_err());
			})
	}

	#[test]
	fn rejects_valid_currency_fee_exchange_with_zero_max_payment() {
		ExtBuilder::default()
			.fees(5, 1, 1)
			.balance_factor(1)
			.build()
			.execute_with(|| {
				let len = 10;
				let fee_exchange = FeeExchange::new_v1(VALID_ASSET_TO_BUY_FEE, 0);
				assert!(ChargeTransactionPayment::<Runtime>::from(10, Some(fee_exchange))
					.pre_dispatch(&1, CALL, info_from_weight(3), len)
					.is_err());
			})
	}

	#[test]
	fn fee_exchange_temporary_storage_for_gas_metered_calls() {
		ExtBuilder::default()
			.fees(5, 1, 1)
			.balance_factor(1000)
			.build()
			.execute_with(|| {
				let len: u64 = 10;
				let base_fee = ChargeTransactionPayment::<Runtime>::compute_fee(0, info_from_weight(3), len);

				// `GAS_FEE_EXCHANGE_KEY` temporary storage should be set
				let fee_exchange = FeeExchange::new_v1(VALID_ASSET_TO_BUY_FEE, 111);
				let transaction_payment_with_fee_exchange =
					ChargeTransactionPayment::<Runtime>::from(0, Some(fee_exchange.clone()));
				assert!(transaction_payment_with_fee_exchange
					.pre_dispatch(&1, METERED_CALL, info_from_weight(3), len as usize)
					.is_ok());
				// fee exchange `max_payment` is decremented by the payment cost
				assert_eq!(
					FeeExchange::new_v1(fee_exchange.asset_id(), fee_exchange.max_payment() - base_fee),
					storage::unhashed::get(&GAS_FEE_EXCHANGE_KEY).expect("it is stored")
				);

				// `GAS_FEE_EXCHANGE_KEY` temporary storage should not be set without fee_exchange
				let transaction_payment_without_fee_exchange = ChargeTransactionPayment::<Runtime>::from(0, None);
				assert!(transaction_payment_without_fee_exchange
					.pre_dispatch(&1, METERED_CALL, info_from_weight(3), len as usize)
					.is_ok());
				let stored: Option<()> = storage::unhashed::get(&GAS_FEE_EXCHANGE_KEY);
				assert!(stored.is_none());
			});
	}

	#[test]
	fn fee_exchange_temporary_storage_unused_for_normal_calls() {
		ExtBuilder::default()
			.fees(5, 1, 1)
			.balance_factor(1000)
			.build()
			.execute_with(|| {
				let len: u64 = 10;

				// `GAS_FEE_EXCHANGE_KEY` temporary storage should be unset
				let fee_exchange = FeeExchange::new_v1(VALID_ASSET_TO_BUY_FEE, 111);
				let transaction_payment_with_fee_exchange =
					ChargeTransactionPayment::<Runtime>::from(0, Some(fee_exchange.clone()));
				assert!(transaction_payment_with_fee_exchange
					.pre_dispatch(&1, CALL, info_from_weight(3), len as usize)
					.is_ok());
				assert!(storage::unhashed::get::<Option<()>>(&GAS_FEE_EXCHANGE_KEY).is_none());

				// `GAS_FEE_EXCHANGE_KEY` temporary storage should be unset
				let transaction_payment_without_fee_exchange = ChargeTransactionPayment::<Runtime>::from(0, None);
				assert!(transaction_payment_without_fee_exchange
					.pre_dispatch(&1, CALL, info_from_weight(3), len as usize)
					.is_ok());
				assert!(storage::unhashed::get::<Option<()>>(&GAS_FEE_EXCHANGE_KEY).is_none());
			});
	}
}
