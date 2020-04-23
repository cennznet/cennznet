// Copyright 2018-2020 Parity Technologies (UK) Ltd. and Centrality Investments Ltd.
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

#![warn(unused_extern_crates)]

//! Service implementation. Specialized wrapper over substrate service.

use std::sync::Arc;

use cennznet_executor;
use cennznet_primitives::types::Block;
use cennznet_runtime::RuntimeApi;
use grandpa::{self, FinalityProofProvider as GrandpaFinalityProofProvider, StorageAndProofProvider};
use sc_client::{self, LongestChain};
use sc_consensus_babe;
use sc_service::{config::Configuration, error::Error as ServiceError, AbstractService, ServiceBuilder};
use sp_inherents::InherentDataProviders;

use cennznet_executor::NativeExecutor;
use sc_client::{Client, LocalCallExecutor};
use sc_client_db::Backend;
use sc_network::NetworkService;
use sc_offchain::OffchainWorkers;
use sc_service::{NetworkStatus, Service};
use sp_runtime::traits::Block as BlockT;

/// Starts a `ServiceBuilder` for a full service.
///
/// Use this macro if you don't actually need the full service, but just the builder in order to
/// be able to perform chain operations.
macro_rules! new_full_start {
	($config:expr) => {{
		use std::sync::Arc;
		type RpcExtension = jsonrpc_core::IoHandler<sc_rpc::Metadata>;
		let mut import_setup = None;
		let inherent_data_providers = sp_inherents::InherentDataProviders::new();

		let builder = sc_service::ServiceBuilder::new_full::<
			cennznet_primitives::types::Block,
			cennznet_runtime::RuntimeApi,
			cennznet_executor::Executor,
		>($config)?
		.with_select_chain(|_config, backend| Ok(sc_client::LongestChain::new(backend.clone())))?
		.with_transaction_pool(|config, client, _fetcher| {
			let pool_api = sc_transaction_pool::FullChainApi::new(client.clone());
			Ok(sc_transaction_pool::BasicPool::new(
				config,
				std::sync::Arc::new(pool_api),
			))
		})?
		.with_import_queue(|_config, client, mut select_chain, _transaction_pool| {
			let select_chain = select_chain
				.take()
				.ok_or_else(|| sc_service::Error::SelectChainRequired)?;
			let (grandpa_block_import, grandpa_link) =
				grandpa::block_import(client.clone(), &(client.clone() as Arc<_>), select_chain)?;
			let justification_import = grandpa_block_import.clone();

			let (block_import, babe_link) = sc_consensus_babe::block_import(
				sc_consensus_babe::Config::get_or_compute(&*client)?,
				grandpa_block_import,
				client.clone(),
			)?;

			let import_queue = sc_consensus_babe::import_queue(
				babe_link.clone(),
				block_import.clone(),
				Some(Box::new(justification_import)),
				None,
				client,
				inherent_data_providers.clone(),
			)?;

			import_setup = Some((block_import, grandpa_link, babe_link));
			Ok(import_queue)
		})?
		.with_rpc_extensions(|builder| -> Result<RpcExtension, _> {
			let babe_link = import_setup
				.as_ref()
				.map(|s| &s.2)
				.expect("BabeLink is present for full services or set up failed; qed.");
			let deps = cennznet_rpc::FullDeps {
				client: builder.client().clone(),
				pool: builder.pool(),
				select_chain: builder
					.select_chain()
					.cloned()
					.expect("SelectChain is present for full services or set up failed; qed."),
				babe: cennznet_rpc::BabeDeps {
					keystore: builder.keystore(),
					babe_config: sc_consensus_babe::BabeLink::config(babe_link).clone(),
					shared_epoch_changes: sc_consensus_babe::BabeLink::epoch_changes(babe_link).clone(),
				},
			};
			Ok(cennznet_rpc::create_full(deps))
		})?;

		(builder, import_setup, inherent_data_providers)
		}};
}

/// Creates a full service from the configuration.
///
/// We need to use a macro because the test suit doesn't work with an opaque service. It expects
/// concrete types instead.
macro_rules! new_full {
	($config:expr, $with_startup_data: expr) => {{
		use futures::prelude::*;
		use sc_client_api::ExecutorProvider;
		use sc_network::Event;

		let (is_authority, force_authoring, name, disable_grandpa, sentry_nodes) = (
			$config.roles.is_authority(),
			$config.force_authoring,
			$config.name.clone(),
			$config.disable_grandpa,
			$config.network.sentry_nodes.clone(),
			);

		// sentry nodes announce themselves as authorities to the network
		// and should run the same protocols authorities do, but it should
		// never actively participate in any consensus process.
		let participates_in_consensus = is_authority && !$config.sentry_mode;

		let (builder, mut import_setup, inherent_data_providers) = new_full_start!($config);

		let service = builder
			.with_finality_proof_provider(|client, backend| {
				// GenesisAuthoritySetProvider is implemented for StorageAndProofProvider
				let provider = client as Arc<dyn grandpa::StorageAndProofProvider<_, _>>;
				Ok(Arc::new(grandpa::FinalityProofProvider::new(backend, provider)) as _)
			})?
			.build()?;

		let (block_import, grandpa_link, babe_link) = import_setup
			.take()
			.expect("Link Half and Block Import are present for Full Services or setup failed before. qed");

		($with_startup_data)(&block_import, &babe_link);

		if participates_in_consensus {
			let proposer = sc_basic_authorship::ProposerFactory::new(service.client(), service.transaction_pool());

			let client = service.client();
			let select_chain = service.select_chain().ok_or(sc_service::Error::SelectChainRequired)?;

			let can_author_with = sp_consensus::CanAuthorWithNativeVersion::new(client.executor().clone());

			let babe_config = sc_consensus_babe::BabeParams {
				keystore: service.keystore(),
				client,
				select_chain,
				env: proposer,
				block_import,
				sync_oracle: service.network(),
				inherent_data_providers: inherent_data_providers.clone(),
				force_authoring,
				babe_link,
				can_author_with,
			};

			let babe = sc_consensus_babe::start_babe(babe_config)?;
			service.spawn_essential_task("babe-proposer", babe);

			let network = service.network();
			let dht_event_stream = network
				.event_stream()
				.filter_map(|e| async move {
					match e {
						Event::Dht(e) => Some(e),
						_ => None,
					}
				})
				.boxed();
			let authority_discovery = sc_authority_discovery::AuthorityDiscovery::new(
				service.client(),
				network,
				sentry_nodes,
				service.keystore(),
				dht_event_stream,
			);

			service.spawn_task("authority-discovery", authority_discovery);
			}

		// if the node isn't actively participating in consensus then it doesn't
		// need a keystore, regardless of which protocol we use below.
		let keystore = if participates_in_consensus {
			Some(service.keystore())
		} else {
			None
			};

		let config = grandpa::Config {
			// FIXME #1578 make this available through chainspec
			gossip_duration: std::time::Duration::from_millis(333),
			justification_period: 512,
			name: Some(name),
			observer_enabled: false,
			keystore,
			is_authority,
			};

		let enable_grandpa = !disable_grandpa;
		if enable_grandpa {
			// start the full GRANDPA voter
			// NOTE: non-authorities could run the GRANDPA observer protocol, but at
			// this point the full voter should provide better guarantees of block
			// and vote data availability than the observer. The observer has not
			// been tested extensively yet and having most nodes in a network run it
			// could lead to finality stalls.
			let grandpa_config = grandpa::GrandpaParams {
				config,
				link: grandpa_link,
				network: service.network(),
				inherent_data_providers: inherent_data_providers.clone(),
				telemetry_on_connect: Some(service.telemetry_on_connect_stream()),
				voting_rule: grandpa::VotingRulesBuilder::default().build(),
				prometheus_registry: service.prometheus_registry(),
			};

			// the GRANDPA voter task is considered infallible, i.e.
			// if it fails we take down the service with it.
			service.spawn_essential_task("grandpa-voter", grandpa::run_grandpa_voter(grandpa_config)?);
		} else {
			grandpa::setup_disabled_grandpa(service.client(), &inherent_data_providers, service.network())?;
			}

		Ok((service, inherent_data_providers))
		}};
	($config:expr) => {{
		new_full!($config, |_, _| {})
		}};
}

type ConcreteBlock = cennznet_primitives::types::Block;
type ConcreteClient = Client<
	Backend<ConcreteBlock>,
	LocalCallExecutor<Backend<ConcreteBlock>, NativeExecutor<cennznet_executor::Executor>>,
	ConcreteBlock,
	cennznet_runtime::RuntimeApi,
>;
type ConcreteBackend = Backend<ConcreteBlock>;
type ConcreteTransactionPool =
	sc_transaction_pool::BasicPool<sc_transaction_pool::FullChainApi<ConcreteClient, ConcreteBlock>, ConcreteBlock>;

/// Builds a new service for a full client.
pub fn new_full(
	config: Configuration,
) -> Result<
	Service<
		ConcreteBlock,
		ConcreteClient,
		LongestChain<ConcreteBackend, ConcreteBlock>,
		NetworkStatus<ConcreteBlock>,
		NetworkService<ConcreteBlock, <ConcreteBlock as BlockT>::Hash>,
		ConcreteTransactionPool,
		OffchainWorkers<
			ConcreteClient,
			<ConcreteBackend as sc_client_api::backend::Backend<Block>>::OffchainStorage,
			ConcreteBlock,
		>,
	>,
	ServiceError,
> {
	new_full!(config).map(|(service, _)| service)
}

/// Builds a new service for a light client.
pub fn new_light(config: Configuration) -> Result<impl AbstractService, ServiceError> {
	type RpcExtension = jsonrpc_core::IoHandler<sc_rpc::Metadata>;
	let inherent_data_providers = InherentDataProviders::new();

	let service = ServiceBuilder::new_light::<Block, RuntimeApi, cennznet_executor::Executor>(config)?
		.with_select_chain(|_config, backend| Ok(LongestChain::new(backend.clone())))?
		.with_transaction_pool(|config, client, fetcher| {
			let fetcher = fetcher.ok_or_else(|| "Trying to start light transaction pool without active fetcher")?;
			let pool_api = sc_transaction_pool::LightChainApi::new(client.clone(), fetcher.clone());
			let pool = sc_transaction_pool::BasicPool::with_revalidation_type(
				config,
				Arc::new(pool_api),
				sc_transaction_pool::RevalidationType::Light,
			);
			Ok(pool)
		})?
		.with_import_queue_and_fprb(|_config, client, backend, fetcher, _select_chain, _tx_pool| {
			let fetch_checker = fetcher
				.map(|fetcher| fetcher.checker().clone())
				.ok_or_else(|| "Trying to start light import queue without active fetch checker")?;
			let grandpa_block_import = grandpa::light_block_import(
				client.clone(),
				backend,
				&(client.clone() as Arc<_>),
				Arc::new(fetch_checker),
			)?;

			let finality_proof_import = grandpa_block_import.clone();
			let finality_proof_request_builder = finality_proof_import.create_finality_proof_request_builder();

			let (babe_block_import, babe_link) = sc_consensus_babe::block_import(
				sc_consensus_babe::Config::get_or_compute(&*client)?,
				grandpa_block_import,
				client.clone(),
			)?;

			let import_queue = sc_consensus_babe::import_queue(
				babe_link,
				babe_block_import,
				None,
				Some(Box::new(finality_proof_import)),
				client.clone(),
				inherent_data_providers.clone(),
			)?;

			Ok((import_queue, finality_proof_request_builder))
		})?
		.with_finality_proof_provider(|client, backend| {
			// GenesisAuthoritySetProvider is implemented for StorageAndProofProvider
			let provider = client as Arc<dyn StorageAndProofProvider<_, _>>;
			Ok(Arc::new(GrandpaFinalityProofProvider::new(backend, provider)) as _)
		})?
		.with_rpc_extensions(|builder| -> Result<RpcExtension, _> {
			let fetcher = builder
				.fetcher()
				.ok_or_else(|| "Trying to start node RPC without active fetcher")?;
			let remote_blockchain = builder
				.remote_backend()
				.ok_or_else(|| "Trying to start node RPC without active remote blockchain")?;

			let light_deps = cennznet_rpc::LightDeps {
				remote_blockchain,
				fetcher,
				client: builder.client().clone(),
				pool: builder.pool(),
			};
			Ok(cennznet_rpc::create_light(light_deps))
		})?
		.build()?;

	Ok(service)
}

#[cfg(test)]
mod tests {
	use crate::service::{new_full, new_light};
	use cennznet_primitives::types::{AccountId, Block, DigestItem, Signature};
	use cennznet_runtime::constants::{currency::CENTS, time::SLOT_DURATION};
	use cennznet_runtime::{constants::asset::SPENDING_ASSET_ID, GenericAssetCall};
	use cennznet_runtime::{Call, UncheckedExtrinsic};
	use codec::{Decode, Encode};
	use sc_consensus_babe::{BabeIntermediate, CompatibleDigestItem, INTERMEDIATE_KEY};
	use sc_consensus_epochs::descendent_query;
	use sc_service::AbstractService;
	use sp_consensus::{
		BlockImport, BlockImportParams, BlockOrigin, Environment, ForkChoiceStrategy, Proposer, RecordProof,
	};
	use sp_core::{crypto::Pair as CryptoPair, H256};
	use sp_finality_tracker;
	use sp_keyring::AccountKeyring;
	use sp_runtime::traits::IdentifyAccount;
	use sp_runtime::{
		generic::{BlockId, Digest, Era, SignedPayload},
		traits::Verify,
		traits::{Block as BlockT, Header as HeaderT},
		OpaqueExtrinsic,
	};
	use sp_timestamp;
	use std::{any::Any, borrow::Cow, sync::Arc};

	type AccountPublic = <Signature as Verify>::Signer;

	#[cfg(feature = "rhd")]
	fn test_sync() {
		use sp_core::ed25519::Pair;

		use sc_client::{BlockImportParams, BlockOrigin};
		use {service_test, Factory};

		let alice: Arc<ed25519::Pair> = Arc::new(Keyring::Alice.into());
		let bob: Arc<ed25519::Pair> = Arc::new(Keyring::Bob.into());
		let validators = vec![alice.public().0.into(), bob.public().0.into()];
		let keys: Vec<&ed25519::Pair> = vec![&*alice, &*bob];
		let dummy_runtime = ::tokio::runtime::Runtime::new().unwrap();
		let block_factory = |service: &<Factory as service::ServiceFactory>::FullService| {
			let block_id = BlockId::number(service.client().chain_info().best_number);
			let parent_header = service.client().header(&block_id).unwrap().unwrap();
			let consensus_net = ConsensusNetwork::new(service.network(), service.client().clone());
			let proposer_factory = consensus::ProposerFactory {
				client: service.client().clone(),
				transaction_pool: service.transaction_pool().clone(),
				network: consensus_net,
				force_delay: 0,
				handle: dummy_runtime.executor(),
			};
			let (proposer, _, _) = proposer_factory
				.init(&parent_header, &validators, alice.clone())
				.unwrap();
			let block = proposer.propose().expect("Error making test block");
			BlockImportParams {
				origin: BlockOrigin::File,
				justification: Vec::new(),
				internal_justification: Vec::new(),
				finalized: false,
				body: Some(block.extrinsics),
				storage_changes: None,
				header: block.header,
				auxiliary: Vec::new(),
			}
		};
		let extrinsic_factory = |service: &SyncService<<Factory as service::ServiceFactory>::FullService>| {
			let payload = (
				0,
				Call::GenericAsset(GenericAssetCall::transfer(
					SPENDING_ASSET_ID,
					RawAddress::Id(bob.public().0.into()),
					69.into(),
				)),
				Era::immortal(),
				service.client().genesis_hash(),
			);
			let signature = alice.sign(&payload.encode()).into();
			let id = alice.public().0.into();
			let xt = UncheckedExtrinsic {
				signature: Some((RawAddress::Id(id), signature, payload.0, Era::immortal())),
				function: payload.1,
			}
			.encode();
			let v: Vec<u8> = Decode::decode(&mut xt.as_slice()).unwrap();
			OpaqueExtrinsic(v)
		};
		sc_service_test::sync(
			sc_chain_spec::integration_test_config(),
			|config| new_full(config),
			|mut config| new_light(config),
			block_factory,
			extrinsic_factory,
		);
	}
}
