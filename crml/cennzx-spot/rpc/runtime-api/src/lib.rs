// Copyright 2019-2020 Parity Technologies (UK) Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

//! Runtime API definition required by CENNZX RPC extensions.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::Codec;

sp_api::decl_runtime_apis! {
	/// The RPC API to interact with CENNZX Spot Exchange
	pub trait CENNZXApi<AssetId, Balance> where
		AssetId: Codec,
		Balance: Codec,
	{
		/// Query how much `asset_to_buy` will be given in exchange for `amount` of `asset_to_sell`
		fn sell_price(
			asset_to_sell: AssetId,
			amount: Balance,
			asset_to_buy: AssetId,
		) -> Balance;
		/// Query how much `asset_to_sell` is required to buy `amount` of `asset_to_buy`
		fn buy_price(
			asset_to_buy: AssetId,
			amount: Balance,
			asset_to_sell: AssetId,
		) -> Balance;
	}
}
