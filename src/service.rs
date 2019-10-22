// Copyright 2019 Centrality Investments Limited
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
#![warn(unused_extern_crates)]

//! Service and ServiceFactory implementation. Specialized wrapper over substrate service.

use std::sync::Arc;
use std::time::Duration;

use cennznet_primitives::Block;
use cennznet_runtime::{GenesisConfig, RuntimeApi};
use client;
use consensus::{import_queue, start_aura, AuraImportQueue, NothingExtra, SlotDuration};
use grandpa;
use inherents::InherentDataProviders;
use primitives::{ed25519, Pair};
use substrate_service::TelemetryOnConnect;
use substrate_service::{
	FactoryFullConfiguration, FullBackend, FullClient, FullComponents, FullExecutor, LightBackend, LightClient,
	LightComponents, LightExecutor, TaskExecutor,
};
use transaction_pool::{self, txpool::Pool as TransactionPool};

pub use substrate_executor::NativeExecutor;
native_executor_instance!(
	pub Executor,
	cennznet_runtime::api::dispatch,
	cennznet_runtime::native_version,
	include_bytes!("../runtime/wasm/target/wasm32-unknown-unknown/release/cennznet_runtime.compact.wasm")
);

construct_simple_protocol! {
	/// Demo protocol attachment for substrate.
	pub struct NodeProtocol where Block = Block { }
}

/// Node specific configuration
pub struct NodeConfig<F: substrate_service::ServiceFactory> {
	/// grandpa connection to import block
	// FIXME: rather than putting this on the config, let's have an actual intermediate setup state
	// https://github.com/paritytech/substrate/issues/1134
	pub grandpa_import_setup: Option<(Arc<grandpa::BlockImportForService<F>>, grandpa::LinkHalfForService<F>)>,
	inherent_data_providers: InherentDataProviders,
}

impl<F> Default for NodeConfig<F>
where
	F: substrate_service::ServiceFactory,
{
	fn default() -> NodeConfig<F> {
		NodeConfig {
			grandpa_import_setup: None,
			inherent_data_providers: InherentDataProviders::new(),
		}
	}
}

construct_service_factory! {
	struct Factory {
		Block = Block,
		RuntimeApi = RuntimeApi,
		NetworkProtocol = NodeProtocol { |config| Ok(NodeProtocol::new()) },
		RuntimeDispatch = Executor,
		FullTransactionPoolApi = transaction_pool::ChainApi<client::Client<FullBackend<Self>, FullExecutor<Self>, Block, RuntimeApi>, Block>
			{ |config, client| Ok(TransactionPool::new(config, transaction_pool::ChainApi::new(client))) },
		LightTransactionPoolApi = transaction_pool::ChainApi<client::Client<LightBackend<Self>, LightExecutor<Self>, Block, RuntimeApi>, Block>
			{ |config, client| Ok(TransactionPool::new(config, transaction_pool::ChainApi::new(client))) },
		Genesis = GenesisConfig,
		Configuration = NodeConfig<Self>,
		FullService = FullComponents<Self>
			{ |config: FactoryFullConfiguration<Self>, executor: TaskExecutor|
				FullComponents::<Factory>::new(config, executor) },
		AuthoritySetup = {
			|mut service: Self::FullService, executor: TaskExecutor, local_key: Option<Arc<ed25519::Pair>>| {
				let (block_import, link_half) = service.config.custom.grandpa_import_setup.take()
					.expect("Link Half and Block Import are present for Full Services or setup failed before. qed");

				if let Some(ref key) = local_key {
					info!("Using authority key {}", key.public());
					let proposer = Arc::new(substrate_basic_authorship::ProposerFactory {
						client: service.client(),
						transaction_pool: service.transaction_pool(),
						inherents_pool: service.inherents_pool(),
					});

					let client = service.client();
					executor.spawn(start_aura(
						SlotDuration::get_or_compute(&*client)?,
						key.clone(),
						client,
						block_import.clone(),
						proposer,
						service.network(),
						service.on_exit(),
						service.config.custom.inherent_data_providers.clone(),
						service.config.force_authoring,
					)?);

					info!("Running Grandpa session as Authority {}", key.public());
				}

				let local_key = if service.config.disable_grandpa {
					None
				} else {
					local_key
				};

				let config = grandpa::Config {
					local_key,
					// FIXME #1578 make this available through chainspec
					gossip_duration: Duration::from_millis(1000),
					justification_period: 4096,
					name: Some(service.config.name.clone())
				};

				match config.local_key {
					None => {
						executor.spawn(grandpa::run_grandpa_observer(
							config,
							link_half,
							service.network(),
							service.on_exit(),
						)?);
					},
					Some(_) => {
						let telemetry_on_connect = TelemetryOnConnect {
						  on_exit: Box::new(service.on_exit()),
						  telemetry_connection_sinks: service.telemetry_on_connect_stream(),
						  executor: &executor,
						};
						let grandpa_config = grandpa::GrandpaParams {
						  config: config,
						  link: link_half,
						  network: service.network(),
						  inherent_data_providers: service.config.custom.inherent_data_providers.clone(),
						  on_exit: service.on_exit(),
						  telemetry_on_connect: Some(telemetry_on_connect),
						};
						executor.spawn(grandpa::run_grandpa_voter(grandpa_config)?);
					},
				}

				Ok(service)
			}
		},
		LightService = LightComponents<Self>
			{ |config, executor| <LightComponents<Factory>>::new(config, executor) },
		FullImportQueue = AuraImportQueue<Self::Block>
			{ |config: &mut FactoryFullConfiguration<Self> , client: Arc<FullClient<Self>>| {
				let slot_duration = SlotDuration::get_or_compute(&*client)?;
				let (block_import, link_half) =
					grandpa::block_import::<_, _, _, RuntimeApi, FullClient<Self>>(
						client.clone(), client.clone()
					)?;
				let block_import = Arc::new(block_import);
				let justification_import = block_import.clone();

				config.custom.grandpa_import_setup = Some((block_import.clone(), link_half));

				import_queue::<_, _, _, ed25519::Pair>(
					slot_duration,
					block_import,
					Some(justification_import),
					client,
					NothingExtra,
					config.custom.inherent_data_providers.clone(),
				).map_err(Into::into)
			}},
		LightImportQueue = AuraImportQueue<Self::Block>
			{ |config: &FactoryFullConfiguration<Self>, client: Arc<LightClient<Self>>| {
				import_queue::<_, _, _, ed25519::Pair>(
					SlotDuration::get_or_compute(&*client)?,
					client.clone(),
					None,
					client,
					NothingExtra,
					config.custom.inherent_data_providers.clone(),
				).map_err(Into::into)
			}
		},
	}
}

#[cfg(test)]
mod tests {
	#[cfg(feature = "rhd")]
	fn test_sync() {
		use client::{BlockOrigin, ImportBlock};
		use {service_test, Factory};

		let alice: Arc<ed25519::Pair> = Arc::new(Keyring::Alice.into());
		let bob: Arc<ed25519::Pair> = Arc::new(Keyring::Bob.into());
		let validators = vec![alice.public().0.into(), bob.public().0.into()];
		let keys: Vec<&ed25519::Pair> = vec![&*alice, &*bob];
		let dummy_runtime = ::tokio::runtime::Runtime::new().unwrap();
		let block_factory = |service: &<Factory as service::ServiceFactory>::FullService| {
			let block_id = BlockId::number(service.client().info().unwrap().chain.best_number);
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
			ImportBlock {
				origin: BlockOrigin::File,
				justification: Vec::new(),
				internal_justification: Vec::new(),
				finalized: true,
				body: Some(block.extrinsics),
				header: block.header,
				auxiliary: Vec::new(),
			}
		};
		let extrinsic_factory = |service: &<Factory as service::ServiceFactory>::FullService| {
			let payload = (
				0,
				Call::Balances(BalancesCall::transfer(RawAddress::Id(bob.public().0.into()), 69.into())),
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
		service_test::sync::<Factory, _, _>(chain_spec::integration_test_config(), block_factory, extrinsic_factory);
	}
}
