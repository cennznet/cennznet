//! CENNZnet node cli entrypoint
use sc_cli;
use cennznet_cli::command;

fn main() -> sc_cli::Result<()> {
	command::run()
}
