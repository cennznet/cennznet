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
use codec::{Codec, Decode, Encode};
use crml_generic_asset::AssetInfo;
pub use crml_generic_asset_rpc_runtime_api::GenericAssetRuntimeApi;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use serde::{Deserialize, Serialize};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::{fmt::Display, str::FromStr, sync::Arc};

#[rpc]
pub trait GenericAssetApi<AssetId, Balance, AccountId, BlockHash, ResponseType>
where
	Balance: FromStr + Display,
{
	/// Get all assets data paired with their ids.
	#[rpc(name = "genericAsset_registeredAssets")]
	fn asset_meta(&self, at: Option<BlockHash>) -> Result<ResponseType>;

	#[rpc(name = "genericAsset_getBalance")]
	fn get_balance(
		&self,
		account_id: AccountId,
		asset_id: AssetId,
		at: Option<BlockHash>,
	) -> Result<BalanceInformation<Balance>>;
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

#[derive(Eq, PartialEq, Decode, Encode, Default, Debug, Serialize, Deserialize)]
#[serde(bound(serialize = "Balance: std::fmt::Display"))]
#[serde(bound(deserialize = "Balance: std::str::FromStr"))]
pub struct BalanceInformation<Balance> {
	#[serde(with = "serde_balance")]
	reserved: Balance,
	#[serde(with = "serde_balance")]
	staked: Balance,
	#[serde(with = "serde_balance")]
	available: Balance,
}

mod serde_balance {
	use serde::{Deserialize, Deserializer, Serializer};

	pub fn serialize<S: Serializer, Balance: std::fmt::Display>(t: &Balance, serializer: S) -> Result<S::Ok, S::Error> {
		serializer.serialize_str(&t.to_string())
	}

	pub fn deserialize<'de, D: Deserializer<'de>, Balance: std::str::FromStr>(
		deserializer: D,
	) -> Result<Balance, D::Error> {
		let s = String::deserialize(deserializer)?;
		s.parse::<Balance>()
			.map_err(|_| serde::de::Error::custom("Parse from string failed"))
	}
}

/// Error type of this RPC api.
pub enum Error {
	/// The call to runtime failed.
	RuntimeError,
}

impl<C, Block, AssetId, Balance, AccountId>
	GenericAssetApi<AssetId, Balance, AccountId, <Block as BlockT>::Hash, Vec<(AssetId, AssetInfo)>>
	for GenericAsset<C, (Block, AssetId, Balance, AccountId)>
where
	Block: BlockT,
	C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	C::Api: GenericAssetRuntimeApi<Block, AssetId, Balance, AccountId>,
	AssetId: Decode + Encode + Send + Sync + 'static,
	Balance: Codec + Sync + std::marker::Send + 'static + Display + FromStr,
	AccountId: Codec + Sync + std::marker::Send + 'static,
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

	fn get_balance(
		&self,
		account_id: AccountId,
		asset_id: AssetId,
		at: Option<<Block as BlockT>::Hash>,
	) -> Result<BalanceInformation<Balance>> {
		let api = self.client.runtime_api();
		let at = BlockId::hash(at.unwrap_or_else(||
			// If the block hash is not supplied assume the best block.
			self.client.info().best_hash));

		let result = api.get_balance(&at, account_id, asset_id).map_err(|e| RpcError {
			code: ErrorCode::ServerError(Error::RuntimeError as i64),
			message: "Unable to query balances.".into(),
			data: Some(format!("{:?}", e).into()),
		})?;

		Ok(BalanceInformation {
			reserved: result.reserved,
			staked: result.staked,
			available: result.available,
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
