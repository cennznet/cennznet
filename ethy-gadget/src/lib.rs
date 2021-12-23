// Copyright (C) 2020-2021 Parity Technologies (UK) Ltd. & Centrality Investments Ltd
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

//! 'Ethy' is the CENNZnet event proving protocol
//! It is based on the same architecture as substrate's 'BEEFY' protocol.
//!
//! Active validators receive requests to witness events from runtime messages added to blocks.
//! Validators then sign the event and share with peers
//! Once a threshold hold of votes have been assembled a proof is generated, stored in auxiliary db storage and
// shared over RPC to subscribers.
//!
//! The current implementation simply assembles signatures from individual validators.
//!

use std::sync::Arc;

use log::debug;
use prometheus::Registry;

use sc_client_api::{Backend, BlockchainEvents, Finalizer};
use sc_network_gossip::{GossipEngine, Network as GossipNetwork};

use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_keystore::SyncCryptoStorePtr;
use sp_runtime::traits::Block;

use cennznet_primitives::eth::EthyApi;

mod error;
mod gossip;
mod keystore;
mod metrics;
mod witness_record;
mod worker;

pub mod notification;

/// The p2p protocol name for Eth bridge messages
pub const ETHY_PROTOCOL_NAME: &'static str = "/cennznet/ethy/1";

/// Returns the configuration value to put in
/// [`sc_network::config::NetworkConfiguration::extra_sets`].
pub fn ethy_peers_set_config() -> sc_network::config::NonDefaultSetConfig {
	sc_network::config::NonDefaultSetConfig {
		notifications_protocol: ETHY_PROTOCOL_NAME.into(),
		max_notification_size: 1024 * 1024,
		set_config: sc_network::config::SetConfig {
			in_peers: 25,
			out_peers: 25,
			reserved_nodes: Vec::new(),
			non_reserved_mode: sc_network::config::NonReservedPeerMode::Accept,
		},
		fallback_names: vec![],
	}
}

/// A convenience ETHY client trait that defines all the type bounds a ETHY client
/// has to satisfy. Ideally that should actually be a trait alias. Unfortunately as
/// of today, Rust does not allow a type alias to be used as a trait bound. Tracking
/// issue is <https://github.com/rust-lang/rust/issues/41517>.
pub trait Client<B, BE>:
	BlockchainEvents<B> + HeaderBackend<B> + Finalizer<B, BE> + ProvideRuntimeApi<B> + Send + Sync
where
	B: Block,
	BE: Backend<B>,
{
	// empty
}

impl<B, BE, T> Client<B, BE> for T
where
	B: Block,
	BE: Backend<B>,
	T: BlockchainEvents<B> + HeaderBackend<B> + Finalizer<B, BE> + ProvideRuntimeApi<B> + Send + Sync,
{
	// empty
}

/// ETHY gadget initialization parameters.
pub struct EthyParams<B, BE, C, N>
where
	B: Block,
	BE: Backend<B>,
	C: Client<B, BE>,
	C::Api: EthyApi<B>,
	N: GossipNetwork<B> + Clone + Send + 'static,
{
	/// ETHY client
	pub client: Arc<C>,
	/// Client Backend
	pub backend: Arc<BE>,
	/// Local key store
	pub key_store: Option<SyncCryptoStorePtr>,
	/// Gossip network
	pub network: N,
	/// ETHY signed witness sender
	pub event_proof_sender: notification::EthyEventProofSender,
	/// Prometheus metric registry
	pub prometheus_registry: Option<Registry>,
	pub _phantom: std::marker::PhantomData<B>,
}

/// Start the ETHY gadget.
///
/// This is a thin shim around running and awaiting a ETHY worker.
pub async fn start_ethy_gadget<B, BE, C, N>(ethy_params: EthyParams<B, BE, C, N>)
where
	B: Block,
	BE: Backend<B>,
	C: Client<B, BE>,
	C::Api: EthyApi<B>,
	N: GossipNetwork<B> + Clone + Send + 'static,
{
	let EthyParams {
		client,
		backend,
		key_store,
		network,
		event_proof_sender,
		prometheus_registry,
		_phantom: std::marker::PhantomData,
	} = ethy_params;

	let gossip_validator = Arc::new(gossip::GossipValidator::new(Default::default()));
	let gossip_engine = GossipEngine::new(network, ETHY_PROTOCOL_NAME, gossip_validator.clone(), None);

	let metrics = prometheus_registry
		.as_ref()
		.map(metrics::Metrics::register)
		.and_then(|result| match result {
			Ok(metrics) => {
				debug!(target: "ethy", "💎 Registered metrics");
				Some(metrics)
			}
			Err(err) => {
				debug!(target: "ethy", "💎 Failed to register metrics: {:?}", err);
				None
			}
		});

	let worker_params = worker::WorkerParams {
		client,
		backend,
		key_store: key_store.into(),
		event_proof_sender,
		gossip_engine,
		gossip_validator,
		metrics,
	};

	let worker = worker::EthyWorker::<_, _, _>::new(worker_params);

	worker.run().await
}
