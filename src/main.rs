// Copyright 2019 Centrality Investments Limited
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//! CENNZNET CLI

#![warn(missing_docs)]

mod chain_spec;
mod cli;
mod service;

use cli::VersionInfo;

fn main() {
	let version = VersionInfo {
		name: "CENNZnet Node",
		commit: env!("VERGEN_SHA_SHORT"),
		version: env!("CARGO_PKG_VERSION"),
		executable_name: "cennznet",
		author: "Centrality Developers <support@centrality.ai>",
		description: "CENNZnet node",
		support_url: "https://github.com/cennznet/cennznet/issues",
	};

	if let Err(e) = cli::run(::std::env::args(), cli::Exit, version) {
		eprintln!("Fatal error: {}\n\n{:?}", e, e);
		std::process::exit(1)
	}
}
