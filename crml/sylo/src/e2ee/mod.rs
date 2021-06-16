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

use crate::{
	device::{self, DeviceId},
	groups, inbox, response,
};
use frame_support::{decl_module, decl_storage, dispatch::Vec};

pub trait Config: inbox::Config + response::Config + device::Config + groups::Config {}

// Serialized pre key bundle used to establish one to one e2ee
pub type PreKeyBundle = Vec<u8>;

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin, system = frame_system {}
}

decl_storage! {
	trait Store for Module<T: Config> as SyloE2EE {
		PreKeyBundles get(fn pkbs): map hasher(blake2_128_concat) (T::AccountId, DeviceId) => Vec<PreKeyBundle>;
	}
}
