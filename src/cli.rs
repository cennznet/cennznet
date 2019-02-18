use service;
use tokio::prelude::Future;
use tokio::runtime::Runtime;
use substrate_service::{ServiceFactory, Roles as ServiceRoles};
use chain_spec;
use std::ops::Deref;
pub use substrate_cli::{VersionInfo, IntoExit, NoCustom, error};
use substrate_cli as cli;

/// The chain specification option.
#[derive(Clone, Debug)]
pub enum ChainSpec {
	/// Whatever the current runtime is, with just Alice as an auth.
	Development,
	/// The CENNZnet Kauri testnet.
	CennznetKauri,
	/// The CENNZnet Rumi testnet.
	CennznetRimu,
	/// The CENNZnet Kauri testnet, with latest runtime
	CennznetKauriLatest,
	/// The CENNZnet Rumi testnet, with latest runtime
	CennznetRimuLatest,
	/// The CENNZnet Kauri for local test purpose
	CennznetKauriLocal,
}

/// Get a chain config from a spec setting.
impl ChainSpec {
	pub(crate) fn load(self) -> Result<chain_spec::ChainSpec, String> {
		match self {
			ChainSpec::Development => chain_spec::local_dev_config(),
			ChainSpec::CennznetKauri => chain_spec::cennznet_dev_config(),
			ChainSpec::CennznetRimu => chain_spec::cennznet_uat_config(),
			ChainSpec::CennznetKauriLatest => chain_spec::cennznet_dev_config_latest(),
			ChainSpec::CennznetRimuLatest => chain_spec::cennznet_uat_config_latest(),
			ChainSpec::CennznetKauriLocal => chain_spec::cennznet_dev_local_config(),
		}
	}

	pub(crate) fn from(s: &str) -> Option<Self> {
		match s {
			"dev" => Some(ChainSpec::Development),
			"kauri" => Some(ChainSpec::CennznetKauri),
			"" | "rimu" => Some(ChainSpec::CennznetRimu),
			"kauri-latest" => Some(ChainSpec::CennznetKauriLatest),
			"rimu-latest" => Some(ChainSpec::CennznetRimuLatest),
			"kauri-dev" => Some(ChainSpec::CennznetKauriLocal),
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
pub fn run<I, T, E>(args: I, exit: E, version: cli::VersionInfo) -> error::Result<()> where
	I: IntoIterator<Item = T>,
	T: Into<std::ffi::OsString> + Clone,
	E: IntoExit,
{
	cli::parse_and_execute::<service::Factory, NoCustom, NoCustom, _, _, _, _, _>(
		load_spec, &version, "cennznet-node", args, exit,
		|exit, _custom_args, config| {
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
					exit
				),
				_ => run_until_exit(
					runtime,
					service::Factory::new_full(config, executor).map_err(|e| format!("{:?}", e))?,
					exit
				),
			}.map_err(|e| format!("{:?}", e))
		}
	).map_err(Into::into).map(|_| ())
}

fn run_until_exit<T, C, E>(
	mut runtime: Runtime,
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
