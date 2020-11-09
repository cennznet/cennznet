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

//! Sylo inbox benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_system::RawOrigin;
use sp_std::boxed::Box;
use sp_std::{vec, vec::Vec};

use crate::inbox::Module as SyloInbox;

const SEED: u32 = 0;

benchmarks! {
	_{ }

	add_value {
		let sender: T::AccountId = whitelisted_caller();
		let recipient: T::AccountId = account("recipient", 0, SEED);
		let message = Vec::<u8>::from(*b"Hey buddy!");
	}: _(RawOrigin::Signed(sender.clone()), recipient.clone(), message.clone())
	verify {
		assert!(<SyloInbox<T>>::inbox(recipient).contains(&message));
	}

	delete_values {
		let sender: T::AccountId = whitelisted_caller();
		let recipient: T::AccountId = account("recipient", 0, SEED);
		let message0 = Vec::<u8>::from(*b"Hey buddy!");
		let message1 = Vec::<u8>::from(*b"How are you doing?");
		let message2 = Vec::<u8>::from(*b"Have you got some CPAY to spare?");
		let message3 = Vec::<u8>::from(*b"I need to buy a coke.");
		let message4 = Vec::<u8>::from(*b"I will pay you back with Sylos.");
		let _ = <SyloInbox<T>>::add_value(RawOrigin::Signed(sender.clone()).into(), recipient.clone(), message0.clone());
		let _ = <SyloInbox<T>>::add_value(RawOrigin::Signed(sender.clone()).into(), recipient.clone(), message1.clone());
		let _ = <SyloInbox<T>>::add_value(RawOrigin::Signed(sender.clone()).into(), recipient.clone(), message2.clone());
		let _ = <SyloInbox<T>>::add_value(RawOrigin::Signed(sender.clone()).into(), recipient.clone(), message3.clone());
		let _ = <SyloInbox<T>>::add_value(RawOrigin::Signed(sender.clone()).into(), recipient.clone(), message4.clone());
	}: _(RawOrigin::Signed(recipient.clone()), Vec::<MessageId>::from([0,1,3]))
	verify {
		assert!(!<SyloInbox<T>>::inbox(recipient.clone()).contains(&message0));
		assert!(!<SyloInbox<T>>::inbox(recipient.clone()).contains(&message1));
		assert!(<SyloInbox<T>>::inbox(recipient.clone()).contains(&message2));
		assert!(!<SyloInbox<T>>::inbox(recipient.clone()).contains(&message3));
		assert!(<SyloInbox<T>>::inbox(recipient).contains(&message4));
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{ExtBuilder, Test};
	use frame_support::assert_ok;

	#[test]
	fn add_value() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_add_value::<Test>());
		});
	}

	#[test]
	fn delete_values() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_delete_values::<Test>());
		});
	}
}
