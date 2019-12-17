// Copyright 2018-2019 Parity Technologies (UK) Ltd. and Centrality Investments Ltd.
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

use cennznet_primitives::{traits::BuyFeeAsset, types::FeeExchangeV1 as FeeExchange};
use codec::{Decode, Encode};
use rstd::{fmt::Debug, prelude::*};
use sr_primitives::{
	traits::{
		Convert, MaybeSerializeDeserialize, Member, SaturatedConversion, Saturating, SignedExtension, SimpleArithmetic,
		Zero,
	},
	transaction_validity::{
		InvalidTransaction, TransactionPriority, TransactionValidity, TransactionValidityError, ValidTransaction,
	},
	weights::{DispatchInfo, Weight},
	Fixed64,
};
use support::{
	decl_module, decl_storage,
	traits::{Currency, ExistenceRequirement, Get, OnUnbalanced, WithdrawReason},
	Parameter,
};

type Multiplier = Fixed64;
type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;
type NegativeImbalanceOf<T> = <<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::NegativeImbalance;

pub trait Trait: system::Trait {
	/// The units in which we record balances.
	type Balance: Parameter + Member + SimpleArithmetic + Default + Copy + MaybeSerializeDeserialize + Debug;

	/// The arithmetic type of asset identifier.
	type AssetId: Parameter + Member + SimpleArithmetic + Default + Copy;

	/// The currency type in which fees will be paid.
	type Currency: Currency<Self::AccountId>;

	/// Handler for the unbalanced reduction when taking transaction fees.
	type OnTransactionPayment: OnUnbalanced<NegativeImbalanceOf<Self>>;

	/// The fee to be paid for making a transaction; the base.
	type TransactionBaseFee: Get<BalanceOf<Self>>;

	/// The fee to be paid for making a transaction; the per-byte portion.
	type TransactionByteFee: Get<BalanceOf<Self>>;

	/// Convert a weight value into a deductible fee based on the currency type.
	type WeightToFee: Convert<Weight, BalanceOf<Self>>;

	/// Update the multiplier of the next block, based on the previous block's weight.
	// TODO: maybe this does not need previous weight and can just read it
	type FeeMultiplierUpdate: Convert<(Weight, Multiplier), Multiplier>;

	/// A function which buys fee asset if signalled by the extrinsic.
	type BuyFeeAsset: BuyFeeAsset<Self::AccountId, BalanceOf<Self>, FeeExchange = FeeExchange<BalanceOf<Self>>>;
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
			let current_weight = <system::Module<T>>::all_extrinsics_weight();
			NextFeeMultiplier::mutate(|fm| {
				*fm = T::FeeMultiplierUpdate::convert((current_weight, *fm))
			});
		}
	}
}

impl<T: Trait> Module<T> {}

/// Require the transactor pay for themselves and maybe include a tip to gain additional priority
/// in the queue.
#[derive(Encode, Decode, Clone, Eq, PartialEq)]
pub struct ChargeTransactionPayment<T: Trait + Send + Sync> {
	#[codec(compact)]
	tip: BalanceOf<T>,
	fee_exchange: Option<FeeExchange<BalanceOf<T>>>,
}

impl<T: Trait + Send + Sync> ChargeTransactionPayment<T> {
	/// utility constructor. Used only in client/factory code.
	pub fn from(tip: BalanceOf<T>, fee_exchange: Option<FeeExchange<BalanceOf<T>>>) -> Self {
		Self { tip, fee_exchange }
	}

	/// Compute the final fee value for a particular transaction.
	///
	/// The final fee is composed of:
	///   - _length-fee_: This is the amount paid merely to pay for size of the transaction.
	///   - _weight-fee_: This amount is computed based on the weight of the transaction. Unlike
	///      size-fee, this is not input dependent and reflects the _complexity_ of the execution
	///      and the time it consumes.
	///   - (optional) _tip_: if included in the transaction, it will be added on top. Only signed
	///      transactions can have a tip.
	fn compute_fee(len: usize, info: DispatchInfo, tip: BalanceOf<T>) -> BalanceOf<T> {
		let len_fee = if info.pay_length_fee() {
			let len = <BalanceOf<T>>::from(len as u32);
			let base = T::TransactionBaseFee::get();
			let per_byte = T::TransactionByteFee::get();
			base.saturating_add(per_byte.saturating_mul(len))
		} else {
			Zero::zero()
		};

		let weight_fee = {
			// cap the weight to the maximum defined in runtime, otherwise it will be the `Bounded`
			// maximum of its data type, which is not desired.
			let capped_weight = info.weight.min(<T as system::Trait>::MaximumBlockWeight::get());
			T::WeightToFee::convert(capped_weight)
		};

		// everything except for tip
		let basic_fee = len_fee.saturating_add(weight_fee);
		let fee_update = NextFeeMultiplier::get();
		let adjusted_fee = fee_update.saturated_multiply_accumulate(basic_fee);

		adjusted_fee.saturating_add(tip)
	}
}

impl<T: Trait + Send + Sync> rstd::fmt::Debug for ChargeTransactionPayment<T> {
	#[cfg(feature = "std")]
	fn fmt(&self, f: &mut rstd::fmt::Formatter) -> rstd::fmt::Result {
		write!(f, "ChargeTransactionPayment<{:?}>", self.tip)
	}
	#[cfg(not(feature = "std"))]
	fn fmt(&self, _: &mut rstd::fmt::Formatter) -> rstd::fmt::Result {
		Ok(())
	}
}

impl<T: Trait + Send + Sync> SignedExtension for ChargeTransactionPayment<T>
where
	BalanceOf<T>: Send + Sync,
{
	type AccountId = T::AccountId;
	type Call = T::Call;
	type AdditionalSigned = ();
	type Pre = ();
	fn additional_signed(&self) -> rstd::result::Result<(), TransactionValidityError> {
		Ok(())
	}

	fn validate(
		&self,
		who: &Self::AccountId,
		_call: &Self::Call,
		info: DispatchInfo,
		len: usize,
	) -> TransactionValidity {
		// pay any fees.
		let fee = Self::compute_fee(len, info, self.tip);
		if let Some(fee_exchange) = &self.fee_exchange {
			if let Err(_) = T::BuyFeeAsset::buy_fee_asset(who, fee, fee_exchange) {
				return InvalidTransaction::Payment.into();
			}
		}
		let imbalance = match T::Currency::withdraw(
			who,
			fee,
			WithdrawReason::TransactionPayment,
			ExistenceRequirement::KeepAlive,
		) {
			Ok(imbalance) => imbalance,
			Err(_) => return InvalidTransaction::Payment.into(),
		};
		T::OnTransactionPayment::on_unbalanced(imbalance);

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
	use cennznet_primitives::traits::BuyFeeAsset;
	use primitives::{crypto::UncheckedInto, sr25519, H256};
	use rstd::cell::RefCell;
	use sr_primitives::{
		testing::Header,
		traits::{BlakeTwo256, IdentityLookup, Verify},
		weights::DispatchClass,
		Perbill,
	};
	use support::{dispatch::Result, impl_outer_origin, parameter_types};

	const CALL: &<Runtime as system::Trait>::Call = &();
	const VALID_ASSET_TO_BUY_FEE: u32 = 1;
	const INVALID_ASSET_TO_BUY_FEE: u32 = 2;

	#[derive(Clone, PartialEq, Eq, Debug)]
	pub struct Runtime;

	impl_outer_origin! {
		pub enum Origin for Runtime {}
	}

	parameter_types! {
		pub const BlockHashCount: u64 = 250;
		pub const MaximumBlockWeight: u32 = 1024;
		pub const MaximumBlockLength: u32 = 2 * 1024;
		pub const AvailableBlockRatio: Perbill = Perbill::one();
	}

	type AccountId = <sr25519::Signature as Verify>::Signer;

	impl system::Trait for Runtime {
		type Origin = Origin;
		type Index = u64;
		type BlockNumber = u64;
		type Call = ();
		type Hash = H256;
		type Hashing = BlakeTwo256;
		type AccountId = AccountId;
		type Lookup = IdentityLookup<Self::AccountId>;
		type Header = Header;
		type Event = ();
		type BlockHashCount = BlockHashCount;
		type MaximumBlockWeight = MaximumBlockWeight;
		type MaximumBlockLength = MaximumBlockLength;
		type AvailableBlockRatio = AvailableBlockRatio;
		type Version = ();
		type DelegatedDispatchVerifier = ();
		type Doughnut = ();
	}

	parameter_types! {
		pub const TransferFee: u64 = 0;
		pub const CreationFee: u64 = 0;
		pub const ExistentialDeposit: u64 = 0;
	}

	impl balances::Trait for Runtime {
		type Balance = u64;
		type OnFreeBalanceZero = ();
		type OnNewAccount = ();
		type Event = ();
		type TransferPayment = ();
		type DustRemoval = ();
		type ExistentialDeposit = ExistentialDeposit;
		type TransferFee = TransferFee;
		type CreationFee = CreationFee;
	}

	impl generic_asset::Trait for Runtime {
		type Balance = u128;
		type AssetId = u32;
		type Event = ();
	}

	thread_local! {
		static TRANSACTION_BASE_FEE: RefCell<u64> = RefCell::new(0);
		static TRANSACTION_BYTE_FEE: RefCell<u64> = RefCell::new(1);
		static WEIGHT_TO_FEE: RefCell<u64> = RefCell::new(1);
	}

	pub struct TransactionBaseFee;
	impl Get<u64> for TransactionBaseFee {
		fn get() -> u64 {
			TRANSACTION_BASE_FEE.with(|v| *v.borrow())
		}
	}

	pub struct TransactionByteFee;
	impl Get<u64> for TransactionByteFee {
		fn get() -> u64 {
			TRANSACTION_BYTE_FEE.with(|v| *v.borrow())
		}
	}

	pub struct WeightToFee(u64);
	impl Convert<Weight, u64> for WeightToFee {
		fn convert(t: Weight) -> u64 {
			WEIGHT_TO_FEE.with(|v| *v.borrow() * (t as u64))
		}
	}

	/// Implement a fake BuyFeeAsset for tests
	impl<T: Trait> BuyFeeAsset<AccountId, u64> for Module<T> {
		type FeeExchange = FeeExchange<u64>;
		fn buy_fee_asset(who: &AccountId, amount: u64, exchange_op: &Self::FeeExchange) -> Result {
			if exchange_op.asset_id == VALID_ASSET_TO_BUY_FEE {
				if exchange_op.max_payment == 0 {
					return Err("no money");
				}
				let _ = Balances::deposit_into_existing(who, amount)?;
			}
			Ok(())
		}
	}

	impl Trait for Runtime {
		type Balance = u128;
		type AssetId = u32;
		type Currency = balances::Module<Runtime>;
		type OnTransactionPayment = ();
		type TransactionBaseFee = TransactionBaseFee;
		type TransactionByteFee = TransactionByteFee;
		type WeightToFee = WeightToFee;
		type FeeMultiplierUpdate = ();
		type BuyFeeAsset = Module<Self>;
	}

	type Balances = balances::Module<Runtime>;

	macro_rules! user_account {
		($x:expr) => {
			H256::from_low_u64_be($x).unchecked_into()
		};
	}

	pub struct ExtBuilder {
		balance: u64,
		base_fee: u64,
		byte_fee: u64,
		weight_to_fee: u64,
	}

	impl Default for ExtBuilder {
		fn default() -> Self {
			Self {
				balance: 1,
				base_fee: 0,
				byte_fee: 1,
				weight_to_fee: 1,
			}
		}
	}

	impl ExtBuilder {
		pub fn fees(mut self, base: u64, byte: u64, weight: u64) -> Self {
			self.base_fee = base;
			self.byte_fee = byte;
			self.weight_to_fee = weight;
			self
		}
		pub fn set_balance(mut self, balance: u64) -> Self {
			self.balance = balance;
			self
		}
		fn set_constants(&self) {
			TRANSACTION_BASE_FEE.with(|v| *v.borrow_mut() = self.base_fee);
			TRANSACTION_BYTE_FEE.with(|v| *v.borrow_mut() = self.byte_fee);
			WEIGHT_TO_FEE.with(|v| *v.borrow_mut() = self.weight_to_fee);
		}
		pub fn build(self) -> runtime_io::TestExternalities {
			self.set_constants();
			let mut t = system::GenesisConfig::default().build_storage::<Runtime>().unwrap();
			balances::GenesisConfig::<Runtime> {
				balances: vec![(user_account!(1), self.balance)],
				vesting: vec![],
			}
			.assimilate_storage(&mut t)
			.unwrap();
			t.into()
		}
	}

	/// create a transaction info struct from weight. Handy to avoid building the whole struct.
	pub fn info_from_weight(w: Weight) -> DispatchInfo {
		DispatchInfo {
			weight: w,
			..Default::default()
		}
	}

	#[test]
	fn signed_extension_transaction_payment_work() {
		ExtBuilder::default()
			.set_balance(100)
			.fees(5, 1, 1) // 5 fixed, 1 per byte, 1 per weight
			.build()
			.execute_with(|| {
				let len = 10;
				let user = user_account!(1);
				assert!(ChargeTransactionPayment::<Runtime>::from(0, None)
					.pre_dispatch(&user, CALL, info_from_weight(5), len)
					.is_ok());
				assert_eq!(Balances::free_balance(&user), 100 - 5 - 5 - 10);
			});
	}

	#[test]
	fn signed_extension_transaction_payment_work_with_tip() {
		ExtBuilder::default()
			.set_balance(100)
			.fees(5, 1, 1) // 5 fixed, 1 per byte, 1 per weight
			.build()
			.execute_with(|| {
				let len = 10;
				let user = user_account!(1);
				assert!(ChargeTransactionPayment::<Runtime>::from(5, None)
					.pre_dispatch(&user, CALL, info_from_weight(3), len)
					.is_ok());
				assert_eq!(Balances::free_balance(&user), 100 - 5 - 10 - 3 - 5);
			});
	}

	#[test]
	fn signed_extension_transaction_payment_is_bounded() {
		ExtBuilder::default()
			.set_balance(10000)
			.fees(0, 0, 1)
			.build()
			.execute_with(|| {
				use sr_primitives::weights::Weight;
				let user = user_account!(1);

				// maximum weight possible
				assert!(ChargeTransactionPayment::<Runtime>::from(0, None)
					.pre_dispatch(&user, CALL, info_from_weight(Weight::max_value()), 10)
					.is_ok());
				// fee will be proportional to what is the actual maximum weight in the runtime.
				assert_eq!(
					Balances::free_balance(&user),
					(10000 - <Runtime as system::Trait>::MaximumBlockWeight::get()) as u64
				);
			});
	}

	#[test]
	fn signed_extension_allows_free_operational_transactions() {
		ExtBuilder::default()
			.fees(100, 1, 1)
			.set_balance(0)
			.build()
			.execute_with(|| {
				let user = user_account!(1);

				// 1 ain't have a penny.
				assert_eq!(Balances::free_balance(&user), 0);

				// like a FreeOperational
				let operational_transaction = DispatchInfo {
					weight: 0,
					class: DispatchClass::Operational,
				};
				let len = 100;
				assert!(ChargeTransactionPayment::<Runtime>::from(0, None)
					.validate(&user, CALL, operational_transaction, len)
					.is_ok());
			});
	}

	#[test]
	fn signed_extension_rejects_free_normal_transactions() {
		ExtBuilder::default()
			.fees(100, 1, 1)
			.set_balance(0)
			.build()
			.execute_with(|| {
				let user = user_account!(1);

				// 1 ain't have a penny.
				assert_eq!(Balances::free_balance(&user), 0);

				// like a FreeNormal
				let free_transaction = DispatchInfo {
					weight: 0,
					class: DispatchClass::Normal,
				};
				let len = 100;
				assert!(ChargeTransactionPayment::<Runtime>::from(0, None)
					.validate(&user, CALL, free_transaction, len)
					.is_err());
			});
	}

	#[test]
	fn signed_ext_length_fee_is_also_updated_per_congestion() {
		ExtBuilder::default()
			.fees(5, 1, 1)
			.set_balance(100)
			.build()
			.execute_with(|| {
				// all fees should be x1.5
				NextFeeMultiplier::put(Fixed64::from_rational(1, 2));
				let len = 10;
				let user = user_account!(1);

				assert!(ChargeTransactionPayment::<Runtime>::from(10, None)
					.pre_dispatch(&user, CALL, info_from_weight(3), len)
					.is_ok());
				assert_eq!(Balances::free_balance(&user), 100 - 10 - (5 + 10 + 3) * 3 / 2);
			})
	}

	#[test]
	fn uses_valid_currency_fee_exchange() {
		ExtBuilder::default()
			.fees(5, 1, 1)
			.set_balance(1)
			.build()
			.execute_with(|| {
				let user = user_account!(1);
				let len = 10;
				let fee_exchange = FeeExchange::new(VALID_ASSET_TO_BUY_FEE, 100_000);
				assert!(ChargeTransactionPayment::<Runtime>::from(10, Some(fee_exchange))
					.pre_dispatch(&user, CALL, info_from_weight(3), len)
					.is_ok());
			})
	}

	#[test]
	fn uses_invalid_currency_fee_exchange() {
		ExtBuilder::default()
			.fees(5, 1, 1)
			.set_balance(1)
			.build()
			.execute_with(|| {
				let user = user_account!(1);
				let len = 10;
				let fee_exchange = FeeExchange::new(INVALID_ASSET_TO_BUY_FEE, 100_000);
				assert!(ChargeTransactionPayment::<Runtime>::from(10, Some(fee_exchange))
					.pre_dispatch(&user, CALL, info_from_weight(3), len)
					.is_err());
			})
	}

	#[test]
	fn rejects_valid_currency_fee_exchange_with_zero_max_payment() {
		ExtBuilder::default()
			.fees(5, 1, 1)
			.set_balance(1)
			.build()
			.execute_with(|| {
				let user = user_account!(1);
				let len = 10;
				let fee_exchange = FeeExchange::new(VALID_ASSET_TO_BUY_FEE, 0);
				assert!(ChargeTransactionPayment::<Runtime>::from(10, Some(fee_exchange))
					.pre_dispatch(&user, CALL, info_from_weight(3), len)
					.is_err());
			})
	}
}
