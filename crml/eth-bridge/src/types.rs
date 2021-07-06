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

use serde::{Deserialize, Serialize};
use web3::types::{Address, Index, Log, H2048, H256, U256, U64};

// Copied from https://docs.rs/web3/0.14.0/src/web3/types/transaction.rs.html#40-73
// missing from/to fields
// remove after: https://github.com/tomusdrw/rust-web3/issues/513 is solved
/// "Receipt" of an executed transaction: details of its execution.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransactionReceipt {
	/// Transaction hash.
	#[serde(rename = "transactionHash")]
	pub transaction_hash: H256,
	/// Index within the block.
	#[serde(rename = "transactionIndex")]
	pub transaction_index: Index,
	/// Hash of the block this transaction was included within.
	#[serde(rename = "blockHash")]
	pub block_hash: Option<H256>,
	/// Number of the block this transaction was included within.
	#[serde(rename = "blockNumber")]
	pub block_number: Option<U64>,
	/// Address of the sender.
	pub from: Address,
	/// Address of the receiver, or `None` if a contract deployment
	pub to: Option<Address>,
	/// Cumulative gas used within the block after this was executed.
	#[serde(rename = "cumulativeGasUsed")]
	pub cumulative_gas_used: U256,
	/// Gas used by this transaction alone.
	///
	/// Gas used is `None` if the the client is running in light client mode.
	#[serde(rename = "gasUsed")]
	pub gas_used: Option<U256>,
	/// Contract address created, or `None` if not a deployment.
	#[serde(rename = "contractAddress")]
	pub contract_address: Option<Address>,
	/// Logs generated within this transaction.
	pub logs: Vec<Log>,
	/// Status: either 1 (success) or 0 (failure).
	pub status: Option<U64>,
	/// State root.
	pub root: Option<H256>,
	/// Logs bloom
	#[serde(rename = "logsBloom")]
	pub logs_bloom: H2048,
	/// Transaction type, Some(1) for AccessList transaction, None for Legacy
	#[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
	pub transaction_type: Option<U64>,
}

#[repr(C)]
#[derive(Serialize, Debug)]
pub struct GetTxReceiptRequest {
	#[serde(rename = "jsonrpc")]
	/// The version of the JSON RPC spec
	pub json_rpc: String,
	/// The method which is called
	pub method: String,
	/// Arguments supplied to the method. Can be an empty Vec.
	pub params: Vec<H256>,
	/// The id for the request
	pub id: usize,
}
