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
use crate::mock::{AccountId, Event, ExtBuilder, Test};
use frame_support::{assert_noop, assert_ok, parameter_types, traits::OnInitialize};
use sp_runtime::Permill;

type Nft = Module<Test>;
type GenericAsset = prml_generic_asset::Module<Test>;
type System = frame_system::Module<Test>;

parameter_types! {
	pub const DefaultListingDuration: u64 = 5;
	pub const MaxAttributeLength: u8 = 140;
}
impl Trait for Test {
	type Event = Event;
	type MultiCurrency = prml_generic_asset::Module<Test>;
	type MaxAttributeLength = MaxAttributeLength;
	type DefaultListingDuration = DefaultListingDuration;
	type WeightInfo = ();
}

// Check the test system contains an event record `event`
fn has_event(
	event: RawEvent<CollectionId, AccountId, AssetId, Balance, AuctionClosureReason, TokenCount>,
) -> bool {
	System::events()
		.iter()
		.find(|e| e.event == Event::nft(event.clone()))
		.is_some()
}

/// Generate the first `TokenId` for a collection's first series
fn first_token_id(collection_id: &CollectionId) -> TokenId {
	(collection_id, 0, 0)
}

// Create an NFT collection with schema
// Returns the created `collection_id`
fn setup_collection(owner: AccountId) -> CollectionId {
	let collection_id = b"test-collection".to_vec();
	assert_ok!(Nft::create_collection(
		Some(owner).into(),
		collection_id.clone(),
		Some(b"https://example.com/metadata".to_vec()),
		None,
	));

	collection_id
}

/// Setup a token, return collection id, token id, token owner
fn setup_token() -> (CollectionId, TokenId, <Test as frame_system::Trait>::AccountId) {
	let collection_owner = 1_u64;
	let collection_id = setup_collection(collection_owner);
	let token_owner = 2_u64;
	let token_id = first_token_id(&collection_id);
	assert_ok!(Nft::mint_unique(
		Some(collection_owner).into(),
		collection_id.clone(),
		Some(token_owner),
		vec![NFTAttributeValue::I32(500)],
		None,
		None,
	));

	(collection_id, token_id, token_owner)
}

/// Setup a token, return collection id, token id, token owner
fn setup_token_with_royalties(
	token_royalties: RoyaltiesSchedule<AccountId>,
	quantity: TokenCount,
) -> (CollectionId, TokenId, <Test as frame_system::Trait>::AccountId) {
	let collection_owner = 1_u64;
	let collection_id = setup_collection(collection_owner);
	let token_owner = 2_u64;
	let token_id = first_token_id(&collection_id);
	assert_ok!(Nft::mint_series(
		Some(collection_owner).into(),
		collection_id.clone(),
		quantity,
		Some(token_owner),
		vec![NFTAttributeValue::I32(500)],
		Some(token_royalties),
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

		assert_ok!(Nft::set_owner(
			Some(collection_owner).into(),
			collection_id.clone(),
			new_owner
		));
		assert_noop!(
			Nft::set_owner(Some(collection_owner).into(), collection_id, new_owner),
			Error::<Test>::NoPermission
		);
		assert_noop!(
			Nft::set_owner(Some(collection_owner).into(), b"no-collection".to_vec(), new_owner),
			Error::<Test>::NoCollection
		);
	});
}

#[test]
fn create_collection() {
	ExtBuilder::default().build().execute_with(|| {
		let owner = 1_u64;
		let collection_id = setup_collection(owner);
		assert!(has_event(RawEvent::CreateCollection(collection_id.clone(), owner)));

		assert_eq!(
			Nft::collection_owner(collection_id.clone()).expect("owner should exist"),
			owner
		);
		assert_eq!(
			Nft::collection_metadata_uri(collection_id.clone()),
			b"https://example.com/metadata".to_vec()
		);
		assert_eq!(Nft::collection_royalties(collection_id), None);
	});
}

#[test]
fn create_collection_invalid_id() {
	ExtBuilder::default().build().execute_with(|| {
		// too long
		let bad_collection_id = b"someidentifierthatismuchlongerthanthe32bytelimitsoshouldfail".to_vec();
		assert_noop!(
			Nft::create_collection(Some(1_u64).into(), bad_collection_id, vec![], None, None),
			Error::<Test>::CollectionIdInvalid
		);

		// empty id
		assert_noop!(
			Nft::create_collection(Some(1_u64).into(), vec![], vec![], None, None),
			Error::<Test>::CollectionIdInvalid
		);

		// non UTF-8 chars
		// kudos: https://www.cl.cam.ac.uk/~mgk25/ucs/examples/UTF-8-test.txt
		let bad_collection_id = vec![0xfe, 0xff];
		assert_noop!(
			Nft::create_collection(Some(1_u64).into(), bad_collection_id, vec![], None, None),
			Error::<Test>::CollectionIdInvalid
		);
	});
}

#[test]
fn create_collection_royalties_invalid() {
	ExtBuilder::default().build().execute_with(|| {
		let owner = 1_u64;

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
			Error::<Test>::RoyaltiesOvercommitment
		);
	})
}

#[test]
fn mint_unique() {
	ExtBuilder::default().build().execute_with(|| {
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner);

		let token_owner = 2_u64;
		let token_id = (&collection_id, 0);
		let royalties_schedule = RoyaltiesSchedule {
			entitlements: vec![(collection_owner, Permill::from_percent(10))],
		};
		assert_eq!(Nft::next_series_id(&collection_id), 0);
		assert_ok!(Nft::mint_unique(
			Some(collection_owner).into(),
			collection_id.clone(),
			Some(token_owner),
			vec![
				NFTAttributeValue::I32(-33),
				NFTAttributeValue::U8(0),
				NFTAttributeValue::Bytes32([1_u8; 32])
			],
			Some(royalties_schedule.clone()),
		));
		assert!(has_event(RawEvent::CreateToken(
			collection_id.clone(),
			token_id,
			1,
			token_owner.clone()
		)));

		let token_attributes = Nft::token_attributes(token_id);
		assert_eq!(
			token_attributes,
			vec![
				NFTAttributeValue::I32(-33),
				NFTAttributeValue::U8(Default::default()),
				NFTAttributeValue::Bytes32([1_u8; 32])
			],
		);

		assert_eq!(
			Nft::token_royalties(token_id).expect("royalties plan set"),
			royalties_schedule
		);
		assert_eq!(Nft::collected_tokens(&collection_id, &token_owner), vec![token_id]);
		assert_eq!(Nft::next_series_id(&collection_id), 1);
		assert_eq!(Nft::series_issuance(&token_id), 1);
	});
}

#[test]
fn create_multiple_unique_tokens() {
	ExtBuilder::default().build().execute_with(|| {
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner);
		let token_owner = 2_u64;
		let metadata_path = b"/series/test/";

		let token_1 = first_token_id(&collection_id);
		let token_2 = (&collection_id, 1 as SeriesId, 0 as SerialNumber);

		assert_ok!(Nft::mint_unique(
			Some(collection_owner).into(),
			collection_id.clone(),
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
			collection_id.clone(),
			Some(token_owner),
			vec![
				NFTAttributeValue::I32(33),
				NFTAttributeValue::U8(0),
				NFTAttributeValue::String("💎🙌".to_vec())
			],
			None,
			Some(metadata_path),
		));
		assert!(has_event(RawEvent::CreateToken(
			token_2.0, // collection Id
			token_2.1, // series Id
			token_2.2, // token Id
			token_owner.clone()
		)));

		assert_eq!(
			Nft::series_metadata_path(&collection_id, 1),
			Some(metadata_path.to_vec())
		);
		assert_eq!(
			Nft::collected_tokens(&collection_id, &token_owner),
			vec![token_1, token_2]
		);
		assert_eq!(Nft::next_series_id(&collection_id), 2);
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
				collection_id.clone(),
				collection_owner,
				Default::default(),
				None,
				None,
			),
			Error::<Test>::NoPermission
		);

		assert_noop!(
			Nft::mint_unique(
				Some(collection_owner).into(),
				b"this-collection-doesn't-exist".to_vec(),
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
				collection_id.clone(),
				None,
				too_many_attributes,
				None,
				None,
			),
			Error::<Test>::SchemaMaxAttributes
		);

		// royalties > 100%
		assert_noop!(
			Nft::mint_unique(
				Some(collection_owner).into(),
				collection_id.clone(),
				None,
				vec![NFTAttributeValue::I32(0), NFTAttributeValue::Url(b"test".to_vec())],
				Some(RoyaltiesSchedule::<AccountId> {
					entitlements: vec![
						(3_u64, Permill::from_fraction(1.2)),
						(3_u64, Permill::from_fraction(1.2))
					]
				}),
				None,
			),
			Error::<Test>::RoyaltiesOvercommitment
		);

		// attribute value too long
		assert_noop!(
			Nft::mint_unique(
				Some(collection_owner).into(),
				collection_id,
				None,
				vec![
					NFTAttributeValue::I32(0),
					NFTAttributeValue::Url([1_u8; <Test as Trait>::MaxAttributeLength::get() as usize + 1].to_vec())
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
		let token_id = first_token_id(&collection_id);
		assert_ok!(Nft::mint_unique(
			Some(collection_owner).into(),
			collection_id.clone(),
			Some(token_owner),
			vec![NFTAttributeValue::I32(500)],
			None,
			None,
		));

		// test
		let new_owner = 3_u64;
		assert_ok!(Nft::transfer(Some(token_owner).into(), token_id, new_owner,));
		assert!(has_event(RawEvent::Transfer(vec![(token_id, 1)], new_owner)));

		assert!(Nft::collected_tokens(&collection_id, &token_owner).is_empty());
		assert_eq!(Nft::collected_tokens(&collection_id, &new_owner), vec![token_id]);
	});
}

#[test]
fn transfer_fails_prechecks() {
	ExtBuilder::default().build().execute_with(|| {
		// setup token collection + one token
		let collection_owner = 1_u64;

		let collection_id = setup_collection(collection_owner);
		let token_owner = 2_u64;
		let token_id = first_token_id(&collection_id);

		// no token yet
		assert_noop!(
			Nft::transfer(Some(token_owner).into(), token_id, token_owner),
			Error::<Test>::NoToken,
		);

		assert_ok!(Nft::mint_unique(
			Some(collection_owner).into(),
			collection_id.clone(),
			Some(token_owner),
			vec![NFTAttributeValue::I32(500)],
			None,
			None,
		));

		let not_the_owner = 3_u64;
		assert_noop!(
			Nft::transfer(Some(not_the_owner).into(), token_id, not_the_owner),
			Error::<Test>::NoToken,
		);

		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			token_id,
			1,
			Some(5),
			16_000,
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

// TODO: burn handles duplicates
// TODO: check mint additional can't make an NFT increase by 1

#[test]
fn burn() {
	ExtBuilder::default().build().execute_with(|| {
		// setup token collection + one token
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner);
		let token_owner = 2_u64;
		let token_id = first_token_id(&collection_id);

		assert_ok!(Nft::mint_series(
			Some(collection_owner).into(),
			collection_id.clone(),
			3,
			Some(token_owner),
			vec![NFTAttributeValue::I32(500)],
			None,
			None,
		));

		// test
		assert_ok!(Nft::burn(Some(token_owner).into(), collection_id, 0, vec![0, 1, 2]));
		assert!(has_event(RawEvent::Burn(collection_id, 0, vec![0, 1, 2])));

		let series_id = (&collection_id, 0);

		assert!(!<SeriesIssuance<Test>>::contains_key(series_id));
		assert!(!<SeriesRoyalties<Test>>::contains_key(series_id));
		assert!(!<SeriesMetadataURI<Test>>::contains_key(series_id));
		assert!(!<SeriesAttributes<Test>>::contains_key(series_id));
		assert!(!<TokenOwner<Test>>::contains_key(&token_id));
		assert!(Nft::collected_tokens(&collection_id, &token_owner).is_empty());
	});
}

#[test]
fn burn_fails_prechecks() {
	ExtBuilder::default().build().execute_with(|| {
		// setup token collection + one token
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner);
		let series_id = Nft::next_series_id(&collection_id);
		let token_owner = 2_u64;

		// token doesn't exist yet
		assert_noop!(Nft::burn(Some(token_owner).into(), collection_id, series_id, vec![0]), Error::<Test>::NoToken);
		// token empty
		assert_noop!(Nft::burn(Some(token_owner).into(), collection_id, series_id, vec![]), Error::<Test>::NoToken);

		assert_ok!(Nft::mint_series(
			Some(collection_owner).into(),
			collection_id.clone(),
			100,
			Some(token_owner),
			vec![NFTAttributeValue::I32(500)],
			None,
			None,
		));

		// Not owner
		assert_noop!(
			Nft::burn(Some(token_owner + 1).into(), collection_id, series_id, vec![0]),
			Error::<Test>::NoToken,
		);

		// Fails with duplicate serials
		assert_noop!(
			Nft::burn(Some(token_owner).into(), collection_id, series_id, vec![0, 1, 1]),
			Error::<Test>::NoToken,
		);

		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			first_token_id(&collection_id),
			1,
			None,
			16_000,
			1_000,
			None,
		));
		// cannot burn while listed
		assert_noop!(
			Nft::burn(Some(token_owner).into(), collection_id, series_id, vec![0]),
			Error::<Test>::TokenListingProtection,
		);
	});
}

#[test]
fn sell() {
	ExtBuilder::default().build().execute_with(|| {
		let (_, token_id, token_owner) = setup_token();
		let quantity = 1;
		let listing_id = Nft::next_listing_id();

		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			token_id,
			quantity,
			Some(5),
			16_000,
			1_000,
			None,
		));

		assert_eq!(Nft::token_locks(token_id, token_owner), quantity);

		let expected = Listing::<Test>::FixedPrice(FixedPriceListing::<Test> {
			payment_asset: 16_000,
			fixed_price: 1_000,
			close: System::block_number() + <Test as Trait>::DefaultListingDuration::get(),
			buyer: Some(5),
			token_id,
			seller: token_owner,
		});

		let listing = Nft::listings(listing_id).expect("token is listed");
		assert_eq!(listing, expected);

		// current block is 1 + duration
		assert!(Nft::listing_end_schedule(
			System::block_number() + <Test as Trait>::DefaultListingDuration::get(),
			listing_id
		));

		// Can't transfer while listed for sale
		assert_noop!(
			Nft::transfer(Some(token_owner).into(), token_id, token_owner + 1),
			Error::<Test>::TokenListingProtection
		);

		assert!(has_event(RawEvent::FixedPriceSaleListed(
			listing_id,
			Some(5),
			16_000,
			1_000
		)));
	});
}

#[test]
fn sell_prechecks() {
	ExtBuilder::default().build().execute_with(|| {
		let (_, token_id, token_owner) = setup_token();
		// Not owner
		assert_noop!(
			Nft::sell(Some(token_owner + 1).into(), token_id, 1, Some(5), 16_000, 1_000, None),
			Error::<Test>::NoToken
		);

		// Sell zero
		assert_noop!(
			Nft::sell(Some(token_owner).into(), token_id, 0, None, 16_000, 1_000, None),
			Error::<Test>::NoToken
		);

		// token listed already
		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			token_id,
			1,
			Some(5),
			16_000,
			1_000,
			None,
		));
		assert_noop!(
			Nft::sell(Some(token_owner).into(), token_id, 1, Some(5), 16_000, 1_000, None),
			Error::<Test>::TokenListingProtection
		);

		// can't auction, listed for fixed price sale
		assert_noop!(
			Nft::auction(Some(token_owner).into(), token_id, 1, 16_000, 1_000, None),
			Error::<Test>::TokenListingProtection
		);
	});
}

#[test]
fn cancel_sell() {
	ExtBuilder::default().build().execute_with(|| {
		let (_, token_id, token_owner) = setup_token();
		let listing_id = Nft::next_listing_id();
		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			token_id,
			1,
			Some(5),
			16_000,
			1_000,
			None,
		));
		assert_ok!(Nft::cancel_sale(Some(token_owner).into(), listing_id));
		assert!(has_event(RawEvent::FixedPriceSaleClosed(listing_id)));

		// storage cleared up
		assert!(Nft::listings(listing_id).is_none());
		assert!(!Nft::listing_end_schedule(
			System::block_number() + <Test as Trait>::DefaultListingDuration::get(),
			listing_id
		));

		// it should be free to operate on the token
		assert_ok!(Nft::transfer(Some(token_owner).into(), token_id, token_owner + 1,));
	});
}

#[test]
fn sell_closes_on_schedule() {
	ExtBuilder::default().build().execute_with(|| {
		let quantity = 50;
		let (_, token_id, token_owner) = setup_token_with_royalties(RoyaltiesSchedule::default(), quantity);
		let listing_duration = 100;
		let listing_id = Nft::next_listing_id();

		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			token_id,
			quantity,
			Some(5),
			16_000,
			1_000,
			Some(listing_duration),
		));

		// sale should close after the duration expires
		Nft::on_initialize(System::block_number() + listing_duration);

		// seller should have tokens
		assert_eq!(Nft::balance_of(token_id, token_owner), quantity);
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
		let payment_asset = 16_000;
		let price = 1_000;
		let listing_id = Nft::next_listing_id();

		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			token_id,
			1,
			Some(buyer),
			payment_asset,
			price,
			None,
		));

		let _ = <Test as Trait>::MultiCurrency::deposit_creating(&buyer, Some(payment_asset), price);
		assert_ok!(Nft::buy(Some(buyer).into(), listing_id));
		// no royalties, all proceeds to token owner
		assert_eq!(GenericAsset::free_balance(payment_asset, &token_owner), price,);

		// listing removed
		assert!(Nft::listings(listing_id).is_none());
		assert!(!Nft::listing_end_schedule(
			System::block_number() + <Test as Trait>::DefaultListingDuration::get(),
			listing_id
		));

		// ownership changed
		assert!(Nft::token_locks(token_id, token_owner).is_zero());
		assert_eq!(Nft::balance_of(token_id, buyer), 1);
		assert_eq!(Nft::collected_tokens(&collection_id, &buyer), vec![token_id]);
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
		let quantity = 100;
		let (collection_id, token_id, token_owner) = setup_token_with_royalties(royalties_schedule.clone(), quantity);
		let buyer = 5;
		let payment_asset = 16_000;
		let sale_price = 1_000_008;
		let _ = <Test as Trait>::MultiCurrency::deposit_creating(&buyer, Some(payment_asset), sale_price * 2);

		// Test token royalties on 1st iteration
		// Test collection royalties on 2nd iteration
		for test_index in 0..=1_u32 {
			if test_index == 1 {
				SeriesRoyalties::<Test>::remove(token_id);
				CollectionRoyalties::<Test>::insert(&collection_id, &royalties_schedule);
			}
			let listing_id = Nft::next_listing_id();
			assert_eq!(listing_id, test_index as ListingId);
			assert_ok!(Nft::sell(
				Some(token_owner).into(),
				token_id,
				quantity / 2,
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
				System::block_number() + <Test as Trait>::DefaultListingDuration::get(),
				listing_id
			));

			// ownership changed
			assert_eq!(Nft::balance_of(token_id, buyer), quantity / (2 - test_index)); // loop1: 50, loop2: 100
			assert_eq!(Nft::collected_tokens(&collection_id, &buyer), vec![token_id]);
		}
	});
}

#[test]
fn buy_fails_prechecks() {
	ExtBuilder::default().build().execute_with(|| {
		let (_, token_id, token_owner) = setup_token();
		let buyer = 5;
		let payment_asset = 16_000;
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
			1,
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
		let _ = <Test as Trait>::MultiCurrency::deposit_creating(&buyer, Some(payment_asset), price - 1);
		assert_noop!(
			Nft::buy(Some(buyer).into(), listing_id),
			prml_generic_asset::Error::<Test>::InsufficientBalance,
		);
	});
}

#[test]
fn sell_to_anybody() {
	ExtBuilder::default().build().execute_with(|| {
		let (collection_id, token_id, token_owner) = setup_token();
		let payment_asset = 16_000;
		let price = 1_000;
		let listing_id = Nft::next_listing_id();

		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			token_id,
			1,
			None,
			payment_asset,
			price,
			None,
		));

		let buyer = 11;
		let _ = <Test as Trait>::MultiCurrency::deposit_creating(&buyer, Some(payment_asset), price);
		assert_ok!(Nft::buy(Some(buyer).into(), listing_id));

		// paid
		assert!(GenericAsset::free_balance(payment_asset, &buyer).is_zero());

		// listing removed
		assert!(Nft::listings(listing_id).is_none());
		assert!(!Nft::listing_end_schedule(
			System::block_number() + <Test as Trait>::DefaultListingDuration::get(),
			listing_id
		));

		// ownership changed
		assert_eq!(Nft::balance_of(token_id, buyer), 1);
		assert_eq!(Nft::collected_tokens(&collection_id, &buyer), vec![token_id]);
	});
}

#[test]
fn buy_with_overcommitted_royalties() {
	ExtBuilder::default().build().execute_with(|| {
		// royalties are > 100% total which could create funds out of nothing
		// in this case, default to 0 royalties.
		// royalty schedules should not make it into storage but we protect against it anyway
		let (_, token_id, token_owner) = setup_token();
		let bad_schedule = RoyaltiesSchedule {
			entitlements: vec![
				(11_u64, Permill::from_fraction(0.125)),
				(12_u64, Permill::from_fraction(0.9)),
			],
		};
		SeriesRoyalties::<Test>::insert(token_id, bad_schedule.clone());
		let listing_id = Nft::next_listing_id();

		let buyer = 5;
		let payment_asset = 16_000;
		let price = 1_000;
		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			token_id,
			1,
			Some(buyer),
			payment_asset,
			price,
			None,
		));

		let _ = <Test as Trait>::MultiCurrency::deposit_creating(&buyer, Some(payment_asset), price);
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
		let (_, token_id, token_owner) = setup_token();
		let payment_asset = 16_000;
		let reserve_price = 100_000;
		let listing_id = Nft::next_listing_id();

		assert_ok!(Nft::auction(
			Some(token_owner).into(),
			token_id,
			1,
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
fn auction() {
	ExtBuilder::default().build().execute_with(|| {
		let (collection_id, token_id, token_owner) = setup_token();
		let payment_asset = 16_000;
		let reserve_price = 100_000;
		let quantity = 1;

		let listing_id = Nft::next_listing_id();

		assert_ok!(Nft::auction(
			Some(token_owner).into(),
			token_id,
			quantity,
			payment_asset,
			reserve_price,
			Some(1),
		));
		assert_eq!(Nft::next_listing_id(), listing_id + 1);
		assert_eq!(Nft::token_locks(token_id, token_owner), 1);

		// first bidder at reserve price
		let bidder_1 = 10;
		let _ = <Test as Trait>::MultiCurrency::deposit_creating(&bidder_1, Some(payment_asset), reserve_price);
		assert_ok!(Nft::bid(Some(bidder_1).into(), listing_id, reserve_price,));
		assert_eq!(GenericAsset::reserved_balance(payment_asset, &bidder_1), reserve_price);

		// second bidder raises bid
		let winning_bid = reserve_price + 1;
		let bidder_2 = 11;
		let _ = <Test as Trait>::MultiCurrency::deposit_creating(&bidder_2, Some(payment_asset), reserve_price + 1);
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
		assert!(Nft::token_locks(token_id, token_owner).is_zero());
		assert_eq!(Nft::collected_tokens(&collection_id, &bidder_2), vec![token_id]);

		// event logged
		assert!(has_event(RawEvent::AuctionSold(
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
		let payment_asset = 16_000;
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
			1,
			payment_asset,
			reserve_price,
			Some(1),
		));

		// first bidder at reserve price
		let bidder = 10;
		let _ = <Test as Trait>::MultiCurrency::deposit_creating(&bidder, Some(payment_asset), reserve_price);
		assert_ok!(Nft::bid(Some(bidder).into(), listing_id, reserve_price,));

		// end auction
		let _ = Nft::on_initialize(System::block_number() + 1);

		// royaties paid out
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
		assert_eq!(Nft::collected_tokens(&collection_id, &bidder), vec![token_id]);
	});
}

#[test]
fn close_listings_at_removes_listing_data() {
	ExtBuilder::default().build().execute_with(|| {
		let collection_id = b"test-collection".to_vec();
		let payment_asset = 16_000;
		let price = 123_456;

		let token_1 = (&collection_id, 0);

		let listings = vec![
			// an open sale which won't be bought before closing
			Listing::<Test>::FixedPrice(FixedPriceListing::<Test> {
				payment_asset,
				fixed_price: price,
				buyer: None,
				close: System::block_number() + 1,
				seller: 1,
				token_id: token_1,
			}),
			// an open auction which has no bids before closing
			Listing::<Test>::Auction(AuctionListing::<Test> {
				payment_asset,
				reserve_price: price,
				close: System::block_number() + 1,
				seller: 1,
				token_id: token_1,
			}),
			// an open auction which has a winning bid before closing
			Listing::<Test>::Auction(AuctionListing::<Test> {
				payment_asset,
				reserve_price: price,
				close: System::block_number() + 1,
				seller: 1,
				token_id: token_1,
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

		assert!(has_event(RawEvent::FixedPriceSaleClosed(0)));
		assert!(has_event(RawEvent::AuctionClosed(
			1,
			AuctionClosureReason::ExpiredNoBids
		)));
		assert!(has_event(RawEvent::AuctionClosed(
			2,
			AuctionClosureReason::SettlementFailed
		)));
	});
}

#[test]
fn auction_fails_prechecks() {
	ExtBuilder::default().build().execute_with(|| {
		let (collection_id, token_id, token_owner) = setup_token();
		let payment_asset = 16_000;
		let reserve_price = 100_000;

		let missing_token_id = (&collection_id, 2);

		// token doesn't exist
		assert_noop!(
			Nft::auction(
				Some(token_owner).into(),
				missing_token_id,
				1,
				payment_asset,
				reserve_price,
				Some(1),
			),
			Error::<Test>::NoToken
		);

		// Sell zero
		assert_noop!(
			Nft::auction(
				Some(token_owner).into(),
				missing_token_id,
				0,
				payment_asset,
				reserve_price,
				Some(1),
			),
			Error::<Test>::NoToken
		);

		// not owner
		assert_noop!(
			Nft::auction(
				Some(token_owner + 1).into(),
				token_id,
				1,
				payment_asset,
				reserve_price,
				Some(1),
			),
			Error::<Test>::NoToken
		);

		// setup listed token, and try list it again
		assert_ok!(Nft::auction(
			Some(token_owner).into(),
			token_id,
			1,
			payment_asset,
			reserve_price,
			Some(1),
		));
		// already listed
		assert_noop!(
			Nft::auction(
				Some(token_owner).into(),
				token_id,
				1,
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
				1,
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
		let payment_asset = 16_000;
		let reserve_price = 100_000;
		let listing_id = Nft::next_listing_id();

		assert_ok!(Nft::auction(
			Some(token_owner).into(),
			token_id,
			1,
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
			prml_generic_asset::Error::<Test>::InsufficientBalance
		);

		// balance already reserved for other reasons
		let _ = <Test as Trait>::MultiCurrency::deposit_creating(&bidder, Some(payment_asset), reserve_price + 100);
		assert_ok!(<<Test as Trait>::MultiCurrency as MultiCurrencyAccounting>::reserve(
			&bidder,
			Some(payment_asset),
			reserve_price
		));
		assert_noop!(
			Nft::bid(Some(bidder).into(), listing_id, reserve_price),
			prml_generic_asset::Error::<Test>::InsufficientBalance
		);
		let _ = <<Test as Trait>::MultiCurrency as MultiCurrencyAccounting>::unreserve(
			&bidder,
			Some(payment_asset),
			reserve_price,
		);

		// <= current bid
		assert_ok!(Nft::bid(Some(bidder).into(), listing_id, reserve_price,));
		assert_noop!(
			Nft::bid(Some(bidder).into(), listing_id, reserve_price),
			Error::<Test>::BidTooLow
		);
	});
}

#[test]
fn batch_transfer() {
	ExtBuilder::default().build().execute_with(|| {
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner, vec![]);
		let token_owner = 2_u64;
		let token_1_quantity = 3;
		let token_2_quantity = 1;
		let token_1 = first_token_id(&collection_id);
		let token_2 = second_token_id(&collection_id);

		assert_ok!(Nft::mint_series(
			Some(collection_owner).into(),
			collection_id.clone(),
			token_1_quantity,
			Some(token_owner),
			vec![],
			None,
		));

		assert_ok!(Nft::mint_series(
			Some(collection_owner).into(),
			collection_id.clone(),
			token_2_quantity,
			Some(token_owner),
			vec![],
			None,
		));

		// test
		let transferred = vec![(token_1, token_1_quantity), (token_2, token_2_quantity)];
		let new_owner = 3_u64;
		assert_ok!(Nft::batch_transfer(
			Some(token_owner).into(),
			transferred.clone(),
			new_owner,
		));
		assert!(has_event(RawEvent::Transfer(transferred, new_owner)));

		assert_eq!(Nft::balance_of(token_1, new_owner), token_1_quantity);
		assert_eq!(Nft::balance_of(token_2, new_owner), token_2_quantity);

		assert_eq!(
			Nft::collected_tokens(&collection_id, &new_owner),
			vec![token_1, token_2]
		);
		assert!(Nft::collected_tokens(&collection_id, &token_owner).is_empty());
		// we minted 0 & 1
		assert_eq!(Nft::next_series_id(&collection_id), 2);
	});
}

#[test]
fn batch_transfer_fails() {
	ExtBuilder::default().build().execute_with(|| {
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner, vec![]);
		let token_owner = 2_u64;

		// Create two tokens
		// token 1: quantity 3
		// token 2: quantity: 1
		assert_ok!(Nft::mint_series(
			Some(collection_owner).into(),
			collection_id.clone(),
			3,
			Some(token_owner),
			vec![],
			None,
		));
		assert_ok!(Nft::mint_series(
			Some(collection_owner).into(),
			collection_id.clone(),
			1,
			Some(token_owner),
			vec![],
			None,
		));

		let token_1 = first_token_id(&collection_id);
		let token_2 = second_token_id(&collection_id);
		let token_missing = (&collection_id, 100);

		// token 5 doesn't exist
		let new_owner = 3_u64;
		assert_noop!(
			Nft::batch_transfer(
				Some(token_owner).into(),
				vec![(token_1, 2), (token_missing, 1)],
				new_owner,
			),
			Error::<Test>::NoToken
		);

		// quantity > balance
		assert_noop!(
			Nft::batch_transfer(Some(token_owner).into(), vec![(token_1, 1), (token_2, 2)], new_owner),
			Error::<Test>::NoToken
		);

		// not owner
		assert_noop!(
			Nft::batch_transfer(Some(token_owner + 1).into(), vec![(token_1, 2)], new_owner),
			Error::<Test>::NoToken
		);

		// transfer empty ids should fail
		assert_noop!(
			Nft::batch_transfer(Some(token_owner).into(), vec![], new_owner),
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
		let series_id = Nft::next_series_id(&collection_id);

		// mint token Ids 0-4
		assert_ok!(Nft::mint_series(
			Some(collection_owner).into(),
			collection_id.clone(),
			quantity,
			Some(token_owner),
			series_attributes.clone(),
			None,
			None,
		));

		let token_id = (&collection_id, series_id, 0);

		assert!(has_event(RawEvent::CreateSeries(
			collection_id.clone(),
			series_id,
			quantity,
			token_owner
		)));

		// check token ownership and attributes correct
		assert_eq!(Nft::series_attributes(&collection_id, series_id), series_attributes.clone());
		assert_eq!(Nft::series_issuance(&collection_id, series_id), quantity);
		// We minted collection token 0, next collection token id is 1
		assert_eq!(Nft::next_series_id(&collection_id), 1);

		assert_eq!(Nft::collected_tokens(&collection_id, &token_owner), vec![0, 1, 2, 3, 4]);

		// check we can mint some more

		// mint token Ids 5-7
		let additional_quantity = 3;
		assert_ok!(Nft::mint_additional(
			Some(collection_owner).into(),
			collection_id.clone(),
			series_id,
			additional_quantity,
			Some(token_owner + 1), // new owner this time
		));

		assert_eq!(Nft::collected_tokens(&collection_id, &token_owner), vec![0, 1, 2, 3, 4]);
		assert_eq!(Nft::collected_tokens(&collection_id, &token_owner + 1), vec![5, 6, 7]);
		assert_eq!(Nft::series_issuance(&collection_id, series_id), quantity + additional_quantity);
	});
}

#[test]
fn mint_series_fails() {
	ExtBuilder::default().build().execute_with(|| {
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner, vec![]);

		// create 0 should fail
		assert_noop!(
			Nft::mint_series(
				Some(collection_owner).into(),
				collection_id.clone(),
				0,
				collection_owner,
				vec![],
				None,
				None,
			),
			Error::<Test>::NoToken
		);
	});
}

#[test]
fn mint_additional_fails() {
	ExtBuilder::default().build().execute_with(|| {
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner);
		let series_id = Nft::next_series_id(&collection_id);

		// mint token Ids 0-4
		assert_ok!(Nft::mint_series(
			Some(collection_owner).into(),
			collection_id.clone(),
			5,
			None,
			vec![],
			None,
			None,
		));

		// add 0 additional fails
		assert_noop!(
			Nft::mint_additional(
				Some(collection_owner).into(),
				collection_id.clone(),
				series_id,
				0,
				None,
			),
			Error::<Test>::NoToken
		);

		// add to non-existing series fails
		assert_noop!(
			Nft::mint_additional(
				Some(collection_owner).into(),
				collection_id.clone(),
				series_id + 1,
				5,
				None,
			),
			Error::<Test>::NoToken
		);

		// not collection owner
		assert_noop!(
			Nft::mint_additional(
				Some(collection_owner + 1).into(),
				collection_id.clone(),
				series_id,
				5,
				None,
			),
			Error::<Test>::NoPermission
		);
	});
}
