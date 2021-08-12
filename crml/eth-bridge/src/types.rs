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
pub use crml_support::{H160, H256, U256};
use ethereum_types::{Bloom as H2048, U64};
use serde::de::{Error, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use sp_runtime::RuntimeDebug;
use sp_std::{prelude::*, vec::Vec};

/// Possible outcomes from attempting to verify an Ethereum event claim
#[derive(Decode, Encode, Debug, PartialEq, Clone)]
pub enum EventClaimResult {
	/// It's valid
	Valid,
	/// Couldn't request data from the Eth client
	DataProviderErr,
	/// The eth tx is marked failed
	TxStatusFailed,
	/// The transaction recipient was not the expected contract
	UnexpectedContractAddress,
	/// The expected tx logs were not present
	NoTxLogs,
	/// Not enough block confirmations yet
	NotEnoughConfirmations,
	/// Tx event logs indicated this claim does not match the event
	UnexpectedData,
	/// The deposit tx is past the expiration deadline
	Expired,
}

/// An independent notarization vote on a claim
/// This is signed and shared with the runtime after verification by a particular validator
#[derive(Encode, Decode, Clone, PartialEq, RuntimeDebug)]
pub struct NotarizationPayload {
	/// The message Id being notarized
	pub event_claim_id: EventClaimId,
	/// The ordinal index of the signer in the notary set
	/// It may be used with chain storage to lookup the public key of the notary
	pub authority_index: u16,
	/// Result of the notarization check by this authority
	pub result: EventClaimResult,
}

/// Config for handling Ethereum messages
/// Ethereum messages are simply events deposited at known contract addresses
#[derive(Encode, Decode, Default, Debug, PartialEq, Eq, Clone)]
pub struct HandlerConfig {
	/// The EVM event signature of the relevant event
	pub event_signature: [u8; 32],
	/// The deployed Ethereum contract address that deposits the event
	pub contract_address: [u8; 20],
	/// The CENNZnet function to invoke with the message result
	/// It should accept a type `T`
	/// Encoded call type `T::Call`
	pub encoded_callback: Vec<u8>,
}

/// A type of Ethereum message supported by the bridge
#[derive(Encode, Decode, PartialEq, Clone)]
pub enum MessageType {
	/// A message for an ERC-20 deposit claim
	Erc20Deposit,
}

type Index = U64;
/// The ethereum block number data type
pub type EthBlockNumber = U64;
/// The ethereum address data type
pub type EthAddress = H160;
/// The ethereum transaction hash type
pub type EthHash = H256;

/// Log
#[derive(Debug, Deserialize, PartialEq, Eq, Hash, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Log {
	/// address
	pub address: EthAddress,
	/// Topics
	pub topics: Vec<H256>,
	/// Data
	#[serde(deserialize_with = "deserialize_hex")]
	pub data: Vec<u8>,
	/// Block Hash
	pub block_hash: H256,
	/// Block Number
	pub block_number: EthBlockNumber,
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
	pub block_number: EthBlockNumber,
	/// Contract address created, or `None` if not a deployment.
	pub contract_address: Option<EthAddress>,
	/// Cumulative gas used within the block after this was executed.
	pub cumulative_gas_used: U256,
	pub effective_gas_price: Option<U256>,
	/// Address of the sender.
	pub from: EthAddress,
	/// Gas used by this transaction alone.
	///
	/// Gas used is `None` if the the client is running in light client mode.
	pub gas_used: Option<U256>,
	/// Logs generated within this transaction.
	pub logs: Vec<Log>,
	/// Status: either 1 (success) or 0 (failure).
	pub status: Option<U64>,
	/// Address of the receiver, or `None` if a contract deployment
	pub to: Option<EthAddress>,
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

/// Standard Eth block type
///
/// NB: for the bridge we only need the `timestamp` however the only RPCs available require fetching the whole block
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct EthBlock {
	pub number: Option<U64>,
	pub hash: Option<H256>,
	pub timestamp: U256,
	// don't deserialize anything else
	#[serde(rename = "parentHash", skip_deserializing)]
	pub parent_hash: H256,
	#[serde(skip_deserializing)]
	pub nonce: Option<U64>,
	#[serde(rename = "sha3Uncles", skip_deserializing)]
	pub sha3_uncles: H256,
	#[serde(rename = "logsBloom", skip_deserializing)]
	pub logs_bloom: Option<H2048>,
	#[serde(rename = "transactionsRoot", skip_deserializing)]
	pub transactions_root: H256,
	#[serde(rename = "stateRoot", skip_deserializing)]
	pub state_root: H256,
	#[serde(rename = "receiptsRoot", skip_deserializing)]
	pub receipts_root: H256,
	#[serde(skip_deserializing)]
	pub miner: EthAddress,
	#[serde(skip_deserializing)]
	pub difficulty: U256,
	#[serde(rename = "totalDifficulty", skip_deserializing)]
	pub total_difficulty: U256,
	#[serde(rename = "extraData", skip_deserializing)]
	pub extra_data: Vec<u8>,
	#[serde(skip_deserializing)]
	pub size: U256,
	#[serde(rename = "gasLimit", skip_deserializing)]
	pub gas_limit: U256,
	#[serde(rename = "gasUsed", skip_deserializing)]
	pub gas_used: U256,
	#[serde(skip_deserializing)]
	pub transactions: Vec<H256>,
	#[serde(skip_deserializing)]
	pub uncles: Vec<H256>,
}

#[derive(Debug, Default, Clone, Eq, PartialEq, Deserialize)]
pub struct EthResponse<'a, D> {
	jsonrpc: &'a str,
	id: u32,
	pub result: Option<D>,
}

/// JSON-RPC protocol version header
const JSONRPC: &str = "2.0";

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
const METHOD_TX: &str = "eth_getTransactionReceipt";
impl GetTxReceiptRequest {
	pub fn new(tx_hash: H256, id: usize) -> Self {
		Self {
			json_rpc: JSONRPC,
			method: METHOD_TX,
			params: [tx_hash],
			id,
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
const METHOD_BLOCK: &str = "eth_blockNumber";
impl GetBlockNumberRequest {
	pub fn new(id: usize) -> Self {
		Self {
			json_rpc: JSONRPC,
			method: METHOD_BLOCK,
			params: Default::default(),
			id,
		}
	}
}

const METHOD_GET_BLOCK_BY_NUMBER: &str = "eth_getBlockByNumber";
/// Request for 'eth_blockNumber'
#[derive(Serialize, Debug)]
pub struct GetBlockByNumberRequest {
	#[serde(rename = "jsonrpc")]
	/// The version of the JSON RPC spec
	pub json_rpc: &'static str,
	/// The method which is called
	pub method: &'static str,
	/// Arguments supplied to the method. Can be an empty Vec.
	pub params: Vec<u32>,
	/// The id for the request
	pub id: usize,
}

/// JSON-RPC method name for the request
impl GetBlockByNumberRequest {
	pub fn new(id: usize, block_number: u32) -> Self {
		Self {
			json_rpc: JSONRPC,
			method: METHOD_GET_BLOCK_BY_NUMBER,
			params: vec![block_number],
			id,
		}
	}
}

// Serde deserialize hex string, expects prefix '0x'
pub fn deserialize_hex<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
	deserializer.deserialize_str(BytesVisitor)
}

/// Deserializes "0x" prefixed hex strings into Vec<u8>s
struct BytesVisitor;
impl<'a> Visitor<'a> for BytesVisitor {
	type Value = Vec<u8>;

	fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		write!(formatter, "a 0x-prefixed, hex-encoded vector of bytes")
	}

	fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
	where
		E: Error,
	{
		if value.len() >= 2 && value.starts_with("0x") && value.len() & 1 == 0 {
			Ok(decode_hex(&value[2..]).expect("it is hex"))
		} else {
			Err(Error::custom(
				"Invalid bytes format. Expected a 0x-prefixed hex string with even length",
			))
		}
	}
}

// decode a non-0x prefixed hex string into a `Vec<u8>`
fn decode_hex(s: &str) -> Result<Vec<u8>, core::num::ParseIntError> {
	(0..s.len())
		.step_by(2)
		.map(|i| u8::from_str_radix(&s[i..i + 2], 16))
		.collect()
}

#[derive(Debug, Default, Clone, PartialEq, Decode, Encode)]
/// Info required to claim an Ethereum event happened
pub struct EventClaim {
	/// The ethereum transaction hash
	pub tx_hash: EthHash,
	/// The event data as logged on Ethereum
	pub data: Vec<u8>,
	/// Ethereum contract address
	pub contract_address: EthAddress,
	/// The contract event signature
	pub event_signature: H256,
}

/// A bridge message id
pub type EventClaimId = u64;
/// A bridge event type id
pub type EventTypeId = u32;

#[cfg(test)]
mod tests {
	use super::*;
	use std::str::FromStr;

	#[test]
	fn serialize_eth_block_number_request() {
		let result = serde_json::to_string(&GetBlockNumberRequest::new(1)).unwrap();
		assert_eq!(
			result,
			r#"{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}"#
		)
	}

	#[test]
	fn serialize_eth_tx_receipt_request() {
		let result = serde_json::to_string(&GetTxReceiptRequest::new(
			EthHash::from_str("0x185e85beb3296c7339954811cc682e3f992573ad3eecd37409e0ed763448d303").unwrap(),
			1,
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

		let _result: EthResponse<EthBlockNumber> = serde_json::from_str(response).expect("it deserializes");
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

		let _result: EthResponse<TransactionReceipt> = serde_json::from_str(response).expect("it deserializes");
	}

	#[test]
	fn deserialize_log_data() {
		let data = decode_hex("000000000000000000000000e7f1725e7734ce288f8367e1bb143e90bb3f0512000000000000000000000000000000000000000000000000000000000000007bacd6118e217e552ba801f7aa8a934ea6a300a5b394e7c3f42cd9d6dd9a457c10").expect("it's valid hex");
		assert!(EthDepositEvent::decode(&data).is_some());
	}

	#[test]
	fn deserialize_null_response_as_none() {
		assert_eq!(
			serde_json::from_str::<EthResponse<EthBlockNumber>>(r#"{"jsonrpc":"2.0","id":1,"result":null}"#).unwrap(),
			EthResponse {
				id: 1,
				jsonrpc: "2.0",
				result: None,
			}
		);
	}
}
