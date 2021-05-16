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

/// payment asset
const PAYMENT_ASSET: u32 = 16_000;
/// sale price, 1 million 4dp asset
const PRICE: u128 = 1_000_000 * 10_000;
/// QUANTITY
const QUANTITY: u32 = 1_000_000;

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

// Create a token for benchmarking
fn setup_token<T: Trait>(owner: T::AccountId) -> CollectionId {
	let creator: T::AccountId = whitelisted_caller();
	let (collection_id, schema, royalties) = setup_collection::<T>(creator.clone());
	let _ = <Nft<T>>::create_collection(
		RawOrigin::Signed(creator.clone()).into(),
		collection_id.clone(),
		schema.clone(),
		Some(b"https://example.com/nfts/more/paths/thatmakethisunreasonablylong/tostresstestthis".to_vec()),
		Some(royalties.clone()),
	)
	.expect("created collection");
	assert_eq!(140, T::MaxAttributeLength::get() as usize);
	// all attributes max. length
	let attributes = schema
		.iter()
		.map(|_| NFTAttributeValue::String([1_u8; 140].to_vec()))
		.collect::<Vec<NFTAttributeValue>>();
	let _ = <Nft<T>>::batch_create_token(
		RawOrigin::Signed(creator.clone()).into(),
		collection_id.clone(),
		QUANTITY, // QUANTITY
		owner.clone(),
		attributes.clone(),
		Some(royalties),
	)
	.expect("created token");

	return collection_id;
}

benchmarks! {
	_{}

	set_owner {
		let creator: T::AccountId = account("creator", 0, 0);
		let new_owner: T::AccountId = account("new_owner", 0, 0);
		let (collection_id, schema, royalties) = setup_collection::<T>(creator.clone());
		let _ = <Nft<T>>::create_collection(RawOrigin::Signed(creator.clone()).into(), collection_id.clone(), schema.clone(), None, None).expect("created collection");

	}: _(RawOrigin::Signed(creator.clone()), collection_id.clone(), new_owner.clone())
	verify {
		assert_eq!(<Nft<T>>::collection_owner(&collection_id), Some(new_owner));
	}

	create_collection {
		let creator: T::AccountId = account("creator", 0, 0);
		let (collection_id, schema, royalties) = setup_collection::<T>(creator.clone());

	}: _(RawOrigin::Signed(creator.clone()), collection_id.clone(), schema, Some(b"https://example.com/nfts/more/paths/thatmakethisunreasonablylong/tostresstestthis".to_vec()), Some(royalties))
	verify {
		assert_eq!(<Nft<T>>::collection_owner(&collection_id), Some(creator));
	}

	create_token {
		let creator: T::AccountId = whitelisted_caller();
		let owner: T::AccountId = account("owner", 0, 0);

		let (collection_id, schema, royalties) = setup_collection::<T>(creator.clone());
		let token_id = generate_token_id::<T>(&collection_id, 0);
		let _ = <Nft<T>>::create_collection(RawOrigin::Signed(creator.clone()).into(), collection_id.clone(), schema.clone(), None, Some(royalties.clone())).expect("created collection");
		// all attributes max. length
		let attributes = schema.iter().map(|_| NFTAttributeValue::String([1_u8; 140_usize].to_vec())).collect::<Vec<NFTAttributeValue>>();

	}: _(RawOrigin::Signed(creator.clone()), collection_id.clone(), owner.clone(), attributes, Some(royalties))
	verify {
		assert_eq!(<Nft<T>>::balance_of(token_id, owner), 1);
	}

	transfer {
		let owner: T::AccountId = account("owner", 0, 0);
		let collection_id = setup_token::<T>(owner.clone());
		let new_owner: T::AccountId = account("new_owner", 0, 0);
		let token_id = generate_token_id::<T>(&collection_id, 0);

	}: _(RawOrigin::Signed(owner.clone()), token_id, new_owner.clone())
	verify {
		assert_eq!(<Nft<T>>::balance_of(token_id, new_owner), 1);
	}

	burn {
		let owner: T::AccountId = account("owner", 0, 0);
		let collection_id = setup_token::<T>(owner.clone());
		let token_id = generate_token_id::<T>(&collection_id, 0);

	}: _(RawOrigin::Signed(owner.clone()), token_id, QUANTITY)
	verify {
		assert!(<Nft<T>>::token_attributes(token_id).is_empty());
	}

	sell {
		let owner: T::AccountId = account("owner", 0, 0);
		let collection_id = setup_token::<T>(owner.clone());
		let token_id = generate_token_id::<T>(&collection_id, 0);
		let listing_id = <Nft<T>>::next_listing_id();

	}: _(RawOrigin::Signed(owner.clone()), token_id, QUANTITY, Some(owner.clone()), PAYMENT_ASSET, PRICE, Some(T::BlockNumber::from(1_u32)))
	verify {
		assert!(<Nft<T>>::listings(listing_id).is_some());
	}

	buy {
		let owner: T::AccountId = account("owner", 0, 0);
		let buyer: T::AccountId = account("buyer", 0, 0);
		let collection_id = setup_token::<T>(owner.clone());
		let token_id = generate_token_id::<T>(&collection_id, 0);
		let _ = T::MultiCurrency::deposit_creating(&buyer, Some(PAYMENT_ASSET), PRICE);
		let listing_id = <Nft<T>>::next_listing_id();
		let _ = <Nft<T>>::sell(RawOrigin::Signed(owner.clone()).into(), token_id, QUANTITY, Some(buyer.clone()), PAYMENT_ASSET, PRICE, None).expect("listed ok");

	}: _(RawOrigin::Signed(buyer.clone()), listing_id)
	verify {
		assert_eq!(<Nft<T>>::balance_of(&token_id, buyer), QUANTITY);
	}

	auction {
		let owner: T::AccountId = account("owner", 0, 0);
		let collection_id = setup_token::<T>(owner.clone());
		let token_id = generate_token_id::<T>(&collection_id, 0);
		let duration = T::BlockNumber::from(100_u32);
		let listing_id = <Nft<T>>::next_listing_id();

	}: _(RawOrigin::Signed(owner.clone()), token_id, QUANTITY, PAYMENT_ASSET, PRICE, Some(duration))
	verify {
		assert!(<Nft<T>>::listings(listing_id).is_some());
	}

	bid {
		let owner: T::AccountId = account("owner", 0, 0);
		let buyer: T::AccountId = account("buyer", 0, 0);

		let collection_id = setup_token::<T>(owner.clone());
		let token_id = generate_token_id::<T>(&collection_id, 0);
		let duration = T::BlockNumber::from(100_u32);

		let _ = T::MultiCurrency::deposit_creating(&owner, Some(PAYMENT_ASSET), PRICE);
		let _ = T::MultiCurrency::deposit_creating(&buyer, Some(PAYMENT_ASSET), PRICE + 1);
		let listing_id = <Nft<T>>::next_listing_id();
		let _ = <Nft<T>>::auction(RawOrigin::Signed(owner.clone()).into(), token_id, QUANTITY, PAYMENT_ASSET, PRICE, Some(duration)).expect("listed ok");
		// worst case path is to replace an existing bid
		let _ = <Nft<T>>::bid(RawOrigin::Signed(owner.clone()).into(), listing_id, PRICE);

	}: _(RawOrigin::Signed(buyer.clone()), listing_id, PRICE + 1)
	verify {
		assert_eq!(<Nft<T>>::listing_winning_bid(listing_id), Some((buyer.clone(), PRICE + 1)));
	}

	cancel_sale {
		let owner: T::AccountId = account("owner", 0, 0);
		let collection_id = setup_token::<T>(owner.clone());
		let token_id = generate_token_id::<T>(&collection_id, 0);
		let duration = T::BlockNumber::from(100_u32);
		let listing_id = <Nft<T>>::next_listing_id();
		let _ = <Nft<T>>::auction(RawOrigin::Signed(owner.clone()).into(), token_id, QUANTITY, PAYMENT_ASSET, PRICE, Some(duration)).expect("listed ok");

	}: _(RawOrigin::Signed(owner.clone()), listing_id)
	verify {
		assert!(<Nft<T>>::listings(listing_id).is_none());
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{ExtBuilder, Test};
	use frame_support::assert_ok;

	#[test]
	fn set_owner() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_set_owner::<Test>());
		});
	}

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

	#[test]
	fn sell() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_sell::<Test>());
		});
	}

	#[test]
	fn buy() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_buy::<Test>());
		});
	}

	#[test]
	fn auction() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_auction::<Test>());
		});
	}

	#[test]
	fn bid() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_bid::<Test>());
		});
	}

	#[test]
	fn cancel_sale() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_cancel_sale::<Test>());
		});
	}
}
