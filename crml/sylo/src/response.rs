// Copyright 2019 Centrality Investments Limited
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

use codec::{Decode, Encode};
use support::{decl_module, decl_storage, dispatch::Result, dispatch::Vec};
use system::{self, ensure_signed};

pub trait Trait: system::Trait {}

#[derive(Encode, Decode, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum Response<T: Encode + Decode> {
	DeviceId(u32),
	PreKeyBundles(Vec<(T, u32, Vec<u8>)>),
	None,
}

impl<T: Encode + Decode> Default for Response<T> {
	fn default() -> Response<T> {
		Response::None
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn remove_response(origin, request_id: T::Hash) -> Result {
			let sender = ensure_signed(origin)?;
			<Responses<T>>::remove((sender, request_id));
			Ok(())
		}
	}
}

// The data that is stored
decl_storage! {
	trait Store for Module<T: Trait> as SyloResponse {
		Responses get(response): map (T::AccountId, T::Hash /* request_id */) => Response<T::AccountId>;
	}
}

impl<T: Trait> Module<T> {
	pub(super) fn set_response(sender: T::AccountId, request_id: T::Hash, response: Response<T::AccountId>) {
		if response != Response::None {
			<Responses<T>>::insert((sender, request_id), response);
		}
	}
}

#[cfg(test)]
pub(super) mod tests {
	use super::*;
	use crate::mock::{new_test_ext, Origin, Test};
	use primitives::H256;
	use support::assert_ok;

	type Responses = Module<Test>;

	#[test]
	fn should_set_response() {
		new_test_ext().execute_with(|| {
			let request_id = H256::from([1; 32]);
			let resp_number = Response::DeviceId(111);

			// setting number
			Responses::set_response(H256::from_low_u64_be(1), request_id.clone(), resp_number.clone());
			assert_eq!(
				Responses::response((H256::from_low_u64_be(1), request_id.clone())),
				resp_number.clone()
			);

			// // setting pkb type
			let resp_pkb = Response::PreKeyBundles(vec![(H256::from_low_u64_be(1), 2, b"test data".to_vec())]);
			Responses::set_response(H256::from_low_u64_be(1), request_id.clone(), resp_pkb.clone());
			assert_eq!(
				Responses::response((H256::from_low_u64_be(1), request_id.clone())),
				resp_pkb.clone()
			);

			// // remove response
			assert_ok!(Responses::remove_response(
				Origin::signed(H256::from_low_u64_be(1)),
				request_id.clone()
			));
			assert_eq!(
				Responses::response((H256::from_low_u64_be(1), request_id.clone())),
				Response::None
			);
		});
	}
}
