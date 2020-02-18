// Copyright 2018-2020 Centrality Investments Ltd.
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

//! Common implementation used by CENNZnet node.

use crate::types::{FeeExchange, FeeExchangeV1};

impl<AssetId, Balance> FeeExchangeV1<AssetId, Balance> {
	/// Create a new FeeExchange
	pub fn new(asset_id: AssetId, max_payment: Balance) -> Self {
		Self { asset_id, max_payment }
	}
}

impl<AssetId: Clone, Balance: Clone> FeeExchange<AssetId, Balance> {
	/// Make and return a v1 FeeExchange
	pub fn new_v1(id: AssetId, balance: Balance) -> Self {
		FeeExchange::V1(FeeExchangeV1 {
			asset_id: id,
			max_payment: balance,
		})
	}

	/// Return the specified asset id
	pub fn get_asset_id(&self) -> AssetId {
		match self {
			FeeExchange::V1(x) => x.asset_id.clone(),
		}
	}

	/// Return the specified balance/max_payment for the fee exchange
	pub fn get_balance(&self) -> Balance {
		match self {
			FeeExchange::V1(x) => x.max_payment.clone(),
		}
	}
}
