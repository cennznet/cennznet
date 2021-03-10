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

use codec::Codec;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use sp_api::ProvideRuntimeApi;
use sp_arithmetic::traits::{BaseArithmetic, SaturatedConversion};
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
	fn buy_price(&self, asset_to_buy: AssetId, amount_to_buy: Balance, asset_to_pay: AssetId) -> Result<u64>;

	#[rpc(name = "cennzx_sellPrice")]
	// TODO: change to Result<Balance> once https://github.com/serde-rs/serde/pull/1679 is merged
	fn sell_price(&self, asset_to_sell: AssetId, amount_to_buy: Balance, asset_to_payout: AssetId) -> Result<u64>;

	#[rpc(name = "cennzx_liquidityValue")]
	// TODO: change to Result<Balance> once https://github.com/serde-rs/serde/pull/1679 is merged
	fn liquidity_value(&self, account_id: AccountId, asset_id: AssetId) -> Result<(u64, u64, u64)>;

	#[rpc(name = "cennzx_liquidityPrice")]
	// TODO: change to Result<Balance> once https://github.com/serde-rs/serde/pull/1679 is merged
	fn liquidity_price(&self, asset_id: AssetId, liquidity_to_buy: Balance) -> Result<(u64, u64)>;
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
	fn buy_price(&self, asset_to_buy: AssetId, amount_to_buy: Balance, asset_to_pay: AssetId) -> Result<u64> {
		let api = self.client.runtime_api();
		let best = self.client.info().best_hash;
		let at = BlockId::hash(best);

		let result = api
			.buy_price(&at, asset_to_buy, amount_to_buy, asset_to_pay)
			.map_err(|e| RpcError {
				code: ErrorCode::ServerError(Error::Runtime.into()),
				message: "Unable to query buy price.".into(),
				data: Some(format!("{:?}", e).into()),
			})?;
		match result {
			CennzxResult::Success(price) => TryInto::<u64>::try_into(price.saturated_into::<u128>()).map_err(|e| RpcError {
				code: ErrorCode::ServerError(Error::PriceOverflow.into()),
				message: "Price too large.".into(),
				data: Some(format!("{:?}", e).into()),
			}),
			CennzxResult::Error => Err(RpcError {
				code: ErrorCode::ServerError(Error::CannotExchange.into()),
				message: "Cannot exchange for requested amount.".into(),
				data: Some("".into()),
			}),
		}
	}

	fn sell_price(&self, asset_to_sell: AssetId, amount_to_sell: Balance, asset_to_payout: AssetId) -> Result<u64> {
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
			CennzxResult::Success(price) => TryInto::<u64>::try_into(price.saturated_into::<u128>()).map_err(|e| RpcError {
				code: ErrorCode::ServerError(Error::PriceOverflow.into()),
				message: "Price too large.".into(),
				data: Some(format!("{:?}", e).into()),
			}),
			CennzxResult::Error => Err(RpcError {
				code: ErrorCode::ServerError(Error::CannotExchange.into()),
				message: "Cannot exchange by requested amount.".into(),
				data: Some("".into()),
			}),
		}
	}

	fn liquidity_value(&self, account: AccountId, asset_id: AssetId) -> Result<(u64, u64, u64)> {
		let api = self.client.runtime_api();
		let best = self.client.info().best_hash;
		let at = BlockId::hash(best);

		let result = api.liquidity_value(&at, account, asset_id).map_err(|e| RpcError {
			code: ErrorCode::ServerError(Error::Runtime.into()),
			message: "Unable to query liquidity value.".into(),
			data: Some(format!("{:?}", e).into()),
		})?;

		let liquidity = TryInto::<u64>::try_into(result.0.saturated_into::<u128>()).map_err(|e| RpcError {
			code: ErrorCode::ServerError(Error::PriceOverflow.into()),
			message: "Liquidity too large.".into(),
			data: Some(format!("{:?}", e).into()),
		})?;
		let core = TryInto::<u64>::try_into(result.1.saturated_into::<u128>()).map_err(|e| RpcError {
			code: ErrorCode::ServerError(Error::PriceOverflow.into()),
			message: "Core asset too large.".into(),
			data: Some(format!("{:?}", e).into()),
		})?;
		let asset = TryInto::<u64>::try_into(result.2.saturated_into::<u128>()).map_err(|e| RpcError {
			code: ErrorCode::ServerError(Error::PriceOverflow.into()),
			message: "Trade asset too large.".into(),
			data: Some(format!("{:?}", e).into()),
		})?;
		Ok((liquidity, core, asset))
	}

	fn liquidity_price(&self, asset_id: AssetId, liquidity_to_buy: Balance) -> Result<(u64, u64)> {
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

		let core = TryInto::<u64>::try_into(result.0.saturated_into::<u128>()).map_err(|e| RpcError {
			code: ErrorCode::ServerError(Error::PriceOverflow.into()),
			message: "Core asset too large.".into(),
			data: Some(format!("{:?}", e).into()),
		})?;
		let asset = TryInto::<u64>::try_into(result.1.saturated_into::<u128>()).map_err(|e| RpcError {
			code: ErrorCode::ServerError(Error::PriceOverflow.into()),
			message: "Trade asset too large.".into(),
			data: Some(format!("{:?}", e).into()),
		})?;
		Ok((core, asset))
	}
}
