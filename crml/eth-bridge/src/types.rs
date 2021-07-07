/* Copyright 2021 Centrality Investments Limited
*
* Licensed under the LGPL, Version 3.0 (the "License");
* you may not use this file except in compliance with the License.
* Unless required by applicable law or agreed to in writing, software
* distributed under the License is distributed on an "AS IS" BASIS,
* WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
* See the License for the specific language governing permissions and
* limitations under the License.
* You may obtain a copy of the License at the root of this project source code,
* or at:
*     https://centrality.ai/licenses/gplv3.txt
*     https://centrality.ai/licenses/lgplv3.txt
*/

//! CENNZnet Eth Bridge Types

use core::fmt;
use ethereum_types::{Address, Bloom as H2048, H256, U256, U64};
use serde::de::{Error, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use sp_std::vec::Vec;

type Index = U64;
pub type EthBlockNumber = U64;

/// Log
#[derive(Debug, Deserialize, PartialEq, Eq, Hash, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Log {
	/// address
	pub address: Address,
	/// Topics
	pub topics: Vec<H256>,
	/// Data
	pub data: Bytes,
	/// Block Hash
	pub block_hash: H256,
	/// Block Number
	pub block_number: U64,
	/// Transaction Hash
	pub transaction_hash: Option<H256>,
	/// Transaction Index
	pub transaction_index: U64,
	/// Log Index in Block
	pub log_index: U256,
	/// Whether Log Type is Removed (Geth Compatibility Field)
	#[serde(default)]
	pub removed: bool,
}

// Copied from https://docs.rs/web3/0.14.0/src/web3/types/transaction.rs.html#40-73
// missing from/to fields
// remove after: https://github.com/tomusdrw/rust-web3/issues/513 is solved
/// "Receipt" of an executed transaction: details of its execution.
#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionReceipt {
	/// Hash of the block this transaction was included within.
	pub block_hash: H256,
	/// Number of the block this transaction was included within.
	pub block_number: U64,
	/// Contract address created, or `None` if not a deployment.
	pub contract_address: Option<Address>,
	/// Cumulative gas used within the block after this was executed.
	pub cumulative_gas_used: U256,
	pub effective_gas_price: U256,
	/// Address of the sender.
	pub from: Address,
	/// Gas used by this transaction alone.
	///
	/// Gas used is `None` if the the client is running in light client mode.
	pub gas_used: Option<U256>,
	/// Logs generated within this transaction.
	pub logs: Vec<Log>,
	/// Status: either 1 (success) or 0 (failure).
	pub status: Option<U64>,
	/// Address of the receiver, or `None` if a contract deployment
	pub to: Option<Address>,
	/// Transaction hash.
	pub transaction_hash: H256,
	/// Index within the block.
	pub transaction_index: Index,
	/// State root.
	pub root: Option<H256>,
	/// Logs bloom
	pub logs_bloom: H2048,
	/// Transaction type, Some(1) for AccessList transaction, None for Legacy
	#[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
	pub transaction_type: Option<U64>,
	#[serde(default)]
	pub removed: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
pub struct EthResponse<'a, D> {
	jsonrpc: &'a str,
	id: u32,
	pub result: D,
}

/// JSON-RPC protocol version header
const JSONRPC: &'static str = "2.0";

/// Request for 'eth_getTransactionReceipt'
#[derive(Serialize, Debug)]
pub struct GetTxReceiptRequest {
	#[serde(rename = "jsonrpc")]
	/// The version of the JSON RPC spec
	pub json_rpc: &'static str,
	/// The method which is called
	pub method: &'static str,
	/// Arguments supplied to the method. Can be an empty Vec.
	pub params: [H256; 1],
	/// The id for the request
	pub id: usize,
}

/// JSON-RPC method name for the request
const METHOD_TX: &'static str = "eth_getTransactionReceipt";
impl GetTxReceiptRequest {
	pub fn new(tx_hash: H256) -> Self {
		Self {
			json_rpc: JSONRPC,
			method: METHOD_TX,
			params: [tx_hash],
			id: 1,
		}
	}
}

/// Request for 'eth_blockNumber'
#[derive(Serialize, Debug)]
pub struct GetBlockNumberRequest {
	#[serde(rename = "jsonrpc")]
	/// The version of the JSON RPC spec
	pub json_rpc: &'static str,
	/// The method which is called
	pub method: &'static str,
	/// Arguments supplied to the method. Can be an empty Vec.
	pub params: Vec<u8>,
	/// The id for the request
	pub id: usize,
}

/// JSON-RPC method name for the request
const METHOD_BLOCK: &'static str = "eth_blockNumber";
impl GetBlockNumberRequest {
	pub fn new() -> Self {
		Self {
			json_rpc: JSONRPC,
			method: METHOD_BLOCK,
			params: Default::default(),
			id: 1,
		}
	}
}

// Serializable wrapper around vector of bytes
/// Wrapper structure around vector of bytes.
#[derive(Debug, PartialEq, Eq, Default, Hash, Clone)]
pub struct Bytes(pub Vec<u8>);

impl Bytes {
	/// Simple constructor.
	pub fn new(bytes: Vec<u8>) -> Bytes {
		Bytes(bytes)
	}
	pub fn into_vec(self) -> Vec<u8> {
		self.0
	}
}

impl From<Vec<u8>> for Bytes {
	fn from(bytes: Vec<u8>) -> Bytes {
		Bytes(bytes)
	}
}

impl Into<Vec<u8>> for Bytes {
	fn into(self) -> Vec<u8> {
		self.0
	}
}

impl<'a> Deserialize<'a> for Bytes {
	fn deserialize<D>(deserializer: D) -> Result<Bytes, D::Error>
	where
		D: Deserializer<'a>,
	{
		deserializer.deserialize_any(BytesVisitor)
	}
}

pub fn decode_hex(s: &str) -> Result<Vec<u8>, core::num::ParseIntError> {
	(0..s.len())
		.step_by(2)
		.map(|i| u8::from_str_radix(&s[i..i + 2], 16))
		.collect()
}

struct BytesVisitor;

impl<'a> Visitor<'a> for BytesVisitor {
	type Value = Bytes;

	fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		write!(formatter, "a 0x-prefixed, hex-encoded vector of bytes")
	}

	fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
	where
		E: Error,
	{
		if value.len() >= 2 && value.starts_with("0x") && value.len() & 1 == 0 {
			Ok(Bytes::new(
				decode_hex(&value[2..]).expect("it is hex")
			))
		} else {
			Err(Error::custom(
				"Invalid bytes format. Expected a 0x-prefixed hex string with even length",
			))
		}
	}
}
