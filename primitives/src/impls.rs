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

use crate::types::{AssetId, Balance, FeeExchangeV1};

impl FeeExchangeV1 {
	/// Create a new FeeExchange
	pub fn new(asset_id: AssetId, max_payment: Balance) -> Self {
		Self { asset_id, max_payment }
	}
}
