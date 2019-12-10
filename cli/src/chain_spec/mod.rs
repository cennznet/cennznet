// Copyright 2018-2019 Parity Technologies (UK) Ltd. and Centrality Investments Ltd.
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

//! CENNZnet chain configurations.

use substrate_service;

use dev::dev_config_genesis;
use kauri::kauri_config_genesis;
use rimu::rimu_config_genesis;

/// Node `ChainSpec` extensions.
///
/// Additional parameters for some Substrate core modules,
/// customizable from the chain spec.
#[derive(Default, Clone, Serialize, Deserialize, ChainSpecExtension)]
pub struct Extensions {
	/// Block numbers with known hashes.
	pub fork_blocks: client::ForkBlocks<Block>,
}

/// Specialised `ChainSpec`.
pub type ChainSpec = substrate_service::ChainSpec<GenesisConfig, Extensions>;

pub fn dev_config() -> ChainSpec {
	ChainSpec::from_genesis(
		"Development",
		"dev",
		dev_config_genesis,
		vec![],
		None,
		None,
		None,
		Default::default(),
	)
}

pub fn kauri_config() -> ChainSpec {
	ChainSpec::from_genesis(
		"Kauri CENNZnet",
		"Kauri",
		kauri_config_genesis,
		vec![],
		None,
		Some("kauri"),
		None,
		Default::default(),
	)
}

pub fn rimu_config() -> ChainSpec {
	ChainSpec::from_genesis(
		"Rimu CENNZnet",
		"Rimu",
		rimu_config_genesis,
		vec![],
		None,
		Some("rimu"),
		None,
		Default::default(),
	)
}
