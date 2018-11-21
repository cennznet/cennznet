// Copyright 2018 Parity Technologies (UK) Ltd.
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

//! Substrate CLI library.

#![warn(missing_docs)]

pub use substrate_cli::error;

use tokio::runtime::Runtime;
pub use substrate_cli::{VersionInfo, IntoExit};
use substrate_service::{ServiceFactory, Roles as ServiceRoles};
use std::ops::Deref;

use chain_spec;
use service;

/// The chain specification option.
#[derive(Clone, Debug)]
pub enum ChainSpec {
	/// Whatever the current runtime is, with just Alice as an auth.
	Development,
	/// Whatever the current runtime is, with simple Alice/Bob auths.
	LocalTestnet,
	/// Whatever the current runtime is with the "global testnet" defaults.
	StagingTestnet,
}

/// Get a chain config from a spec setting.
impl ChainSpec {
	pub(crate) fn load(self) -> Result<chain_spec::ChainSpec, String> {
		Ok(match self {
			ChainSpec::Development => chain_spec::development_config(),
			ChainSpec::LocalTestnet => chain_spec::local_testnet_config(),
			ChainSpec::StagingTestnet => chain_spec::staging_testnet_config(),
		})
	}

	pub(crate) fn from(s: &str) -> Option<Self> {
		match s {
			"" | "dev" => Some(ChainSpec::Development),
			"local" => Some(ChainSpec::LocalTestnet),
			"staging" => Some(ChainSpec::StagingTestnet),
			_ => None,
		}
	}
}

fn load_spec(id: &str) -> Result<Option<chain_spec::ChainSpec>, String> {
	Ok(match ChainSpec::from(id) {
		Some(spec) => Some(spec.load()?),
		None => None,
	})
}

/// Parse command line arguments into service configuration.
pub fn run<I, T, E>(args: I, exit: E, version: substrate_cli::VersionInfo) -> error::Result<()> where
	I: IntoIterator<Item = T>,
	T: Into<std::ffi::OsString> + Clone,
	E: IntoExit,
{
	match substrate_cli::prepare_execution::<service::Factory, _, _, _, _>(args, exit, version, load_spec, "substrate-node")? {
		substrate_cli::Action::ExecutedInternally => (),
		substrate_cli::Action::RunService((config, exit)) => {
			info!("Substrate Node");
			info!("  version {}", config.full_version());
			info!("  by Parity Technologies, 2017, 2018");
			info!("Chain specification: {}", config.chain_spec.name());
			info!("Node name: {}", config.name);
			info!("Roles: {:?}", config.roles);
			let mut runtime = Runtime::new()?;
			let executor = runtime.executor();
			match config.roles == ServiceRoles::LIGHT {
				true => run_until_exit(&mut runtime, service::Factory::new_light(config, executor)?, exit)?,
				false => run_until_exit(&mut runtime, service::Factory::new_full(config, executor)?, exit)?,
			}
		}
	}
	Ok(())
}

fn run_until_exit<T, C, E>(
	runtime: &mut Runtime,
	service: T,
	e: E,
) -> error::Result<()>
	where
	    T: Deref<Target=substrate_service::Service<C>>,
		C: substrate_service::Components,
		E: IntoExit,
{
	let (exit_send, exit) = exit_future::signal();

	let executor = runtime.executor();
	substrate_cli::informant::start(&service, exit.clone(), executor.clone());

	let _ = runtime.block_on(e.into_exit());
	exit_send.fire();
	Ok(())
}
