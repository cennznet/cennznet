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

use codec::{Decode, Encode};
use frame_support::{decl_module, decl_storage, dispatch::Vec};
use sp_core::hash::H256;

use crate::{
	device::{self, DeviceId},
	inbox, vault,
};

pub trait Config: inbox::Config + device::Config + vault::Config {}

// Meta type stored on group, members and invites
pub type Meta = Vec<(Text, Text)>;

pub type Text = Vec<u8>;

#[derive(Encode, Decode, Clone, Eq, PartialEq, Debug)]
pub enum MemberRoles {
	Admin,
	Member,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, Debug)]
pub struct Invite<AccountId> {
	peer_id: AccountId,
	invite_data: Vec<u8>,
	invite_key: H256,
	meta: Meta,
	roles: Vec<MemberRoles>,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, Debug)]
pub struct PendingInvite<Hash> {
	invite_key: Hash,
	meta: Meta,
	roles: Vec<MemberRoles>,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, Debug)]
pub struct AcceptPayload<AccountId: Encode + Decode> {
	account_id: AccountId,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, Debug)]
pub struct Member<AccountId: Encode + Decode> {
	user_id: AccountId,
	roles: Vec<MemberRoles>,
	meta: Meta,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, Default, Debug)]
pub struct Group<AccountId, Hash>
where
	AccountId: Encode + Decode,
	Hash: Encode + Decode,
{
	group_id: Hash,
	members: Vec<Member<AccountId>>,
	invites: Vec<PendingInvite<H256>>,
	meta: Meta,
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin, system = frame_system {}
}

decl_storage! {
	trait Store for Module<T: Config> as SyloGroups {
		Groups get(fn group): map hasher(blake2_128_concat) T::Hash => Group<T::AccountId, T::Hash>;

		/// Stores the group ids that a user is a member of
		pub Memberships get(fn memberships): map hasher(blake2_128_concat) T::AccountId => Vec<T::Hash>;

		/// Stores the known member/deviceId tuples for a particular group
		MemberDevices get(fn member_devices): map hasher(blake2_128_concat) T::Hash => Vec<(T::AccountId, DeviceId)>;
	}
}
