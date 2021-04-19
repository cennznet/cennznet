// This file is part of Substrate.

// Copyright (C) 2021 Parity Technologies (UK) Ltd.
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

//! Types for transaction-payment RPC.

use codec::{Decode, Encode};
use frame_support::weights::{DispatchClass, Weight};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::traits::{AtLeast32BitUnsigned, Zero};
use sp_std::prelude::*;

/// The base fee and adjusted weight and length fees constitute the _inclusion fee_.
#[derive(Encode, Decode, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct InclusionFee<Balance> {
	/// This is the minimum amount a user pays for a transaction. It is declared
	/// as a base _weight_ in the runtime and converted to a fee using `WeightToFee`.
	pub base_fee: Balance,
	/// The length fee, the amount paid for the encoded length (in bytes) of the transaction.
	pub len_fee: Balance,
	/// - `targeted_fee_adjustment`: This is a multiplier that can tune the final fee based on
	///     the congestion of the network.
	/// - `weight_fee`: This amount is computed based on the weight of the transaction. Weight
	/// accounts for the execution time of a transaction.
	///
	/// adjusted_weight_fee = targeted_fee_adjustment * weight_fee
	pub adjusted_weight_fee: Balance,
}

impl<Balance: AtLeast32BitUnsigned + Copy> InclusionFee<Balance> {
	/// Returns the total of inclusion fee.
	///
	/// ```ignore
	/// inclusion_fee = base_fee + len_fee + adjusted_weight_fee
	/// ```
	pub fn inclusion_fee(&self) -> Balance {
		self.base_fee
			.saturating_add(self.len_fee)
			.saturating_add(self.adjusted_weight_fee)
	}
}

/// The `FeeDetails` is composed of:
///   - (Optional) `inclusion_fee`: Only the `Pays::Yes` transaction can have the inclusion fee.
///   - `tip`: If included in the transaction, the tip will be added on top. Only
///     signed transactions can have a tip.
#[derive(Encode, Decode, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct FeeDetails<Balance> {
	/// The minimum fee for a transaction to be included in a block.
	pub inclusion_fee: Option<InclusionFee<Balance>>,
	// Do not serialize and deserialize `tip` as we actually can not pass any tip to the RPC.
	#[cfg_attr(feature = "std", serde(skip))]
	pub tip: Balance,
}

impl<Balance: AtLeast32BitUnsigned + Copy> FeeDetails<Balance> {
	/// Returns the final fee.
	///
	/// ```ignore
	/// final_fee = inclusion_fee + tip;
	/// ```
	pub fn final_fee(&self) -> Balance {
		self.inclusion_fee
			.as_ref()
			.map(|i| i.inclusion_fee())
			.unwrap_or_else(|| Zero::zero())
			.saturating_add(self.tip)
	}
}

/// Information related to a dispatchable's class, weight, and fee that can be queried from the runtime.
#[derive(Eq, PartialEq, Encode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "std", serde(bound(serialize = "Balance: std::fmt::Display")))]
#[cfg_attr(feature = "std", serde(bound(deserialize = "Balance: std::str::FromStr")))]
pub struct RuntimeDispatchInfo<Balance> {
	/// Weight of this dispatch.
	pub weight: Weight,
	/// Class of this dispatch.
	pub class: DispatchClass,
	/// The inclusion fee of this dispatch.
	///
	/// This does not include a tip or anything else that
	/// depends on the signature (i.e. depends on a `SignedExtension`).
	#[cfg_attr(feature = "std", serde(with = "serde_balance"))]
	pub partial_fee: Balance,
}

// The weight type used by legacy runtimes (pre-frame 2.0.0 versions)
type LegacyWeight = u32;
// Encoding of `RuntimeDispatchInfo` is approximately (assuming `u128` balance)
// old byte length (u32, u8, u128) = 168 / 8 = 21
// new byte length (u64, u8, u128) = 200 / 8 = 25
/// Byte length of an encoded legacy `RuntimeDispatchInfo` i.e. Weight = u32
const LEGACY_RUNTIME_DISPATCH_INFO_BYTE_LENGTH: usize = 21;

impl<Balance: Decode> Decode for RuntimeDispatchInfo<Balance> {
	// Custom decode implementation to handle the differences between the `RuntimeDispatchInfo` type
	// between client version vs. runtime version
	// Concretely, `Weight` type changed from `u32` in some legacy runtimes to now `u64`
	fn decode<I: codec::Input>(value: &mut I) -> Result<Self, codec::Error> {
		// Check `value` len to see whether we should decode legacy or new Weight type
		let input_len = value.remaining_len()?.ok_or("empty buffer while decoding")?;
		let weight: Weight = if input_len == LEGACY_RUNTIME_DISPATCH_INFO_BYTE_LENGTH {
			LegacyWeight::decode(value)?.into()
		} else {
			Weight::decode(value)?
		};

		let class = DispatchClass::decode(value)?;
		let partial_fee = Balance::decode(value)?;

		return Ok(Self {
			weight,
			class,
			partial_fee,
		});
	}
}

#[cfg(feature = "std")]
mod serde_balance {
	use serde::{Deserialize, Deserializer, Serializer};

	pub fn serialize<S: Serializer, T: std::fmt::Display>(t: &T, serializer: S) -> Result<S::Ok, S::Error> {
		serializer.serialize_str(&t.to_string())
	}

	pub fn deserialize<'de, D: Deserializer<'de>, T: std::str::FromStr>(deserializer: D) -> Result<T, D::Error> {
		let s = String::deserialize(deserializer)?;
		s.parse::<T>()
			.map_err(|_| serde::de::Error::custom("Parse from string failed"))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn it_decodes_legacy_runtime_dispatch_info() {
		// older runtimes pre-frame 2.0.0 use `type Weight = u32`
		let legacy_dispatch_info = (1_u32, DispatchClass::Normal, 1_u128);
		let decoded = RuntimeDispatchInfo::<u128>::decode(&mut &legacy_dispatch_info.encode()[..]).expect("it decodes");
		assert_eq!(decoded.weight, legacy_dispatch_info.0 as u64);
		assert_eq!(decoded.class, legacy_dispatch_info.1);
		assert_eq!(decoded.partial_fee, legacy_dispatch_info.2);
	}

	#[test]
	fn it_decodes_new_runtime_dispatch_info() {
		// newer runtimes post frame 2.0.0 use `type Weight = u64`
		let runtime_dispatch_info = RuntimeDispatchInfo {
			weight: 1,
			class: DispatchClass::Normal,
			partial_fee: 1_u128,
		};
		let decoded =
			RuntimeDispatchInfo::<u128>::decode(&mut &runtime_dispatch_info.encode()[..]).expect("it decodes");
		assert_eq!(decoded, runtime_dispatch_info);
	}

	#[test]
	fn should_serialize_and_deserialize_properly_with_string() {
		let info = RuntimeDispatchInfo {
			weight: 5,
			class: DispatchClass::Normal,
			partial_fee: 1_000_000_u64,
		};

		let json_str = r#"{"weight":5,"class":"normal","partialFee":"1000000"}"#;

		assert_eq!(serde_json::to_string(&info).unwrap(), json_str);
		assert_eq!(
			serde_json::from_str::<RuntimeDispatchInfo<u64>>(json_str).unwrap(),
			info
		);

		// should not panic
		serde_json::to_value(&info).unwrap();
	}

	#[test]
	fn should_serialize_and_deserialize_properly_large_value() {
		let info = RuntimeDispatchInfo {
			weight: 5,
			class: DispatchClass::Normal,
			partial_fee: u128::max_value(),
		};

		let json_str = r#"{"weight":5,"class":"normal","partialFee":"340282366920938463463374607431768211455"}"#;

		assert_eq!(serde_json::to_string(&info).unwrap(), json_str);
		assert_eq!(
			serde_json::from_str::<RuntimeDispatchInfo<u128>>(json_str).unwrap(),
			info
		);

		// should not panic
		serde_json::to_value(&info).unwrap();
	}
}
