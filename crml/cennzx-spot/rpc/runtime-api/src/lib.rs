// Copyright 2020 Parity Technologies (UK) Ltd. and Centrality Investments Ltd.
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

//! Runtime API definition required by CENNZX RPC extensions.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Codec, Decode, Encode};
#[cfg(feature = "std")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sp_runtime::RuntimeDebug;

/// A result of querying the exchange
#[derive(Eq, PartialEq, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum CennzxSpotResult<Balance> {
	/// The exchange returned successfully.
	#[cfg_attr(feature = "std", serde(bound(serialize = "Balance: std::fmt::Display")))]
	#[cfg_attr(feature = "std", serde(serialize_with = "serialize_as_string"))]
	#[cfg_attr(feature = "std", serde(bound(deserialize = "Balance: std::str::FromStr")))]
	#[cfg_attr(feature = "std", serde(deserialize_with = "deserialize_from_string"))]
	Success(Balance),
	/// There was an issue querying the exchange
	Error,
}

#[cfg(feature = "std")]
fn serialize_as_string<S: Serializer, T: std::fmt::Display>(t: &T, serializer: S) -> Result<S::Ok, S::Error> {
	serializer.serialize_str(&t.to_string())
}

#[cfg(feature = "std")]
fn deserialize_from_string<'de, D: Deserializer<'de>, T: std::str::FromStr>(deserializer: D) -> Result<T, D::Error> {
	let s = String::deserialize(deserializer)?;
	s.parse::<T>()
		.map_err(|_| serde::de::Error::custom("Parse from string failed"))
}

sp_api::decl_runtime_apis! {
	/// The RPC API to interact with CENNZX Spot Exchange
	pub trait CennzxSpotApi<AssetId, Balance> where
		AssetId: Codec,
		Balance: Codec,
	{
		/// Query how much `asset_to_buy` will be given in exchange for `amount` of `asset_to_sell`
		fn buy_price(
			asset_to_buy: AssetId,
			amount: Balance,
			asset_to_sell: AssetId,
		) -> CennzxSpotResult<Balance>;
		/// Query how much `asset_to_sell` is required to buy `amount` of `asset_to_buy`
		fn sell_price(
			asset_to_sell: AssetId,
			amount: Balance,
			asset_to_buy: AssetId,
		) -> CennzxSpotResult<Balance>;
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn serde_works_with_string() {
		let result = CennzxSpotResult::Success(123_456_u128);
		let json_str = r#"{"success":"123456"}"#;
		assert_eq!(serde_json::to_string(&result).unwrap(), json_str);
		assert_eq!(
			serde_json::from_str::<CennzxSpotResult<u128>>(json_str).unwrap(),
			result
		);
		serde_json::to_value(&result).unwrap(); // should not panic
	}

	#[test]
	fn serde_works_with_large_integer() {
		let result = CennzxSpotResult::Success(u128::max_value());
		let json_str = r#"{"success":"340282366920938463463374607431768211455"}"#;
		assert_eq!(serde_json::to_string(&result).unwrap(), json_str);
		assert_eq!(
			serde_json::from_str::<CennzxSpotResult<u128>>(json_str).unwrap(),
			result
		);
		serde_json::to_value(&result).unwrap(); // should not panic
	}
}
