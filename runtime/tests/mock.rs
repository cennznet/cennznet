// Copyright (C) 2020 Centrality Investments Limited
// This file is part of CENNZnet.
//
// CENNZnet is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// CENNZnet is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with CENNZnet.  If not, see <http://www.gnu.org/licenses/>.

#![allow(dead_code)]
use cennznet_runtime::{constants::asset::*, Runtime, VERSION};
use cennznet_testing::keyring::*;
use core::convert::TryFrom;
use crml_cennzx_spot::{FeeRate, PerMilli, PerMillion};

pub const GENESIS_HASH: [u8; 32] = [69u8; 32];
pub const SPEC_VERSION: u32 = VERSION.spec_version;

#[derive(Default)]
pub struct ExtBuilder {
	initial_balance: u128,
	gas_price: u128,
}

impl ExtBuilder {
	pub fn initial_balance(mut self, initial_balance: u128) -> Self {
		self.initial_balance = initial_balance;
		self
	}
	pub fn gas_price(mut self, gas_price: u128) -> Self {
		self.gas_price = gas_price;
		self
	}
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();
		crml_cennzx_spot::GenesisConfig::<Runtime> {
			fee_rate: FeeRate::<PerMillion>::try_from(FeeRate::<PerMilli>::from(3u128)).unwrap(),
			core_asset_id: CENTRAPAY_ASSET_ID,
		}
		.assimilate_storage(&mut t)
		.unwrap();
		pallet_contracts::GenesisConfig::<Runtime> {
			current_schedule: Default::default(),
			gas_price: self.gas_price,
		}
		.assimilate_storage(&mut t)
		.unwrap();
		pallet_generic_asset::GenesisConfig::<Runtime> {
			assets: vec![
				CENNZ_ASSET_ID,
				CENTRAPAY_ASSET_ID,
				PLUG_ASSET_ID,
				SYLO_ASSET_ID,
				CERTI_ASSET_ID,
				ARDA_ASSET_ID,
			],
			initial_balance: self.initial_balance,
			endowed_accounts: vec![alice(), bob(), charlie(), dave(), eve(), ferdie()],
			next_asset_id: NEXT_ASSET_ID,
			staking_asset_id: STAKING_ASSET_ID,
			spending_asset_id: SPENDING_ASSET_ID,
		}
		.assimilate_storage(&mut t)
		.unwrap();
		t.into()
	}
}
