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

/*
eth_call
	route
	- erc20shim
		- address prefix: 1 + AssetId Hex
		- handler(asset_id, call_data)
	- erc721shim
		- address prefix: 2
		- handler(collection_id, series_id, call_data)
	fallback
	- frontier/evm
*/
#![cfg_attr(not(feature = "std"), no_std)]

use cennznet_primitives::types::{AssetId, Balance};
use crml_support::{EthAddressResolver, MultiCurrency, H160, U256};
use frame_support::{decl_module, log};
use hex_literal::hex;
use sp_runtime::traits::Zero;
use sp_std::{convert::TryInto, prelude::*};

pub(crate) const LOG_TARGET: &str = "erc20-shim";

// syntactic sugar for logging.
#[macro_export]
macro_rules! log {
	($level:tt, $patter:expr $(, $values:expr)* $(,)?) => {
		log::$level!(
			target: crate::LOG_TARGET,
			$patter $(, $values)*
		)
	};
}

pub trait Config: frame_system::Config {
	/// Resolves CENNZnet addresses given an ethereum address
	type EthAddressResolver: EthAddressResolver<AccountId = Self::AccountId>;
	/// Currency functions
	type MultiCurrency: MultiCurrency<AccountId = Self::AccountId, Balance = Balance, CurrencyId = AssetId>;
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin { }
}

impl<T: Config> Module<T> {
	/// Get native CENNZ balance given an ethereum address
	pub fn get_balance(eth_address: &H160) -> U256 {
		log!(debug, "💵 get balance for: {:?}", eth_address);
		let balance = if let Some(cennznet_address) = T::EthAddressResolver::resolve(eth_address) {
			log!(debug, "💵 get balance resolved: {:?}", cennznet_address);
			T::MultiCurrency::free_balance(&cennznet_address, T::MultiCurrency::staking_currency())
		} else {
			Zero::zero()
		};
		log!(debug, "💵 got balance: {:?}", balance);
		return U256::from(balance);
	}
	pub fn call(asset_id: AssetId, calldata: &[u8]) -> Result<Vec<u8>, ()> {
		let function_selector = &calldata[0..4];
		log!(debug, "💵 function selector: {:?}", function_selector);
		match function_selector {
			// "balanceOf(address)",
			hex!("70a08231") => {
				// 4 byte selector + 12 byte padding
				log!(debug, "💵 erc20 get balance: calldata {:?}", &calldata);
				let address_raw: [u8; 20] = calldata[16..36].try_into().map_err(|_| ())?;
				log!(debug, "💵 erc20 get balance: address {:?}", address_raw);
				// check length is address.raw
				let balance = if let Some(cennznet_address) = T::EthAddressResolver::resolve(&address_raw.into()) {
					T::MultiCurrency::free_balance(&cennznet_address, asset_id)
				} else {
					Zero::zero()
				};
				let mut balance_word = vec![0_u8; 16];
				balance_word.extend_from_slice(&balance.to_be_bytes());
				log!(debug, "💵 erc20 got balance: {:?}", balance_word);
				return Ok(balance_word);
			}
			// "name()", | "symbol()",
			hex!("06fdde03") | hex!("95d89b41") => {
				if T::MultiCurrency::currency_exists(asset_id) {
					let (symbol, _decimals) = T::MultiCurrency::meta(asset_id);
					return Ok(symbol);
				}
			}
			// "decimals()",
			hex!("313ce567") => {
				if T::MultiCurrency::currency_exists(asset_id) {
					let (_symbol, decimals) = T::MultiCurrency::meta(asset_id);
					return Ok(vec![decimals]);
				}
			}
			// "totalSupply()",
			hex!("18160ddd") => {
				let issuance = T::MultiCurrency::total_issuance(asset_id);
				let mut issuance_word = issuance.to_be_bytes().to_vec();
				issuance_word.extend_from_slice(&0_u128.to_be_bytes());
				return Ok(issuance_word);
			}
			_ => {
				log!(debug, "💵 erc20 could not match function selector");
				return Err(());
			}
		}

		return Err(());
	}
}
