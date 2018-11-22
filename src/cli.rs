use service;
use tokio::runtime::Runtime;
pub use substrate_cli::{VersionInfo, IntoExit, error};
use substrate_cli::{Action, parse_matches, execute_default, CoreParams};
use substrate_service::{ServiceFactory, Roles as ServiceRoles};
use chain_spec;
use std::ops::Deref;
use structopt::StructOpt;

/// Extend params for Node
#[derive(Debug, StructOpt)]
pub struct NodeParams {
	/// Should run as a GRANDPA authority node
	#[structopt(long = "grandpa-authority", help = "Run Node as a GRANDPA authority, implies --validator")]
	grandpa_authority: bool,

	/// Should run as a GRANDPA authority node only
	#[structopt(long = "grandpa-authority-only", help = "Run Node as a GRANDPA authority only, don't as a usual validator, implies --grandpa-authority")]
	grandpa_authority_only: bool,

	#[structopt(flatten)]
	core: CoreParams
}

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

	let (spec, mut config) = parse_matches::<service::Factory, _>(load_spec, version, "substrate-node", &matches)?;

	if matches.is_present("grandpa_authority_only") {
		config.custom.grandpa_authority = true;
		config.custom.grandpa_authority_only = true;
		// Authority Setup is only called if validator is set as true
		config.roles = ServiceRoles::AUTHORITY;
	} else if matches.is_present("grandpa_authority") {
		config.custom.grandpa_authority = true;
		// Authority Setup is only called if validator is set as true
		config.roles = ServiceRoles::AUTHORITY;
	}

	match execute_default::<service::Factory, _>(spec, exit, &matches)? {
		Action::ExecutedInternally => (),
		Action::RunService(exit) => {
			info!("CENNZNET Node");
			info!("  version {}", config.full_version());
			info!("  by Centrality");
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
