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
use frame_system::{Module as System, RawOrigin};
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
	let _ = <Nft<T>>::create_token(
		RawOrigin::Signed(creator.clone()).into(),
		collection_id.clone(),
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
		let _ = <Nft<T>>::create_collection(RawOrigin::Signed(creator.clone()).into(), collection_id.clone(), schema.clone(), None, Some(royalties.clone())).expect("created collection");
		// all attributes max. length
		let attributes = schema.iter().map(|_| NFTAttributeValue::String([1_u8; 140_usize].to_vec())).collect::<Vec<NFTAttributeValue>>();

	}: _(RawOrigin::Signed(creator.clone()), collection_id.clone(), owner.clone(), attributes, Some(royalties))
	verify {
		assert_eq!(<Nft<T>>::token_owner(&collection_id, T::TokenId::from(0_u32)), owner);
	}

	transfer {
		let owner: T::AccountId = account("owner", 0, 0);
		let collection_id = setup_token::<T>(owner.clone());
		let new_owner: T::AccountId = account("new_owner", 0, 0);
		let token_id = T::TokenId::from(0_u32);
		// Add some tokens to stress test the ownership transfer process
		for fake_token_id in 1..1000_u32 {
			<TokenOwner<T>>::insert(&collection_id, T::TokenId::from(fake_token_id), owner.clone());
			<TokenOwner<T>>::insert(&collection_id, T::TokenId::from(1000 + fake_token_id), new_owner.clone());
		}

	}: _(RawOrigin::Signed(owner.clone()), collection_id.clone(), token_id, new_owner.clone())
	verify {
		assert_eq!(<Nft<T>>::token_owner(&collection_id, token_id), new_owner);
	}

	burn {
		let owner: T::AccountId = account("owner", 0, 0);
		let collection_id = setup_token::<T>(owner.clone());
		let token_id = T::TokenId::from(0_u32);
		// Add some tokens to stress test the ownership removal process
		for fake_token_id in 1..1000_u32 {
			<TokenOwner<T>>::insert(&collection_id, T::TokenId::from(fake_token_id), owner.clone());
		}

	}: _(RawOrigin::Signed(owner.clone()), collection_id.clone(), token_id)
	verify {
		assert!(<Nft<T>>::token_attributes(&collection_id, token_id).is_empty());
	}

	sell {
		let owner: T::AccountId = account("owner", 0, 0);
		let collection_id = setup_token::<T>(owner.clone());
		let token_id = T::TokenId::from(0_u32);
		let payment_asset = 16_000;
		let price = 1_000_000 * 10_000; // 1 million 4dp asset

	}: _(RawOrigin::Signed(owner.clone()), collection_id.clone(), token_id, Some(owner.clone()), payment_asset, price, Some(T::BlockNumber::from(1_u32)))
	verify {
		assert!(<Nft<T>>::listings(&collection_id, token_id).is_some());
	}

	buy {
		let owner: T::AccountId = account("owner", 0, 0);
		let buyer: T::AccountId = account("buyer", 0, 0);
		let collection_id = setup_token::<T>(owner.clone());
		let token_id = T::TokenId::from(0_u32);
		let payment_asset = 16_000;
		let price = 1_000_000 * 10_000; // 1 million 4dp asset
		let _ = T::MultiCurrency::deposit_creating(&buyer, Some(payment_asset), price);
		let _ = <Nft<T>>::sell(RawOrigin::Signed(owner.clone()).into(), collection_id.clone(), token_id, Some(buyer.clone()), payment_asset, price, None).expect("listed ok");

		// Add some tokens to stress test the ownership transfer process
		for fake_token_id in 1..1000_u32 {
			<TokenOwner<T>>::insert(&collection_id, T::TokenId::from(fake_token_id), owner.clone());
			<TokenOwner<T>>::insert(&collection_id, T::TokenId::from(1000 + fake_token_id), buyer.clone());
		}

	}: _(RawOrigin::Signed(buyer.clone()), collection_id.clone(), token_id)
	verify {
		assert_eq!(<Nft<T>>::token_owner(&collection_id, token_id), buyer);
	}

	auction {
		let owner: T::AccountId = account("owner", 0, 0);
		let collection_id = setup_token::<T>(owner.clone());
		let token_id = T::TokenId::from(0_u32);
		let payment_asset = 16_000;
		let reserve_price = 1_000_000 * 10_000; // 1 million 4dp asset
		let duration = T::BlockNumber::from(100_u32);

		// stress test add
		let fake_listing: (CollectionId, T::TokenId) = (b"some-collection-id".to_vec(), T::TokenId::from(1_u32));
		for i in 0..1000_u32 {
			ListingEndSchedule::<T>::insert(<System<T>>::block_number() + duration, (fake_listing.0.clone(), T::TokenId::from(i)), ());
		}

	}: _(RawOrigin::Signed(owner.clone()), collection_id.clone(), token_id, payment_asset, reserve_price, Some(duration))
	verify {
		assert!(<Nft<T>>::listings(&collection_id, token_id).is_some());
	}

	bid {
		let owner: T::AccountId = account("owner", 0, 0);
		let buyer: T::AccountId = account("buyer", 0, 0);

		let collection_id = setup_token::<T>(owner.clone());
		let token_id = T::TokenId::from(0_u32);
		let payment_asset = 16_000;
		let reserve_price = 1_000_000 * 10_000; // 1 million 4dp asset
		let duration = T::BlockNumber::from(100_u32);

		let _ = T::MultiCurrency::deposit_creating(&owner, Some(payment_asset), reserve_price);
		let _ = T::MultiCurrency::deposit_creating(&buyer, Some(payment_asset), reserve_price + 1);
		let _ = <Nft<T>>::auction(RawOrigin::Signed(owner.clone()).into(), collection_id.clone(), token_id, payment_asset, reserve_price, Some(duration)).expect("listed ok");
		// worst case path is to replace an existing bid
		let _ = <Nft<T>>::bid(RawOrigin::Signed(owner.clone()).into(), collection_id.clone(), token_id, reserve_price);

	}: _(RawOrigin::Signed(buyer.clone()), collection_id.clone(), token_id, reserve_price + 1)
	verify {
		assert_eq!(<Nft<T>>::listing_winning_bid(&collection_id, token_id), Some((buyer.clone(), reserve_price + 1)));
	}

	cancel_sale {
		let owner: T::AccountId = account("owner", 0, 0);
		let collection_id = setup_token::<T>(owner.clone());
		let token_id = T::TokenId::from(0_u32);
		let payment_asset = 16_000;
		let reserve_price = 1_000_000 * 10_000; // 1 million 4dp asset
		let duration = T::BlockNumber::from(100_u32);
		// Add some listing data to stress test the listing removal process
		let _ = <Nft<T>>::auction(RawOrigin::Signed(owner.clone()).into(), collection_id.clone(), token_id, payment_asset, reserve_price, Some(duration)).expect("listed ok");

		// stress test removal
		let fake_listing: (CollectionId, T::TokenId) = (b"some-collection-id".to_vec(), T::TokenId::from(1_u32));
		for i in 0..1000_u32 {
			ListingEndSchedule::<T>::insert(<System<T>>::block_number() + duration, (fake_listing.0.clone(), T::TokenId::from(i)), ());
		}

	}: _(RawOrigin::Signed(owner.clone()), collection_id.clone(), token_id)
	verify {
		assert!(<Nft<T>>::listings(&collection_id, token_id).is_none());
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
