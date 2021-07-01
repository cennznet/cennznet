/* Copyright 2019-2021 Centrality Investments Limited
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
//! Mocks for the module.

#![cfg(test)]

use crate as crml_snake;
use frame_support::{assert_noop, assert_ok, parameter_types, traits::OnInitialize};

use frame_support::traits::TestRandomness;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	ModuleId,
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;
//type Randomness = TestRandomness;

// test accounts
pub const ALICE: u64 = 1;
pub const BOB: u64 = 2;
/*
pub const SNAKE: Snake = Snake {
	body: vec![(3, 0), (2, 0), (1, 0), (0, 0)],
	dir: Direction::Right,
	direction_changed: false,
};
pub const WINDOW_SIZE: WindowSize = WindowSize {
	window_width: 20,
	window_height: 20,
};
pub const FOOD: Food = Food { x: 5, y: 5 };
*/

frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Module, Call, Config, Storage, Event<T>},
		Snake: crml_snake::{Module, Call, Storage, Event<T>},
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
}
impl frame_system::Config for Test {
	type BlockWeights = ();
	type BlockLength = ();
	type BaseCallFilter = ();
	type Origin = Origin;
	type Index = u64;
	type BlockNumber = u64;
	type Call = Call;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type BlockHashCount = BlockHashCount;
	type Event = Event;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
}

impl crate::Config for Test {
	type Event = Event;
	type WeightInfo = ();
	type RandomnessSource = TestRandomness;
}

pub struct ExtBuilder {
	start: bool,
	window_size: i8,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			start: false,
			window_size: 20,
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut ext: sp_io::TestExternalities = frame_system::GenesisConfig::default()
			.build_storage::<Test>()
			.unwrap()
			.into();

		if self.start {
			assert_ok!(ext.execute_with(|| Snake::start(Origin::signed(ALICE), self.window_size, self.window_size)));
		}

		//ext.execute_with(|| frame_system::Module::<Test>::set_block_number(1));

		ext
	}

	pub fn start_game(mut self, start_game: bool) -> Self {
		self.start = start_game;
		self
	}

	pub fn window_size(mut self, window_size: i8) -> Self {
		self.window_size = window_size;
		self
	}
}
