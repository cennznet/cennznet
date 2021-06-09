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

//! Node-specific RPC methods for interaction with CENNZX.

use std::{convert::TryInto, sync::Arc, str::FromStr, fmt::Display};

use codec::{Codec, Decode, Encode};
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use serde::{Deserialize, Serialize};
use sp_api::ProvideRuntimeApi;
use sp_arithmetic::traits::BaseArithmetic;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};

pub use self::gen_client::Client as CennzxClient;
pub use crml_cennzx_rpc_runtime_api::{self as runtime_api, CennzxApi as CennzxRuntimeApi, CennzxResult};

/// Contracts RPC methods.
#[rpc]
pub trait CennzxApi<AssetId, Balance, AccountId> where
	Balance: FromStr + Display,
{
	#[rpc(name = "cennzx_buyPrice")]
	fn buy_price(&self, asset_to_buy: AssetId, amount_to_buy: Balance, asset_to_pay: AssetId) -> Result<BuyPriceResponse<Balance>>;

	#[rpc(name = "cennzx_sellPrice")]
	fn sell_price(&self, asset_to_sell: AssetId, amount_to_buy: Balance, asset_to_payout: AssetId) -> Result<SellPriceResponse<Balance>>;

	#[rpc(name = "cennzx_liquidityValue")]
	fn liquidity_value(&self, account_id: AccountId, asset_id: AssetId) -> Result<LiquidityValueResponse<Balance>>;

	#[rpc(name = "cennzx_liquidityPrice")]
	fn liquidity_price(&self, asset_id: AssetId, liquidity_to_buy: Balance) -> Result<LiquidityPriceResponse<Balance>>;
}

/// An implementation of CENNZX Spot Exchange specific RPC methods.
pub struct Cennzx<C, T> {
	client: Arc<C>,
	_marker: std::marker::PhantomData<T>,
}

impl<C, T> Cennzx<C, T> {
	/// Create new `Cennzx` with the given reference to the client.
	pub fn new(client: Arc<C>) -> Self {
		Cennzx {
			client,
			_marker: Default::default(),
		}
	}
}

#[derive(Eq, PartialEq, Decode, Encode, Default, Debug, Serialize, Deserialize)]
#[serde(bound(serialize = "Balance: std::fmt::Display"))]
#[serde(bound(deserialize = "Balance: std::str::FromStr"))]
pub struct BuyPriceResponse<Balance> {
	#[serde(with = "serde_balance")]
	price: Balance,
}

#[derive(Eq, PartialEq, Decode, Encode, Default, Debug, Serialize, Deserialize)]
#[serde(bound(serialize = "Balance: std::fmt::Display"))]
#[serde(bound(deserialize = "Balance: std::str::FromStr"))]
pub struct SellPriceResponse<Balance> {
	#[serde(with = "serde_balance")]
	price: Balance,
}

#[derive(Eq, PartialEq, Decode, Encode, Default, Debug, Serialize, Deserialize)]
#[serde(bound(serialize = "Balance: std::fmt::Display"))]
#[serde(bound(deserialize = "Balance: std::str::FromStr"))]
pub struct LiquidityValueResponse<Balance> {
	#[serde(with = "serde_balance")]
	liquidity: Balance,

	#[serde(with = "serde_balance")]
	core: Balance,

	#[serde(with = "serde_balance")]
	asset: Balance,
}

#[derive(Eq, PartialEq, Decode, Encode, Default, Debug, Serialize, Deserialize)]
#[serde(bound(serialize = "Balance: std::fmt::Display"))]
#[serde(bound(deserialize = "Balance: std::str::FromStr"))]
pub struct LiquidityPriceResponse<Balance> {
	#[serde(with = "serde_balance")]
	core: Balance,

	#[serde(with = "serde_balance")]
	asset: Balance,
}

mod serde_balance {
	use serde::{Deserialize, Deserializer, Serializer};

	pub fn serialize<S: Serializer, Balance: std::fmt::Display>(t: &Balance, serializer: S) -> Result<S::Ok, S::Error> {
		serializer.serialize_str(&t.to_string())
	}

	pub fn deserialize<'de, D: Deserializer<'de>, Balance: std::str::FromStr>(deserializer: D) -> Result<Balance, D::Error> {
		let s = String::deserialize(deserializer)?;
		s.parse::<Balance>()
			.map_err(|_| serde::de::Error::custom("Parse from string failed"))
	}
}

/// Error type of this RPC api.
pub enum Error {
	/// The call to runtime failed.
	Runtime,
	CannotExchange,
	PriceOverflow,
}

impl From<Error> for i64 {
	fn from(e: Error) -> i64 {
		match e {
			Error::Runtime => 1,
			Error::CannotExchange => 2,
			Error::PriceOverflow => 3,
		}
	}
}

impl<C, Block, AssetId, Balance, AccountId> CennzxApi<AssetId, Balance, AccountId> for Cennzx<C, Block>
where
	Block: BlockT,
	C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	C::Api: CennzxRuntimeApi<Block, AssetId, Balance, AccountId>,
	AssetId: Codec,
	Balance: Codec + BaseArithmetic + FromStr + Display,
	AccountId: Codec,
{
	fn buy_price(&self, asset_to_buy: AssetId, amount_to_buy: Balance, asset_to_pay: AssetId) -> Result<BuyPriceResponse<Balance>> {
		let api = self.client.runtime_api();
		let best = self.client.info().best_hash;
		let at = BlockId::hash(best);

		//let amount_to_buy: Balance = Decode::decode(&mut amount_to_buy.as_slice()).unwrap();

		let result = api
			.buy_price(&at, asset_to_buy, amount_to_buy, asset_to_pay)
			.map_err(|e| RpcError {
				code: ErrorCode::ServerError(Error::Runtime.into()),
				message: "Unable to query buy price.".into(),
				data: Some(format!("{:?}", e).into()),
			})?;

		match result {
			CennzxResult::Success(price) => {
				TryInto::<Balance>::try_into(price).map_err(|e| RpcError {
					code: ErrorCode::ServerError(Error::PriceOverflow.into()),
					message: "Price too large.".into(),
					data: Some(format!("{:?}", e).into()),
				})
			}
			CennzxResult::Error => Err(RpcError {
				code: ErrorCode::ServerError(Error::CannotExchange.into()),
				message: "Cannot exchange for requested amount.".into(),
				data: Some("".into()),
			}),
		}
	}

	fn sell_price(&self, asset_to_sell: AssetId, amount_to_sell: Balance, asset_to_payout: AssetId) -> Result<SellPriceResponse<Balance>> {
		let api = self.client.runtime_api();
		let best = self.client.info().best_hash;
		let at = BlockId::hash(best);

		let result = api
			.sell_price(&at, asset_to_sell, amount_to_sell, asset_to_payout)
			.map_err(|e| RpcError {
				code: ErrorCode::ServerError(Error::Runtime.into()),
				message: "Unable to query sell price.".into(),
				data: Some(format!("{:?}", e).into()),
			})?;

		match result {
			CennzxResult::Success(price) => {
				TryInto::<Balance>::try_into(price).map_err(|e| RpcError {
					code: ErrorCode::ServerError(Error::PriceOverflow.into()),
					message: "Price too large.".into(),
					data: Some(format!("{:?}", e).into()),
				})
			}
			CennzxResult::Error => Err(RpcError {
				code: ErrorCode::ServerError(Error::CannotExchange.into()),
				message: "Cannot exchange by requested amount.".into(),
				data: Some("".into()),
			}),
		}
	}

	fn liquidity_value(&self, account: AccountId, asset_id: AssetId) -> Result<LiquidityValueResponse<Balance>> {
		let api = self.client.runtime_api();
		let best = self.client.info().best_hash;
		let at = BlockId::hash(best);

		let result = api.liquidity_value(&at, account, asset_id).map_err(|e| RpcError {
			code: ErrorCode::ServerError(Error::Runtime.into()),
			message: "Unable to query liquidity value.".into(),
			data: Some(format!("{:?}", e).into()),
		})?;

		Ok(LiquidityValueResponse {
			liquidity: result.0,
			core: result.1,
			asset: result.2,
		})
	}

	fn liquidity_price(&self, asset_id: AssetId, liquidity_to_buy: Balance) -> Result<LiquidityPriceResponse<Balance>> {
		let api = self.client.runtime_api();
		let best = self.client.info().best_hash;
		let at = BlockId::hash(best);

		let result = api
			.liquidity_price(&at, asset_id, liquidity_to_buy)
			.map_err(|e| RpcError {
				code: ErrorCode::ServerError(Error::Runtime.into()),
				message: "Unable to query liquidity price.".into(),
				data: Some(format!("{:?}", e).into()),
			})?;

		Ok(LiquidityPriceResponse {
			core: result.0,
			asset: result.1,
		})
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use codec::{Decode, Encode};

	#[test]
	fn working_cennzx_u128_rpc() {
		let b: u128 = u128::MAX;
		let info: Vec<u8> = b.encode();
		let json_str = "[255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255]";

		assert_eq!(serde_json::to_string(&info).unwrap(), String::from(json_str));
		assert_eq!(serde_json::from_str::<Vec<u8>>(json_str).unwrap(), info);

		let info: u128 = Decode::decode(&mut info.as_slice()).unwrap();
		assert_eq!(info, b);
	}

	#[test]
	fn working_cennzx_struct_rpc() {
		let info = LiquidityValueResponse {
			liquidity: u128::MAX.encode(),
			core: u128::MAX.encode(),
			asset: u128::MAX.encode(),
		};

		let json_str = "{\"liquidity\":[255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255],\
			\"core\":[255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255],\
			\"asset\":[255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255]}";

		assert_eq!(serde_json::to_string(&info).unwrap(), String::from(json_str));
		assert_eq!(serde_json::from_str::<LiquidityValueResponse>(json_str).unwrap(), info);

		//let info: u128 = Decode::decode(&mut info.as_slice()).unwrap();
		//assert_eq!(info, b);
	}
}
