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

//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::weights::{constants::RocksDbWeight as DbWeight, Weight};

impl crate::groups::WeightInfo for () {
	fn create_group(i: usize, m: usize) -> Weight {
		(0 as Weight)
			.saturating_add((156_781_000 as Weight).saturating_mul(i as Weight))
			.saturating_add((17_403_000 as Weight).saturating_mul(m as Weight))
			.saturating_add(DbWeight::get().reads(4 as Weight))
			.saturating_add(DbWeight::get().reads((2 as Weight).saturating_mul(i as Weight)))
			.saturating_add(DbWeight::get().writes(4 as Weight))
			.saturating_add(DbWeight::get().writes((2 as Weight).saturating_mul(i as Weight)))
	}
	fn leave_group() -> Weight {
		(262_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(2 as Weight))
			.saturating_add(DbWeight::get().writes(2 as Weight))
	}
	fn update_member(m: usize) -> Weight {
		(379_289_000 as Weight)
			.saturating_add((2_344_000 as Weight).saturating_mul(m as Weight))
			.saturating_add(DbWeight::get().reads(1 as Weight))
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn upsert_group_meta(m: usize) -> Weight {
		(187_501_000 as Weight)
			.saturating_add((110_064_000 as Weight).saturating_mul(m as Weight))
			.saturating_add(DbWeight::get().reads(1 as Weight))
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn create_invites(i: usize) -> Weight {
		(2_713_730_000 as Weight)
			.saturating_add((5_793_000 as Weight).saturating_mul(i as Weight))
			.saturating_add(DbWeight::get().reads(31 as Weight))
			.saturating_add(DbWeight::get().writes(31 as Weight))
	}
	fn accept_invite() -> Weight {
		(544_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(6 as Weight))
			.saturating_add(DbWeight::get().writes(5 as Weight))
	}
	fn revoke_invites(_i: usize) -> Weight {
		(435_834_000 as Weight)
			.saturating_add(DbWeight::get().reads(1 as Weight))
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
}
