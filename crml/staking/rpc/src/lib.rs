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

use codec::Codec;

use jsonrpsee::{
	core::{Error as RpcError, RpcResult},
	proc_macros::rpc,
};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};

use std::sync::Arc;

pub use crml_staking_rpc_runtime_api::StakingApi as StakingRuntimeApi;

/// Staking custom RPC methods
#[rpc(client, server, namespace = "staking")]
pub trait StakingApi<BlockHash, AccountId> {
	/// Return the currently accrued reward for the specified stash (validator or nominator)
	///
	/// The actual reward to the stash at the end of the current era would be higher or equal the
	/// result of this method.
	///
	/// Returns error if the payee is not in the list of the stakers
	// TODO: we should return Result<Balance>, however we need to update Plug to bring in the latest sp-rpc package before that
	#[method(name = "accruedPayout")]
	fn accrued_payout(&self, stash: AccountId, at: Option<BlockHash>) -> RpcResult<u64>;
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

impl<C, Block, AccountId> StakingApiServer<<Block as BlockT>::Hash, AccountId> for Staking<C, Block>
where
	Block: BlockT,
	C: 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	C::Api: StakingRuntimeApi<Block, AccountId>,
	AccountId: Codec,
{
	fn accrued_payout(&self, stash: AccountId, at: Option<<Block as BlockT>::Hash>) -> RpcResult<u64> {
		let api = self.client.runtime_api();

		if at.is_some() {
			return Err(RpcError::Custom("Unsupported query when block hash is given.".into()));
		}

		let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

		api.accrued_payout(&at, &stash).map_err(|e| RpcError::to_call_error(e))
	}
}
