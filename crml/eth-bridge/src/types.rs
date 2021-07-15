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

use codec::{Decode, Encode};
use core::fmt;
use ethereum_types::{Address, Bloom as H2048, H256, U256, U64};
use serde::de::{Error, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use sp_std::{prelude::*, vec::Vec};

type Index = U64;
/// The ethereum block number data type
pub type EthBlockNumber = U64;
/// The ethereum address data type
pub type EthAddress = Address;
/// The ethereum transaction hash type
pub type EthTxHash = H256;

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

#[derive(Serialize, Debug)]
pub struct EthCallParams {
	/// The receiving contract
	to: EthAddress,
	/// The data to receive
	data: Vec<u8>,
}

/// Request for 'eth_call'
#[derive(Serialize, Debug)]
pub struct EthCallRequest {
	#[serde(rename = "jsonrpc")]
	/// The version of the JSON RPC spec
	pub json_rpc: &'static str,
	/// The method which is called
	pub method: &'static str,
	/// Arguments supplied to the method. Can be an empty Vec.
	pub params: EthCallParams,
	/// The id for the request
	pub id: usize,
}

/// JSON-RPC method name for the request
const METHOD_ETH_CALL: &'static str = "eth_call";
impl EthCallRequest {
	/// Eth call to `address` with function `decimals()`
	pub fn erc20_decimals(address: EthAddress) -> Self {
		Self {
			json_rpc: JSONRPC,
			method: METHOD_ETH_CALL,
			params: EthCallParams {
				to: address,
				// EVM selector code: 313ce567 (decimals)
				data: vec![0x31, 0x3c, 0xe5, 0x67],
			},
			id: 1,
		}
	}
	/// Eth call to `address` with function `symbol()`
	pub fn erc20_symbol(address: EthAddress) -> Self {
		Self {
			json_rpc: JSONRPC,
			method: METHOD_ETH_CALL,
			params: EthCallParams {
				to: address,
				// EVM selector code: 95d89b41 (symbol)
				data: vec![0x95, 0xd8, 0x9b, 0x41],
			},
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
			Ok(Bytes::new(decode_hex(&value[2..]).expect("it is hex")))
		} else {
			Err(Error::custom(
				"Invalid bytes format. Expected a 0x-prefixed hex string with even length",
			))
		}
	}
}

/// A bridge claim id
pub type ClaimId = u64;

/// A deposit event made by the CENNZnet bridge contract on Ethereum
#[derive(Debug, Clone, PartialEq, Decode, Encode)]
pub struct EthDepositEvent {
	/// The ERC20 token address / type deposited
	pub token_type: Address,
	/// The amount (in 'wei') of the deposit
	pub amount: U256,
	/// The CENNZnet beneficiary address
	pub beneficiary: H256,
	/// The reported timestamp of the claim
	pub timestamp: U256,
}

impl EthDepositEvent {
	/// Take some bytes e.g. Ethereum log data and attempt to
	/// decode a deposit event
	/// the deposit event matches the one emitted by the Solidity bridge contract ('Deposit')
	/// Returns `None` on failure
	pub fn try_decode_from_log(log: &Log) -> Option<Self> {
		let data = &log.data.0;

		// we're expecting 4 fields in the log.data represented as a single &[u8] of
		// concatenated `bytes32` / `U256`
		// 32 * 4
		if data.len() != 128 {
			return None;
		}
		// Eth addresses are 20 bytes, the first 12 bytes are empty when encoded to log.data as a U256
		let token_type = Address::from_slice(&data[12..32]);
		let amount = U256::from(&data[32..64]);
		let beneficiary = H256::from_slice(&data[64..96]);
		let timestamp = U256::from(&data[96..]);

		Some(Self {
			token_type,
			amount,
			beneficiary,
			timestamp,
		})
	}
}

#[cfg(test)]
mod tests2 {
	use crate::{
		decode_hex,
		types::{EthBlockNumber, EthResponse, GetBlockNumberRequest, GetTxReceiptRequest, TransactionReceipt},
	};
	use ethereum_types::{Address, H256, U256};
	use std::str::FromStr;

	#[test]
	fn serialize_eth_block_number_request() {
		let result =
			serde_json_core::to_string::<serde_json_core::consts::U512, _>(&GetBlockNumberRequest::new()).unwrap();
		assert_eq!(
			result,
			r#"{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}"#
		)
	}

	#[test]
	fn serialize_eth_tx_receipt_request() {
		let result = serde_json_core::to_string::<serde_json_core::consts::U512, _>(&GetTxReceiptRequest::new(
			H256::from_str("0x185e85beb3296c7339954811cc682e3f992573ad3eecd37409e0ed763448d303").unwrap(),
		))
		.unwrap();
		assert_eq!(
			result,
			r#"{"jsonrpc":"2.0","method":"eth_getTransactionReceipt","params":["0x185e85beb3296c7339954811cc682e3f992573ad3eecd37409e0ed763448d303"],"id":1}"#
		)
	}

	#[test]
	fn deserialize_eth_block_number() {
		let response = r#"
		{
			"jsonrpc":"2.0",
			"id":1,
  			"result": "0x65a8db"
		}
		"#;

		let _result: EthResponse<EthBlockNumber> = serde_json_core::from_str(response).expect("it deserializes");
	}

	#[test]
	fn deserialize_eth_transaction_receipt() {
		let response = r#"
			{
				"jsonrpc":"2.0",
				"id":1,
				"result":{
					"blockHash":"0xa97fa85e0f38526be39a29eb77c07ad9f18c315f8eb6ab7d44028581c1518ec1",
					"blockNumber":"0x5",
					"contractAddress":null,
					"cumulativeGasUsed":"0x1685c",
					"effectiveGasPrice":"0x30cb962f",
					"from":"0xec2c80a819ee8e42c624f6a5de930e8184c0801f",
					"gasUsed":"0x1685c",
					"logs":[
						{"address":"0x17c54edee4d6bccf2379daa328dcc0fbd9c6ce2b",
						"topics":["0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef","0x000000000000000000000000ec2c80a819ee8e42c624f6a5de930e8184c0801f","0x00000000000000000000000087015d61b82a3808d9720a79573bf75deb8a1e90"],
						"data":"0x000000000000000000000000000000000000000000000000000000000000007b","blockNumber":"0x5","transactionHash":"0x185e85beb3296c7339954811cc682e3f992573ad3eecd37409e0ed763448d303","transactionIndex":"0x0",
						"blockHash":"0xa97fa85e0f38526be39a29eb77c07ad9f18c315f8eb6ab7d44028581c1518ec1","logIndex":"0x0","removed":false},{"address":"0x17c54edee4d6bccf2379daa328dcc0fbd9c6ce2b",
						"topics":["0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925","0x000000000000000000000000ec2c80a819ee8e42c624f6a5de930e8184c0801f","0x00000000000000000000000087015d61b82a3808d9720a79573bf75deb8a1e90"],
						"data":"0x000000000000000000000000000000000000000000000000000000000001e1c5",
						"blockNumber":"0x5",
						"transactionHash":"0x185e85beb3296c7339954811cc682e3f992573ad3eecd37409e0ed763448d303",
						"transactionIndex":"0x0",
						"blockHash":"0xa97fa85e0f38526be39a29eb77c07ad9f18c315f8eb6ab7d44028581c1518ec1",
						"logIndex":"0x1",
						"removed":false},
						{"address":"0x87015d61b82a3808d9720a79573bf75deb8a1e90",
						"topics":["0x76bb911c362d5b1feb3058bc7dc9354703e4b6eb9c61cc845f73da880cf62f61","0x000000000000000000000000ec2c80a819ee8e42c624f6a5de930e8184c0801f"],
						"data":"0x00000000000000000000000017c54edee4d6bccf2379daa328dcc0fbd9c6ce2b000000000000000000000000000000000000000000000000000000000000007bacd6118e217e552ba801f7aa8a934ea6a300a5b394e7c3f42cd9d6dd9a457c10","blockNumber":"0x5","transactionHash":"0x185e85beb3296c7339954811cc682e3f992573ad3eecd37409e0ed763448d303","transactionIndex":"0x0","blockHash":"0xa97fa85e0f38526be39a29eb77c07ad9f18c315f8eb6ab7d44028581c1518ec1","logIndex":"0x2","removed":false}],
						"logsBloom":"0x00000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000010000000200200000000000000000000000008000000000000000000000000000000000000000000000000000000001000000000000000000000000010000000000010000000800000000000000000000000000002000000000000000000000000000040000000020000000000000000000010000000000000000000000000000000000000000000000002000000000000000000000000200000000000008000000004000000000010001000000000000000020000000000000000000000000000001000000000",
						"status":"0x1",
						"to":"0x87015d61b82a3808d9720a79573bf75deb8a1e90",
						"transactionHash":"0x185e85beb3296c7339954811cc682e3f992573ad3eecd37409e0ed763448d303",
						"transactionIndex":"0x0",
						"type":"0x0"
				}
			}
		"#;

		let _result: EthResponse<TransactionReceipt> = serde_json_core::from_str(response).expect("it deserializes");
	}

	#[test]
	fn deserialize_log_data() {
		// 00000000000000000000000017c54edee4d6bccf2379daa328dcc0fbd9c6ce2b // bytes32
		// 000000000000000000000000000000000000000000000000000000000000007b // bytes32
		// bytes32

		let buf = decode_hex("00000000000000000000000017c54edee4d6bccf2379daa328dcc0fbd9c6ce2b000000000000000000000000000000000000000000000000000000000000007bacd6118e217e552ba801f7aa8a934ea6a300a5b394e7c3f42cd9d6dd9a457c10").expect("it's valid hex");
		let token_address = Address::from_slice(&buf[12..32]);
		let amount = U256::from(&buf[32..64]);
		let cennznet_address = H256::from_slice(&buf[64..96]);
		let timestamp = U256::from_slice(&buf[96..]);
		println!(
			"{:?} {:?} {:?} {:?}",
			token_address, amount, cennznet_address, timestamp
		);

		assert!(false);
	}
}
