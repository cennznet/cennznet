use cennznet_primitives::types::FeePreferences;
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
use cennznet_primitives::types::Balance;
use codec::{Decode, Encode};
pub use crml_support::{H160 as EthAddress, H256, U256};
use scale_info::TypeInfo;

/// Identifies remote call requests
pub type RequestId = U256;

/// Details of a remote 'eth_call' request
#[derive(Debug, Clone, PartialEq, Decode, Encode, TypeInfo)]
pub struct CallRequest {
	/// Digest (blake256) of the input data for the remote call
	pub input_digest: [u8; 32],
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
	/// A bounty for fulfiling the request successfully
	pub bounty: Balance,
}

/// Reported response of an executed remote call
#[derive(Debug, Clone, PartialEq, Decode, Encode, TypeInfo)]
pub struct CallResponse<AccountId> {
	/// Digest (blake256) of the return data
	pub return_data_digest: [u8; 32],
	/// The ethereum block number where the result was recorded
	pub eth_block_number: u64,
	/// Address of the relayer that reported this
	pub reporter: AccountId,
}
