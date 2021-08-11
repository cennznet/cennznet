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
use codec::{Decode, Encode};
pub use sp_core::{H160, H256, U256};
use sp_std::convert::TryInto;

/// Ethereum addres type
pub type EthAddress = H160;

/// A deposit event made by the CENNZnet bridge contract on Ethereum
#[derive(Debug, Default, Clone, PartialEq, Decode, Encode)]
pub struct Erc20DepositEvent {
	/// The ERC20 token address / type deposited
	pub token_type: EthAddress,
	/// The amount (in 'wei') of the deposit
	pub amount: U256,
	/// The CENNZnet beneficiary address
	pub beneficiary: H256,
}

/// Something that can be decoded from eth log data/ ABI
pub trait EthAbiCodec: Sized {
	fn encode(&self) -> Vec<u8>;
	/// Decode `Self` from Eth log data
	fn decode(data: &[u8]) -> Option<Self>;
}

// todo: could make a macro for this
impl EthAbiCodec for Erc20DepositEvent {
	fn encode(&self) -> Vec<u8> {
		ethabi::encode(&[
			ethabi::Token::Address(self.token_type),
			ethabi::Token::Uint(self.amount),
			ethabi::Token::FixedBytes(self.beneficiary.as_bytes().to_vec()),
		])
	}
	/// Receives Ethereum log 'data' and decodes it
	fn decode(data: &[u8]) -> Option<Self> {
		let tokens = ethabi::decode(
			&[
				ethabi::ParamType::Address,
				ethabi::ParamType::Uint(256),
				ethabi::ParamType::FixedBytes(32),
			],
			data,
		)
		.unwrap();

		let token_type = tokens[0].clone().into_address()?;
		let amount = tokens[1].clone().into_uint()?;
		let beneficiary: [u8; 32] = tokens[2].clone().into_fixed_bytes()?.try_into().unwrap();

		Some(Self {
			token_type,
			amount,
			beneficiary: beneficiary.into(),
		})
	}
}
