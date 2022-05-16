// This file is part of Substrate.

// Copyright (C) 2018-2021 Parity Technologies (UK) Ltd. and Centrality Investments Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

#![warn(unused_extern_crates)]

//! Service implementation. Specialized wrapper over substrate service.

use fc_mapping_sync::{MappingSyncWorker, SyncStrategy};
use fc_rpc::EthTask;
use fc_rpc_core::types::{FeeHistoryCache, FilterPool};
use futures::prelude::*;
use log::{debug, warn};
use sc_cli::SubstrateCli;
use sc_client_api::{Backend, BlockchainEvents, ExecutorProvider};
use sc_consensus_babe::SlotProportion;
pub use sc_executor::NativeElseWasmExecutor;
use sc_network::{Event, NetworkService};
use sc_service::{config::Configuration, error::Error as ServiceError, BasePath, TaskManager};
use sc_telemetry::{Telemetry, TelemetryWorker};
use sp_core::offchain::OffchainStorage;
use sp_runtime::traits::Block as BlockT;
use std::{
	collections::BTreeMap,
	str::FromStr,
	sync::{Arc, Mutex},
	time::Duration,
};

use crate::rpc as node_rpc;
use cennznet_primitives::types::Block;
use cennznet_runtime::{constants::config::ETH_HTTP_URI, RuntimeApi};

// Declare an instance of the native executor named `Executor`. Include the wasm binary as
// the equivalent wasm code.
pub struct Executor;

impl sc_executor::NativeExecutionDispatch for Executor {
	#[cfg(feature = "runtime-benchmarks")]
	type ExtendHostFunctions = frame_benchmarking::benchmarking::HostFunctions;
	#[cfg(not(feature = "runtime-benchmarks"))]
	type ExtendHostFunctions = (
		sp_io::SubstrateHostFunctions,
		cennznet_runtime::legacy_host_functions::storage::HostFunctions,
	);

	fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
		cennznet_runtime::api::dispatch(method, data)
	}

	fn native_version() -> sc_executor::NativeVersion {
		cennznet_runtime::native_version()
	}
}

/// The full client type definition.
type FullClient = sc_service::TFullClient<Block, RuntimeApi, NativeElseWasmExecutor<Executor>>;
type FullBackend = sc_service::TFullBackend<Block>;
type FullSelectChain = sc_consensus::LongestChain<FullBackend, Block>;
/// GRANDPA block importer type
type FullGrandpaBlockImport = sc_finality_grandpa::GrandpaBlockImport<FullBackend, Block, FullClient, FullSelectChain>;
/// BABE block importer type additionally wraps `FullGrandpaBlockImport`
type FullBabeBlockImport = sc_consensus_babe::BabeBlockImport<Block, FullClient, FullGrandpaBlockImport>;
/// CENNZnet block importer type
/// Provides GRANDPA and BABE block import protocols
/// Frontier is intentionally excluded see: https://github.com/cennznet/cennznet/issues/596
type CENNZnetBlockImport = FullBabeBlockImport;
/// The transaction pool type definition.
pub type TransactionPool = sc_transaction_pool::FullPool<Block, FullClient>;

/// Creates a new partial node.
pub fn new_partial(
	config: &Configuration,
) -> Result<
	sc_service::PartialComponents<
		FullClient,
		FullBackend,
		FullSelectChain,
		sc_consensus::DefaultImportQueue<Block, FullClient>,
		sc_transaction_pool::FullPool<Block, FullClient>,
		(
			(
				CENNZnetBlockImport,
				sc_finality_grandpa::LinkHalf<Block, FullClient, FullSelectChain>,
				sc_consensus_babe::BabeLink<Block>,
			),
			sc_finality_grandpa::SharedVoterState,
			Option<FilterPool>,
			Arc<fc_db::Backend<Block>>,
			Option<Telemetry>,
			FeeHistoryCache,
		),
	>,
	ServiceError,
> {
	let telemetry = config
		.telemetry_endpoints
		.clone()
		.filter(|x| !x.is_empty())
		.map(|endpoints| -> Result<_, sc_telemetry::Error> {
			let worker = TelemetryWorker::new(16)?;
			let telemetry = worker.handle().new_telemetry(endpoints);
			Ok((worker, telemetry))
		})
		.transpose()?;

	let executor = NativeElseWasmExecutor::<Executor>::new(
		config.wasm_method,
		config.default_heap_pages,
		config.max_runtime_instances,
	);

	let (client, backend, keystore_container, task_manager) = sc_service::new_full_parts::<Block, RuntimeApi, _>(
		config,
		telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
		executor,
	)?;
	let client = Arc::new(client);

	let telemetry = telemetry.map(|(worker, telemetry)| {
		task_manager.spawn_handle().spawn("telemetry", None, worker.run());
		telemetry
	});

	let select_chain = sc_consensus::LongestChain::new(backend.clone());

	let transaction_pool = sc_transaction_pool::BasicPool::new_full(
		config.transaction_pool.clone(),
		config.role.is_authority().into(),
		config.prometheus_registry(),
		task_manager.spawn_essential_handle(),
		client.clone(),
	);

	let filter_pool: Option<FilterPool> = Some(Arc::new(Mutex::new(BTreeMap::new())));
	let fee_history_cache: FeeHistoryCache = Arc::new(Mutex::new(BTreeMap::new()));

	let frontier_backend = open_frontier_backend(config)?;

	let (grandpa_block_import, grandpa_link) = sc_finality_grandpa::block_import(
		client.clone(),
		&(client.clone() as Arc<_>),
		select_chain.clone(),
		telemetry.as_ref().map(|x| x.handle()),
	)?;
	let justification_import = grandpa_block_import.clone();

	let (block_import, babe_link) = sc_consensus_babe::block_import(
		sc_consensus_babe::Config::get_or_compute(&*client)?,
		grandpa_block_import,
		client.clone(),
	)?;

	let slot_duration = babe_link.config().slot_duration();
	let import_queue = sc_consensus_babe::import_queue(
		babe_link.clone(),
		block_import.clone(),
		Some(Box::new(justification_import)),
		client.clone(),
		select_chain.clone(),
		move |_, ()| async move {
			let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

			let slot = sp_consensus_babe::inherents::InherentDataProvider::from_timestamp_and_duration(
				*timestamp,
				slot_duration,
			);

			let uncles = sp_authorship::InherentDataProvider::<<Block as BlockT>::Header>::check_inherents();

			Ok((timestamp, slot, uncles))
		},
		&task_manager.spawn_essential_handle(),
		config.prometheus_registry(),
		sp_consensus::CanAuthorWithNativeVersion::new(client.executor().clone()),
		telemetry.as_ref().map(|x| x.handle()),
	)?;

	let import_setup = (block_import, grandpa_link, babe_link);
	let shared_voter_state = sc_finality_grandpa::SharedVoterState::empty();
	let rpc_setup = shared_voter_state.clone();
	let client = client.clone();
	let select_chain = select_chain.clone();

	Ok(sc_service::PartialComponents {
		client,
		backend,
		task_manager,
		keystore_container,
		select_chain,
		import_queue,
		transaction_pool,
		other: (
			import_setup,
			rpc_setup,
			filter_pool,
			frontier_backend,
			telemetry,
			fee_history_cache,
		),
	})
}

/// Result of [`new_full_base`].
pub struct NewFullBase {
	/// The task manager of the node.
	pub task_manager: TaskManager,
	/// The client instance of the node.
	pub client: Arc<FullClient>,
	/// The networking service of the node.
	pub network: Arc<NetworkService<Block, <Block as BlockT>::Hash>>,
	/// The transaction pool of the node.
	pub transaction_pool: Arc<TransactionPool>,
}

/// Creates a full service from the configuration.
pub fn new_full_base(
	mut config: Configuration,
	cli: &crate::cli::Cli,
	with_startup_data: impl FnOnce(&CENNZnetBlockImport, &sc_consensus_babe::BabeLink<Block>),
) -> Result<NewFullBase, ServiceError> {
	let sc_service::PartialComponents {
		client,
		backend,
		mut task_manager,
		import_queue,
		keystore_container,
		select_chain,
		transaction_pool,
		other: (import_setup, rpc_setup, filter_pool, frontier_backend, mut telemetry, fee_history_cache),
	} = new_partial(&config)?;

	// Set eth http bridge config
	// the config is stored into the offchain context where it can
	// be accessed later by the crml-eth-bridge offchain worker.
	if let Some(ref eth_http_uri) = cli.run.eth_http {
		backend.offchain_storage().unwrap().set(
			sp_core::offchain::STORAGE_PREFIX,
			&ETH_HTTP_URI,
			eth_http_uri.as_bytes(),
		);
	}

	let shared_voter_state = rpc_setup;
	let auth_disc_publish_non_global_ips = config.network.allow_non_globals_in_dht;

	config
		.network
		.extra_sets
		.push(sc_finality_grandpa::grandpa_peers_set_config());

	config.network.extra_sets.push(ethy_gadget::ethy_peers_set_config());
	let warp_sync = Arc::new(sc_finality_grandpa::warp_proof::NetworkProvider::new(
		backend.clone(),
		import_setup.1.shared_authority_set().clone(),
		Vec::default(),
	));

	let (network, system_rpc_tx, network_starter) = sc_service::build_network(sc_service::BuildNetworkParams {
		config: &config,
		client: client.clone(),
		transaction_pool: transaction_pool.clone(),
		spawn_handle: task_manager.spawn_handle(),
		import_queue,
		block_announce_validator_builder: None,
		warp_sync: Some(warp_sync),
	})?;

	if config.offchain_worker.enabled {
		sc_service::build_offchain_workers(&config, task_manager.spawn_handle(), client.clone(), network.clone());
	}

	let (block_import, grandpa_link, babe_link) = import_setup;

	let role = config.role.clone();
	let babe_config = babe_link.config().clone();
	let force_authoring = config.force_authoring;
	let backoff_authoring_blocks = Some(sc_consensus_slots::BackoffAuthoringOnFinalizedHeadLagging::default());
	let name = config.network.node_name.clone();
	let enable_grandpa = !config.disable_grandpa;
	let shared_authority_set = grandpa_link.shared_authority_set().clone();
	let finality_proof_provider = sc_finality_grandpa::FinalityProofProvider::new_for_service(
		backend.clone(),
		Some(shared_authority_set.clone()),
	);
	let justification_stream = grandpa_link.justification_stream();
	let shared_epoch_changes = babe_link.epoch_changes().clone();
	let keystore = keystore_container.sync_keystore();
	let prometheus_registry = config.prometheus_registry().cloned();
	let fee_history_limit = cli.run.fee_history_limit;
	let subscription_task_executor = sc_rpc::SubscriptionTaskExecutor::new(task_manager.spawn_handle());
	let overrides = crate::rpc::overrides_handle(client.clone());
	let block_data_cache = Arc::new(fc_rpc::EthBlockDataCache::new(
		task_manager.spawn_handle(),
		overrides.clone(),
		50,
		50,
	));
	let (event_proof_sender, event_proof_stream) = ethy_gadget::notification::EthyEventProofStream::channel();

	let rpc_extensions_builder = {
		let client = client.clone();
		let backend = backend.clone();
		let select_chain = select_chain.clone();
		let chain_spec = config.chain_spec.cloned_box();
		let shared_voter_state = shared_voter_state.clone();
		let pool = transaction_pool.clone();
		let network = network.clone();
		let filter_pool = filter_pool.clone();
		let frontier_backend = frontier_backend.clone();
		let overrides = overrides.clone();
		let fee_history_cache = fee_history_cache.clone();
		let max_past_logs = cli.run.max_past_logs;

		Box::new(move |deny_unsafe, _| {
			let deps = crate::rpc::FullDeps {
				backend: backend.clone(),
				client: client.clone(),
				command_sink: None,
				deny_unsafe,
				filter_pool: filter_pool.clone(),
				frontier_backend: frontier_backend.clone(),
				graph: pool.pool().clone(),
				pool: pool.clone(),
				is_authority: false,
				max_past_logs,
				fee_history_limit,
				fee_history_cache: fee_history_cache.clone(),
				network: network.clone(),
				select_chain: select_chain.clone(),
				chain_spec: chain_spec.cloned_box(),
				block_data_cache: block_data_cache.clone(),
				babe: node_rpc::BabeDeps {
					babe_config: babe_config.clone(),
					shared_epoch_changes: shared_epoch_changes.clone(),
					keystore: keystore.clone(),
				},
				ethy: node_rpc::EthyDeps {
					event_proof_stream: event_proof_stream.clone(),
					subscription_executor: subscription_task_executor.clone(),
				},
				grandpa: node_rpc::GrandpaDeps {
					shared_voter_state: shared_voter_state.clone(),
					shared_authority_set: shared_authority_set.clone(),
					justification_stream: justification_stream.clone(),
					subscription_executor: subscription_task_executor.clone(),
					finality_provider: finality_proof_provider.clone(),
				},
			};

			Ok(crate::rpc::create_full(
				deps,
				subscription_task_executor.clone(),
				overrides.clone(),
			))
		})
	};

	// load frontier sync block from the chain genesis config
	// this signals the client to start mapping ethereum blocks from a set height.
	// This is important for chains which were upgraded to include frontier, in suchcases
	// clients should only scan back to the point of the first frontier digest/block (post runtime-upgrade).
	//
	// NB: default value of `0` will cause the node to rescan all blocks from current back to `0`
	let frontier_sync_from = match config.chain_spec.properties().get("frontierGenesisBlockNumber") {
		Some(serde_json::Value::String(number)) => u32::from_str(number).unwrap_or(0),
		_ => 0,
	};
	if frontier_sync_from == 0 {
		warn!(target: "mapping-sync", "scanning all blocks from latest back to genesis!\nthis should not happen outside of test environment!");
	}
	debug!(target: "mapping-sync", "starting frontier mapping sync from block: {}", frontier_sync_from);

	let rpc_extensions_builder = rpc_extensions_builder;

	let _rpc_handlers = sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		config,
		backend: backend.clone(),
		client: client.clone(),
		keystore: keystore_container.sync_keystore(),
		network: network.clone(),
		rpc_extensions_builder,
		transaction_pool: transaction_pool.clone(),
		task_manager: &mut task_manager,
		system_rpc_tx,
		telemetry: telemetry.as_mut(),
	})?;

	task_manager.spawn_essential_handle().spawn(
		"frontier-mapping-sync-worker",
		None,
		MappingSyncWorker::new(
			client.import_notification_stream(),
			Duration::new(6, 0),
			client.clone(),
			backend.clone(),
			frontier_backend.clone(),
			3,
			frontier_sync_from,
			SyncStrategy::Normal,
		)
		.for_each(|()| futures::future::ready(())),
	);

	// Spawn Frontier EthFilterApi maintenance task.
	if let Some(filter_pool) = filter_pool {
		// Each filter is allowed to stay in the pool for 100 blocks.
		const FILTER_RETAIN_THRESHOLD: u64 = 100;
		task_manager.spawn_essential_handle().spawn(
			"frontier-filter-pool",
			None,
			EthTask::filter_pool_task(Arc::clone(&client), filter_pool, FILTER_RETAIN_THRESHOLD),
		);
	}

	// Spawn Frontier FeeHistory cache maintenance task.
	task_manager.spawn_essential_handle().spawn(
		"frontier-fee-history",
		None,
		EthTask::fee_history_task(
			Arc::clone(&client),
			Arc::clone(&overrides),
			fee_history_cache,
			fee_history_limit,
		),
	);

	task_manager.spawn_essential_handle().spawn(
		"frontier-schema-cache-task",
		None,
		EthTask::ethereum_schema_cache_task(Arc::clone(&client), Arc::clone(&frontier_backend)),
	);

	(with_startup_data)(&block_import, &babe_link);

	if let sc_service::config::Role::Authority { .. } = &role {
		let proposer = sc_basic_authorship::ProposerFactory::new(
			task_manager.spawn_handle(),
			client.clone(),
			transaction_pool.clone(),
			prometheus_registry.as_ref(),
			telemetry.as_ref().map(|x| x.handle()),
		);

		let can_author_with = sp_consensus::CanAuthorWithNativeVersion::new(client.executor().clone());

		let client_clone = client.clone();
		let slot_duration = babe_link.config().slot_duration();
		let babe_config = sc_consensus_babe::BabeParams {
			keystore: keystore_container.sync_keystore(),
			client: client.clone(),
			select_chain,
			env: proposer,
			block_import,
			sync_oracle: network.clone(),
			justification_sync_link: network.clone(),
			create_inherent_data_providers: move |parent, ()| {
				let client_clone = client_clone.clone();
				async move {
					let uncles = sc_consensus_uncles::create_uncles_inherent_data_provider(&*client_clone, parent)?;

					let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

					let slot = sp_consensus_babe::inherents::InherentDataProvider::from_timestamp_and_duration(
						*timestamp,
						slot_duration,
					);

					let storage_proof =
						sp_transaction_storage_proof::registration::new_data_provider(&*client_clone, &parent)?;

					Ok((timestamp, slot, uncles, storage_proof))
				}
			},
			force_authoring,
			backoff_authoring_blocks,
			babe_link,
			can_author_with,
			block_proposal_slot_portion: SlotProportion::new(0.5),
			max_block_proposal_slot_portion: None,
			telemetry: telemetry.as_ref().map(|x| x.handle()),
		};

		let babe = sc_consensus_babe::start_babe(babe_config)?;
		task_manager
			.spawn_essential_handle()
			.spawn_blocking("babe-proposer", Some("block-authoring"), babe);
	}

	// Spawn authority discovery module.
	if role.is_authority() {
		let authority_discovery_role = sc_authority_discovery::Role::PublishAndDiscover(keystore_container.keystore());
		let dht_event_stream = network.event_stream("authority-discovery").filter_map(|e| async move {
			match e {
				Event::Dht(e) => Some(e),
				_ => None,
			}
		});
		let (authority_discovery_worker, _service) = sc_authority_discovery::new_worker_and_service_with_config(
			sc_authority_discovery::WorkerConfig {
				publish_non_global_ips: auth_disc_publish_non_global_ips,
				..Default::default()
			},
			client.clone(),
			network.clone(),
			Box::pin(dht_event_stream),
			authority_discovery_role,
			prometheus_registry.clone(),
		);

		task_manager.spawn_handle().spawn(
			"authority-discovery-worker",
			Some("networking"),
			authority_discovery_worker.run(),
		);
	}

	// if the node isn't actively participating in consensus then it doesn't
	// need a keystore, regardless of which protocol we use below.
	let keystore = if role.is_authority() {
		Some(keystore_container.sync_keystore())
	} else {
		None
	};

	let ethy_params = ethy_gadget::EthyParams {
		client: client.clone(),
		backend,
		key_store: keystore.clone(),
		network: network.clone(),
		event_proof_sender,
		prometheus_registry: prometheus_registry.clone(),
		_phantom: std::marker::PhantomData,
	};

	// Start the ETHY bridge gadget.
	task_manager.spawn_essential_handle().spawn_blocking(
		"ethy-gadget",
		None,
		ethy_gadget::start_ethy_gadget::<_, _, _, _>(ethy_params),
	);

	let config = sc_finality_grandpa::Config {
		// FIXME #1578 make this available through chainspec
		gossip_duration: std::time::Duration::from_millis(333),
		justification_period: 512,
		name: Some(name),
		observer_enabled: false,
		keystore,
		local_role: role,
		telemetry: telemetry.as_ref().map(|x| x.handle()),
	};

	if enable_grandpa {
		// start the full GRANDPA voter
		// NOTE: non-authorities could run the GRANDPA observer protocol, but at
		// this point the full voter should provide better guarantees of block
		// and vote data availability than the observer. The observer has not
		// been tested extensively yet and having most nodes in a network run it
		// could lead to finality stalls.
		let grandpa_config = sc_finality_grandpa::GrandpaParams {
			config,
			link: grandpa_link,
			network: network.clone(),
			telemetry: telemetry.as_ref().map(|x| x.handle()),
			voting_rule: sc_finality_grandpa::VotingRulesBuilder::default().build(),
			prometheus_registry,
			shared_voter_state,
		};

		// the GRANDPA voter task is considered infallible, i.e.
		// if it fails we take down the service with it.
		task_manager.spawn_essential_handle().spawn_blocking(
			"grandpa-voter",
			None,
			sc_finality_grandpa::run_grandpa_voter(grandpa_config)?,
		);
	}

	network_starter.start_network();
	Ok(NewFullBase {
		task_manager,
		client,
		network,
		transaction_pool,
	})
}

/// Builds a new service for a full client.
pub fn new_full(config: Configuration, cli: &crate::cli::Cli) -> Result<TaskManager, ServiceError> {
	new_full_base(config, cli, |_, _| ()).map(|NewFullBase { task_manager, .. }| task_manager)
}

pub fn frontier_database_dir(config: &Configuration) -> std::path::PathBuf {
	let config_dir = config
		.base_path
		.as_ref()
		.map(|base_path| base_path.config_dir(config.chain_spec.id()))
		.unwrap_or_else(|| {
			BasePath::from_project("", "", &crate::cli::Cli::executable_name()).config_dir(config.chain_spec.id())
		});
	config_dir.join("frontier").join("db")
}

pub fn open_frontier_backend(config: &Configuration) -> Result<Arc<fc_db::Backend<Block>>, String> {
	Ok(Arc::new(fc_db::Backend::<Block>::new(&fc_db::DatabaseSettings {
		source: fc_db::DatabaseSettingsSrc::RocksDb {
			path: frontier_database_dir(&config),
			cache_size: 0,
		},
	})?))
}
