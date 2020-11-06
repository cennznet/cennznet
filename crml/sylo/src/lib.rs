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

#![cfg_attr(not(feature = "std"), no_std)]

pub mod device;
pub mod e2ee;
pub mod groups;
pub mod inbox;
pub mod payment;
pub mod response;
pub mod vault;

#[cfg(test)]
pub(crate) mod mock;

use frame_support::weights::Weight;

pub trait WeightInfo {
	fn register_device() -> Weight;
	fn replenish_pkbs() -> Weight;
	fn withdraw_pkbs() -> Weight;
	fn add_value() -> Weight;
	fn delete_values() -> Weight;
	fn remove_response() -> Weight;
	fn upsert_value() -> Weight;
	fn create_group() -> Weight;
	fn leave_group() -> Weight;
	fn update_member() -> Weight;
	fn upsert_group_meta() -> Weight;
	fn create_invites() -> Weight;
	fn accept_invite() -> Weight;
	fn revoke_invites() -> Weight;
}

impl WeightInfo for () {
	fn register_device() -> Weight {
		0 as Weight
	}
	fn replenish_pkbs() -> Weight {
		0 as Weight
	}
	fn withdraw_pkbs() -> Weight {
		0 as Weight
	}
	fn add_value() -> Weight {
		0 as Weight
	}
	fn delete_values() -> Weight {
		0 as Weight
	}
	fn remove_response() -> Weight {
		0 as Weight
	}
	fn upsert_value() -> Weight {
		0 as Weight
	}
	fn create_group() -> Weight {
		0 as Weight
	}
	fn leave_group() -> Weight {
		0 as Weight
	}
	fn update_member() -> Weight {
		0 as Weight
	}
	fn upsert_group_meta() -> Weight {
		0 as Weight
	}
	fn create_invites() -> Weight {
		0 as Weight
	}
	fn accept_invite() -> Weight {
		0 as Weight
	}
	fn revoke_invites() -> Weight {
		0 as Weight
	}
}

pub trait Trait: frame_system::Trait {
	type WeightInfo: WeightInfo;
}
