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

//! RPC interface for the governance module.

use cennznet_primitives::types::AssetId;
pub use crml_eth_wallet_rpc_runtime_api::EthWalletApi as EthWalletRuntimeApi;
use crml_support::H160 as EthAddress;
use ethereum_types::U256;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{
	generic::BlockId,
	traits::{Block as BlockT, UniqueSaturatedInto},
};
use std::{convert::TryInto, sync::Arc};
mod types;
use types::*;

#[rpc]
pub trait EthWalletApi<BlockHash> {
	/// Get all governance proposal votes
	#[rpc(name = "ethWallet_addressNonce")]
	fn address_nonce(&self, eth_address: EthAddress, at: Option<BlockHash>) -> Result<u32>;
	/// Call contract, returning the output data.
	#[rpc(name = "eth_call")]
	fn eth_call(&self, _call: CallRequest, block: U256) -> Result<Bytes>;
	/// Call contract, returning the output data.
	#[rpc(name = "eth_chainId")]
	fn eth_chain_id(&self) -> Result<U256>;
	#[rpc(name = "net_version")]
	fn net_version(&self) -> Result<U256>;
	/// Get native token balance (CENNZ)
	#[rpc(name = "eth_getBalance")]
	fn eth_get_balance(&self, address: EthAddress, block: U256) -> Result<U256>;
	/// Get the latest block number
	#[rpc(name = "eth_blockNumber")]
	fn eth_block_number(&self) -> Result<U256>;
	/// Get the latest block hash by number
	#[rpc(name = "eth_getBlockByNumber")]
	fn eth_get_block_by_number(&self, _block_number: U256, details: bool) -> Result<Bytes>;
	/// Get the current gas price
	#[rpc(name = "eth_gasPrice")]
	fn eth_gas_price(&self) -> Result<U256>;
}

/// A struct that implements the [`GovernanceApi`].
pub struct EthWallet<C, P> {
	client: Arc<C>,
	_marker: std::marker::PhantomData<P>,
}

impl<C, P> EthWallet<C, P> {
	/// Create new `Governance` with the given reference to the client.
	pub fn new(client: Arc<C>) -> Self {
		Self {
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

impl<C, Block> EthWalletApi<<Block as BlockT>::Hash> for EthWallet<C, Block>
where
	Block: BlockT,
	C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	C::Api: EthWalletRuntimeApi<Block>,
{
	fn address_nonce(&self, eth_address: EthAddress, at: Option<<Block as BlockT>::Hash>) -> Result<u32> {
		let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

		self.client
			.runtime_api()
			.address_nonce(&at, &eth_address)
			.map_err(|e| RpcError {
				code: ErrorCode::ServerError(Error::RuntimeError as i64),
				message: "Unable to query Eth address nonce.".into(),
				data: Some(format!("{:?}", e).into()),
			})
	}

	fn eth_gas_price(&self) -> Result<U256> {
		Ok(U256::from(1))
	}

	fn eth_chain_id(&self) -> Result<U256> {
		Ok(U256::from(77))
	}

	fn net_version(&self) -> Result<U256> {
		Ok(U256::from(1))
	}

	fn eth_get_balance(&self, address: EthAddress, _block: U256) -> Result<U256> {
		let at = BlockId::hash(self.client.info().best_hash);
		let balance = self
			.client
			.runtime_api()
			.get_balance(&at, &address)
			.map_err(|e| RpcError {
				code: ErrorCode::ServerError(Error::RuntimeError as i64),
				message: "Unable to query CENNZ balance.".into(),
				data: Some(format!("{:?}", e).into()),
			})?;

		// metamask only supports 18 decimal places
		// scale up balances by a factor of 14 here to present the proper amount
		Ok(balance.saturating_mul(U256::from(10_u128.pow(14))))
	}

	fn eth_get_block_by_number(&self, _block_number: U256, _details: bool) -> Result<Bytes> {
		// let block_hash = self.client.hash(block_number.into())
		// 	.map_err(|e| RpcError {
		// 		code: ErrorCode::ServerError(Error::RuntimeError as i64),
		// 		message: "Unable to query block hash.".into(),
		// 		data: Some(format!("{:?}", e).into()),
		// })?;
		Ok(Bytes::new(Default::default()))
	}

	fn eth_block_number(&self) -> Result<U256> {
		let block_number = self.client.info().best_number;
		Ok(U256::from(UniqueSaturatedInto::<u128>::unique_saturated_into(
			block_number,
		)))
	}

	fn eth_call(&self, call_request: CallRequest, _block: U256) -> Result<Bytes> {
		let at = BlockId::hash(self.client.info().best_hash);

		let to_contract = if let Some(contract) = call_request.to {
			contract.0
		} else {
			return Err(RpcError {
				code: ErrorCode::ServerError(Error::RuntimeError as i64),
				message: "expected 'to' address.".into(),
				data: None,
			});
		};
		let address_prefix = to_contract[0];
		match address_prefix {
			// generic-asset
			1 => {
				let asset_id = AssetId::from_be_bytes(to_contract[1..5].try_into().unwrap_or_default());
				let calldata = call_request.data.unwrap_or_default();
				let result = self
					.client
					.runtime_api()
					.erc20_call(&at, asset_id, &calldata.into_vec())
					.map_err(|e| RpcError {
						code: ErrorCode::ServerError(Error::RuntimeError as i64),
						message: "Unable to query Eth address nonce.".into(),
						data: Some(format!("{:?}", e).into()),
					})?;

				Ok(result.into())
			}
			// e.g
			// nft
			// 2 => {
			// 	let collection_id = prefix[1..5];
			// 	let series_id = prefix[5..9];
			// 	erc721_shim::call(collection_id, series_id, call_request.data)
			// },
			// todo: return not found
			_ => Ok(Bytes::new(Default::default())),
		}
	}
}
