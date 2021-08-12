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

pub use self::gen_client::Client as GovernanceClient;
use codec::Codec;
use crml_governance::ProposalId;
pub use crml_governance_rpc_runtime_api::GovernanceRuntimeApi;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use serde::{Deserialize, Serialize};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::sync::Arc;

#[rpc]
pub trait GovernanceApi<AccountId, BlockHash> {
	/// Get all governance proposal votes
	#[rpc(name = "governance_getProposalVotes")]
	fn proposal_votes(&self, at: Option<BlockHash>) -> Result<Vec<ProposalVotes<AccountId>>>;
}

/// A struct that implements the [`GovernanceApi`].
pub struct Governance<C, P> {
	client: Arc<C>,
	_marker: std::marker::PhantomData<P>,
}

impl<C, P> Governance<C, P> {
	/// Create new `Governance` with the given reference to the client.
	pub fn new(client: Arc<C>) -> Self {
		Governance {
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

#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Clone)]
pub struct ProposalVotes<AccountId> {
	proposal_id: ProposalId,
	votes: Vec<(AccountId, Option<bool>)>,
}

impl<C, Block, AccountId> GovernanceApi<AccountId, <Block as BlockT>::Hash> for Governance<C, (Block, AccountId)>
where
	Block: BlockT,
	C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	C::Api: GovernanceRuntimeApi<Block, AccountId>,
	AccountId: Codec + Clone + Send + Sync + 'static,
{
	fn proposal_votes(&self, at: Option<<Block as BlockT>::Hash>) -> Result<Vec<ProposalVotes<AccountId>>> {
		let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

		let mut proposal_votes_info = self.client.runtime_api().proposal_votes(&at).map_err(|e| RpcError {
			code: ErrorCode::ServerError(Error::RuntimeError as i64),
			message: "Unable to query proposal votes.".into(),
			data: Some(format!("{:?}", e).into()),
		})?;
		// sort by proposal Id for the receiver
		proposal_votes_info.sort_by(|(id_1, _), (id_2, _)| id_1.partial_cmp(id_2).expect("it's a valid id"));

		let council = self.client.runtime_api().council(&at).map_err(|e| RpcError {
			code: ErrorCode::ServerError(Error::RuntimeError as i64),
			message: "Unable to query council.".into(),
			data: Some(format!("{:?}", e).into()),
		})?;

		Ok(proposal_votes_info
			.iter()
			.map(|(proposal_id, votes)| {
				let votes: Vec<(AccountId, Option<bool>)> = (0..council.len())
					.map(|idx| (council[idx].clone(), votes.get_vote(idx as u8)))
					.collect();
				ProposalVotes {
					proposal_id: *proposal_id,
					votes,
				}
			})
			.collect())
	}
}
