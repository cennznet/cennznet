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

use std::{convert::TryInto, sync::Arc};

use codec::{Codec, Decode};
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
pub trait CennzxApi<AssetId, Balance, AccountId> {
	#[rpc(name = "cennzx_buyPrice")]
	// TODO: prefer to return Result<Balance>, however Serde JSON library only allows u64.
	//  - change to Result<Balance> once https://github.com/serde-rs/serde/pull/1679 is merged
	fn buy_price(&self, asset_to_buy: AssetId, amount_to_buy: Vec<u8>, asset_to_pay: AssetId) -> Result<Vec<u8>>;

	#[rpc(name = "cennzx_sellPrice")]
	// TODO: change to Result<Balance> once https://github.com/serde-rs/serde/pull/1679 is merged
	fn sell_price(&self, asset_to_sell: AssetId, amount_to_buy: Vec<u8>, asset_to_payout: AssetId) -> Result<Vec<u8>>;

	#[rpc(name = "cennzx_liquidityValue")]
	// TODO: change to Result<Balance> once https://github.com/serde-rs/serde/pull/1679 is merged
	fn liquidity_value(&self, account_id: AccountId, asset_id: AssetId) -> Result<LiquidityValueRPC>;

	#[rpc(name = "cennzx_liquidityPrice")]
	// TODO: change to Result<Balance> once https://github.com/serde-rs/serde/pull/1679 is merged
	fn liquidity_price(&self, asset_id: AssetId, liquidity_to_buy: Balance) -> Result<LiquidityPriceRPC>;
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

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct LiquidityValueRPC {
	liquidity: Vec<u8>,
	core: Vec<u8>,
	asset: Vec<u8>,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct LiquidityPriceRPC {
	core: Vec<u8>,
	asset: Vec<u8>,
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
	Balance: Codec + BaseArithmetic,
	AccountId: Codec,
{
	fn buy_price(&self, asset_to_buy: AssetId, amount_to_buy: Vec<u8>, asset_to_pay: AssetId) -> Result<Vec<u8>> {
		let api = self.client.runtime_api();
		let best = self.client.info().best_hash;
		let at = BlockId::hash(best);

		let amount_to_buy: Balance = Decode::decode(&mut amount_to_buy.as_slice()).unwrap();

		let result = api
			.buy_price(&at, asset_to_buy, amount_to_buy, asset_to_pay)
			.map_err(|e| RpcError {
				code: ErrorCode::ServerError(Error::Runtime.into()),
				message: "Unable to query buy price.".into(),
				data: Some(format!("{:?}", e).into()),
			})?;

		match result {
			CennzxResult::Success(price) => {
				let price = price.encode();
				TryInto::<Vec<u8>>::try_into(price).map_err(|e| RpcError {
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

	fn sell_price(&self, asset_to_sell: AssetId, amount_to_sell: Vec<u8>, asset_to_payout: AssetId) -> Result<Vec<u8>> {
		let api = self.client.runtime_api();
		let best = self.client.info().best_hash;
		let at = BlockId::hash(best);

		let amount_to_sell: Balance = Decode::decode(&mut amount_to_sell.as_slice()).unwrap();

		let result = api
			.sell_price(&at, asset_to_sell, amount_to_sell, asset_to_payout)
			.map_err(|e| RpcError {
				code: ErrorCode::ServerError(Error::Runtime.into()),
				message: "Unable to query sell price.".into(),
				data: Some(format!("{:?}", e).into()),
			})?;
		match result {
			CennzxResult::Success(price) => {
				let price = price.encode();
				TryInto::<Vec<u8>>::try_into(price).map_err(|e| RpcError {
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

	fn liquidity_value(&self, account: AccountId, asset_id: AssetId) -> Result<LiquidityValueRPC> {
		let api = self.client.runtime_api();
		let best = self.client.info().best_hash;
		let at = BlockId::hash(best);

		let result = api.liquidity_value(&at, account, asset_id).map_err(|e| RpcError {
			code: ErrorCode::ServerError(Error::Runtime.into()),
			message: "Unable to query liquidity value.".into(),
			data: Some(format!("{:?}", e).into()),
		})?;

		Ok(LiquidityValueRPC {
			liquidity: result.0.encode(),
			core: result.1.encode(),
			asset: result.2.encode(),
		})
	}

	fn liquidity_price(&self, asset_id: AssetId, liquidity_to_buy: Balance) -> Result<LiquidityPriceRPC> {
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

		Ok(LiquidityPriceRPC {
			core: result.0.encode(),
			asset: result.1.encode(),
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
		let info = LiquidityValueRPC {
			liquidity: u128::MAX.encode(),
			core: u128::MAX.encode(),
			asset: u128::MAX.encode(),
		};

		let json_str = "{\"liquidity\":[255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255],\
			\"core\":[255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255],\
			\"asset\":[255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255]}";

		assert_eq!(serde_json::to_string(&info).unwrap(), String::from(json_str));
		assert_eq!(serde_json::from_str::<LiquidityValueRPC>(json_str).unwrap(), info);

		//let info: u128 = Decode::decode(&mut info.as_slice()).unwrap();
		//assert_eq!(info, b);
	}
}
