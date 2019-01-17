use service;
use tokio::prelude::Future;
use tokio::runtime::Runtime;
pub use substrate_cli::{VersionInfo, IntoExit, error};
use substrate_cli::{Action, parse_matches, execute_default, CoreParams, informant};
use substrate_service::{ServiceFactory, Roles as ServiceRoles};
use chain_spec;
use structopt::StructOpt;
use std::ops::Deref;
use app_dirs::AppInfo;

/// Extend params for Node
#[derive(Debug, StructOpt)]
pub struct NodeParams {
	#[structopt(flatten)]
	core: CoreParams
}

const APP_INFO: AppInfo = AppInfo {
	name: "CENNZnet Node",
	author: "Centrality"
};

/// The chain specification option.
#[derive(Clone, Debug)]
pub enum ChainSpec {
	/// Whatever the current runtime is, with just Alice as an auth.
	Development,
	/// The CENNZnet DEV testnet.
	CennznetDev,
	/// The CENNZnet UAT testnet.
	CennznetUat,
	/// Whatever the current runtime is, with lunch DEV testnet defaults.
	LocalCennznetDev,
}

/// Get a chain config from a spec setting.
impl ChainSpec {
	pub(crate) fn load(self) -> Result<chain_spec::ChainSpec, String> {
		match self {
			ChainSpec::Development => chain_spec::local_dev_config(),
			ChainSpec::CennznetDev => chain_spec::cennznet_dev_config(),
			ChainSpec::CennznetUat => chain_spec::cennznet_uat_config(),
			ChainSpec::LocalCennznetDev => chain_spec::local_cennznet_dev_config(),
		}
	}

	pub(crate) fn from(s: &str) -> Option<Self> {
		match s {
			"dev" => Some(ChainSpec::Development),
			"local-cennznet-dev" => Some(ChainSpec::LocalCennznetDev),
			"cennznet-dev" => Some(ChainSpec::CennznetDev),
			"" | "cennznet-uat" => Some(ChainSpec::CennznetUat),
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
pub fn run<I, T, E>(args: I, exit: E, version: VersionInfo) -> error::Result<()> where
	I: IntoIterator<Item = T>,
	T: Into<std::ffi::OsString> + Clone,
	E: IntoExit,
{
	let full_version = substrate_service::config::full_version_from_strs(
		version.version,
		version.commit
	);

	let matches = match NodeParams::clap()
		.name(version.executable_name)
		.author(version.author)
		.about(version.description)
		.version(&(full_version + "\n")[..])
		.get_matches_from_safe(args) {
			Ok(m) => m,
			Err(e) => e.exit(),
		};

	let (spec, config) = parse_matches::<service::Factory, _>(
		load_spec, version, "centrality-cennznet", &matches, &APP_INFO
	)?;

	match execute_default::<service::Factory, _>(spec, exit, &matches, &config, &APP_INFO)? {
		Action::ExecutedInternally => (),
		Action::RunService(exit) => {
			info!("{}", APP_INFO.name);
			info!("  version {}", config.full_version());
			info!("  by {}", APP_INFO.author);
			info!("Chain specification: {}", config.chain_spec.name());
			info!("Node name: {}", config.name);
			info!("Roles: {:?}", config.roles);
			let mut runtime = Runtime::new()?;
			let executor = runtime.executor();
			match config.roles == ServiceRoles::LIGHT {
				true => run_until_exit(runtime, service::Factory::new_light(config, executor)?, exit)?,
				false => run_until_exit(runtime, service::Factory::new_full(config, executor)?, exit)?,
			}
		}
	}
	Ok(())
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
	informant::start(&service, exit.clone(), executor.clone());

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
