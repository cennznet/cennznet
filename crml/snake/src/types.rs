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
//!
//! Snake Types

use codec::{Decode, Encode};
use sp_std::vec::Vec;

//#[cfg(feature = "std")]

pub const MAX_WINDOW_SIZE: i8 = 100;
pub const MIN_WINDOW_SIZE: i8 = 5;

#[derive(Encode, Decode, Default, Debug, Clone, PartialEq, Eq)]
pub struct WindowSize {
	pub window_width: i8,
	pub window_height: i8,
}

#[derive(Encode, Decode, Default, Debug, Clone, PartialEq, Eq)]
pub struct Snake {
	pub body: Vec<(i8, i8)>,
	pub dir: Direction,
	pub direction_changed: bool,
}

#[derive(Encode, Decode, Default, Debug, Clone, PartialEq, Eq)]
pub struct Food {
	pub x: i8,
	pub y: i8,
}

#[derive(Encode, Decode, Clone, Debug, PartialEq, Eq)]
pub enum Direction {
	Up,
	Left,
	Down,
	Right,
}

impl Default for Direction {
	fn default() -> Direction {
		Direction::Right
	}
}
