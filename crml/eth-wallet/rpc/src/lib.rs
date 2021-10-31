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

pub use crml_eth_wallet_rpc_runtime_api::EthWalletApi as EthWalletRuntimeApi;
use crml_support::H160 as EthAddress;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::sync::Arc;

#[rpc]
pub trait EthWalletApi<BlockHash> {
	/// Get all governance proposal votes
	#[rpc(name = "ethWallet_addressNonce")]
	fn address_nonce(&self, eth_address: EthAddress, at: Option<BlockHash>) -> Result<u32>;
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
}
