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
use ethereum_types::{Bloom, U64};
use rustc_hex::ToHex;
use scale_info::TypeInfo;
use serde::de::{Error, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sp_runtime::RuntimeDebug;
use sp_std::{prelude::*, vec::Vec};
// following imports support serializing values to hex strings in no_std
#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::borrow::ToOwned;
#[cfg(not(feature = "std"))]
use alloc::string::String;
#[cfg(feature = "std")]
use std::string::String;

/// An EthCallOracle call Id
pub type EthCallId = u64;
/// An EthCallOracle request
#[derive(Encode, Decode, PartialEq, Clone, TypeInfo)]
pub struct EthCallRequest {
	pub timestamp: u64,
	pub target: EthAddress,
	pub input: Vec<u8>,
}
#[derive(Encode, Decode, PartialEq, Clone, TypeInfo)]
pub enum EthCallResponse {
	Ok([u8; 32]),
	ExceedsLengthLimit,
	DataProviderErr,
}
/// A bridge message id
pub type EventClaimId = u64;
/// A bridge event type id
pub type EventTypeId = u32;
/// A bridge proof id
pub type EventProofId = u64;
type Index = U64;
/// The ethereum block number data type
pub type EthBlockNumber = U64;
/// The ethereum address data type
pub type EthAddress = H160;
/// The ethereum transaction hash type
pub type EthHash = H256;

#[derive(Debug, Default, Clone, PartialEq, Decode, Encode, TypeInfo)]
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

#[derive(Debug, Clone, PartialEq, TypeInfo)]
/// Error type for BridgeEthereumRpcApi
pub enum BridgeRpcError {
	/// HTTP network request failed
	HttpFetch,
	/// Unable to decode response payload as JSON
	InvalidJSON,
	/// offchain worker not configured properly
	OcwConfig,
}

/// Provides request/responses according to a minimal subset of Ethereum RPC API
/// required for the bridge
pub trait BridgeEthereumRpcApi {
	/// Returns an ethereum block given a block height
	fn get_block_by_number(block_number: LatestOrNumber) -> Result<Option<EthBlock>, BridgeRpcError>;
	/// Returns an ethereum transaction receipt given a tx hash
	fn get_transaction_receipt(hash: EthHash) -> Result<Option<TransactionReceipt>, BridgeRpcError>;
	/// Performs an `eth_call` request
	/// Returns the Ethereum abi encoded returndata as a Vec<u8>
	fn eth_call(target: EthAddress, input: &[u8], at_block: LatestOrNumber) -> Result<Vec<u8>, BridgeRpcError>;
}

/// Possible outcomes from attempting to verify an Ethereum event claim
#[derive(Decode, Encode, Debug, PartialEq, Clone, TypeInfo)]
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
#[derive(Encode, Decode, Clone, PartialEq, RuntimeDebug, TypeInfo)]
pub struct NotarizationPayload {
	/// The message Id being notarized
	pub event_claim_id: EventClaimId,
	/// The ordinal index of the signer in the notary set
	/// It may be used with chain storage to lookup the public key of the notary
	pub authority_index: u16,
	/// Result of the notarization check by this authority
	pub result: EventClaimResult,
}

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
	pub logs_bloom: Bloom,
	/// Transaction type, Some(1) for AccessList transaction, None for Legacy
	#[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
	pub transaction_type: Option<U64>,
	#[serde(default)]
	pub removed: bool,
}

/// Standard Eth block type
///
/// NB: for the bridge we only need the `timestamp` however the only RPCs available require fetching the whole block
#[derive(Clone, Debug, PartialEq, Deserialize, Default)]
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
	pub logs_bloom: Option<Bloom>,
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

#[derive(Debug, Default, Clone, Eq, PartialEq, Deserialize, TypeInfo)]
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

const METHOD_GET_BLOCK_BY_NUMBER: &str = "eth_getBlockByNumber";
/// Request for 'eth_blockNumber'
#[derive(Serialize, Debug)]
pub struct GetBlockRequest {
	#[serde(rename = "jsonrpc")]
	/// The version of the JSON RPC spec
	pub json_rpc: &'static str,
	/// The method which is called
	pub method: &'static str,
	/// Arguments supplied to the method. (blockNumber, fullTxData?)
	#[serde(serialize_with = "serialize_params")]
	pub params: (LatestOrNumber, bool),
	/// The id for the request
	pub id: usize,
}

#[derive(Debug)]
pub enum LatestOrNumber {
	Latest,
	Number(u32),
}

const METHOD_ETH_CALL: &str = "eth_call";
/// Request for 'eth_call'
#[derive(Serialize, Debug)]
pub struct EthCallRpcRequest {
	#[serde(rename = "jsonrpc")]
	/// The version of the JSON RPC spec
	pub json_rpc: &'static str,
	/// The method which is called
	pub method: &'static str,
	/// Arguments supplied to the method. (blockNumber, fullTxData?)
	#[serde(serialize_with = "serialize_params_eth_call")]
	pub params: (EthCallRpcParams, LatestOrNumber),
	/// The id for the request
	pub id: usize,
}
#[derive(Serialize, Debug)]
pub struct EthCallRpcParams {
	/// The contract to call
	pub to: EthAddress,
	/// The input buffer to pass to `to`
	pub data: Bytes,
}
impl EthCallRpcRequest {
	pub fn new(target: EthAddress, input: &[u8], id: usize, block: LatestOrNumber) -> Self {
		Self {
			json_rpc: JSONRPC,
			method: METHOD_ETH_CALL,
			params: (
				EthCallRpcParams {
					to: target,
					data: Bytes::new(input.to_vec()),
				},
				block,
			),
			id,
		}
	}
}

/// Serializes the parameters for `GetBlockRequest`
pub fn serialize_params<S: serde::Serializer>(v: &(LatestOrNumber, bool), s: S) -> Result<S::Ok, S::Error> {
	use core::fmt::Write;
	use serde::ser::SerializeTuple;

	let mut tup = s.serialize_tuple(2)?;
	match v.0 {
		LatestOrNumber::Latest => tup.serialize_element(&"latest")?,
		LatestOrNumber::Number(n) => {
			// Ethereum JSON RPC API expects the block number as a hex string
			let mut hex_block_number = sp_std::Writer::default();
			write!(&mut hex_block_number, "{:#x}", n).expect("valid bytes");
			// this should always be valid utf8
			tup.serialize_element(&core::str::from_utf8(hex_block_number.inner()).expect("valid bytes"))?;
		}
	}
	tup.serialize_element(&v.1)?;
	tup.end()
}

/// Serializes the parameters for `EthCallRequest`
pub fn serialize_params_eth_call<S: serde::Serializer>(
	v: &(EthCallRpcParams, LatestOrNumber),
	s: S,
) -> Result<S::Ok, S::Error> {
	use core::fmt::Write;
	use serde::ser::SerializeTuple;

	let mut tup = s.serialize_tuple(2)?;
	tup.serialize_element(&v.0)?;
	match v.1 {
		LatestOrNumber::Latest => tup.serialize_element(&"latest")?,
		LatestOrNumber::Number(n) => {
			// Ethereum JSON RPC API expects the block number as a hex string
			let mut hex_block_number = sp_std::Writer::default();
			write!(&mut hex_block_number, "{:#x}", n).expect("valid bytes");
			// this should always be valid utf8
			tup.serialize_element(&core::str::from_utf8(hex_block_number.inner()).expect("valid bytes"))?;
		}
	}
	tup.end()
}

/// JSON-RPC method name for the request
impl GetBlockRequest {
	pub fn for_number(id: usize, block_number: u32) -> Self {
		Self {
			json_rpc: JSONRPC,
			method: METHOD_GET_BLOCK_BY_NUMBER,
			params: (LatestOrNumber::Number(block_number), false), // `false` = return tx hashes not full tx data
			id,
		}
	}
	pub fn latest(id: usize) -> Self {
		Self {
			json_rpc: JSONRPC,
			method: METHOD_GET_BLOCK_BY_NUMBER,
			params: (LatestOrNumber::Latest, false), // `false` = return tx hashes not full tx data
			id,
		}
	}
}

// Serde deserialize hex string, expects prefix '0x'
pub fn deserialize_hex<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
	deserializer.deserialize_str(BytesVisitor)
}

/// Deserializes "0x" prefixed hex strings into Vec<u8>s
pub(crate) struct BytesVisitor;
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

/// Wrapper structure around vector of bytes.
#[derive(Debug, PartialEq, Eq, Default, Hash, Clone)]
pub struct Bytes(pub Vec<u8>);

impl Bytes {
	/// Simple constructor.
	pub fn new(bytes: Vec<u8>) -> Bytes {
		Bytes(bytes)
	}
}

impl Serialize for Bytes {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		let mut serialized = "0x".to_owned();
		serialized.push_str(self.0.to_hex::<String>().as_ref());
		serializer.serialize_str(serialized.as_ref())
	}
}

impl<'a> Deserialize<'a> for Bytes {
	fn deserialize<D>(deserializer: D) -> Result<Bytes, D::Error>
	where
		D: Deserializer<'a>,
	{
		deserializer.deserialize_any(BytesVisitor).map(|x| Bytes::new(x))
	}
}

// decode a non-0x prefixed hex string into a `Vec<u8>`
fn decode_hex(s: &str) -> Result<Vec<u8>, core::num::ParseIntError> {
	(0..s.len())
		.step_by(2)
		.map(|i| u8::from_str_radix(&s[i..i + 2], 16))
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::str::FromStr;

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
	fn deserialize_eth_block_request() {
		let response = r#"
			{
				"jsonrpc": "2.0",
				"id": 1,
				"result": {
				  "difficulty": "0xbfabcdbd93dda",
				  "extraData": "0x737061726b706f6f6c2d636e2d6e6f64652d3132",
				  "gasLimit": "0x79f39e",
				  "gasUsed": "0x79ccd3",
				  "hash": "0xb3b20624f8f0f86eb50dd04688409e5cea4bd02d700bf6e79e9384d47d6a5a35",
				  "logsBloom": "0x4848112002a2020aaa0812180045840210020005281600c80104264300080008000491220144461026015300100000128005018401002090a824a4150015410020140400d808440106689b29d0280b1005200007480ca950b15b010908814e01911000054202a020b05880b914642a0000300003010044044082075290283516be82504082003008c4d8d14462a8800c2990c88002a030140180036c220205201860402001014040180002006860810ec0a1100a14144148408118608200060461821802c081000042d0810104a8004510020211c088200420822a082040e10104c00d010064004c122692020c408a1aa2348020445403814002c800888208b1",
				  "miner": "0x5a0b54d5dc17e0aadc383d2db43b0a0d3e029c4c",
				  "mixHash": "0x3d1fdd16f15aeab72e7db1013b9f034ee33641d92f71c0736beab4e67d34c7a7",
				  "nonce": "0x4db7a1c01d8a8072",
				  "number": "0x5bad55",
				  "parentHash": "0x61a8ad530a8a43e3583f8ec163f773ad370329b2375d66433eb82f005e1d6202",
				  "receiptsRoot": "0x5eced534b3d84d3d732ddbc714f5fd51d98a941b28182b6efe6df3a0fe90004b",
				  "sha3Uncles": "0x8a562e7634774d3e3a36698ac4915e37fc84a2cd0044cb84fa5d80263d2af4f6",
				  "size": "0x41c7",
				  "stateRoot": "0xf5208fffa2ba5a3f3a2f64ebd5ca3d098978bedd75f335f56b705d8715ee2305",
				  "timestamp": "0x5b541449",
				  "totalDifficulty": "0x12ac11391a2f3872fcd",
				  "transactions": [
					"0x8784d99762bccd03b2086eabccee0d77f14d05463281e121a62abfebcf0d2d5f",
					"0x311be6a9b58748717ac0f70eb801d29973661aaf1365960d159e4ec4f4aa2d7f",
					"0xe42b0256058b7cad8a14b136a0364acda0b4c36f5b02dea7e69bfd82cef252a2",
					"0x4eb05376055c6456ed883fc843bc43df1dcf739c321ba431d518aecd7f98ca11",
					"0x994dd9e72b212b7dc5fd0466ab75adf7d391cf4f206a65b7ad2a1fd032bb06d7",
					"0xf6feecbb9ab0ac58591a4bc287059b1133089c499517e91a274e6a1f5e7dce53",
					"0x7e537d687a5525259480440c6ea2e1a8469cd98906eaff8597f3d2a44422ff97",
					"0xa762220e92bed6d77a2c19ffc60dad77d71bd5028c5230c896ab4b9552a39b50",
					"0xf1fa677edda7e5add8e794732c7554cd5459a5c12781dc71de73c7937dfb2775",
					"0x3220af8e317fde6dac80b1199f9ceeafe60ada4974a7e04a75fbce1ac4cb46c3",
					"0x5566528978250828168f0d30bcc8a3689d129c75d820d604f7eb84c25b34ec81",
					"0x646c98e323a05862778f0c9063a989b6aefd94f28842a3a09d2edb37a050717d",
					"0xe951ea55764f7e8e0720f7042dd1db67525965302ed974a0c8e3b769bc1818e3",
					"0x7ecf2528b7df3831712501f5c60ef156bf5fcac9912199e0a64afcb963ea91ca",
					"0xc43b89783f68b2844918ea515cc146c006e5f162c9be9aedf5e7a6ae1f32e164",
					"0xd74503ede63d6fd41367796433aa14439902e8f57293a0583e19aa6ebf3f128e",
					"0x021e5b7d3ddac97b4c6cb9c3f333766a533c1ed9fbcfb8b2515c38ecd0c53f89",
					"0xbb3a336e3f823ec18197f1e13ee875700f08f03e2cab75f0d0b118dabb44cba0",
					"0x25f65866dba34783200c25fb1c120b36326c9ad3a47e8bc34c3edbc9208f1378",
					"0x5336f5c4132ef00e8b469ecfd4ee0d6800f6bd60aefb1c62232cbce81c085ae2",
					"0xb87410cfe0a75c004f7637736b3de1e8f4e08e9e2b05ab963622a40a5505664d",
					"0x990857a27ec7cfd6dfd88015173adf81959b5abaff6eefbe8e9df6b0f40f2711",
					"0x3563ccb5734b7b5015122a20b558723afe992ff1109a04b57e02f26edd5a6a38",
					"0xd7885d9412cc494fbe680b016bf7402b633c34c66833b35cad59af2a4aff4f0b",
					"0x48e60927d6fb9ae76f69a6400490b5ffcb2f9da3105fad6c61f21256ef0c217c",
					"0x9e30af26ff3836c4b55af62ba134bc55db662cf1d396cca437d12a8195bfcbe4",
					"0x2476eeede4764c6871f50f3235ebeb9a56d33b41bc3bb1ce3c18c5d710a0609c",
					"0x1cd3520fbb1eb6f2f6f257ab7c3cba957806b0b87182baedb4f81c62868064c1",
					"0x78ae3aee0ff16d8ea4f394b7b80021804e1d9f35cdbb9c6189bb6cbf58bc52c4",
					"0xfcc75bad728b8d302ba0674ebe3122fc50e3b78fe4948ddfc0d37ee987e666ca",
					"0xd2175464d72bcc61b2e07aa3aac742b4184480d7a9f6ae5c2ba24d9c9bb9f304",
					"0x42b56b504e59e42a3dc94e740bb4231e6326daaac7a73ef93ee8db7b96ac5d71",
					"0xd42681091641cd2a71f18299e8e206d5659c3076b1c63adc26f5b7740e230d2b",
					"0x1202c354f0a00b31adf9e3d895e0c8f3896182bb3ab9fc69d6c21d31a1bf279c",
					"0xa5cea1f6957431caf589a8dbb58c102fb191b39967fbe8d26cecf6f28bb835da",
					"0x2045efeb2f5ea9176690ece680d3fd7ca9e945d0d572d17786810d323628f98c",
					"0xbf55d13976616a23114b724b14049eaaf91db3f1950320b5306006a6b648b24f",
					"0x9e5c5ea885eb1d6b1b3ffcf703e3381b7681f7420f35408d30ba93ec0cdf0792",
					"0x6f1a61dc4306ca5e976a1706afe1f32279548df98e0373c5fee0ea189ddb77a0",
					"0xc5c16b30c22ee4f90c3a2de70554f7975eb476592ff13c61986d760da6cf7f9d",
					"0xb09de28497227c0537df0a78797fa00407dcd04a4f90d9de602484b61f7bf169",
					"0x1bfea966fa7772a26b4b2c8add15ceedcb70a903618f5d4603d69f52b9954025",
					"0xe58be9c0e3cedd4444c76d1adc098ba40cbe21ef886b2bfc2edb6ed96ba8d966",
					"0x3a29096f712ccdafd56e9a3c635d4fe2e6224ac3666d466c21da66c8829bbfd6",
					"0x31feab77d7c1c87eb79af54193400c8edad16645e1ea5fcc10f2eaec51fe3992",
					"0x4e0278fce62dca8e23cfae6a020fcd3b2facc03244d54b964bbde424f902ffe1",
					"0x300239a64a50ad0e646c232f85cfa4f3d3ed30090cd574329c782d95c2b42532",
					"0x41755f354b06b4b8a452db1cc9b5c810c75b1bbe236603cbc0950c3c81b80c51",
					"0x1e3fbeffc326f1ffd8559c6024c12557e6014bc02c12d65dbc1baa4e1aed94b7",
					"0x4a459a32cf68e9b7697a3a656432b340d6d27c3d4a513e6cce770d63df99839a",
					"0x3ef484913d185de728c787a1053ec1444ec1c7a5827eecba521d3b406b088a89",
					"0x43afa584c21f27a2747a8397b00d3ec4b460d929b61b510d017f01037a3ded3f",
					"0x44e6a37a6c1d8696fa0537385b9d1bb535b2b3309b5482209e95b5b6c58fc8da",
					"0x2a8bca48147955efcfd697f1a97304ae4cc467a7778741c2c47e516610f0a876",
					"0x4c6bd64c8974f8b949cfe265da1c1bb997e3c886f024b38c99d170acc70b83df",
					"0x103f0cca1ae13600c5be5b217e92430a72b0471d05e283c105f5d0df36438b2a",
					"0x00a06bf6fbd07b3a89ef9031a2108c8fa31b467b33a6edcd6eb3687c158743cf",
					"0x0175496d8265dedd693cf88884626c33b699ebcf4f2110e4c7fb7603c53215b2",
					"0x11fb433ab551b33f30d00a34396835fab72e316e81d1e0afcbc92e79801f30c4",
					"0x060dc4541fd534d107f6e49b96d84f5ec6dbe4eb714890e800bd02399a6bfb7f",
					"0x01956de9f96f9a268c6524fffb9919d7fa3de7a4c25d53c2ccc43d0cb022a7ff",
					"0x15057378f2d223829269ec0f31ba4bb03146134220d34eb8eb7c403aa4a2e569",
					"0x16ea0218d72b5e3f69d0ae4daa8085150f5f7e69ee22a3b054744e35e2082879",
					"0x0baf4e8ff92058c1cac3b95c237edb4d2c12ad41d210356c209f1e0bf0d2d12a",
					"0x1a8ac77aff614caeca16a5a3a0931375a5a4fbe0ef1e15d6d15bf6f8e3c60f4f",
					"0xdb899136f41a3d4710907345b09d241490776383271e6b9887499fd05b80fcd4",
					"0x1007e17b1120d37fb930f953d8a3440ca11b8fd84470eb107c8b4a402a9813fd",
					"0x0910324706ffeebf8aa25ca0784636518bf67e5d173c22438a64dd43d5f4aa2a",
					"0x028f2bee56aee7005abcb2258d6d9f0f078a85a65c3d669aca40564ef4bd7f94",
					"0x14adac9bc94cde3166f4b7d42e8862a745483c708e51afbe89ecd6532acc532e",
					"0x54bed12ccad43523ba8527d1b99f5fa04a55b3a7724cfff2e0a21ec90b08590e",
					"0xcdf05df923f6e418505750069d6486276b15fcc3cd2f42a7044c642d19a86d51",
					"0x0c66977ed87db75074cb2bea66b254af3b20bb3315e8095290ceb1260b1b7449",
					"0x22626e2678da34b505b233ef08fc91ea79c5006dff00e33a442fa51a11e34c25",
					"0xe2989560000a1fc7c434c5e9c4bba82e1501bf435292ac25acc3cb182c1c2cd0",
					"0x348cfc85c58b7f3b2e8bdaa517dc8e3c5f8fb41e3ba235f28892b46bc3484756",
					"0x4ac009cebc1f2416b9e39bcc5b41cd53b1a9239e8f6c0ab043b8830ef1ffc563",
					"0xf2a96682362b9ffe9a77190bcbc47937743b6e1da2c56257f9b562f15bbd3cfa",
					"0xf1cd627c97746bc75727c2f0efa2d0dc66cca1b36d8e45d897e18a9b19af2f60",
					"0x241d89f7888fbcfadfd415ee967882fec6fdd67c07ca8a00f2ca4c910a84c7dd"
				  ],
				  "transactionsRoot": "0xf98631e290e88f58a46b7032f025969039aa9b5696498efc76baf436fa69b262",
				  "uncles": [
					"0x824cce7c7c2ec6874b9fa9a9a898eb5f27cbaf3991dfa81084c3af60d1db618c"
				  ]
				}
		}
		"#;

		let _result: EthResponse<EthBlock> = serde_json::from_str(response).expect("it deserializes");
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

	#[test]
	fn serialize_get_block_by_number_request() {
		let expected = r#"{"jsonrpc":"2.0","method":"eth_getBlockByNumber","params":["0x37",false],"id":1}"#;
		let result = serde_json::to_string(&GetBlockRequest::for_number(1, 55)).unwrap();
		assert_eq!(expected, result);

		let expected = r#"{"jsonrpc":"2.0","method":"eth_getBlockByNumber","params":["latest",false],"id":1}"#;
		let result = serde_json::to_string(&GetBlockRequest::latest(1)).unwrap();
		assert_eq!(expected, result);
	}

	#[test]
	fn serialize_get_block_by_number_request_max() {
		let expected = r#"{"jsonrpc":"2.0","method":"eth_getBlockByNumber","params":["0xfffffffe",false],"id":1}"#;
		let result = serde_json::to_string(&GetBlockRequest::for_number(1, u32::MAX - 1)).unwrap();
		assert_eq!(expected, result);
	}

	#[test]
	fn serialize_eth_call_request() {
		let expected = r#"{"jsonrpc":"2.0","method":"eth_call","params":[{"to":"0x00000000000000000000000000000000075bcd15","data":"0x0101010101"},"latest"],"id":1}"#;
		let result = serde_json::to_string(&EthCallRpcRequest::new(
			EthAddress::from_low_u64_be(123456789),
			&[1_u8; 5],
			1,
			LatestOrNumber::Latest,
		))
		.unwrap();
		assert_eq!(expected, result);

		// empty input data, max block number
		let expected = r#"{"jsonrpc":"2.0","method":"eth_call","params":[{"to":"0x00000000000000000000000000000000075bcd15","data":"0x"},"0xfffffffe"],"id":1}"#;
		let result = serde_json::to_string(&EthCallRpcRequest::new(
			EthAddress::from_low_u64_be(123456789),
			Default::default(),
			1,
			LatestOrNumber::Number(u32::MAX - 1),
		))
		.unwrap();
		assert_eq!(expected, result);
	}
}
