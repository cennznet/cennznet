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

//! Sylo e2ee benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_system::RawOrigin;
use sp_runtime::traits::Hash;
use sp_std::boxed::Box;
use sp_std::{vec, vec::Vec};

use crate::device::Module as SyloDevice;
use crate::e2ee::Module as SyloE2EE;
use crate::groups::{Group, Member, MemberRoles, Meta, Module as SyloGroups, Text};
use crate::response::{Module as SyloResponse, Response};

const SEED: u32 = 0;

fn setup_groups<T: Trait>(caller: T::AccountId) {
	let text_tuple0 = (Text::from(*b"t0m0"), Text::from(*b"t0m1"));
	let text_tuple1 = (Text::from(*b"t1m0"), Text::from(*b"t1m1"));
	let meta = Meta::from([text_tuple0, text_tuple1]);

	let admin = Member::<T::AccountId>::new(
		caller.clone(),
		Vec::<MemberRoles>::from([MemberRoles::Admin, MemberRoles::Member]),
		Meta::new(),
	);

	let member = Member::<T::AccountId>::new(
		account("member", 0, SEED),
		Vec::<MemberRoles>::from([MemberRoles::Member]),
		Meta::new(),
	);

	let group_id_0 = T::Hashing::hash(b"group0");
	let group_0 = Group::<T::AccountId, T::Hash>::new(
		group_id_0,
		Vec::<Member<T::AccountId>>::from([admin.clone(), member.clone()]),
		Vec::new(),
		meta.clone(),
	);
	<SyloGroups<T>>::insert(&caller, &group_id_0, group_0);

	let group_id_1 = T::Hashing::hash(b"group1");
	let group_1 = Group::<T::AccountId, T::Hash>::new(
		group_id_1,
		Vec::<Member<T::AccountId>>::from([admin, member]),
		Vec::new(),
		meta,
	);
	<SyloGroups<T>>::insert(&caller, &group_id_1, group_1);
}

fn create_pre_key_bundles<T: Trait>(n: u32) -> Vec<PreKeyBundle> {
	let mut pre_key_bundles = Vec::<PreKeyBundle>::new();
	for i in 0..n {
		let mut pre_key_bundle = PreKeyBundle::from(*b"prekeybundle");
		pre_key_bundle.extend_from_slice(&i.to_be_bytes());
		pre_key_bundles.push(pre_key_bundle);
	}
	pre_key_bundles
}

benchmarks! {
	_{ }

	register_device {
		let p in 0 .. MAX_PKBS as u32;
		let sender: T::AccountId = whitelisted_caller();
		let device_id: DeviceId = 11;
		setup_groups::<T>(sender.clone());
		let pre_key_bundles = create_pre_key_bundles::<T>(p);
	}: _(RawOrigin::Signed(sender.clone()), device_id, pre_key_bundles)
	verify {
		assert!(<SyloDevice<T>>::devices(sender).contains(&device_id));
	}

	replenish_pkbs {
		let p in 0 .. MAX_PKBS as u32;
		let sender: T::AccountId = whitelisted_caller();
		let device_id: DeviceId = 11;
		setup_groups::<T>(sender.clone());
		let pre_key_bundles = create_pre_key_bundles::<T>(0); // Initially empty so replenish_pkbs can extend it fully.
		let _ = <SyloE2EE<T>>::register_device(RawOrigin::Signed(sender.clone()).into(), device_id, pre_key_bundles);
		let pre_key_bundles = create_pre_key_bundles::<T>(p);
	}: _(RawOrigin::Signed(sender.clone()), device_id, pre_key_bundles)
	verify {
		assert_eq!(<SyloE2EE<T>>::pkbs((sender, device_id)).len(), p as usize);
	}

	withdraw_pkbs {
		let p in 0 .. WITHDRAW_LIST_MAX_LEN as u32;
		let sender: T::AccountId = whitelisted_caller();
		let request_id = T::Hashing::hash(b"req_0");
		setup_groups::<T>(sender.clone());
		let mut user_device_list = Vec::<(T::AccountId, DeviceId)>::new();
		for i in 0 .. p {
			let user: T::AccountId = account("user", i, SEED);
			let device_id: DeviceId = i;
			let pre_key_bundles = create_pre_key_bundles::<T>(MAX_PKBS as u32);
			let _ = <SyloE2EE<T>>::register_device(RawOrigin::Signed(user.clone()).into(), device_id, pre_key_bundles);
			user_device_list.push((user, device_id));
		}
	}: _(RawOrigin::Signed(sender.clone()), request_id, user_device_list.clone())
	verify {
		if let Response::PreKeyBundles(l) = <SyloResponse<T>>::response((sender, request_id)) {
			assert_eq!(l.len(), user_device_list.len());
		}
		else {
			assert!(false);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{ExtBuilder, Test};
	use frame_support::assert_ok;

	#[test]
	fn register_device() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_register_device::<Test>());
		});
	}

	#[test]
	fn replenish_pkbs() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_register_device::<Test>());
		});
	}

	#[test]
	fn withdraw_pkbs() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_register_device::<Test>());
		});
	}
}
