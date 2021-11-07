// Copyright 2019-2021
//     by  Centrality Investments Ltd.
//     and Parity Technologies (UK) Ltd.
// This file is part of Plug-blockchain.

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

//! Runtime API definition required by Governance RPC extensions.
//!
//! This API should be imported and implemented by the runtime,
//! of a node that wants to use the custom RPC extension
//! adding System access methods.

#![cfg_attr(not(feature = "std"), no_std)]

use cennznet_primitives::types::AssetId;
use crml_support::{H160 as EthAddress, U256};
use sp_std::vec::Vec;

sp_api::decl_runtime_apis! {
	pub trait EthWalletApi {
		/// Get CENNZnet nonce for an Ethereum address
		fn address_nonce(address: &EthAddress) -> u32;
		/// eth_call shim for erc20 / generic asset compatibility
		fn erc20_call(_asset_id: AssetId, _calldata: &Vec<u8>) -> Vec<u8>;
		/// get native CENNZ balance for an ethereum address
		fn get_balance(address: &EthAddress) -> U256;
	}
}
