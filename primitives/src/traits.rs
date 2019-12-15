// Copyright (C) 2019 Centrality Investments Limited
// This file is part of CENNZnet.

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

//! Common traits used by CENNZnet node.

use support::dispatch::Result;

/// A trait which enables buying some fee asset using another asset.
/// It is targeted at the CENNZX Spot exchange and the CennznetExtrinsic format.
pub trait BuyFeeAsset<AccountId, Balance> {
	/// A type to handle fee and asset exchange.
	type FeeExchange;
	/// Buy `amount` of fee asset for `who` using asset info from `fee_exchange.
	/// Note: It does not charge the fee asset, that is left to a `ChargeFee` implementation
	fn buy_fee_asset(who: &AccountId, amount: Balance, fee_exchange: &Self::FeeExchange) -> Result;
}
