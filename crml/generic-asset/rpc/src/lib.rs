// Copyright 2019-2021 Parity Technologies (UK) Ltd.
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

//! RPC interface for the generic asset module.

pub use self::gen_client::Client as GenericAssetClient;
use codec::{Decode, Encode};
use crml_generic_asset::AssetInfo;
pub use crml_generic_asset_rpc_runtime_api::AssetMetaApi;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::sync::Arc;

#[rpc]
pub trait GenericAssetApi<BlockHash, ResponseType> {
	/// Get all assets data paired with their ids.
	#[rpc(name = "genericAsset_registeredAssets")]
	fn asset_meta(&self, at: Option<BlockHash>) -> Result<ResponseType>;
}

/// A struct that implements the [`GenericAssetApi`].
pub struct GenericAsset<C, P> {
	client: Arc<C>,
	_marker: std::marker::PhantomData<P>,
}

impl<C, P> GenericAsset<C, P> {
	/// Create new `GenericAsset` with the given reference to the client.
	pub fn new(client: Arc<C>) -> Self {
		GenericAsset {
			client,
			_marker: Default::default(),
		}
	}
}

/// Error type of this RPC api.
pub enum Error {
	/// The call to runtime failed.
	RuntimeError,
}

impl<C, Block, AssetId> GenericAssetApi<<Block as BlockT>::Hash, Vec<(AssetId, AssetInfo)>>
	for GenericAsset<C, (Block, AssetId)>
where
	Block: BlockT,
	C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	C::Api: AssetMetaApi<Block, AssetId>,
	AssetId: Decode + Encode + Send + Sync + 'static,
{
	fn asset_meta(&self, at: Option<<Block as BlockT>::Hash>) -> Result<Vec<(AssetId, AssetInfo)>> {
		let at = BlockId::hash(at.unwrap_or_else(||
			// If the block hash is not supplied assume the best block.
			self.client.info().best_hash));

		self.client.runtime_api().asset_meta(&at).map_err(|e| RpcError {
			code: ErrorCode::ServerError(Error::RuntimeError as i64),
			message: "Unable to query asset meta data.".into(),
			data: Some(format!("{:?}", e).into()),
		})
	}
}

// #[cfg(test)]
// mod test {
// 	use super::{GenericAsset, GenericAssetApi};
// 	use jsonrpc_core::IoHandler;
// 	use std::sync::Arc;
// 	use substrate_test_runtime_client::{
// 		DefaultTestClientBuilderExt, TestClient, TestClientBuilder, TestClientBuilderExt,
// 	};

// 	fn test_ga_rpc_handler<P>() -> GenericAsset<TestClient, P> {
// 		let builder = TestClientBuilder::new();
// 		let (client, _) = builder.build_with_longest_chain();
// 		let client = Arc::new(client);

// 		GenericAsset::new(client)
// 	}

// 	#[test]
// 	fn working_registered_assets_rpc() {
// 		let handler = test_ga_rpc_handler();
// 		let mut io = IoHandler::new();
// 		io.extend_with(GenericAssetApi::to_delegate(handler));

// 		let request = r#"{
// 			"id":"1", "jsonrpc":"2.0",
// 			"method": "genericAsset_registeredAssets",
// 			"params":[]}"#;
// 		let response = "{\"jsonrpc\":\"2.0\",\
// 			\"result\":[[0,{\
// 			\"decimal_places\":4,\
// 			\"existential_deposit\":1,\
// 			\"symbol\":[]}]],\
// 			\"id\":\"1\"}";

// 		assert_eq!(Some(response.into()), io.handle_request_sync(request));
// 	}
// }
