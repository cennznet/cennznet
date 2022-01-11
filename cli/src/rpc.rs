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

use cennznet_primitives::types::{AccountId, AssetId, Balance, Block, BlockNumber, Hash, Index};
use cennznet_runtime::Runtime;
use ethy_gadget::notification::EthyEventProofStream;
use fc_rpc::{
	EthApi, EthApiServer, EthBlockDataCache, EthFilterApi, EthFilterApiServer, EthPubSubApi, EthPubSubApiServer,
	HexEncodedIdProvider, NetApi, NetApiServer, OverrideHandle, RuntimeApiStorageOverride, SchemaV1Override,
	SchemaV2Override, StorageOverride, Web3Api, Web3ApiServer,
};
use fc_rpc_core::types::{FeeHistoryCache, FilterPool};
use jsonrpc_pubsub::manager::SubscriptionManager;
use pallet_ethereum::EthereumStorageSchema;
use sc_client_api::{AuxStore, Backend, BlockchainEvents, StateBackend, StorageProvider};
use sc_consensus_babe::{Config, Epoch};
use sc_consensus_babe_rpc::BabeRpcHandler;
use sc_consensus_epochs::SharedEpochChanges;
use sc_consensus_manual_seal::rpc::EngineCommand;
use sc_finality_grandpa::{FinalityProofProvider, GrandpaJustificationStream, SharedAuthoritySet, SharedVoterState};
use sc_finality_grandpa_rpc::GrandpaRpcHandler;
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
}

/// A IO handler that uses all Full RPC extensions.
pub type IoHandler = jsonrpc_core::IoHandler<sc_rpc::Metadata>;

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
) -> jsonrpc_core::IoHandler<sc_rpc::Metadata>
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
	C::Api: substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Index>,
	C::Api: BabeApi<Block>,
	C::Api: BlockBuilder<Block>,
	C::Api: crml_cennzx_rpc::CennzxRuntimeApi<Block, AssetId, Balance, AccountId>,
	C::Api: crml_eth_wallet_rpc::EthWalletRuntimeApi<Block>,
	C::Api: crml_nft_rpc::NftRuntimeApi<Block, AccountId, Runtime>,
	C::Api: crml_staking_rpc::StakingRuntimeApi<Block, AccountId>,
	C::Api: crml_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>,
	C::Api: crml_generic_asset_rpc::GenericAssetRuntimeApi<Block, AssetId, Balance, AccountId>,
	C::Api: crml_governance_rpc::GovernanceRuntimeApi<Block, AccountId>,
	P: TransactionPool<Block = Block> + 'static,
	SC: SelectChain<Block> + 'static,
{
	use crml_cennzx_rpc::{Cennzx, CennzxApi};
	use crml_eth_wallet_rpc::{EthWallet, EthWalletApi};
	use crml_generic_asset_rpc::{GenericAsset, GenericAssetApi};
	use crml_governance_rpc::{Governance, GovernanceApi};
	use crml_nft_rpc::{Nft, NftApi};
	use crml_staking_rpc::{Staking, StakingApi};
	use crml_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApi};
	use substrate_frame_rpc_system::{FullSystem, SystemApi};

	let mut io = jsonrpc_core::IoHandler::default();
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

	io.extend_with(SystemApi::to_delegate(FullSystem::new(
		client.clone(),
		pool.clone(),
		deny_unsafe,
	)));
	io.extend_with(sc_consensus_babe_rpc::BabeApi::to_delegate(BabeRpcHandler::new(
		client.clone(),
		shared_epoch_changes.clone(),
		keystore,
		babe_config,
		select_chain,
		deny_unsafe,
	)));
	io.extend_with(ethy_gadget_rpc::EthyApi::to_delegate(
		ethy_gadget_rpc::EthyRpcHandler::new(ethy.event_proof_stream, ethy.subscription_executor, client.clone()),
	));
	io.extend_with(sc_finality_grandpa_rpc::GrandpaApi::to_delegate(
		GrandpaRpcHandler::new(
			shared_authority_set.clone(),
			shared_voter_state,
			justification_stream,
			subscription_executor,
			finality_provider,
		),
	));
	io.extend_with(sc_sync_state_rpc::SyncStateRpcApi::to_delegate(
		sc_sync_state_rpc::SyncStateRpcHandler::new(
			chain_spec,
			client.clone(),
			shared_authority_set,
			shared_epoch_changes,
			deny_unsafe,
		)
		.expect("syncstate setup ok"),
	));
	io.extend_with(TransactionPaymentApi::to_delegate(TransactionPayment::new(
		client.clone(),
	)));
	io.extend_with(CennzxApi::to_delegate(Cennzx::new(client.clone())));
	io.extend_with(NftApi::to_delegate(Nft::new(client.clone())));
	io.extend_with(StakingApi::to_delegate(Staking::new(client.clone())));
	io.extend_with(GenericAssetApi::to_delegate(GenericAsset::new(client.clone())));
	io.extend_with(GovernanceApi::to_delegate(Governance::new(client.clone())));
	io.extend_with(EthWalletApi::to_delegate(EthWallet::new(client.clone())));

	// evm stuff
	let block_data_cache = Arc::new(EthBlockDataCache::new(50, 50));

	io.extend_with(EthApiServer::to_delegate(EthApi::new(
		client.clone(),
		pool.clone(),
		graph.clone(),
		cennznet_runtime::TransactionConverter,
		network.clone(),
		Default::default(),
		overrides.clone(),
		frontier_backend.clone(),
		is_authority,
		max_past_logs,
		block_data_cache.clone(),
		fee_history_limit,
		fee_history_cache,
	)));

	if let Some(filter_pool) = filter_pool {
		io.extend_with(EthFilterApiServer::to_delegate(EthFilterApi::new(
			client.clone(),
			frontier_backend,
			filter_pool,
			500_usize, // max stored filters
			overrides.clone(),
			max_past_logs,
			block_data_cache.clone(),
		)));
	}

	io.extend_with(NetApiServer::to_delegate(NetApi::new(
		client.clone(),
		network.clone(),
		true,
	)));
	io.extend_with(Web3ApiServer::to_delegate(Web3Api::new(client.clone())));
	io.extend_with(EthPubSubApiServer::to_delegate(EthPubSubApi::new(
		pool,
		client.clone(),
		network,
		SubscriptionManager::<HexEncodedIdProvider>::with_id_provider(
			HexEncodedIdProvider::default(),
			Arc::new(subscription_task_executor),
		),
		overrides,
	)));

	io
}
