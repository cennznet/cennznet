// This file is part of Substrate.

// Copyright (C) 2019-2021 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Runtime API definition for transaction payment module.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::Codec;
use sp_runtime::traits::MaybeDisplay;

pub use crml_transaction_payment::{FeeDetails, InclusionFee, RuntimeDispatchInfo};

// TODO Fix conflicting implementations of trait `parity_scale_codec::Decode` for type `pallet_transaction_payment::RuntimeDispatchInfo<_>`:
// use codec::Decode;
// use frame_support::weights::{Weight, DispatchClass};
// // The weight type used by legacy runtimes
// type LegacyWeight = u32;
// // Encoding of `RuntimeDispatchInfo` is approximately (assuming `u128` balance)
// // old byte length (u32, u8, u128) = 168 / 8 = 21
// // new byte length (u64, u8, u128) = 200 / 8 = 25
// /// Byte length of an encoded legacy `RuntimeDispatchInfo` i.e. Weight = u32
// const LEGACY_RUNTIME_DISPATCH_INFO_BYTE_LENGTH: usize = 21;

// impl<Balance: Decode> Decode for RuntimeDispatchInfo<Balance> {
// 	// Custom decode implementation to handle the differences between the `RuntimeDispatchInfo` type
// 	// between client version vs. runtime version
// 	// Concretely, `Weight` type changed from `u32` in some legacy runtimes to now `u64`
// 	fn decode<I: codec::Input>(value: &mut I) -> Result<Self, codec::Error> {
// 		// Check `value` len to see whether we should decode legacy or new Weight type
// 		let input_len = value.remaining_len()?.ok_or("empty buffer while decoding")?;
// 		let weight: Weight = if input_len == LEGACY_RUNTIME_DISPATCH_INFO_BYTE_LENGTH {
// 			LegacyWeight::decode(value)?.into()
// 		} else {
// 			Weight::decode(value)?
// 		};
//
// 		let class = DispatchClass::decode(value)?;
// 		let partial_fee = Balance::decode(value)?;
//
// 		return Ok(Self {
// 			weight,
// 			class,
// 			partial_fee,
// 		})
// 	}
// }

sp_api::decl_runtime_apis! {
	pub trait TransactionPaymentApi<Balance> where
		Balance: Codec + MaybeDisplay,
	{
		fn query_info(uxt: Block::Extrinsic, len: u32) -> RuntimeDispatchInfo<Balance>;
		fn query_fee_details(uxt: Block::Extrinsic, len: u32) -> FeeDetails<Balance>;
	}
}

// TODO Fix conflicting implementations of trait `parity_scale_codec::Decode` for type `pallet_transaction_payment::RuntimeDispatchInfo<_>`:
// #[cfg(test)]
// mod tests {
// 	use super::*;
//
// 	#[test]
// 	fn it_decodes_legacy_runtime_dispatch_info() {
// 		// older runtimes pre-2.0.0 use `type Weight = u32`
// 		let legacy_dispatch_info = (1_u32, DispatchClass::Normal, 1_u128);
// 		let decoded = RuntimeDispatchInfo::<u128>::decode(&mut &legacy_dispatch_info.encode()[..]).expect("it decodes");
// 		assert_eq!(decoded.weight, legacy_dispatch_info.0 as u64);
// 		assert_eq!(decoded.class, legacy_dispatch_info.1);
// 		assert_eq!(decoded.partial_fee, legacy_dispatch_info.2);
// 	}
//
// 	#[test]
// 	fn it_decodes_new_runtime_dispatch_info() {
// 		// newer runtimes post-2.0.0 use `type Weight = u64`
// 		let runtime_dispatch_info = RuntimeDispatchInfo { weight: 1, class: DispatchClass::Normal, partial_fee: 1_u128 };
// 		let decoded = RuntimeDispatchInfo::<u128>::decode(&mut &runtime_dispatch_info.encode()[..]).expect("it decodes");
// 		assert_eq!(decoded, runtime_dispatch_info);
// 	}
// }
