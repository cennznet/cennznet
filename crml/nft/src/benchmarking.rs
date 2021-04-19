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

//! NFT benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_system::RawOrigin;
use sp_runtime::Permill;

use crate::types::MAX_ENTITLEMENTS;
use crate::{Module as Nft, MAX_COLLECTION_ID_LENGTH};

// Create a collection for benchmarking
// Returns the collection id, schema, and royalties schedule
fn setup_collection<T: Trait>(creator: T::AccountId) -> (CollectionId, NFTSchema, RoyaltiesSchedule<T::AccountId>) {
	// Collection id with max length
	let collection_id = [1_u8; MAX_COLLECTION_ID_LENGTH as usize].to_vec();

	// Schema with max. attributes
	let schema = (0..MAX_SCHEMA_FIELDS)
		.map(|i| {
			let mut v = b"probably-a-decent-upper-length-for-an-attribte-name".to_vec();
			v.push(i as u8);
			(v, NFTAttributeValue::String(Default::default()).type_id())
		})
		.collect::<NFTSchema>();

	// Royalties with max. entitled addresses
	let royalties = RoyaltiesSchedule::<T::AccountId> {
		entitlements: (0..MAX_ENTITLEMENTS)
			.map(|_| (creator.clone(), Permill::from_percent(1)))
			.collect::<Vec<(T::AccountId, Permill)>>(),
	};

	return (collection_id, schema, royalties);
}

benchmarks! {
	_{}

	create_collection {
		let creator: T::AccountId = account("creator", 0, 0);
		let (collection_id, schema, royalties) = setup_collection::<T>(creator.clone());

	}: _(RawOrigin::Signed(creator.clone()), collection_id.clone(), schema, Some(royalties))
	verify {
		assert_eq!(<Nft<T>>::collection_owner(&collection_id), Some(creator));
	}

	create_token {
		let creator: T::AccountId = whitelisted_caller();
		let owner: T::AccountId = account("owner", 0, 0);

		let (collection_id, schema, royalties) = setup_collection::<T>(creator.clone());
		let _ = <Nft<T>>::create_collection(RawOrigin::Signed(creator.clone()).into(), collection_id.clone(), schema.clone(), Some(royalties.clone())).expect("created collection");
		// all attributes max. length
		let attributes = schema.iter().map(|_| NFTAttributeValue::String([1_u8; 140_usize].to_vec())).collect::<Vec<NFTAttributeValue>>();

	}: _(RawOrigin::Signed(creator.clone()), collection_id.clone(), owner.clone(), attributes, Some(royalties))
	verify {
		assert_eq!(<Nft<T>>::token_owner(&collection_id, T::TokenId::from(0_u32)), owner);
	}

	transfer {
		let creator: T::AccountId = whitelisted_caller();
		let owner: T::AccountId = account("owner", 0, 0);
		let new_owner: T::AccountId = account("new_owner", 0, 0);

		let (collection_id, schema, royalties) = setup_collection::<T>(creator.clone());
		let _ = <Nft<T>>::create_collection(RawOrigin::Signed(creator.clone()).into(), collection_id.clone(), schema.clone(), Some(royalties.clone())).expect("created collection");
		// all attributes max. length
		let attributes = schema.iter().map(|_| NFTAttributeValue::String([1_u8; 140_usize].to_vec())).collect::<Vec<NFTAttributeValue>>();
		let _ = <Nft<T>>::create_token(RawOrigin::Signed(creator.clone()).into(), collection_id.clone(), owner.clone(), attributes.clone(), Some(royalties)).expect("created token");
		let token_id = T::TokenId::from(0_u32);

	}: _(RawOrigin::Signed(owner.clone()), collection_id.clone(), token_id, new_owner.clone())
	verify {
		assert_eq!(<Nft<T>>::token_owner(&collection_id, token_id), new_owner);
	}

	burn {
		let creator: T::AccountId = whitelisted_caller();
		let owner: T::AccountId = account("owner", 0, 0);

		let (collection_id, schema, royalties) = setup_collection::<T>(creator.clone());
		let _ = <Nft<T>>::create_collection(RawOrigin::Signed(creator.clone()).into(), collection_id.clone(), schema.clone(), Some(royalties.clone())).expect("created collection");
		// all attributes max. length
		let attributes = schema.iter().map(|_| NFTAttributeValue::String([1_u8; 140_usize].to_vec())).collect::<Vec<NFTAttributeValue>>();
		let _ = <Nft<T>>::create_token(RawOrigin::Signed(creator.clone()).into(), collection_id.clone(), owner.clone(), attributes.clone(), Some(royalties)).expect("created token");
		let token_id = T::TokenId::from(0_u32);

	}: _(RawOrigin::Signed(owner.clone()), collection_id.clone(), token_id)
	verify {
		assert_eq!(<Nft<T>>::tokens_burnt(&collection_id), 1_u32.into());
	}

	// direct sell
	// direct buy
	// auction
	// bid
	// cancel auction
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{ExtBuilder, Test};
	use frame_support::assert_ok;

	#[test]
	fn create_collection() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_create_collection::<Test>());
		});
	}

	#[test]
	fn create_token() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_create_token::<Test>());
		});
	}

	#[test]
	fn transfer() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_transfer::<Test>());
		});
	}

	#[test]
	fn burn() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_burn::<Test>());
		});
	}
}
