// This file is part of CENNZnet.

// Copyright (C) 2019-2021 Centrality Investments Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! RPC interface for the staking module.

use codec::{Codec, Decode};

use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;

use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_rpc::number::NumberOrHex;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};

use std::{convert::TryFrom, sync::Arc};

pub use crml_staking_rpc_runtime_api::StakingApi as StakingRuntimeApi;

/// Staking custom RPC methods
#[rpc]
pub trait StakingApi<BlockHash, AccountId, Balance> {
	/// Return the currently accrued reward for the specified payee (validator or nominator)
	///
	/// The actual reward to the payee at the end of the current era would be higher or equal the
	/// result of this method.
	///
	/// Returns error if the payee is not in the list of the stakers  
	#[rpc(name = "staking_nextPayout")]
	fn next_payout(&self, payee: AccountId, at: Option<BlockHash>) -> Result<Balance>;
}

/// A struct that implements [`StakingApi`].
pub struct Staking<C, P> {
	client: Arc<C>,
	_marker: std::marker::PhantomData<P>,
}

impl<C, P> Staking<C, P> {
	/// Create new `Staking` with the given reference to the client.
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
	/// The query is not supported.
	UnsupportedError,
}

impl From<Error> for i64 {
	fn from(e: Error) -> i64 {
		match e {
			Error::RuntimeError => 1,
			Error::UnsupportedError => 2,
		}
	}
}

impl<C, Block, Balance, AccountId> StakingApi<<Block as BlockT>::Hash, AccountId, Balance> for Staking<C, Block>
where
	Block: BlockT,
	C: 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	C::Api: StakingRuntimeApi<Block, AccountId, Balance>,
	AccountId: Codec,
	Balance: Codec + TryFrom<NumberOrHex>,
{
	fn next_payout(&self, payee: AccountId, at: Option<<Block as BlockT>::Hash>) -> Result<Balance> {
		let api = self.client.runtime_api();

		if at.is_some() {
			RpcError {
				code: ErrorCode::ServerError(Error::UnsupportedError.into()),
				message: "Unsupported query when block hash is given.".into(),
				data: None,
			}
		}

		api.next_payout(&at, payee).map_err(|e| RpcError {
			code: ErrorCode::ServerError(Error::RuntimeError.into()),
			message: "Unable to query next payout.".into(),
			data: Some(format!("{:?}", e).into()),
		})
	}
}
