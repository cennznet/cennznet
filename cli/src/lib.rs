// Copyright 2018-2020 Parity Technologies (UK) Ltd. and Centrality Investments Ltd.
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

//! CENNZnet CLI library.
//!
//! - `cli` (default): exposes functions that parse command-line options, then start and run the
//! node as a CLI application.

#![warn(missing_docs)]

#[macro_use]
extern crate hex_literal;

pub mod chain_spec;

#[macro_use]
mod service;
#[cfg(feature = "cli")]
mod cli;
#[cfg(feature = "cli")]
mod command;
#[cfg(feature = "cli")]
mod factory_impl;

#[cfg(feature = "cli")]
pub use cli::*;
#[cfg(feature = "cli")]
pub use command::*;

#[cfg(feature = "cli")]
pub use sc_cli::{Result as CliResult, VersionInfo};

/// The chain specification option.
#[derive(Clone, Debug, PartialEq)]
pub enum ChainSpec {
	/// Whatever the current runtime is, with just Alice as an auth.
	Development,
	/// The CENNZnet Kauri testnet.
	CennznetKauri,
	/// The CENNZnet Azalea MainNet
	CennznetAzalea,
}

/// Get a chain config from a spec setting.
impl ChainSpec {
	pub(crate) fn load(self) -> Result<chain_spec::ChainSpec, String> {
		Ok(match self {
			ChainSpec::Development => chain_spec::dev::config(),
			ChainSpec::CennznetKauri => chain_spec::kauri::config(),
			ChainSpec::CennznetAzalea => chain_spec::azalea::config(),
		})
	}

	pub(crate) fn from(s: &str) -> Option<Self> {
		match s {
			"dev" => Some(ChainSpec::Development),
			"kauri" => Some(ChainSpec::CennznetKauri),
			"azalea" => Some(ChainSpec::CennznetAzalea),
			_ => None,
		}
	}
}

fn load_spec(id: &str) -> Result<Box<dyn sc_service::ChainSpec>, String> {
	Ok(match ChainSpec::from(id) {
		Some(spec) => Box::new(spec.load()?),
		None => Box::new(chain_spec::ChainSpec::from_json_file(std::path::PathBuf::from(id))?),
	})
}
