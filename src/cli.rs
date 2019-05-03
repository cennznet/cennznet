use crate::chain_spec;
use crate::service;
use std::ops::Deref;
use substrate_cli as cli;
pub use substrate_cli::{error, IntoExit, NoCustom, VersionInfo};
use substrate_service::{Roles as ServiceRoles, ServiceFactory};
use tokio::prelude::Future;
use tokio::runtime::Runtime;

/// The chain specification option.
#[derive(Clone, Debug)]
pub enum ChainSpec {
	/// The CENNZnet to be mainnet
	CennznetMain,

	/// Whatever the current runtime is, with just Alice as an auth.
	Development,

	/// The CENNZnet Kauri testnet.
	CennznetKauri,
	/// The CENNZnet Rumi testnet.
	CennznetRimu,
	/// The CENNZnet Kauri for local test purpose
	CennznetKauriDev,

	/// The CENNZnet Kauri testnet, with latest runtime
	CennznetKauriLatest,
	/// The CENNZnet Rumi testnet, with latest runtime
	CennznetRimuLatest,
	/// The CENNZnet to be mainnet, with latest runtime
	CennznetMainLatest,
}

/// Get a chain config from a spec setting.
impl ChainSpec {
	pub(crate) fn load(self) -> Result<chain_spec::ChainSpec, String> {
		match self {
			ChainSpec::CennznetMain => chain_spec::mainnet::config(),
			ChainSpec::Development => chain_spec::dev::config(),
			ChainSpec::CennznetKauri => chain_spec::testnet::kauri_config(),
			ChainSpec::CennznetRimu => chain_spec::testnet::rimu_config(),
			ChainSpec::CennznetKauriDev => chain_spec::testnet::kauri_dev_config(),
			ChainSpec::CennznetKauriLatest => chain_spec::testnet::kauri_latest_config(),
			ChainSpec::CennznetRimuLatest => chain_spec::testnet::rimu_latest_config(),
			ChainSpec::CennznetMainLatest => chain_spec::mainnet::latest_config(),
		}
	}

	pub(crate) fn from(s: &str) -> Option<Self> {
		match s {
			"main" | "cennznet" => Some(ChainSpec::CennznetMain),
			"dev" => Some(ChainSpec::Development),
			"kauri" => Some(ChainSpec::CennznetKauri),
			"" | "rimu" => Some(ChainSpec::CennznetRimu),
			"kauri-dev" => Some(ChainSpec::CennznetKauriDev),
			"kauri-latest" => Some(ChainSpec::CennznetKauriLatest),
			"rimu-latest" => Some(ChainSpec::CennznetRimuLatest),
			"main-latest" => Some(ChainSpec::CennznetMainLatest),
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
pub fn run<I, T, E>(args: I, exit: E, version: cli::VersionInfo) -> error::Result<()>
where
	I: IntoIterator<Item = T>,
	T: Into<std::ffi::OsString> + Clone,
	E: IntoExit,
{
	cli::parse_and_execute::<service::Factory, NoCustom, NoCustom, _, _, _, _, _>(
		load_spec,
		&version,
		"cennznet-node",
		args,
		exit,
		|exit, _cli_args, _custom_args, mut config| {
			config.rpc_cors = None; // TODO: remove this when we figured out how react native plays with CORS
			info!("{}", version.name);
			info!("  version {}", config.full_version());
			info!("  by {}", version.author);
			info!("Chain specification: {}", config.chain_spec.name());
			info!("Node name: {}", config.name);
			info!("Roles: {:?}", config.roles);
			let runtime = Runtime::new().map_err(|e| format!("{:?}", e))?;
			let executor = runtime.executor();
			match config.roles {
				ServiceRoles::LIGHT => run_until_exit(
					runtime,
					service::Factory::new_light(config, executor).map_err(|e| format!("{:?}", e))?,
					exit,
				),
				_ => run_until_exit(
					runtime,
					service::Factory::new_full(config, executor).map_err(|e| format!("{:?}", e))?,
					exit,
				),
			}
			.map_err(|e| format!("{:?}", e))
		},
	)
	.map_err(Into::into)
	.map(|_| ())
}

fn run_until_exit<T, C, E>(mut runtime: Runtime, service: T, e: E) -> error::Result<()>
where
	T: Deref<Target = substrate_service::Service<C>>,
	C: substrate_service::Components,
	E: IntoExit,
{
	let (exit_send, exit) = exit_future::signal();

	let executor = runtime.executor();
	cli::informant::start(&service, exit.clone(), executor.clone());

	let _ = runtime.block_on(e.into_exit());
	exit_send.fire();

	// we eagerly drop the service so that the internal exit future is fired,
	// but we need to keep holding a reference to the global telemetry guard
	let _telemetry = service.telemetry();
	drop(service);

	// TODO [andre]: timeout this future #1318
	let _ = runtime.shutdown_on_idle().wait();

	Ok(())
}
