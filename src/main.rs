//! CENNZNET CLI

#![warn(missing_docs)]

extern crate ctrlc;
extern crate futures;

#[macro_use]
extern crate error_chain;

extern crate tokio;

extern crate substrate_primitives as primitives;
extern crate exit_future;
extern crate hex_literal;
// #[cfg(test)]
// extern crate substrate_service_test as service_test;
#[macro_use]
extern crate substrate_network as network;
extern crate substrate_consensus_aura as consensus;
extern crate substrate_client as client;
extern crate substrate_cli;
extern crate cennznet_primitives;
#[macro_use]
extern crate substrate_executor;
extern crate substrate_transaction_pool as transaction_pool;
extern crate substrate_finality_grandpa as grandpa;
extern crate substrate_consensus_common as consensus_common;
#[macro_use]
extern crate substrate_service;
extern crate substrate_keystore;
extern crate cennznet_runtime;
extern crate substrate_inherents as inherents;

extern crate structopt;
#[macro_use]
extern crate log;
extern crate app_dirs;

mod cli;
mod chain_spec;
mod service;

use cli::VersionInfo;
use futures::sync::oneshot;
use futures::{future, Future};

use std::cell::RefCell;

// handles ctrl-c
struct Exit;
impl cli::IntoExit for Exit {
	type Exit = future::MapErr<oneshot::Receiver<()>, fn(oneshot::Canceled) -> ()>;
	fn into_exit(self) -> Self::Exit {
		// can't use signal directly here because CtrlC takes only `Fn`.
		let (exit_send, exit) = oneshot::channel();

		let exit_send_cell = RefCell::new(Some(exit_send));
		ctrlc::set_handler(move || {
			if let Some(exit_send) = exit_send_cell.try_borrow_mut().expect("signal handler not reentrant; qed").take() {
				exit_send.send(()).expect("Error sending exit notification");
			}
		}).expect("Error setting Ctrl-C handler");

		exit.map_err(drop)
	}
}

quick_main!(run);

fn run() -> cli::error::Result<()> {
	let version = VersionInfo {
		name: "CENNZnet Node",
		commit: env!("VERGEN_SHA_SHORT"),
		version: env!("CARGO_PKG_VERSION"),
		// TODO: should be cennnzet but this is also used to get the app dir to store chain db
		// Make this substrate ensure the default base path matches to substrate node to avoid breaking change
		// and for convenience that no need to pass a different base path when switching between substrate node and cennznet node
		executable_name: "substrate",
		author: "Centrality Developers <developers@centrality.ai>",
		description: "CENNZnet node",
	};
	cli::run(::std::env::args(), Exit, version)
}
