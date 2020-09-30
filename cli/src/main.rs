//! CENNZnet node cli entrypoint
use cennznet_cli::command;

fn main() -> sc_cli::Result<()> {
	command::run()
}
