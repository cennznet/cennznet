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

use frame_support::dispatch::DispatchError;

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
