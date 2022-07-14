/* Copyright 2019-2022 Centrality Investments Limited
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

use cennznet_runtime::{Runtime, CENNZNET_EVM_CONFIG};
use crml_support::{H160, H256, U256};
use pallet_evm::{EvmConfig, Runner as RunnerT, RunnerError};

// Runner call for calling the evm precompiles
pub struct RunnerCallBuilder {
	source: H160,
	target: H160,
	input: Vec<u8>,
	value: U256,
	gas_limit: u64,
	max_fee_per_gas: Option<U256>,
	max_priority_fee_per_gas: Option<U256>,
	nonce: Option<U256>,
	access_list: Vec<(H160, Vec<H256>)>,
	is_transactional: bool,
	validate: bool,
	config: EvmConfig,
}

impl RunnerCallBuilder {
	pub fn new(caller: H160, input: Vec<u8>, target_precompile: H160) -> Self {
		// Create a RunnerCall with some default values
		let access_list = Vec::<(H160, Vec<H256>)>::default();
		Self {
			source: caller,
			target: target_precompile,
			input,
			value: U256::zero(),
			gas_limit: 100_000,
			max_fee_per_gas: Some(U256::from(20_000_000_000_000_u64)),
			max_priority_fee_per_gas: None,
			nonce: None,
			access_list,
			is_transactional: false,
			validate: false,
			config: CENNZNET_EVM_CONFIG.clone(),
		}
	}
	pub fn gas_limit(mut self, gas_limit: u64) -> Self {
		self.gas_limit = gas_limit;
		self
	}
	pub fn run(self) -> Result<pallet_evm::CallInfo, RunnerError<pallet_evm::Error<Runtime>>> {
		<Runtime as pallet_evm::Config>::Runner::call(
			self.source,
			self.target,
			self.input,
			self.value,
			self.gas_limit,
			self.max_fee_per_gas,
			self.max_priority_fee_per_gas,
			self.nonce,
			self.access_list,
			self.is_transactional,
			self.validate,
			&self.config,
		)
	}
}
