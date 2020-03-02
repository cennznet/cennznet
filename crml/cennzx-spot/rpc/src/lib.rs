// Copyright 2019-2020 Parity Technologies (UK) Ltd.
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

use std::sync::Arc;

use codec::Codec;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::{Block as BlockT}};

pub use self::gen_client::Client as CennzxSpotClient;
pub use crml_cennzx_spot_rpc_runtime_api::{
	self as runtime_api, CennzxSpotApi as CennzxSpotRuntimeApi,
};

/// Contracts RPC methods.
#[rpc]
pub trait CennzxSpotApi<AssetId, Balance> {
	#[rpc(name = "cennzx_buyPrice")]
	fn buy_price(
		&self,
		asset_to_buy: AssetId,
		amount_to_buy: Balance,
		asset_to_pay: AssetId,
	) -> Result<Balance>;

	#[rpc(name = "cennzx_salePrice")]
	fn sale_price(
		&self,
		asset_to_sell: AssetId,
		amount_to_buy: Balance,
		asset_to_payout: AssetId,
	) -> Result<Balance>;
}

/// An implementation of CENNZX Spot Exchange specific RPC methods.
pub struct CennzxSpot<C, T> {
	client: Arc<C>,
	_marker: std::marker::PhantomData<T>,
}

impl<C, T> CennzxSpot<C, T> {
	/// Create new `CennzxSpot` with the given reference to the client.
	pub fn new(client: Arc<C>) -> Self {
		CennzxSpot { client, _marker: Default::default() }
	}
}

/// Error type of this RPC api.
pub enum Error {
	/// The transaction was not decodable.
	DecodeError,
	/// The call to runtime failed.
	RuntimeError,
}

impl From<Error> for i64 {
	fn from(e: Error) -> i64 {
		match e {
			Error::RuntimeError => 1,
			Error::DecodeError => 2,
		}
	}
}


impl<C, Block, AssetId, Balance> CennzxSpotApi<AssetId, Balance>
	for CennzxSpot<C, Block>
where
	Block: BlockT,
	C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	C::Api: CennzxSpotRuntimeApi<Block, AssetId, Balance>,
	AssetId: Codec,
	Balance: Codec,
{
	fn buy_price(
		&self,
		asset_to_buy: AssetId,
		amount_to_buy: Balance,
		asset_to_pay: AssetId,
	) -> Result<Balance> {
		let api = self.client.runtime_api();
		let best = self.client.info().best_hash;
		let at = BlockId::hash(best);

		api.buy_price(&at, asset_to_buy, amount_to_buy, asset_to_pay).map_err(|e| RpcError {
			code: ErrorCode::ServerError(Error::RuntimeError.into()),
			message: "Unable to query buy price.".into(),
			data: Some(format!("{:?}", e).into()),
		}).map(|price| price.into())
	}

	fn sale_price(
		&self,
		asset_for_sale: AssetId,
		amount_for_sale: Balance,
		asset_to_payout: AssetId,
	) -> Result<Balance> {
		let api = self.client.runtime_api();
		let best = self.client.info().best_hash;
		let at = BlockId::hash(best);

		api.sell_value(&at, asset_for_sale, amount_for_sale, asset_to_payout).map_err(|e| RpcError {
			code: ErrorCode::ServerError(Error::RuntimeError.into()),
			message: "Unable to query sell price.".into(),
			data: Some(format!("{:?}", e).into()),
		}).map(|price| price.into())
	}

}
