//! CENNZnet node cli entrypoint
use cennznet_cli::command;
use sc_cli;

fn main() -> sc_cli::Result<()> {
	command::run()
}
