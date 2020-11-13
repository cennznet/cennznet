// Copyright 2019-2020 Centrality Investments Limited
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Sylo groups benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_system::RawOrigin;
use sp_core::hash::H256;
use sp_runtime::traits::Hash;
use sp_std::{boxed::Box, convert::TryInto, vec, vec::Vec};

use crate::device::{DeviceId, Module as SyloDevice, MAX_DEVICES};
use crate::groups::Module as SyloGroups;
use crate::vault::{VaultKey, VaultValue};

const SEED: u32 = 0;

fn create_invite_list<T: Trait>(n: u32) -> Vec<Invite<T::AccountId>> {
	let mut invites = Vec::<Invite<T::AccountId>>::new();
	for i in 0..n {
		invites.push(Invite {
			peer_id: account("peer", i, SEED),
			invite_data: Vec::<u8>::from(*b"Join The Coolest Group On Earth"),
			invite_key: <H256 as From<[u8; 32]>>::from(i.to_be_bytes().repeat(8).as_slice().try_into().unwrap()),
			meta: Meta::new(),
			roles: Vec::<MemberRoles>::from([MemberRoles::Member]),
		});
	}
	invites
}

fn create_group_data() -> (VaultKey, VaultValue) {
	let key = VaultKey::from(*b"Averylittlekeyopensaheavydoor");
	let value = VaultValue::from(*b"Ourvalueisthesumofourvalues");
	(key, value)
}

fn create_meta() -> Meta {
	let text_tuple0 = (Text::from(*b"t0m0"), Text::from(*b"t0m1"));
	let text_tuple1 = (Text::from(*b"t1m0"), Text::from(*b"t1m1"));
	Meta::from([text_tuple0, text_tuple1])
}

fn setup_devices<T: Trait>(owner: &T::AccountId) {
	for i in 0..MAX_DEVICES {
		let _ = <SyloDevice<T>>::append_device(owner, i as DeviceId);
	}
}

fn find_member<T: Trait>(account_id: T::AccountId, group_id: T::Hash) -> Option<Member<T::AccountId>> {
	<SyloGroups<T>>::group(group_id)
		.members
		.iter()
		.find(|x| x.user_id == account_id)
		.cloned()
}

fn setup_group_with_members<T: Trait>(caller: T::AccountId, group_id: T::Hash, num_of_members: u32) {
	let mut members = <Vec<Member<T::AccountId>>>::new();
	members.push(Member::<T::AccountId>::new(
		caller.clone(),
		Vec::<MemberRoles>::from([MemberRoles::Admin, MemberRoles::Member]),
		Meta::new(),
	));

	for i in 1..num_of_members {
		members.push(Member::<T::AccountId>::new(
			account("member", i, SEED),
			Vec::<MemberRoles>::from([MemberRoles::Member]),
			Meta::new(),
		));
	}

	let group = Group::<T::AccountId, T::Hash>::new(group_id, members, Vec::new(), create_meta());
	<SyloGroups<T>>::insert(&caller, &group_id, group);
}

benchmarks! {
	_{ }

	create_group {
		let i in 0 .. MAX_INVITES as u32;
		let sender: T::AccountId = whitelisted_caller();
		let group_id = T::Hashing::hash(b"group_id");
		setup_devices::<T>(&sender);
	}: _(RawOrigin::Signed(sender.clone()), group_id, create_meta(), create_invite_list::<T>(i), create_group_data())
	verify {
		assert!(<SyloGroups<T>>::is_group_member(&group_id, &sender));
	}

	leave_group {
		let member: T::AccountId = whitelisted_caller();
		let group_id = T::Hashing::hash(b"group_id");
		let (key, value) = create_group_data();
		let _ = <SyloGroups<T>>::create_group(
			RawOrigin::Signed(member.clone()).into(),
			group_id,
			create_meta(),
			create_invite_list::<T>(MAX_INVITES as u32),
			(key.clone(), value),
		);
		assert!(<SyloGroups<T>>::is_group_member(&group_id, &member));
	}: _(RawOrigin::Signed(member.clone()), group_id, Some(key))
	verify {
		assert!(!<SyloGroups<T>>::is_group_member(&group_id, &member));
	}

	update_member {
		let admin: T::AccountId = whitelisted_caller();
		let group_id = T::Hashing::hash(b"group_id");
		setup_group_with_members::<T>(admin.clone(), group_id, MAX_MEMBERS as u32);
		let meta = create_meta();
	}: _(RawOrigin::Signed(admin.clone()), group_id, meta.clone())
	verify {
		let member = find_member::<T>(admin, group_id).unwrap();
		assert_eq!(member.meta, meta);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{ExtBuilder, Test};
	use frame_support::assert_ok;

	#[test]
	fn create_group() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_create_group::<Test>());
		});
	}

	#[test]
	fn leave_group() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_leave_group::<Test>());
		});
	}

	#[test]
	fn update_member() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_update_member::<Test>());
		});
	}
}
