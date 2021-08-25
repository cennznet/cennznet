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
use crate::log;
pub use crml_support::{EthAbiCodec, H160, H256, U256};
use sp_std::convert::TryInto;
use sp_std::prelude::*;

/// Ethereum address type
pub type EthAddress = H160;

/// A deposit event made by the CENNZnet bridge contract on Ethereum
#[derive(Debug, Default, Clone, PartialEq, Decode, Encode)]
pub struct Erc20DepositEvent {
	/// The ERC20 token address / type deposited
	pub token_address: EthAddress,
	/// The amount (in 'wei') of the deposit
	pub amount: U256,
	/// The CENNZnet beneficiary address
	pub beneficiary: H256,
}

/// A withdraw message to prove and submit to Ethereum
/// Allowing redemption of ERC20s
#[derive(Debug, Default, Clone, PartialEq, Decode, Encode)]
pub struct WithdrawMessage {
	/// The ERC20 token address / type deposited
	pub token_address: EthAddress,
	/// The amount (in 'wei') of the deposit
	pub amount: U256,
	/// The Ethereum beneficiary address
	pub beneficiary: EthAddress,
}

impl EthAbiCodec for WithdrawMessage {
	/// Encode `ERC20DepositEvent` into 32-byte words
	/// https://docs.soliditylang.org/en/v0.5.3/abi-spec.html#formal-specification-of-the-encoding
	fn encode(&self) -> Vec<u8> {
		let mut buf = [0_u8; 32 * 3];
		buf[12..32].copy_from_slice(&self.token_address.to_fixed_bytes());
		buf[32..64].copy_from_slice(&Into::<[u8; 32]>::into(self.amount));
		buf[76..96].copy_from_slice(&self.beneficiary.to_fixed_bytes());
		buf.to_vec()
	}

	fn decode(_data: &[u8]) -> Option<Self> {
		unimplemented!();
	}
}

impl EthAbiCodec for Erc20DepositEvent {
	/// Encode `ERC20DepositEvent` into 32-byte words
	/// https://docs.soliditylang.org/en/v0.5.3/abi-spec.html#formal-specification-of-the-encoding
	fn encode(&self) -> Vec<u8> {
		let mut buf = [0_u8; 32 * 3];
		buf[12..32].copy_from_slice(&self.token_address.to_fixed_bytes());
		buf[32..64].copy_from_slice(&Into::<[u8; 32]>::into(self.amount));
		buf[64..96].copy_from_slice(&self.beneficiary.to_fixed_bytes());
		buf.to_vec()
	}
	/// Receives Ethereum log 'data' and decodes it
	fn decode(data: &[u8]) -> Option<Self> {
		// Expect 3 words of data
		if data.len() != 3 * 32 {
			return None;
		}
		let token_address = H160::from(&data[12..32].try_into().expect("20 bytes decode"));
		let amount = data[32..64].into();
		let beneficiary = H256::from(&data[64..96].try_into().expect("32 bytes decode"));

		Some(Self {
			token_address,
			amount,
			beneficiary,
		})
	}
}

#[cfg(test)]
mod test {
	use super::{Erc20DepositEvent, EthAbiCodec};
	use crml_support::{H160, H256, U256};

	#[test]
	fn deposit_event_encode() {
		let event = Erc20DepositEvent {
			token_address: H160::from_low_u64_be(55),
			amount: U256::from(123),
			beneficiary: H256::from_low_u64_be(77),
		};
		assert_eq!(
			event.encode(),
			vec![
				0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 55, 0, 0,
				0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 123, 0, 0, 0, 0,
				0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 77
			]
		);
	}

	#[test]
	fn deposit_event_decode() {
		let raw = vec![
			0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 55, 0, 0, 0,
			0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 123, 0, 0, 0, 0, 0, 0,
			0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 77,
		];
		assert_eq!(
			Erc20DepositEvent::decode(&raw).expect("it decodes"),
			Erc20DepositEvent {
				token_address: H160::from_low_u64_be(55),
				amount: U256::from(123),
				beneficiary: H256::from_low_u64_be(77),
			}
		);
	}
}
