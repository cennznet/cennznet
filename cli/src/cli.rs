pub use sc_cli::Result;
use sc_cli::{Error, RunCmd};
use structopt::StructOpt;

/// Parse `uri`
fn parse_uri(uri: &str) -> Result<String> {
	let _ = url::Url::parse(uri).map_err(|_| Error::Input("invalid eth http URI".into()))?;
	Ok(uri.into())
}

#[derive(Debug, StructOpt)]
pub struct EthClientOpts {
	/// Ethereum JSON-RPC client endpoint
	#[structopt(parse(try_from_str = parse_uri), long = "eth-http", about = "Ethereum client JSON-RPC endpoint")]
	pub eth_http: Option<String>,
}

#[derive(Debug, StructOpt)]
pub struct Cli {
	#[structopt(subcommand)]
	pub subcommand: Option<Subcommand>,
	#[structopt(flatten)]
	pub run: RunCmd,
	#[structopt(flatten)]
	pub eth_opts: EthClientOpts,
}

#[derive(Debug, StructOpt)]
pub enum Subcommand {
	/// Build a chain specification.
	BuildSpec(sc_cli::BuildSpecCmd),

	/// Validate blocks.
	CheckBlock(sc_cli::CheckBlockCmd),

	/// Export blocks.
	ExportBlocks(sc_cli::ExportBlocksCmd),

	/// Export the state of a given block into a chain spec.
	ExportState(sc_cli::ExportStateCmd),

	/// Import blocks.
	ImportBlocks(sc_cli::ImportBlocksCmd),

	/// Remove the whole chain.
	PurgeChain(sc_cli::PurgeChainCmd),

	/// Revert the chain to a previous state.
	Revert(sc_cli::RevertCmd),

	/// The custom benchmark subcommmand benchmarking runtime pallets.
	#[structopt(name = "benchmark", about = "Benchmark runtime pallets.")]
	Benchmark(frame_benchmarking_cli::BenchmarkCmd),
}
