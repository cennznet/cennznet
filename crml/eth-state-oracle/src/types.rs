/* Copyright 2022 Centrality Investments Limited
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
use cennznet_primitives::types::{Balance, BlockNumber, FeePreferences};
use codec::{Decode, Encode};
pub use crml_support::{H160 as EthAddress, H256, U256};
use scale_info::TypeInfo;
use sp_std::prelude::*;

/// Identifies remote call challenges
pub type ChallengeId = u64;

/// Identifies remote call requests
pub type RequestId = U256;

/// Details of a remote 'eth_call' request
#[derive(Debug, Clone, PartialEq, Decode, Encode, TypeInfo)]
pub struct CallRequest {
	/// Destination address for the remote call
	pub destination: EthAddress,
	/// CENNZnet evm address of the caller
	pub caller: EthAddress,
	/// The gas limit for the callback execution
	pub callback_gas_limit: u64,
	/// Function selector of the callback
	pub callback_signature: [u8; 4],
	/// Fee preferences for callback gas payment
	pub fee_preferences: Option<FeePreferences>,
	/// A bounty for fulfilling the request successfully
	pub bounty: Balance,
	/// unix timestamp in seconds the request was placed
	pub timestamp: u64,
	/// cennznet block number where the request will expire (i.e. it is also the response deadline)
	pub expiry_block: BlockNumber,
	/// The input 'data' parameter for the remote eth_call
	pub input_data: Vec<u8>,
}

/// Reported response of an executed remote call
#[derive(Debug, Clone, PartialEq, Decode, Encode, TypeInfo)]
pub struct CallResponse<AccountId> {
	/// The 'returndata' state as claimed by `reporter`
	pub return_data: ReturnDataClaim,
	/// The ethereum block number where the result was obtained
	pub eth_block_number: u64,
	/// The ethereum block timestamp where the result was obtained
	pub eth_block_timestamp: u64,
	/// Address of the relayer that reported this
	pub relayer: AccountId,
	/// The CENNZnet block timestamp at the time of submission
	pub submitted_at: u64,
}

#[derive(Debug, Clone, PartialEq, Decode, Encode, TypeInfo)]
/// A claim about the returndata of an `eth_call` RPC
pub enum ReturnDataClaim {
	/// Normal returndata scenario
	/// Its value is an Ethereum abi encoded word (32 bytes)
	Ok([u8; 32]),
	/// The returndata from the executed call exceeds the 32 byte length limit
	/// It won't be processed so we don't record the data
	ExceedsLengthLimit,
}
