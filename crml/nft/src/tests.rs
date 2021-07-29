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

use super::*;
use crate::mock::{AccountId, Event, ExtBuilder, GenericAsset, Nft, System, Test};
use frame_support::{assert_noop, assert_ok, traits::OnInitialize};
use sp_runtime::Permill;

/// The asset Id used for payment in these tests
const PAYMENT_ASSET: AssetId = 16_001;

// Check the test system contains an event record `event`
fn has_event(
	event: RawEvent<
		CollectionId,
		AccountId,
		AssetId,
		Balance,
		AuctionClosureReason,
		SeriesId,
		SerialNumber,
		TokenCount,
		CollectionNameType,
	>,
) -> bool {
	System::events()
		.iter()
		.find(|e| e.event == Event::crml_nft(event.clone()))
		.is_some()
}

/// Generate the first `TokenId` for a collection's first series
fn first_token_id(collection_id: CollectionId) -> TokenId {
	(collection_id, 0, 0)
}

// Create an NFT collection
// Returns the created `collection_id`
fn setup_collection(owner: AccountId) -> CollectionId {
	let collection_id = Nft::next_collection_id();
	let collection_name = b"test-collection".to_vec();
	assert_ok!(Nft::create_collection(
		Some(owner).into(),
		collection_name,
		Some(MetadataBaseURI::Https(b"example.com/metadata".to_vec())),
		None,
	));

	collection_id
}

/// Setup a token, return collection id, token id, token owner
fn setup_token() -> (CollectionId, TokenId, <Test as frame_system::Config>::AccountId) {
	let collection_owner = 1_u64;
	let collection_id = setup_collection(collection_owner);
	let token_owner = 2_u64;
	let token_id = first_token_id(collection_id);
	assert_ok!(Nft::mint_unique(
		Some(collection_owner).into(),
		collection_id,
		Some(token_owner),
		vec![NFTAttributeValue::I32(500)],
		None,
		None,
	));

	(collection_id, token_id, token_owner)
}

/// Setup a token, return collection id, token id, token owner
fn setup_token_with_royalties(
	royalties_schedule: RoyaltiesSchedule<AccountId>,
	quantity: TokenCount,
) -> (CollectionId, TokenId, <Test as frame_system::Config>::AccountId) {
	let collection_owner = 1_u64;
	let collection_id = setup_collection(collection_owner);
	<SeriesRoyalties<Test>>::insert(collection_id, 0, royalties_schedule);

	let token_owner = 2_u64;
	let token_id = first_token_id(collection_id);
	assert_ok!(Nft::mint_series(
		Some(collection_owner).into(),
		collection_id,
		quantity,
		Some(token_owner),
		vec![NFTAttributeValue::I32(500)],
		None,
		None,
	));

	(collection_id, token_id, token_owner)
}

#[test]
fn set_owner() {
	ExtBuilder::default().build().execute_with(|| {
		// setup token collection + one token
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner);
		let new_owner = 2_u64;

		assert_ok!(Nft::set_owner(Some(collection_owner).into(), collection_id, new_owner));
		assert_noop!(
			Nft::set_owner(Some(collection_owner).into(), collection_id, new_owner),
			Error::<Test>::NoPermission
		);
		assert_noop!(
			Nft::set_owner(Some(collection_owner).into(), collection_id + 1, new_owner),
			Error::<Test>::NoCollection
		);
	});
}

#[test]
fn create_collection() {
	ExtBuilder::default().build().execute_with(|| {
		let owner = 1_u64;
		let collection_id = setup_collection(owner);
		let name = b"test-collection".to_vec();
		assert!(has_event(RawEvent::CreateCollection(
			collection_id,
			name.clone(),
			owner
		)));

		assert_eq!(Nft::collection_owner(collection_id).expect("owner should exist"), owner);
		assert_eq!(Nft::collection_name(collection_id), name);
		assert_eq!(
			Nft::collection_metadata_uri(collection_id),
			Some(MetadataBaseURI::Https(b"example.com/metadata".to_vec()))
		);
		assert_eq!(Nft::collection_royalties(collection_id), None);
		assert_eq!(Nft::next_collection_id(), collection_id + 1);
	});
}

#[test]
fn create_collection_invalid_name() {
	ExtBuilder::default().build().execute_with(|| {
		// too long
		let bad_collection_name = b"someidentifierthatismuchlongerthanthe32bytelimitsoshouldfail".to_vec();
		assert_noop!(
			Nft::create_collection(Some(1_u64).into(), bad_collection_name, None, None),
			Error::<Test>::CollectionNameInvalid
		);

		// empty name
		assert_noop!(
			Nft::create_collection(Some(1_u64).into(), vec![], None, None),
			Error::<Test>::CollectionNameInvalid
		);

		// non UTF-8 chars
		// kudos: https://www.cl.cam.ac.uk/~mgk25/ucs/examples/UTF-8-test.txt
		let bad_collection_name = vec![0xfe, 0xff];
		assert_noop!(
			Nft::create_collection(Some(1_u64).into(), bad_collection_name, None, None),
			Error::<Test>::CollectionNameInvalid
		);
	});
}

#[test]
fn create_collection_royalties_invalid() {
	ExtBuilder::default().build().execute_with(|| {
		let owner = 1_u64;

		//Too big royalties should fail
		assert_noop!(
			Nft::create_collection(
				Some(owner).into(),
				b"test-collection".to_vec(),
				None,
				Some(RoyaltiesSchedule::<AccountId> {
					entitlements: vec![
						(3_u64, Permill::from_fraction(1.2)),
						(4_u64, Permill::from_fraction(3.3))
					]
				}),
			),
			Error::<Test>::RoyaltiesInvalid
		);

		//Empty vector should fail
		assert_noop!(
			Nft::create_collection(
				Some(owner).into(),
				b"test-collection".to_vec(),
				None,
				Some(RoyaltiesSchedule::<AccountId> {
					entitlements: vec![]
				}),
			),
			Error::<Test>::RoyaltiesInvalid
		);
	})
}

#[test]
fn mint_unique() {
	ExtBuilder::default().build().execute_with(|| {
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner);
		let token_id = first_token_id(collection_id);

		let token_owner = 2_u64;
		let series_id = 0;
		assert_eq!(Nft::next_series_id(collection_id), series_id);
		assert_ok!(Nft::mint_unique(
			Some(collection_owner).into(),
			collection_id,
			Some(token_owner),
			vec![
				NFTAttributeValue::I32(-33),
				NFTAttributeValue::U8(0),
				NFTAttributeValue::Bytes32([1_u8; 32])
			],
			None,
			None,
		));
		assert!(has_event(RawEvent::CreateToken(collection_id, token_id, token_owner,)));

		let token_attributes = Nft::series_attributes(collection_id, series_id);
		assert_eq!(
			token_attributes,
			vec![
				NFTAttributeValue::I32(-33),
				NFTAttributeValue::U8(Default::default()),
				NFTAttributeValue::Bytes32([1_u8; 32])
			],
		);

		assert_eq!(Nft::collected_tokens(collection_id, &token_owner), vec![token_id]);
		assert_eq!(Nft::token_owner((collection_id, series_id), 0), token_owner);
		assert!(Nft::is_single_issue(collection_id, series_id));
		assert_eq!(Nft::next_serial_number(collection_id, series_id), 1);
		assert_eq!(Nft::next_series_id(collection_id), series_id + 1);
		assert_eq!(Nft::series_issuance(collection_id, series_id), 1);
	});
}

#[test]
fn create_multiple_unique_tokens() {
	ExtBuilder::default().build().execute_with(|| {
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner);
		let token_owner = 2_u64;
		let metadata_uri = b"/series/test/";

		let token_1 = first_token_id(collection_id);
		let token_2 = (collection_id, 1 as SeriesId, 0 as SerialNumber);

		assert_ok!(Nft::mint_unique(
			Some(collection_owner).into(),
			collection_id,
			Some(token_owner),
			vec![
				NFTAttributeValue::I32(-33),
				NFTAttributeValue::U8(0),
				NFTAttributeValue::Bytes32([1_u8; 32])
			],
			None,
			None,
		));

		assert_ok!(Nft::mint_unique(
			Some(collection_owner).into(),
			collection_id,
			Some(token_owner),
			vec![
				NFTAttributeValue::I32(33),
				NFTAttributeValue::U8(0),
				NFTAttributeValue::String(String::from("💎🙌").as_bytes().to_vec())
			],
			Some(metadata_uri.to_vec()),
			None,
		));
		assert!(has_event(RawEvent::CreateToken(collection_id, token_2, token_owner,)));

		assert_eq!(Nft::series_metadata_uri(collection_id, 1), Some(metadata_uri.to_vec()));
		assert_eq!(
			Nft::collected_tokens(collection_id, &token_owner),
			vec![token_1, token_2]
		);
		assert_eq!(Nft::next_series_id(collection_id), 2);
	});
}

#[test]
fn mint_unique_fails_prechecks() {
	ExtBuilder::default().build().execute_with(|| {
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner);

		assert_noop!(
			Nft::mint_unique(
				Some(2_u64).into(),
				collection_id,
				Some(collection_owner),
				Default::default(),
				None,
				None,
			),
			Error::<Test>::NoPermission
		);

		assert_noop!(
			Nft::mint_unique(
				Some(collection_owner).into(),
				collection_id + 1, // collection doesn't exist
				None,
				Default::default(),
				None,
				None,
			),
			Error::<Test>::NoCollection
		);

		let too_many_attributes = vec![NFTAttributeValue::U8(0).clone(); (MAX_SCHEMA_FIELDS + 1_u32) as usize];
		assert_noop!(
			Nft::mint_unique(
				Some(collection_owner).into(),
				collection_id,
				None,
				too_many_attributes,
				None,
				None,
			),
			Error::<Test>::SchemaMaxAttributes
		);

		// attribute value too long
		assert_noop!(
			Nft::mint_unique(
				Some(collection_owner).into(),
				collection_id,
				None,
				vec![
					NFTAttributeValue::I32(0),
					NFTAttributeValue::Url([1_u8; <Test as Config>::MaxAttributeLength::get() as usize + 1].to_vec())
				],
				None,
				None,
			),
			Error::<Test>::MaxAttributeLength
		);
	});
}

#[test]
fn transfer() {
	ExtBuilder::default().build().execute_with(|| {
		// setup token collection + one token
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner);
		let token_owner = 2_u64;
		let token_id = first_token_id(collection_id);
		assert_ok!(Nft::mint_unique(
			Some(collection_owner).into(),
			collection_id,
			Some(token_owner),
			vec![NFTAttributeValue::I32(500)],
			None,
			None,
		));

		// test
		let new_owner = 3_u64;
		assert_ok!(Nft::transfer(Some(token_owner).into(), token_id, new_owner,));
		assert!(has_event(RawEvent::Transfer(token_owner, vec![token_id], new_owner)));

		assert!(Nft::collected_tokens(collection_id, &token_owner).is_empty());
		assert_eq!(Nft::collected_tokens(collection_id, &new_owner), vec![token_id]);
	});
}

#[test]
fn transfer_fails_prechecks() {
	ExtBuilder::default().build().execute_with(|| {
		// setup token collection + one token
		let collection_owner = 1_u64;

		let collection_id = setup_collection(collection_owner);
		let token_owner = 2_u64;
		let token_id = first_token_id(collection_id);

		// no token yet
		assert_noop!(
			Nft::transfer(Some(token_owner).into(), token_id, token_owner),
			Error::<Test>::NoPermission,
		);

		assert_ok!(Nft::mint_unique(
			Some(collection_owner).into(),
			collection_id,
			Some(token_owner),
			vec![NFTAttributeValue::I32(500)],
			None,
			None,
		));

		let not_the_owner = 3_u64;
		assert_noop!(
			Nft::transfer(Some(not_the_owner).into(), token_id, not_the_owner),
			Error::<Test>::NoPermission,
		);

		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			token_id,
			Some(5),
			PAYMENT_ASSET,
			1_000,
			None,
		));

		// cannot transfer while listed
		assert_noop!(
			Nft::transfer(Some(token_owner).into(), token_id, token_owner),
			Error::<Test>::TokenListingProtection,
		);
	});
}

#[test]
fn burn() {
	ExtBuilder::default().build().execute_with(|| {
		// setup token collection + one token
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner);
		let token_owner = 2_u64;
		let token_id = first_token_id(collection_id);
		let series_id = 0;

		assert_ok!(Nft::mint_series(
			Some(collection_owner).into(),
			collection_id,
			3,
			Some(token_owner),
			vec![NFTAttributeValue::I32(500)],
			None,
			Some(RoyaltiesSchedule::<AccountId>::default()),
		));

		// test
		assert_ok!(Nft::burn(Some(token_owner).into(), token_id));
		assert!(has_event(RawEvent::Burn(collection_id, series_id, vec![0])));

		assert_ok!(Nft::burn_batch(
			Some(token_owner).into(),
			collection_id,
			series_id,
			vec![1, 2]
		));
		assert!(has_event(RawEvent::Burn(collection_id, series_id, vec![1, 2])));

		assert!(!SeriesIssuance::contains_key(collection_id, series_id));
		assert!(!<SeriesRoyalties<Test>>::contains_key(collection_id, series_id));
		assert!(!SeriesMetadataURI::contains_key(collection_id, series_id));
		assert!(!SeriesAttributes::contains_key(collection_id, series_id));
		assert!(!<TokenOwner<Test>>::contains_key((collection_id, series_id), 0));
		assert!(!<TokenOwner<Test>>::contains_key((collection_id, series_id), 1));
		assert!(!<TokenOwner<Test>>::contains_key((collection_id, series_id), 2));
		assert!(Nft::collected_tokens(collection_id, &token_owner).is_empty());
	});
}

#[test]
fn burn_fails_prechecks() {
	ExtBuilder::default().build().execute_with(|| {
		// setup token collection + one token
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner);
		let series_id = Nft::next_series_id(collection_id);
		let token_owner = 2_u64;

		// token doesn't exist yet
		assert_noop!(
			Nft::burn_batch(Some(token_owner).into(), collection_id, series_id, vec![0]),
			Error::<Test>::NoPermission
		);
		// token empty
		assert_noop!(
			Nft::burn_batch(Some(token_owner).into(), collection_id, series_id, vec![]),
			Error::<Test>::NoToken
		);

		assert_ok!(Nft::mint_series(
			Some(collection_owner).into(),
			collection_id,
			100,
			Some(token_owner),
			vec![NFTAttributeValue::I32(500)],
			None,
			None,
		));

		// Not owner
		assert_noop!(
			Nft::burn_batch(Some(token_owner + 1).into(), collection_id, series_id, vec![0]),
			Error::<Test>::NoPermission,
		);

		// Fails with duplicate serials
		assert_noop!(
			Nft::burn_batch(Some(token_owner).into(), collection_id, series_id, vec![0, 1, 1]),
			Error::<Test>::NoPermission,
		);

		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			first_token_id(collection_id),
			None,
			PAYMENT_ASSET,
			1_000,
			None,
		));
		// cannot burn_batch while listed
		assert_noop!(
			Nft::burn_batch(Some(token_owner).into(), collection_id, series_id, vec![0]),
			Error::<Test>::TokenListingProtection,
		);
	});
}

#[test]
fn sell_bundle() {
	ExtBuilder::default().build().execute_with(|| {
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner);
		let quantity = 5;

		assert_ok!(Nft::mint_series(
			Some(collection_owner).into(),
			collection_id,
			quantity,
			None,
			vec![NFTAttributeValue::I32(123)],
			None,
			None,
		));

		let tokens = vec![(collection_id, 0, 1), (collection_id, 0, 3), (collection_id, 0, 4)];
		let listing_id = Nft::next_listing_id();

		assert_ok!(Nft::sell_bundle(
			Some(collection_owner).into(),
			tokens.clone(),
			None,
			PAYMENT_ASSET,
			1_000,
			None,
		));

		for token in tokens.iter() {
			assert!(Nft::token_locks(token));
		}

		let buyer = 3;
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&buyer, PAYMENT_ASSET, 1_000);
		assert_ok!(Nft::buy(Some(buyer).into(), listing_id));
		assert_eq!(Nft::collected_tokens(collection_id, &buyer), tokens);
	})
}

#[test]
fn sell_bundle_fails() {
	ExtBuilder::default().build().execute_with(|| {
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner);
		let collection_id_2 = setup_collection(collection_owner);
		// mint some fake tokens
		<TokenOwner<Test>>::insert((collection_id, 1), 1, collection_owner);
		<TokenOwner<Test>>::insert((collection_id, 2), 2, collection_owner);
		<TokenOwner<Test>>::insert((collection_id_2, 1), 1, collection_owner);

		// empty tokens fails
		assert_noop!(
			Nft::sell_bundle(Some(collection_owner).into(), vec![], None, PAYMENT_ASSET, 1_000, None),
			Error::<Test>::NoToken
		);

		// cannot bundle sell tokens from different collections
		assert_noop!(
			Nft::sell_bundle(
				Some(collection_owner).into(),
				vec![(collection_id, 1, 1), (collection_id_2, 1, 1),],
				None,
				PAYMENT_ASSET,
				1_000,
				None,
			),
			Error::<Test>::MixedBundleSale
		);

		// cannot bundle sell when series have royalties set
		<SeriesRoyalties<Test>>::insert(collection_id, 1, RoyaltiesSchedule::<AccountId>::default());
		<SeriesRoyalties<Test>>::insert(collection_id, 2, RoyaltiesSchedule::<AccountId>::default());
		assert_noop!(
			Nft::sell_bundle(
				Some(collection_owner).into(),
				vec![(collection_id, 1, 1), (collection_id, 2, 2)],
				None,
				PAYMENT_ASSET,
				1_000,
				None,
			),
			Error::<Test>::RoyaltiesProtection
		);
	})
}

#[test]
fn sell() {
	ExtBuilder::default().build().execute_with(|| {
		let (collection_id, token_id, token_owner) = setup_token();
		let listing_id = Nft::next_listing_id();

		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			token_id,
			Some(5),
			PAYMENT_ASSET,
			1_000,
			None,
		));

		assert!(Nft::token_locks(&token_id));
		assert!(Nft::open_collection_listings(collection_id, listing_id));

		let expected = Listing::<Test>::FixedPrice(FixedPriceListing::<Test> {
			payment_asset: PAYMENT_ASSET,
			fixed_price: 1_000,
			close: System::block_number() + <Test as Config>::DefaultListingDuration::get(),
			buyer: Some(5),
			tokens: vec![token_id],
			seller: token_owner,
			royalties_schedule: Default::default(),
		});

		let listing = Nft::listings(listing_id).expect("token is listed");
		assert_eq!(listing, expected);

		// current block is 1 + duration
		assert!(Nft::listing_end_schedule(
			System::block_number() + <Test as Config>::DefaultListingDuration::get(),
			listing_id
		));

		// Can't transfer while listed for sale
		assert_noop!(
			Nft::transfer(Some(token_owner).into(), token_id, token_owner + 1),
			Error::<Test>::TokenListingProtection
		);

		assert!(has_event(RawEvent::FixedPriceSaleListed(collection_id, listing_id,)));
	});
}

#[test]
fn sell_fails() {
	ExtBuilder::default().build().execute_with(|| {
		let (_, token_id, token_owner) = setup_token();
		// Not token owner
		assert_noop!(
			Nft::sell(
				Some(token_owner + 1).into(),
				token_id,
				Some(5),
				PAYMENT_ASSET,
				1_000,
				None
			),
			Error::<Test>::NoPermission
		);

		// token listed already
		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			token_id,
			Some(5),
			PAYMENT_ASSET,
			1_000,
			None,
		));
		assert_noop!(
			Nft::sell(Some(token_owner).into(), token_id, Some(5), PAYMENT_ASSET, 1_000, None),
			Error::<Test>::TokenListingProtection
		);

		// can't auction, listed for fixed price sale
		assert_noop!(
			Nft::auction(Some(token_owner).into(), token_id, PAYMENT_ASSET, 1_000, None),
			Error::<Test>::TokenListingProtection
		);
	});
}

#[test]
fn cancel_sell() {
	ExtBuilder::default().build().execute_with(|| {
		let (collection_id, token_id, token_owner) = setup_token();
		let listing_id = Nft::next_listing_id();
		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			token_id,
			Some(5),
			PAYMENT_ASSET,
			1_000,
			None,
		));
		assert_ok!(Nft::cancel_sale(Some(token_owner).into(), listing_id));
		assert!(has_event(RawEvent::FixedPriceSaleClosed(collection_id, listing_id)));

		// storage cleared up
		assert!(Nft::listings(listing_id).is_none());
		assert!(!Nft::listing_end_schedule(
			System::block_number() + <Test as Config>::DefaultListingDuration::get(),
			listing_id
		));

		// it should be free to operate on the token
		assert_ok!(Nft::transfer(Some(token_owner).into(), token_id, token_owner + 1,));
	});
}

#[test]
fn sell_closes_on_schedule() {
	ExtBuilder::default().build().execute_with(|| {
		let (_, token_id, token_owner) = setup_token_with_royalties(RoyaltiesSchedule::default(), 1);
		let listing_duration = 100;
		let listing_id = Nft::next_listing_id();

		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			token_id,
			Some(5),
			PAYMENT_ASSET,
			1_000,
			Some(listing_duration),
		));

		// sale should close after the duration expires
		Nft::on_initialize(System::block_number() + listing_duration);

		// seller should have tokens
		assert!(Nft::listings(listing_id).is_none());
		assert!(!Nft::listing_end_schedule(
			System::block_number() + listing_duration,
			listing_id
		));

		// should be free to transfer now
		let new_owner = 8;
		assert_ok!(Nft::transfer(Some(token_owner).into(), token_id, new_owner,));
	});
}

#[test]
fn buy() {
	ExtBuilder::default().build().execute_with(|| {
		let (collection_id, token_id, token_owner) = setup_token();
		let buyer = 5;
		let payment_asset = PAYMENT_ASSET;
		let price = 1_000;
		let listing_id = Nft::next_listing_id();

		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			token_id,
			Some(buyer),
			payment_asset,
			price,
			None,
		));

		let _ = <Test as Config>::MultiCurrency::deposit_creating(&buyer, payment_asset, price);
		assert_ok!(Nft::buy(Some(buyer).into(), listing_id));
		// no royalties, all proceeds to token owner
		assert_eq!(GenericAsset::free_balance(payment_asset, &token_owner), price);

		// listing removed
		assert!(Nft::listings(listing_id).is_none());
		assert!(!Nft::listing_end_schedule(
			System::block_number() + <Test as Config>::DefaultListingDuration::get(),
			listing_id
		));

		// ownership changed
		assert!(!Nft::token_locks(&token_id));
		assert!(!Nft::open_collection_listings(collection_id, listing_id));
		assert_eq!(Nft::collected_tokens(collection_id, &buyer), vec![token_id]);
	});
}

#[test]
fn buy_with_royalties() {
	ExtBuilder::default().build().execute_with(|| {
		let collection_owner = 1;
		let beneficiary_1 = 11;
		let beneficiary_2 = 12;
		let royalties_schedule = RoyaltiesSchedule {
			entitlements: vec![
				(collection_owner, Permill::from_fraction(0.111)),
				(beneficiary_1, Permill::from_fraction(0.1111)),
				(beneficiary_2, Permill::from_fraction(0.3333)),
			],
		};
		let (collection_id, _, token_owner) = setup_token_with_royalties(royalties_schedule.clone(), 2);
		let buyer = 5;
		let payment_asset = PAYMENT_ASSET;
		let sale_price = 1_000_008;
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&buyer, payment_asset, sale_price * 2);
		let token_id = first_token_id(collection_id);

		let (collection_id, _, _) = token_id;
		CollectionRoyalties::<Test>::insert(collection_id, &royalties_schedule);
		let listing_id = Nft::next_listing_id();
		assert_eq!(listing_id, 0);
		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			token_id,
			Some(buyer),
			payment_asset,
			sale_price,
			None,
		));

		let initial_balance_owner = GenericAsset::free_balance(payment_asset, &collection_owner);
		let initial_balance_b1 = GenericAsset::free_balance(payment_asset, &beneficiary_1);
		let initial_balance_b2 = GenericAsset::free_balance(payment_asset, &beneficiary_2);
		let initial_balance_seller = GenericAsset::free_balance(payment_asset, &token_owner);

		assert_ok!(Nft::buy(Some(buyer).into(), listing_id));
		let presale_issuance = GenericAsset::total_issuance(payment_asset);
		// royalties distributed according to `entitlements` map
		assert_eq!(
			GenericAsset::free_balance(payment_asset, &collection_owner),
			initial_balance_owner + royalties_schedule.clone().entitlements[0].1 * sale_price
		);
		assert_eq!(
			GenericAsset::free_balance(payment_asset, &beneficiary_1),
			initial_balance_b1 + royalties_schedule.clone().entitlements[1].1 * sale_price
		);
		assert_eq!(
			GenericAsset::free_balance(payment_asset, &beneficiary_2),
			initial_balance_b2 + royalties_schedule.clone().entitlements[2].1 * sale_price
		);
		// token owner gets sale price less royalties
		assert_eq!(
			GenericAsset::free_balance(payment_asset, &token_owner),
			initial_balance_seller + sale_price
				- royalties_schedule
					.clone()
					.entitlements
					.into_iter()
					.map(|(_, e)| e * sale_price)
					.sum::<Balance>()
		);
		assert_eq!(GenericAsset::total_issuance(payment_asset), presale_issuance);

		// listing removed
		assert!(Nft::listings(listing_id).is_none());
		assert!(!Nft::listing_end_schedule(
			System::block_number() + <Test as Config>::DefaultListingDuration::get(),
			listing_id
		));

		// ownership changed
		assert!(Nft::collected_tokens(collection_id, &buyer).contains(&token_id));
	});
}

#[test]
fn buy_fails_prechecks() {
	ExtBuilder::default().build().execute_with(|| {
		let (_, token_id, token_owner) = setup_token();
		let buyer = 5;
		let payment_asset = PAYMENT_ASSET;
		let price = 1_000;
		let listing_id = Nft::next_listing_id();

		// not for sale
		assert_noop!(
			Nft::buy(Some(buyer).into(), listing_id),
			Error::<Test>::NotForFixedPriceSale,
		);

		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			token_id,
			Some(buyer),
			payment_asset,
			price,
			None,
		));

		// no permission
		assert_noop!(
			Nft::buy(Some(buyer + 1).into(), listing_id),
			Error::<Test>::NoPermission,
		);

		// fund the buyer with not quite enough
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&buyer, payment_asset, price - 1);
		assert_noop!(
			Nft::buy(Some(buyer).into(), listing_id),
			crml_generic_asset::Error::<Test>::InsufficientBalance,
		);
	});
}

#[test]
fn sell_to_anybody() {
	ExtBuilder::default().build().execute_with(|| {
		let (collection_id, token_id, token_owner) = setup_token();
		let payment_asset = PAYMENT_ASSET;
		let price = 1_000;
		let listing_id = Nft::next_listing_id();

		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			token_id,
			None,
			payment_asset,
			price,
			None,
		));

		let buyer = 11;
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&buyer, payment_asset, price);
		assert_ok!(Nft::buy(Some(buyer).into(), listing_id));

		// paid
		assert!(GenericAsset::free_balance(payment_asset, &buyer).is_zero());

		// listing removed
		assert!(Nft::listings(listing_id).is_none());
		assert!(!Nft::listing_end_schedule(
			System::block_number() + <Test as Config>::DefaultListingDuration::get(),
			listing_id
		));

		// ownership changed
		assert_eq!(Nft::collected_tokens(collection_id, &buyer), vec![token_id]);
	});
}

#[test]
fn buy_with_overcommitted_royalties() {
	ExtBuilder::default().build().execute_with(|| {
		// royalties are > 100% total which could create funds out of nothing
		// in this case, default to 0 royalties.
		// royalty schedules should not make it into storage but we protect against it anyway
		let (collection_id, token_id, token_owner) = setup_token();
		let bad_schedule = RoyaltiesSchedule {
			entitlements: vec![
				(11_u64, Permill::from_fraction(0.125)),
				(12_u64, Permill::from_fraction(0.9)),
			],
		};
		CollectionRoyalties::<Test>::insert(collection_id, bad_schedule.clone());
		let listing_id = Nft::next_listing_id();

		let buyer = 5;
		let payment_asset = PAYMENT_ASSET;
		let price = 1_000;
		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			token_id,
			Some(buyer),
			payment_asset,
			price,
			None,
		));

		let _ = <Test as Config>::MultiCurrency::deposit_creating(&buyer, payment_asset, price);
		let presale_issuance = GenericAsset::total_issuance(payment_asset);

		assert_ok!(Nft::buy(Some(buyer).into(), listing_id));

		assert!(bad_schedule.calculate_total_entitlement().is_zero());
		assert_eq!(GenericAsset::free_balance(payment_asset, &token_owner), price);
		assert!(GenericAsset::free_balance(payment_asset, &buyer).is_zero());
		assert_eq!(GenericAsset::total_issuance(payment_asset), presale_issuance);
	})
}

#[test]
fn cancel_auction() {
	ExtBuilder::default().build().execute_with(|| {
		let (collection_id, token_id, token_owner) = setup_token();
		let payment_asset = PAYMENT_ASSET;
		let reserve_price = 100_000;
		let listing_id = Nft::next_listing_id();

		assert_ok!(Nft::auction(
			Some(token_owner).into(),
			token_id,
			payment_asset,
			reserve_price,
			Some(System::block_number() + 1),
		));

		assert_noop!(
			Nft::cancel_sale(Some(token_owner + 1).into(), listing_id),
			Error::<Test>::NoPermission
		);

		assert_ok!(Nft::cancel_sale(Some(token_owner).into(), listing_id,));

		assert!(has_event(RawEvent::AuctionClosed(
			collection_id,
			listing_id,
			AuctionClosureReason::VendorCancelled
		)));

		// storage cleared up
		assert!(Nft::listings(listing_id).is_none());
		assert!(!Nft::listing_end_schedule(System::block_number() + 1, listing_id));

		// it should be free to operate on the token
		assert_ok!(Nft::transfer(Some(token_owner).into(), token_id, token_owner + 1,));
	});
}

#[test]
fn auction_bundle() {
	ExtBuilder::default().build().execute_with(|| {
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner);
		let quantity = 5;

		assert_ok!(Nft::mint_series(
			Some(collection_owner).into(),
			collection_id,
			quantity,
			None,
			vec![NFTAttributeValue::I32(123)],
			None,
			None,
		));

		let tokens = vec![(collection_id, 0, 1), (collection_id, 0, 3), (collection_id, 0, 4)];
		let listing_id = Nft::next_listing_id();

		assert_ok!(Nft::auction_bundle(
			Some(collection_owner).into(),
			tokens.clone(),
			PAYMENT_ASSET,
			1_000,
			Some(1),
		));

		assert!(Nft::open_collection_listings(collection_id, listing_id));
		for token in tokens.iter() {
			assert!(Nft::token_locks(token));
		}

		let buyer = 3;
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&buyer, PAYMENT_ASSET, 1_000);
		assert_ok!(Nft::bid(Some(buyer).into(), listing_id, 1_000));
		// end auction
		let _ = Nft::on_initialize(System::block_number() + 1);

		assert_eq!(Nft::collected_tokens(collection_id, &buyer), tokens);
	})
}

#[test]
fn auction_bundle_fails() {
	ExtBuilder::default().build().execute_with(|| {
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner);
		let collection_id_2 = setup_collection(collection_owner);
		// mint some fake tokens
		<TokenOwner<Test>>::insert((collection_id, 1), 1, collection_owner);
		<TokenOwner<Test>>::insert((collection_id, 2), 2, collection_owner);
		<TokenOwner<Test>>::insert((collection_id_2, 1), 1, collection_owner);

		// empty tokens fails
		assert_noop!(
			Nft::auction_bundle(Some(collection_owner).into(), vec![], PAYMENT_ASSET, 1_000, None),
			Error::<Test>::NoToken
		);

		// cannot bundle sell tokens from different collections
		assert_noop!(
			Nft::auction_bundle(
				Some(collection_owner).into(),
				vec![(collection_id, 1, 1), (collection_id_2, 1, 1),],
				PAYMENT_ASSET,
				1_000,
				None,
			),
			Error::<Test>::MixedBundleSale
		);

		// cannot bundle sell when series have royalties set
		<SeriesRoyalties<Test>>::insert(collection_id, 1, RoyaltiesSchedule::<AccountId>::default());
		<SeriesRoyalties<Test>>::insert(collection_id, 2, RoyaltiesSchedule::<AccountId>::default());
		assert_noop!(
			Nft::auction_bundle(
				Some(collection_owner).into(),
				vec![(collection_id, 1, 1), (collection_id, 2, 2)],
				PAYMENT_ASSET,
				1_000,
				None,
			),
			Error::<Test>::RoyaltiesProtection
		);
	})
}

#[test]
fn auction() {
	ExtBuilder::default().build().execute_with(|| {
		let (collection_id, token_id, token_owner) = setup_token();
		let payment_asset = PAYMENT_ASSET;
		let reserve_price = 100_000;

		let listing_id = Nft::next_listing_id();

		assert_ok!(Nft::auction(
			Some(token_owner).into(),
			token_id,
			payment_asset,
			reserve_price,
			Some(1),
		));
		assert_eq!(Nft::next_listing_id(), listing_id + 1);
		assert!(Nft::token_locks(&token_id));
		assert!(Nft::open_collection_listings(collection_id, listing_id));

		// first bidder at reserve price
		let bidder_1 = 10;
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&bidder_1, payment_asset, reserve_price);
		assert_ok!(Nft::bid(Some(bidder_1).into(), listing_id, reserve_price,));
		assert_eq!(GenericAsset::reserved_balance(payment_asset, &bidder_1), reserve_price);

		// second bidder raises bid
		let winning_bid = reserve_price + 1;
		let bidder_2 = 11;
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&bidder_2, payment_asset, reserve_price + 1);
		assert_ok!(Nft::bid(Some(bidder_2).into(), listing_id, winning_bid,));
		assert!(GenericAsset::reserved_balance(payment_asset, &bidder_1).is_zero()); // bidder_1 funds released
		assert_eq!(GenericAsset::reserved_balance(payment_asset, &bidder_2), winning_bid);

		// end auction
		let _ = Nft::on_initialize(System::block_number() + 1);

		// no royalties, all proceeds to token owner
		assert_eq!(GenericAsset::free_balance(payment_asset, &token_owner), winning_bid);
		// bidder2 funds should be all gone (unreserved and transferred)
		assert!(GenericAsset::free_balance(payment_asset, &bidder_2).is_zero());
		assert!(GenericAsset::reserved_balance(payment_asset, &bidder_2).is_zero());

		// listing metadata removed
		assert!(Nft::listings(listing_id).is_none());
		assert!(!Nft::listing_end_schedule(System::block_number() + 1, listing_id));

		// ownership changed
		assert!(!Nft::token_locks(&token_id));
		assert_eq!(Nft::collected_tokens(collection_id, &bidder_2), vec![token_id]);
		assert!(!Nft::open_collection_listings(collection_id, listing_id));

		// event logged
		assert!(has_event(RawEvent::AuctionSold(
			collection_id,
			listing_id,
			payment_asset,
			winning_bid,
			bidder_2
		)));
	});
}

#[test]
fn auction_royalty_payments() {
	ExtBuilder::default().build().execute_with(|| {
		let payment_asset = PAYMENT_ASSET;
		let reserve_price = 100_004;
		let beneficiary_1 = 11;
		let beneficiary_2 = 12;
		let collection_owner = 1;
		let royalties_schedule = RoyaltiesSchedule {
			entitlements: vec![
				(collection_owner, Permill::from_fraction(0.1111)),
				(beneficiary_1, Permill::from_fraction(0.1111)),
				(beneficiary_2, Permill::from_fraction(0.1111)),
			],
		};
		let (collection_id, token_id, token_owner) = setup_token_with_royalties(royalties_schedule.clone(), 1);
		let listing_id = Nft::next_listing_id();

		assert_ok!(Nft::auction(
			Some(token_owner).into(),
			token_id,
			payment_asset,
			reserve_price,
			Some(1),
		));

		// first bidder at reserve price
		let bidder = 10;
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&bidder, payment_asset, reserve_price);
		assert_ok!(Nft::bid(Some(bidder).into(), listing_id, reserve_price,));

		// end auction
		let _ = Nft::on_initialize(System::block_number() + 1);

		// royalties paid out
		let presale_issuance = GenericAsset::total_issuance(payment_asset);
		// royalties distributed according to `entitlements` map
		assert_eq!(
			GenericAsset::free_balance(payment_asset, &collection_owner),
			royalties_schedule.entitlements[0].1 * reserve_price
		);
		assert_eq!(
			GenericAsset::free_balance(payment_asset, &beneficiary_1),
			royalties_schedule.entitlements[1].1 * reserve_price
		);
		assert_eq!(
			GenericAsset::free_balance(payment_asset, &beneficiary_2),
			royalties_schedule.entitlements[2].1 * reserve_price
		);
		// token owner gets sale price less royalties
		assert_eq!(
			GenericAsset::free_balance(payment_asset, &token_owner),
			reserve_price
				- royalties_schedule
					.entitlements
					.into_iter()
					.map(|(_, e)| e * reserve_price)
					.sum::<Balance>()
		);
		assert!(GenericAsset::free_balance(payment_asset, &bidder).is_zero());
		assert!(GenericAsset::reserved_balance(payment_asset, &bidder).is_zero());

		assert_eq!(GenericAsset::total_issuance(payment_asset), presale_issuance);

		// listing metadata removed
		assert!(!Listings::<Test>::contains_key(listing_id));
		assert!(!ListingEndSchedule::<Test>::contains_key(
			System::block_number() + 1,
			listing_id,
		));

		// ownership changed
		assert_eq!(Nft::collected_tokens(collection_id, &bidder), vec![token_id]);
	});
}

#[test]
fn close_listings_at_removes_listing_data() {
	ExtBuilder::default().build().execute_with(|| {
		let collection_id = Nft::next_collection_id();
		let payment_asset = PAYMENT_ASSET;
		let price = 123_456;

		let token_1 = first_token_id(collection_id);

		let listings = vec![
			// an open sale which won't be bought before closing
			Listing::<Test>::FixedPrice(FixedPriceListing::<Test> {
				payment_asset,
				fixed_price: price,
				buyer: None,
				close: System::block_number() + 1,
				seller: 1,
				tokens: vec![token_1],
				royalties_schedule: Default::default(),
			}),
			// an open auction which has no bids before closing
			Listing::<Test>::Auction(AuctionListing::<Test> {
				payment_asset,
				reserve_price: price,
				close: System::block_number() + 1,
				seller: 1,
				tokens: vec![token_1],
				royalties_schedule: Default::default(),
			}),
			// an open auction which has a winning bid before closing
			Listing::<Test>::Auction(AuctionListing::<Test> {
				payment_asset,
				reserve_price: price,
				close: System::block_number() + 1,
				seller: 1,
				tokens: vec![token_1],
				royalties_schedule: Default::default(),
			}),
		];

		// setup listings storage
		for (listing_id, listing) in listings.iter().enumerate() {
			let listing_id = listing_id as ListingId;
			Listings::<Test>::insert(listing_id, listing.clone());
			ListingEndSchedule::<Test>::insert(System::block_number() + 1, listing_id, true);
		}
		// winning bidder has no funds, this should cause settlement failure
		ListingWinningBid::<Test>::insert(2, (11u64, 100u128));

		// Close the listings
		Nft::close_listings_at(System::block_number() + 1);

		// Storage clear
		assert!(
			ListingEndSchedule::<Test>::iter_prefix_values(System::block_number() + 1)
				.count()
				.is_zero()
		);
		for listing_id in 0..listings.len() as ListingId {
			assert!(Nft::listings(listing_id).is_none());
			assert!(Nft::listing_winning_bid(listing_id).is_none());
			assert!(!Nft::listing_end_schedule(System::block_number() + 1, listing_id));
		}

		assert!(has_event(RawEvent::FixedPriceSaleClosed(collection_id, 0)));
		assert!(has_event(RawEvent::AuctionClosed(
			collection_id,
			1,
			AuctionClosureReason::ExpiredNoBids
		)));
		assert!(has_event(RawEvent::AuctionClosed(
			collection_id,
			2,
			AuctionClosureReason::SettlementFailed
		)));
	});
}

#[test]
fn auction_fails_prechecks() {
	ExtBuilder::default().build().execute_with(|| {
		let (collection_id, token_id, token_owner) = setup_token();
		let payment_asset = PAYMENT_ASSET;
		let reserve_price = 100_000;

		let missing_token_id = (collection_id, 0, 2);

		// token doesn't exist
		assert_noop!(
			Nft::auction(
				Some(token_owner).into(),
				missing_token_id,
				payment_asset,
				reserve_price,
				Some(1),
			),
			Error::<Test>::NoPermission
		);

		// not owner
		assert_noop!(
			Nft::auction(
				Some(token_owner + 1).into(),
				token_id,
				payment_asset,
				reserve_price,
				Some(1),
			),
			Error::<Test>::NoPermission
		);

		// setup listed token, and try list it again
		assert_ok!(Nft::auction(
			Some(token_owner).into(),
			token_id,
			payment_asset,
			reserve_price,
			Some(1),
		));
		// already listed
		assert_noop!(
			Nft::auction(
				Some(token_owner).into(),
				token_id,
				payment_asset,
				reserve_price,
				Some(1),
			),
			Error::<Test>::TokenListingProtection
		);

		// listed for auction
		assert_noop!(
			Nft::sell(
				Some(token_owner).into(),
				token_id,
				None,
				payment_asset,
				reserve_price,
				None,
			),
			Error::<Test>::TokenListingProtection
		);
	});
}

#[test]
fn bid_fails_prechecks() {
	ExtBuilder::default().build().execute_with(|| {
		let missing_listing_id = 5;
		assert_noop!(
			Nft::bid(Some(1).into(), missing_listing_id, 100),
			Error::<Test>::NotForAuction
		);

		let (_, token_id, token_owner) = setup_token();
		let payment_asset = PAYMENT_ASSET;
		let reserve_price = 100_000;
		let listing_id = Nft::next_listing_id();

		assert_ok!(Nft::auction(
			Some(token_owner).into(),
			token_id,
			payment_asset,
			reserve_price,
			Some(1),
		));

		let bidder = 5;
		// < reserve
		assert_noop!(
			Nft::bid(Some(bidder).into(), listing_id, reserve_price - 1),
			Error::<Test>::BidTooLow
		);

		// no free balance
		assert_noop!(
			Nft::bid(Some(bidder).into(), listing_id, reserve_price),
			crml_generic_asset::Error::<Test>::InsufficientBalance
		);

		// balance already reserved for other reasons
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&bidder, payment_asset, reserve_price + 100);
		assert_ok!(<<Test as Config>::MultiCurrency as MultiCurrency>::reserve(
			&bidder,
			payment_asset,
			reserve_price
		));
		assert_noop!(
			Nft::bid(Some(bidder).into(), listing_id, reserve_price),
			crml_generic_asset::Error::<Test>::InsufficientBalance
		);
		let _ = <<Test as Config>::MultiCurrency as MultiCurrency>::unreserve(&bidder, payment_asset, reserve_price);

		// <= current bid
		assert_ok!(Nft::bid(Some(bidder).into(), listing_id, reserve_price,));
		assert_noop!(
			Nft::bid(Some(bidder).into(), listing_id, reserve_price),
			Error::<Test>::BidTooLow
		);
	});
}

#[test]
fn transfer_batch() {
	ExtBuilder::default().build().execute_with(|| {
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner);
		let token_owner = 2_u64;
		let token_1_quantity = 3;
		let series_id = Nft::next_series_id(collection_id);

		assert_ok!(Nft::mint_series(
			Some(collection_owner).into(),
			collection_id,
			token_1_quantity,
			Some(token_owner),
			vec![],
			None,
			None,
		));

		// test
		let tokens = vec![
			(collection_id, series_id, 0),
			(collection_id, series_id, 1),
			(collection_id, series_id, 2),
		];
		let new_owner = 3_u64;
		assert_ok!(Nft::transfer_batch(Some(token_owner).into(), tokens.clone(), new_owner,));
		assert!(has_event(RawEvent::Transfer(token_owner, tokens.clone(), new_owner)));

		assert_eq!(Nft::collected_tokens(collection_id, &new_owner), tokens);
		assert!(Nft::collected_tokens(collection_id, &token_owner).is_empty());
	});
}

#[test]
fn transfer_batch_fails() {
	ExtBuilder::default().build().execute_with(|| {
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner);
		let token_owner = 2_u64;
		let series_id = Nft::next_series_id(collection_id);

		assert_ok!(Nft::mint_series(
			Some(collection_owner).into(),
			collection_id,
			3,
			Some(token_owner),
			vec![],
			None,
			None,
		));

		// token 3 doesn't exist
		let new_owner = 3_u64;
		assert_noop!(
			Nft::transfer_batch(
				Some(token_owner).into(),
				vec![
					(collection_id, series_id, 0),
					(collection_id, series_id, 3),
					(collection_id, series_id, 1),
				],
				new_owner,
			),
			Error::<Test>::NoPermission
		);

		// not owner
		assert_noop!(
			Nft::transfer_batch(
				Some(token_owner + 1).into(),
				vec![
					(collection_id, series_id, 0),
					(collection_id, series_id, 1),
					(collection_id, series_id, 2),
				],
				new_owner
			),
			Error::<Test>::NoPermission
		);

		// transfer empty ids should fail
		assert_noop!(
			Nft::transfer_batch(Some(token_owner).into(), vec![], new_owner),
			Error::<Test>::NoToken
		);
	});
}

#[test]
fn mint_series() {
	ExtBuilder::default().build().execute_with(|| {
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner);
		let series_attributes = vec![
			NFTAttributeValue::I32(123),
			NFTAttributeValue::String(b"foobar".to_owned().to_vec()),
		];
		let token_owner = 2_u64;
		let quantity = 5;
		let series_id = Nft::next_series_id(collection_id);
		let royalties_schedule = RoyaltiesSchedule {
			entitlements: vec![(collection_owner, Permill::one())],
		};

		// mint token Ids 0-4
		assert_ok!(Nft::mint_series(
			Some(collection_owner).into(),
			collection_id,
			quantity,
			Some(token_owner),
			series_attributes.clone(),
			None,
			Some(royalties_schedule.clone()),
		));

		assert!(has_event(RawEvent::CreateSeries(
			collection_id,
			series_id,
			quantity,
			token_owner
		)));

		// check token ownership and attributes correct
		assert_eq!(
			Nft::series_attributes(collection_id, series_id),
			series_attributes.clone()
		);
		assert_eq!(Nft::series_issuance(collection_id, series_id), quantity);
		assert_eq!(
			Nft::series_royalties(collection_id, series_id).expect("royalties set"),
			royalties_schedule
		);
		// We minted collection token 0, next collection token id is 1
		assert_eq!(Nft::next_series_id(collection_id), 1);
		assert_eq!(
			Nft::collected_tokens(collection_id, &token_owner),
			vec![0, 1, 2, 3, 4]
				.into_iter()
				.map(|t| (collection_id, series_id, t))
				.collect::<Vec<TokenId>>(),
		);

		// check we can mint some more

		// mint token Ids 5-7
		let additional_quantity = 3;
		assert_ok!(Nft::mint_additional(
			Some(collection_owner).into(),
			collection_id,
			series_id,
			additional_quantity,
			Some(token_owner + 1), // new owner this time
		));
		assert_eq!(
			Nft::next_serial_number(collection_id, series_id),
			quantity + additional_quantity
		);

		assert_eq!(
			Nft::collected_tokens(collection_id, &token_owner),
			vec![0, 1, 2, 3, 4]
				.into_iter()
				.map(|t| (collection_id, series_id, t))
				.collect::<Vec<TokenId>>()
		);
		assert_eq!(
			Nft::collected_tokens(collection_id, &(token_owner + 1)),
			vec![5, 6, 7]
				.into_iter()
				.map(|t| (collection_id, series_id, t))
				.collect::<Vec<TokenId>>()
		);
		assert_eq!(
			Nft::series_issuance(collection_id, series_id),
			quantity + additional_quantity
		);
	});
}

#[test]
fn mint_series_fails() {
	ExtBuilder::default().build().execute_with(|| {
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner);

		// create 0 should fail
		assert_noop!(
			Nft::mint_series(
				Some(collection_owner).into(),
				collection_id,
				0,
				Some(collection_owner),
				vec![],
				None,
				None,
			),
			Error::<Test>::NoToken
		);
	});
}

#[test]
fn mint_series_royalties_invalid() {
	ExtBuilder::default().build().execute_with(|| {
		let token_owner = 1_u64;
		let collection_id = setup_collection(token_owner);
		let quantity = 5;

		// create with royalties over 1000_000 should fail
		assert_noop!(
			Nft::mint_series(
				Some(token_owner).into(),
				collection_id,
				quantity,
				Some(token_owner),
				vec![],
				None,
				Some(RoyaltiesSchedule::<AccountId> {
					entitlements: vec![]
				}),
			),
			Error::<Test>::RoyaltiesInvalid
		);

		// create with royalties over 1000_000 should fail
		assert_noop!(
			Nft::mint_series(
				Some(token_owner).into(),
				collection_id,
				quantity,
				Some(token_owner),
				vec![],
				None,
				Some(RoyaltiesSchedule::<AccountId> {
					entitlements: vec![
						(3_u64, Permill::from_fraction(1.2)),
						(4_u64, Permill::from_fraction(3.3))
					]
				}),
			),
			Error::<Test>::RoyaltiesInvalid
		);
	})
}

#[test]
fn mint_additional_fails() {
	ExtBuilder::default().build().execute_with(|| {
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner);
		let series_id = Nft::next_series_id(collection_id);

		// mint token Ids 0-4
		assert_ok!(Nft::mint_series(
			Some(collection_owner).into(),
			collection_id,
			5,
			None,
			vec![],
			None,
			None,
		));

		// add 0 additional fails
		assert_noop!(
			Nft::mint_additional(Some(collection_owner).into(), collection_id, series_id, 0, None),
			Error::<Test>::NoToken
		);

		// add to non-existing series fails
		assert_noop!(
			Nft::mint_additional(Some(collection_owner).into(), collection_id, series_id + 1, 5, None),
			Error::<Test>::NoToken
		);

		// not collection owner
		assert_noop!(
			Nft::mint_additional(Some(collection_owner + 1).into(), collection_id, series_id, 5, None),
			Error::<Test>::NoPermission
		);

		assert_ok!(Nft::mint_unique(
			Some(collection_owner).into(),
			collection_id,
			None,
			vec![],
			None,
			None,
		));

		// One of one token
		assert_noop!(
			Nft::mint_additional(Some(collection_owner).into(), collection_id, series_id + 1, 5, None),
			Error::<Test>::AddToUniqueIssue
		);
	});
}
