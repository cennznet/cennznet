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

//! Common traits used by CENNZnet node.

use frame_support::{
	dispatch::{DispatchError, DispatchResult},
	Parameter,
};
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, Member},
};

/// A trait which enables buying some fee asset using another asset.
/// It is targeted at the CENNZX Spot exchange and the CennznetExtrinsic format.
pub trait BuyFeeAsset {
	/// The account identifier type
	type AccountId;
	/// The type to denote monetary values
	type Balance;
	/// A type with fee payment information
	type FeeExchange;

	/// Buy `amount` of fee asset for `who` using asset info from `fee_exchange.
	/// If the purchase has been successful, return Ok with sold amount
	/// deducting the actual fee in the users's specified asset id, otherwise return Err.
	/// Note: It does not charge the fee asset, that is left to a `ChargeFee` implementation
	fn buy_fee_asset(
		who: &Self::AccountId,
		amount: Self::Balance,
		fee_exchange: &Self::FeeExchange,
	) -> Result<Self::Balance, DispatchError>;
}

/// Something that can resolve if an extrinsic call requires a gas meter or not
pub trait IsGasMeteredCall {
	/// The extrinsic call type
	type Call;
	/// Return whether this call requires a gas meter or not
	fn is_gas_metered(call: &Self::Call) -> bool;
}

/// A simple interface to manage account balances in any asset
/// It's only used to decouple generic-asset from CENNZ-X
pub trait SimpleAssetSystem {
	/// The system account identifier type
	type AccountId: Parameter;
	/// The asset identifier identifiers
	type AssetId: Parameter + Member + AtLeast32BitUnsigned + Default + Copy;
	/// The type for denoting asset balances
	type Balance: Parameter + Member + AtLeast32BitUnsigned + Default + Copy;
	/// Transfer some `amount` of assets `from` one account `to` another
	fn transfer(
		asset_id: Self::AssetId,
		from: &Self::AccountId,
		to: &Self::AccountId,
		amount: Self::Balance,
	) -> DispatchResult;
	/// Get the liquid asset balance of `account`
	fn free_balance(asset_id: Self::AssetId, account: &Self::AccountId) -> Self::Balance;
	/// Get the default asset/currency ID in the system
	fn default_asset_id() -> Self::AssetId;
}
