// This file is part of Substrate.

// Copyright (C) 2019-2021 Parity Technologies (UK) Ltd. and Centrality Investments Ltd.
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

//! node RPC config

#![warn(missing_docs)]

use cennznet_primitives::{
	txpool::TxPoolRuntimeApi,
	types::{AccountId, AssetId, Balance, Block, BlockNumber, Hash, Index},
};
use cennznet_runtime::Runtime;
use ethy_gadget::notification::EthyEventProofStream;
use fc_rpc::{
	EthBlockDataCacheTask, OverrideHandle, RuntimeApiStorageOverride, SchemaV1Override, SchemaV2Override,
	StorageOverride,
};
use fc_rpc_core::types::{FeeHistoryCache, FilterPool};
use fp_storage::EthereumStorageSchema;
use jsonrpsee::RpcModule;
use sc_client_api::{AuxStore, Backend, BlockchainEvents, StateBackend, StorageProvider};
use sc_consensus_babe::{Config, Epoch};
use sc_consensus_epochs::SharedEpochChanges;
use sc_consensus_manual_seal::rpc::EngineCommand;
use sc_finality_grandpa::{FinalityProofProvider, GrandpaJustificationStream, SharedAuthoritySet, SharedVoterState};
use sc_network::NetworkService;
use sc_rpc::SubscriptionTaskExecutor;
pub use sc_rpc_api::DenyUnsafe;
use sc_transaction_pool::{ChainApi, Pool};
use sc_transaction_pool_api::TransactionPool;
use sp_api::ProvideRuntimeApi;
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Backend as BlockchainBackend, Error as BlockChainError, HeaderBackend, HeaderMetadata};
use sp_consensus::SelectChain;
use sp_consensus_babe::BabeApi;
use sp_keystore::SyncCryptoStorePtr;
use std::{collections::BTreeMap, sync::Arc};

/// Extra dependencies for BABE.
pub struct BabeDeps {
	/// BABE protocol config.
	pub babe_config: Config,
	/// BABE pending epoch changes.
	pub shared_epoch_changes: SharedEpochChanges<Block, Epoch>,
	/// The keystore that manages the keys of the node.
	pub keystore: SyncCryptoStorePtr,
}

/// Extra dependencies for Ethy
pub struct EthyDeps {
	/// Receives notifications about event proofs from Ethy.
	pub event_proof_stream: EthyEventProofStream,
	/// Executor to drive the subscription manager in the Ethy RPC handler.
	pub subscription_executor: SubscriptionTaskExecutor,
}

/// Extra dependencies for GRANDPA
pub struct GrandpaDeps<B> {
	/// Voting round info.
	pub shared_voter_state: SharedVoterState,
	/// Authority set info.
	pub shared_authority_set: SharedAuthoritySet<Hash, BlockNumber>,
	/// Receives notifications about justification events from Grandpa.
	pub justification_stream: GrandpaJustificationStream<Block>,
	/// Executor to drive the subscription manager in the Grandpa RPC handler.
	pub subscription_executor: SubscriptionTaskExecutor,
	/// Finality proof provider.
	pub finality_provider: Arc<FinalityProofProvider<B, Block>>,
}

/// Full client dependencies.
pub struct FullDeps<C, P, A: ChainApi, BE, SC> {
	/// The client instance to use.
	pub client: Arc<C>,
	/// Transaction pool instance.
	pub pool: Arc<P>,
	/// Graph pool instance.
	pub graph: Arc<Pool<A>>,
	/// The SelectChain Strategy
	pub select_chain: SC,
	/// A copy of the chain spec.
	pub chain_spec: Box<dyn sc_chain_spec::ChainSpec>,
	/// Whether to deny unsafe calls
	pub deny_unsafe: DenyUnsafe,
	/// The Node authority flag
	pub is_authority: bool,
	/// BABE specific dependencies.
	pub babe: BabeDeps,
	/// Ethy specific dependencies.
	pub ethy: EthyDeps,
	/// GRANDPA specific dependencies.
	pub grandpa: GrandpaDeps<BE>,
	/// Network service
	pub network: Arc<NetworkService<Block, Hash>>,
	/// EthFilterApi pool.
	pub filter_pool: Option<FilterPool>,
	/// Backend.
	pub backend: Arc<BE>,
	/// Frontier Backend.
	pub frontier_backend: Arc<fc_db::Backend<Block>>,
	/// Manual seal command sink
	pub command_sink: Option<futures::channel::mpsc::Sender<EngineCommand<Hash>>>,
	/// Maximum number of logs in a query.
	pub max_past_logs: u32,
	/// Maximum fee history cache size.
	pub fee_history_limit: u64,
	/// Fee history cache.
	pub fee_history_cache: FeeHistoryCache,
	/// Cache for Ethereum block data.
	pub block_data_cache: Arc<EthBlockDataCacheTask<Block>>,
}

#[allow(missing_docs)]
pub fn overrides_handle<C, BE>(client: Arc<C>) -> Arc<OverrideHandle<Block>>
where
	C: ProvideRuntimeApi<Block> + StorageProvider<Block, BE> + AuxStore,
	C: HeaderBackend<Block> + HeaderMetadata<Block, Error = BlockChainError>,
	C: Send + Sync + 'static,
	C::Api: fp_rpc::EthereumRuntimeRPCApi<Block>,
	BE: Backend<Block> + 'static,
	BE::State: StateBackend<sp_runtime::traits::BlakeTwo256>,
{
	let mut overrides_map = BTreeMap::new();
	overrides_map.insert(
		EthereumStorageSchema::V1,
		Box::new(SchemaV1Override::new(client.clone())) as Box<dyn StorageOverride<_> + Send + Sync>,
	);
	overrides_map.insert(
		EthereumStorageSchema::V2,
		Box::new(SchemaV2Override::new(client.clone())) as Box<dyn StorageOverride<_> + Send + Sync>,
	);

	Arc::new(OverrideHandle {
		schemas: overrides_map,
		fallback: Box::new(RuntimeApiStorageOverride::new(client.clone())),
	})
}

/// Instantiate all Full RPC extensions.
pub fn create_full<C, P, A, B, SC>(
	deps: FullDeps<C, P, A, B, SC>,
	subscription_task_executor: SubscriptionTaskExecutor,
	overrides: Arc<OverrideHandle<Block>>,
) -> Result<RpcModule<()>, Box<dyn std::error::Error + Send + Sync>>
where
	A: ChainApi<Block = Block> + 'static,
	B: Backend<Block> + 'static,
	B::State: StateBackend<sp_runtime::traits::BlakeTwo256>,
	B::Blockchain: BlockchainBackend<Block>,
	C: ProvideRuntimeApi<Block>
		+ AuxStore
		+ BlockchainEvents<Block>
		+ HeaderBackend<Block>
		+ HeaderMetadata<Block, Error = BlockChainError>
		+ StorageProvider<Block, B>
		+ Send
		+ Sync
		+ 'static,
	C::Api: fp_rpc::EthereumRuntimeRPCApi<Block>,
	C::Api: fp_rpc::ConvertTransactionRuntimeApi<Block>,
	C::Api: substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Index>,
	C::Api: BabeApi<Block>,
	C::Api: BlockBuilder<Block>,
	C::Api: crml_cennzx_rpc::CennzxRuntimeApi<Block, AssetId, Balance, AccountId>,
	C::Api: crml_nft_rpc::NftRuntimeApi<Block, AccountId, Runtime>,
	C::Api: crml_staking_rpc::StakingRuntimeApi<Block, AccountId>,
	C::Api: crml_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>,
	C::Api: crml_generic_asset_rpc::GenericAssetRuntimeApi<Block, AssetId, Balance, AccountId>,
	C::Api: crml_governance_rpc::GovernanceRuntimeApi<Block, AccountId>,
	C::Api: TxPoolRuntimeApi<Block>,
	P: TransactionPool<Block = Block> + 'static,
	SC: SelectChain<Block> + 'static,
{
	use cennznet_rpc_core_txpool::TxPoolServer;
	use cennznet_rpc_txpool::TxPool;
	use crml_cennzx_rpc::{Cennzx, CennzxApiServer};
	use crml_generic_asset_rpc::{GenericAsset, GenericAssetApiServer};
	use crml_governance_rpc::{Governance, GovernanceApiServer};
	use crml_nft_rpc::{Nft, NftApiServer};
	use crml_staking_rpc::{Staking, StakingApiServer};
	use crml_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApiServer};
	use ethy_gadget_rpc::{EthyApiServer, EthyRpcHandler};
	use fc_rpc::{
		Eth, EthApiServer, EthFilter, EthFilterApiServer, EthPubSub, EthPubSubApiServer, Net, NetApiServer, Web3,
		Web3ApiServer,
	};
	use sc_consensus_babe_rpc::{Babe, BabeApiServer};
	use sc_finality_grandpa_rpc::{Grandpa, GrandpaApiServer};
	use sc_sync_state_rpc::{SyncState, SyncStateApiServer};
	use substrate_frame_rpc_system::{System, SystemApiServer};

	let mut io = RpcModule::new(());
	let FullDeps {
		client,
		pool,
		graph,
		is_authority,
		backend: _,
		select_chain,
		chain_spec,
		deny_unsafe,
		babe,
		ethy,
		grandpa,
		network,
		filter_pool,
		command_sink: _,
		frontier_backend,
		max_past_logs,
		fee_history_limit,
		fee_history_cache,
		block_data_cache,
	} = deps;

	let BabeDeps {
		keystore,
		babe_config,
		shared_epoch_changes,
	} = babe;
	let GrandpaDeps {
		shared_voter_state,
		shared_authority_set,
		justification_stream,
		subscription_executor,
		finality_provider,
	} = grandpa;

	io.merge(System::new(Arc::clone(&client), pool.clone(), deny_unsafe).into_rpc())?;
	io.merge(
		Babe::new(
			Arc::clone(&client),
			shared_epoch_changes.clone(),
			keystore,
			babe_config,
			select_chain,
			deny_unsafe,
		)
		.into_rpc(),
	)?;
	io.merge(EthyRpcHandler::new(ethy.event_proof_stream, ethy.subscription_executor, Arc::clone(&client)).into_rpc())?;
	io.merge(
		Grandpa::new(
			subscription_executor,
			shared_authority_set.clone(),
			shared_voter_state,
			justification_stream,
			finality_provider,
		)
		.into_rpc(),
	)?;
	io.merge(
		SyncState::new(
			chain_spec,
			Arc::clone(&client),
			shared_authority_set,
			shared_epoch_changes,
		)
		.expect("syncstate setup ok")
		.into_rpc(),
	)?;
	io.merge(TransactionPayment::new(Arc::clone(&client)).into_rpc())?;
	io.merge(Cennzx::new(Arc::clone(&client)).into_rpc())?;
	io.merge(Nft::new(Arc::clone(&client)).into_rpc())?;
	io.merge(Staking::new(Arc::clone(&client)).into_rpc())?;
	io.merge(GenericAsset::new(Arc::clone(&client)).into_rpc())?;
	io.merge(Governance::new(Arc::clone(&client)).into_rpc())?;

	// evm stuff
	io.merge(
		Eth::new(
			Arc::clone(&client),
			Arc::clone(&pool),
			graph.clone(),
			Some(cennznet_runtime::TransactionConverter),
			Arc::clone(&network),
			Default::default(),
			Arc::clone(&overrides),
			Arc::clone(&frontier_backend),
			is_authority,
			Arc::clone(&block_data_cache),
			fee_history_cache,
			fee_history_limit,
		)
		.into_rpc(),
	)?;

	if let Some(filter_pool) = filter_pool {
		io.merge(
			EthFilter::new(
				Arc::clone(&client),
				frontier_backend,
				filter_pool,
				500_usize, // max stored filters
				max_past_logs,
				block_data_cache.clone(),
			)
			.into_rpc(),
		)?;
	}

	io.merge(
		Net::new(
			Arc::clone(&client),
			network.clone(),
			// Whether to format the `peer_count` response as Hex (default) or not.
			true,
		)
		.into_rpc(),
	)?;
	io.merge(Web3::new(Arc::clone(&client)).into_rpc())?;
	io.merge(
		EthPubSub::new(
			pool,
			Arc::clone(&client),
			network,
			subscription_task_executor,
			overrides,
		)
		.into_rpc(),
	)?;
	io.merge(TxPool::new(Arc::clone(&client), graph.clone()).into_rpc())?;
	Ok(io)
}
