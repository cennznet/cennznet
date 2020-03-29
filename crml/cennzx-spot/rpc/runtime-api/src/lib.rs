/* Copyright 2019-2020 Centrality Investments Limited
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

//! Runtime API definition required by CENNZX RPC extensions.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Codec, Decode, Encode};
use sp_arithmetic::traits::BaseArithmetic;
use sp_runtime::RuntimeDebug;

/// A result of querying the exchange
#[derive(Eq, PartialEq, Encode, Decode, RuntimeDebug)]
pub enum CennzxSpotResult<Balance> {
	/// The exchange returned successfully.
	Success(Balance),
	/// There was an issue querying the exchange
	Error,
}

sp_api::decl_runtime_apis! {
	/// The RPC API to interact with CENNZX Spot Exchange
	pub trait CennzxSpotApi<AssetId, Balance> where
		AssetId: Codec,
		Balance: Codec + BaseArithmetic,
	{
		/// Query how much `asset_to_buy` will be given in exchange for `amount` of `asset_to_sell`
		fn buy_price(
			asset_to_buy: AssetId,
			amount: Balance,
			asset_to_sell: AssetId,
		) -> CennzxSpotResult<Balance>;
		/// Query how much `asset_to_sell` is required to buy `amount` of `asset_to_buy`
		fn sell_price(
			asset_to_sell: AssetId,
			amount: Balance,
			asset_to_buy: AssetId,
		) -> CennzxSpotResult<Balance>;
	}
}
