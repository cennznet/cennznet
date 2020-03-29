/* Copyright 2019-2020 Centrality Investments Limited
*
* Licensed under the LGPL, Version 3.0 (the "License");
* you may not use this file except in compliance with the License.
* Unless required by applicable law or agreed to in writing, software
* distributed under the License is distributed on an "AS IS" BASIS,
* WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
* See the License for the specific language governing permissions and
* limitations under the License.
* You may obtain a copy of the License at the root of this project source code,
* or at:
*     https://centrality.ai/licenses/gplv3.txt
*     https://centrality.ai/licenses/lgplv3.txt
*/

//! Node-specific RPC methods for interaction with CENNZX.

use std::{convert::TryInto, sync::Arc};

use codec::Codec;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use sp_api::ProvideRuntimeApi;
use sp_arithmetic::traits::{BaseArithmetic, SaturatedConversion};
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};

pub use self::gen_client::Client as CennzxSpotClient;
pub use crml_cennzx_spot_rpc_runtime_api::{
	self as runtime_api, CennzxSpotApi as CennzxSpotRuntimeApi, CennzxSpotResult,
};

/// Contracts RPC methods.
#[rpc]
pub trait CennzxSpotApi<AssetId, Balance> {
	#[rpc(name = "cennzx_buyPrice")]
	// TODO: prefer to return Result<Balance>, however Serde JSON library only allows u64.
	//  - change to Result<Balance> once https://github.com/serde-rs/serde/pull/1679 is merged
	fn buy_price(&self, asset_to_buy: AssetId, amount_to_buy: Balance, asset_to_pay: AssetId) -> Result<u64>;

	#[rpc(name = "cennzx_sellPrice")]
	// TODO: change to Result<Balance> once https://github.com/serde-rs/serde/pull/1679 is merged
	fn sell_price(&self, asset_to_sell: AssetId, amount_to_buy: Balance, asset_to_payout: AssetId) -> Result<u64>;
}

/// An implementation of CENNZX Spot Exchange specific RPC methods.
pub struct CennzxSpot<C, T> {
	client: Arc<C>,
	_marker: std::marker::PhantomData<T>,
}

impl<C, T> CennzxSpot<C, T> {
	/// Create new `CennzxSpot` with the given reference to the client.
	pub fn new(client: Arc<C>) -> Self {
		CennzxSpot {
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

impl<C, Block, AssetId, Balance> CennzxSpotApi<AssetId, Balance> for CennzxSpot<C, Block>
where
	Block: BlockT,
	C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	C::Api: CennzxSpotRuntimeApi<Block, AssetId, Balance>,
	AssetId: Codec,
	Balance: Codec + BaseArithmetic,
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
			CennzxSpotResult::Success(price) => {
				TryInto::<u64>::try_into(price.saturated_into::<u128>()).map_err(|e| RpcError {
					code: ErrorCode::ServerError(Error::PriceOverflow.into()),
					message: "Price too large.".into(),
					data: Some(format!("{:?}", e).into()),
				})
			}
			CennzxSpotResult::Error => Err(RpcError {
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
			CennzxSpotResult::Success(price) => {
				TryInto::<u64>::try_into(price.saturated_into::<u128>()).map_err(|e| RpcError {
					code: ErrorCode::ServerError(Error::PriceOverflow.into()),
					message: "Price too large.".into(),
					data: Some(format!("{:?}", e).into()),
				})
			}
			CennzxSpotResult::Error => Err(RpcError {
				code: ErrorCode::ServerError(Error::CannotExchange.into()),
				message: "Cannot exchange by requested amount.".into(),
				data: Some("".into()),
			}),
		}
	}
}
