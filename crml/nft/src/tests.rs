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
type TokenId = u32;

parameter_types! {
	pub const DefaultListingDuration: u64 = 5;
	pub const MaxAttributeLength: u8 = 140;
}
impl Trait for Test {
	type Event = Event;
	type TokenId = TokenId;
	type MultiCurrency = prml_generic_asset::Module<Test>;
	type MaxAttributeLength = MaxAttributeLength;
	type DefaultListingDuration = DefaultListingDuration;
	type WeightInfo = ();
}

// Check the test system contains an event record `event`
fn has_event(
	event: RawEvent<CollectionId, TokenId, AccountId, AssetId, Balance, AuctionClosureReason, EditionId<Test>>,
) -> bool {
	System::events()
		.iter()
		.find(|e| e.event == Event::nft(event.clone()))
		.is_some()
}

// Create an NFT collection with schema
// Returns the created `collection_id`
fn setup_collection(owner: AccountId, schema: NFTSchema) -> CollectionId {
	let collection_id = b"test-collection".to_vec();
	assert_ok!(Nft::create_collection(
		Some(owner).into(),
		collection_id.clone(),
		schema.clone(),
		Some(b"https://example.com/metadata".to_vec()),
		None,
	));
	collection_id
}

/// Setup a token, return collection id, token id, token owner
fn setup_token() -> (
	CollectionId,
	<Test as Trait>::TokenId,
	<Test as frame_system::Trait>::AccountId,
) {
	let schema = vec![(
		b"test-attribute".to_vec(),
		NFTAttributeValue::I32(Default::default()).type_id(),
	)];
	let collection_owner = 1_u64;
	let collection_id = setup_collection(collection_owner, schema);
	let token_owner = 2_u64;
	let token_id = Nft::next_token_id(&collection_id);
	assert_ok!(Nft::create_token(
		Some(collection_owner).into(),
		collection_id.clone(),
		token_owner,
		vec![NFTAttributeValue::I32(500)],
		None,
	));

	(collection_id, token_id, token_owner)
}

/// Setup a token, return collection id, token id, token owner
fn setup_token_with_royalties(
	token_royalties: RoyaltiesSchedule<AccountId>,
) -> (
	CollectionId,
	<Test as Trait>::TokenId,
	<Test as frame_system::Trait>::AccountId,
) {
	let schema = vec![(
		b"test-attribute".to_vec(),
		NFTAttributeValue::I32(Default::default()).type_id(),
	)];
	let collection_owner = 1_u64;
	let collection_id = setup_collection(collection_owner, schema);
	let token_owner = 2_u64;
	let token_id = Nft::next_token_id(&collection_id);
	assert_ok!(Nft::create_token(
		Some(collection_owner).into(),
		collection_id.clone(),
		token_owner,
		vec![NFTAttributeValue::I32(500)],
		Some(token_royalties),
	));

	(collection_id, token_id, token_owner)
}

#[test]
fn set_owner() {
	ExtBuilder::default().build().execute_with(|| {
		// setup token collection + one token
		let schema = vec![(
			b"test-attribute".to_vec(),
			NFTAttributeValue::I32(Default::default()).type_id(),
		)];
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner, schema);
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
		let schema = vec![
			(
				b"test-attribute-1".to_vec(),
				NFTAttributeValue::U8(Default::default()).type_id(),
			),
			(
				b"test-attribute-2".to_vec(),
				NFTAttributeValue::U8(Default::default()).type_id(),
			),
			(
				b"test-attribute-3".to_vec(),
				NFTAttributeValue::Bytes32(Default::default()).type_id(),
			),
		];

		let collection_id = setup_collection(owner, schema.clone());
		assert!(has_event(RawEvent::CreateCollection(collection_id.clone(), owner)));

		assert_eq!(
			Nft::collection_owner(collection_id.clone()).expect("owner should exist"),
			owner
		);
		assert_eq!(
			Nft::collection_schema(collection_id.clone()).expect("schema should exist"),
			schema
		);
		assert_eq!(
			Nft::collection_metadata_uri(collection_id.clone()),
			b"https://example.com/metadata".to_vec()
		);
		assert_eq!(Nft::collection_royalties(collection_id), None);
	});
}

#[test]
fn create_collection_invalid_schema() {
	ExtBuilder::default().build().execute_with(|| {
		let collection_id = b"test-collection".to_vec();

		// duplciate attribute names in schema
		assert_noop!(
			Nft::create_collection(
				Some(1_u64).into(),
				collection_id.clone(),
				vec![
					(b"duplicate-attribute".to_vec(), 0),
					(b"duplicate-attribute".to_vec(), 1)
				],
				None,
				None,
			),
			Error::<Test>::SchemaDuplicateAttribute
		);

		let too_many_attributes: NFTSchema = (0..=MAX_SCHEMA_FIELDS as usize)
			.map(|_| (b"test-attribute".to_vec(), 0_u8))
			.collect();
		assert_noop!(
			Nft::create_collection(
				Some(1_u64).into(),
				collection_id.clone(),
				too_many_attributes.to_owned().to_vec(),
				None,
				None,
			),
			Error::<Test>::SchemaMaxAttributes
		);

		let invalid_nft_attribute_type: NFTAttributeTypeId = 200;
		assert_noop!(
			Nft::create_collection(
				Some(1_u64).into(),
				collection_id,
				vec![(b"invalid-attribute".to_vec(), invalid_nft_attribute_type)],
				None,
				None,
			),
			Error::<Test>::SchemaInvalid
		);
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
		let schema = vec![
			(
				b"test-attribute-1".to_vec(),
				NFTAttributeValue::U8(Default::default()).type_id(),
			),
			(
				b"test-attribute-2".to_vec(),
				NFTAttributeValue::U8(Default::default()).type_id(),
			),
			(
				b"test-attribute-3".to_vec(),
				NFTAttributeValue::Bytes32(Default::default()).type_id(),
			),
		];

		assert_noop!(
			Nft::create_collection(
				Some(owner).into(),
				b"test-collection".to_vec(),
				schema.clone(),
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
fn create_token() {
	ExtBuilder::default().build().execute_with(|| {
		let schema = vec![
			(
				b"test-attribute-1".to_vec(),
				NFTAttributeValue::I32(Default::default()).type_id(),
			),
			(
				b"test-attribute-2".to_vec(),
				NFTAttributeValue::U8(Default::default()).type_id(),
			),
			(
				b"test-attribute-3".to_vec(),
				NFTAttributeValue::Bytes32(Default::default()).type_id(),
			),
		];
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner, schema);

		let token_owner = 2_u64;
		let token_id = 0;
		let royalties_schedule = RoyaltiesSchedule {
			entitlements: vec![(collection_owner, Permill::from_percent(10))],
		};
		assert_eq!(Nft::next_token_id(&collection_id), token_id);
		assert_ok!(Nft::create_token(
			Some(collection_owner).into(),
			collection_id.clone(),
			token_owner,
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
			token_owner.clone()
		)));

		let token = Nft::token_attributes(&collection_id, token_id);
		assert_eq!(
			token,
			vec![
				NFTAttributeValue::I32(-33),
				NFTAttributeValue::U8(Default::default()),
				NFTAttributeValue::Bytes32([1_u8; 32])
			],
		);

		assert_eq!(Nft::token_owner(&collection_id, token_id), token_owner);
		assert_eq!(
			Nft::token_royalties(&collection_id, token_id).expect("royalties plan set"),
			royalties_schedule
		);
		assert_eq!(Nft::collected_tokens(&collection_id, &token_owner), vec![token_id]);
		assert_eq!(
			Nft::next_token_id(&collection_id),
			token_id.checked_add(One::one()).unwrap()
		);
		assert_eq!(Nft::token_issuance(&collection_id), 1);
		// no edition created
		assert!(Nft::next_edition_id(&collection_id).is_zero());
	});
}

#[test]
fn create_multiple_tokens() {
	ExtBuilder::default().build().execute_with(|| {
		let schema = vec![
			(
				b"test-attribute-1".to_vec(),
				NFTAttributeValue::I32(Default::default()).type_id(),
			),
			(
				b"test-attribute-2".to_vec(),
				NFTAttributeValue::U8(Default::default()).type_id(),
			),
			(
				b"test-attribute-3".to_vec(),
				NFTAttributeValue::Bytes32(Default::default()).type_id(),
			),
		];
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner, schema);
		let token_owner = 2_u64;

		assert_ok!(Nft::create_token(
			Some(collection_owner).into(),
			collection_id.clone(),
			token_owner,
			vec![
				NFTAttributeValue::I32(-33),
				NFTAttributeValue::U8(0),
				NFTAttributeValue::Bytes32([1_u8; 32])
			],
			None,
		));

		assert_ok!(Nft::create_token(
			Some(collection_owner).into(),
			collection_id.clone(),
			token_owner,
			vec![
				NFTAttributeValue::I32(33),
				NFTAttributeValue::U8(0),
				NFTAttributeValue::Bytes32([2_u8; 32])
			],
			None,
		));
		assert!(has_event(RawEvent::CreateToken(
			collection_id.clone(),
			1,
			token_owner.clone()
		)));

		assert_eq!(Nft::token_owner(&collection_id, 1), token_owner);
		assert_eq!(Nft::collected_tokens(&collection_id, &token_owner), vec![0, 1]);
		assert_eq!(Nft::next_token_id(&collection_id), 2);
		assert_eq!(Nft::token_issuance(collection_id), 2);
	});
}

#[test]
fn create_token_fails_prechecks() {
	ExtBuilder::default().build().execute_with(|| {
		let schema = vec![
			(
				b"test-attribute-1".to_vec(),
				NFTAttributeValue::I32(Default::default()).type_id(),
			),
			(
				b"test-attribute-2".to_vec(),
				NFTAttributeValue::Url(Default::default()).type_id(),
			),
		];
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner, schema);

		assert_noop!(
			Nft::create_token(
				Some(2_u64).into(),
				collection_id.clone(),
				collection_owner,
				Default::default(),
				None
			),
			Error::<Test>::NoPermission
		);

		assert_noop!(
			Nft::create_token(
				Some(collection_owner).into(),
				b"this-collection-doesn't-exist".to_vec(),
				collection_owner,
				Default::default(),
				None
			),
			Error::<Test>::NoCollection
		);

		// additional attribute vs. schema
		assert_noop!(
			Nft::create_token(
				Some(collection_owner).into(),
				collection_id.clone(),
				collection_owner,
				vec![
					NFTAttributeValue::I32(0),
					NFTAttributeValue::Url(b"test".to_vec()),
					NFTAttributeValue::I32(0)
				],
				None
			),
			Error::<Test>::SchemaMismatch
		);

		// different attribute type vs. schema
		assert_noop!(
			Nft::create_token(
				Some(collection_owner).into(),
				collection_id.clone(),
				collection_owner,
				vec![NFTAttributeValue::U32(404), NFTAttributeValue::U32(404)],
				None,
			),
			Error::<Test>::SchemaMismatch
		);

		let too_many_attributes = vec![NFTAttributeValue::U8(0).clone(); (MAX_SCHEMA_FIELDS + 1_u32) as usize];
		assert_noop!(
			Nft::create_token(
				Some(collection_owner).into(),
				collection_id.clone(),
				collection_owner,
				too_many_attributes,
				None,
			),
			Error::<Test>::SchemaMaxAttributes
		);

		// royalties > 100%
		assert_noop!(
			Nft::create_token(
				Some(collection_owner).into(),
				collection_id.clone(),
				collection_owner,
				vec![NFTAttributeValue::I32(0), NFTAttributeValue::Url(b"test".to_vec())],
				Some(RoyaltiesSchedule::<AccountId> {
					entitlements: vec![
						(3_u64, Permill::from_fraction(1.2)),
						(3_u64, Permill::from_fraction(1.2))
					]
				}),
			),
			Error::<Test>::RoyaltiesOvercommitment
		);

		// attribute value too long
		assert_noop!(
			Nft::create_token(
				Some(collection_owner).into(),
				collection_id,
				collection_owner,
				vec![
					NFTAttributeValue::I32(0),
					NFTAttributeValue::Url([1_u8; <Test as Trait>::MaxAttributeLength::get() as usize + 1].to_vec())
				],
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
		let schema = vec![(
			b"test-attribute".to_vec(),
			NFTAttributeValue::I32(Default::default()).type_id(),
		)];
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner, schema);
		let token_owner = 2_u64;
		let token_id = Nft::next_token_id(&collection_id);
		assert_ok!(Nft::create_token(
			Some(collection_owner).into(),
			collection_id.clone(),
			token_owner,
			vec![NFTAttributeValue::I32(500)],
			None,
		));

		// test
		let new_owner = 3_u64;
		assert_ok!(Nft::transfer(
			Some(token_owner).into(),
			collection_id.clone(),
			token_id,
			new_owner,
		));
		assert!(has_event(RawEvent::Transfer(
			collection_id.clone(),
			token_id,
			new_owner
		)));

		assert_eq!(Nft::token_owner(&collection_id, token_id), new_owner);
		assert!(Nft::collected_tokens(&collection_id, &token_owner).is_empty());
		assert_eq!(Nft::collected_tokens(&collection_id, &new_owner), vec![token_id]);
	});
}

#[test]
fn transfer_fails_prechecks() {
	ExtBuilder::default().build().execute_with(|| {
		// setup token collection + one token
		let schema = vec![(
			b"test-attribute".to_vec(),
			NFTAttributeValue::I32(Default::default()).type_id(),
		)];
		let collection_owner = 1_u64;

		// no collection yet
		assert_noop!(
			Nft::transfer(
				Some(collection_owner).into(),
				b"no-collection".to_vec(),
				1,
				collection_owner
			),
			Error::<Test>::NoCollection,
		);

		let collection_id = setup_collection(collection_owner, schema);
		let token_owner = 2_u64;
		let token_id = Nft::next_token_id(&collection_id);

		// no token yet
		assert_noop!(
			Nft::transfer(Some(token_owner).into(), collection_id.clone(), token_id, token_owner),
			Error::<Test>::NoToken,
		);

		assert_ok!(Nft::create_token(
			Some(collection_owner).into(),
			collection_id.clone(),
			token_owner,
			vec![NFTAttributeValue::I32(500)],
			None,
		));

		let not_the_owner = 3_u64;
		assert_noop!(
			Nft::transfer(
				Some(not_the_owner).into(),
				collection_id.clone(),
				token_id,
				not_the_owner
			),
			Error::<Test>::NoPermission,
		);

		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			collection_id.clone(),
			token_id,
			Some(5),
			16_000,
			1_000,
			None,
		));
		// cannot transfer while listed
		assert_noop!(
			Nft::transfer(Some(token_owner).into(), collection_id.clone(), token_id, token_owner),
			Error::<Test>::TokenListingProtection,
		);
	});
}

#[test]
fn burn() {
	ExtBuilder::default().build().execute_with(|| {
		// setup token collection + one token
		let schema = vec![(
			b"test-attribute".to_vec(),
			NFTAttributeValue::I32(Default::default()).type_id(),
		)];
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner, schema);
		let token_owner = 2_u64;
		let token_id = Nft::next_token_id(&collection_id);
		assert_ok!(Nft::create_token(
			Some(collection_owner).into(),
			collection_id.clone(),
			token_owner,
			vec![NFTAttributeValue::I32(500)],
			None,
		));

		// test
		assert_eq!(Nft::token_issuance(&collection_id), 1);
		assert_ok!(Nft::burn(Some(token_owner).into(), collection_id.clone(), token_id));
		assert!(has_event(RawEvent::Burn(collection_id.clone(), token_id)));

		assert!(!<TokenAttributes<Test>>::contains_key(&collection_id, token_id));
		assert!(!<TokenOwner<Test>>::contains_key(&collection_id, token_id));
		assert!(Nft::collected_tokens(&collection_id, &token_owner).is_empty());
		assert!(Nft::token_issuance(&collection_id).is_zero());
	});
}

#[test]
fn burn_fails_prechecks() {
	ExtBuilder::default().build().execute_with(|| {
		// setup token collection + one token
		let schema = vec![(
			b"test-attribute".to_vec(),
			NFTAttributeValue::I32(Default::default()).type_id(),
		)];
		let collection_owner = 1_u64;
		assert_noop!(
			Nft::burn(Some(collection_owner).into(), b"no-collection".to_vec(), 0),
			Error::<Test>::NoCollection
		);

		let collection_id = setup_collection(collection_owner, schema);
		let token_owner = 2_u64;
		let token_id = Nft::next_token_id(&collection_id);
		assert_noop!(
			Nft::burn(Some(token_owner).into(), collection_id.clone(), token_id),
			Error::<Test>::NoToken,
		);

		assert_ok!(Nft::create_token(
			Some(collection_owner).into(),
			collection_id.clone(),
			token_owner,
			vec![NFTAttributeValue::I32(500)],
			None,
		));

		assert_noop!(
			Nft::burn(Some(3_u64).into(), collection_id.clone(), token_id),
			Error::<Test>::NoPermission,
		);

		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			collection_id.clone(),
			token_id,
			None,
			16_000,
			1_000,
			None,
		));
		// cannot burn while listed
		assert_noop!(
			Nft::burn(Some(token_owner).into(), collection_id, token_id),
			Error::<Test>::TokenListingProtection,
		);
	});
}

#[test]
fn sell() {
	ExtBuilder::default().build().execute_with(|| {
		let (collection_id, token_id, token_owner) = setup_token();
		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			collection_id.clone(),
			token_id,
			Some(5),
			16_000,
			1_000,
			None,
		));

		let expected = Listing::<Test>::FixedPrice(FixedPriceListing::<Test> {
			payment_asset: 16_000,
			fixed_price: 1_000,
			close: System::block_number() + <Test as Trait>::DefaultListingDuration::get(),
			buyer: Some(5),
		});

		let listing = Nft::listings(&collection_id, token_id).expect("token is listed");
		assert_eq!(listing, expected);

		// current block is 1 + duration
		assert!(Nft::listing_end_schedule(
			System::block_number() + <Test as Trait>::DefaultListingDuration::get(),
			(collection_id.clone(), token_id)
		)
		.is_some());

		assert!(has_event(RawEvent::FixedPriceSaleListed(
			collection_id,
			token_id,
			Some(5),
			16_000,
			1_000
		)));
	});
}

#[test]
fn sell_prechecks() {
	ExtBuilder::default().build().execute_with(|| {
		let (collection_id, token_id, token_owner) = setup_token();
		// no permission
		assert_noop!(
			Nft::sell(
				Some(token_owner + 1).into(),
				collection_id.clone(),
				token_id,
				Some(5),
				16_000,
				1_000,
				None,
			),
			Error::<Test>::NoPermission
		);
		// token listed already
		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			collection_id.clone(),
			token_id,
			Some(5),
			16_000,
			1_000,
			None,
		));
		assert_noop!(
			Nft::sell(
				Some(token_owner).into(),
				collection_id.clone(),
				token_id,
				Some(5),
				16_000,
				1_000,
				None,
			),
			Error::<Test>::TokenListingProtection
		);

		// can't auction, listed for fixed price sale
		assert_noop!(
			Nft::auction(
				Some(token_owner).into(),
				collection_id.clone(),
				token_id,
				16_000,
				1_000,
				None,
			),
			Error::<Test>::TokenListingProtection
		);
	});
}

#[test]
fn cancel_sell() {
	ExtBuilder::default().build().execute_with(|| {
		let (collection_id, token_id, token_owner) = setup_token();
		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			collection_id.clone(),
			token_id,
			Some(5),
			16_000,
			1_000,
			None,
		));
		assert_ok!(Nft::cancel_sale(
			Some(token_owner).into(),
			collection_id.clone(),
			token_id,
		));
		assert!(has_event(RawEvent::FixedPriceSaleClosed(
			collection_id.clone(),
			token_id
		)));

		// storage cleared up
		assert!(Nft::listings(&collection_id, token_id).is_none());
		assert!(Nft::listing_end_schedule(
			System::block_number() + <Test as Trait>::DefaultListingDuration::get(),
			(collection_id.clone(), token_id)
		)
		.is_none());

		// it should be free to operate on the token
		assert_ok!(Nft::transfer(
			Some(token_owner).into(),
			collection_id.clone(),
			token_id,
			token_owner + 1,
		));
	});
}

#[test]
fn sell_closes_on_schedule() {
	ExtBuilder::default().build().execute_with(|| {
		let (collection_id, token_id, token_owner) = setup_token();
		let listing_duration = 100;
		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			collection_id.clone(),
			token_id,
			Some(5),
			16_000,
			1_000,
			Some(listing_duration),
		));

		// sale should close after the duration expires
		Nft::on_initialize(System::block_number() + listing_duration);

		assert_eq!(Nft::token_owner(&collection_id, token_id), token_owner);
		assert!(Nft::listings(&collection_id, token_id).is_none());
		assert!(Nft::listing_end_schedule(
			System::block_number() + listing_duration,
			(collection_id.clone(), token_id)
		)
		.is_none());

		// should be free to transfer now
		let new_owner = 8;
		assert_ok!(Nft::transfer(
			Some(token_owner).into(),
			collection_id.clone(),
			token_id,
			new_owner,
		));
	});
}

#[test]
fn buy() {
	ExtBuilder::default().build().execute_with(|| {
		let (collection_id, token_id, token_owner) = setup_token();
		let buyer = 5;
		let payment_asset = 16_000;
		let price = 1_000;

		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			collection_id.clone(),
			token_id,
			Some(buyer),
			payment_asset,
			price,
			None,
		));

		let _ = <Test as Trait>::MultiCurrency::deposit_creating(&buyer, Some(payment_asset), price);
		assert_ok!(Nft::buy(Some(buyer).into(), collection_id.clone(), token_id));
		// no royalties, all proceeds to token owner
		assert_eq!(GenericAsset::free_balance(payment_asset, &token_owner), price,);

		// listing removed
		assert!(Nft::listings(&collection_id, token_id).is_none());
		assert!(Nft::listing_end_schedule(
			System::block_number() + <Test as Trait>::DefaultListingDuration::get(),
			(collection_id.clone(), token_id)
		)
		.is_none());

		// ownership changed
		assert_eq!(Nft::token_owner(&collection_id, token_id), buyer);
		assert_eq!(Nft::collected_tokens(&collection_id, &buyer), vec![token_id]);
	});
}

#[test]
fn buy_with_bespoke_token_royalties() {
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
		let (collection_id, token_id, token_owner) = setup_token_with_royalties(royalties_schedule.clone());
		let buyer = 5;
		let payment_asset = 16_000;
		let sale_price = 1_000_004;

		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			collection_id.clone(),
			token_id,
			Some(buyer),
			payment_asset,
			sale_price,
			None,
		));

		let _ = <Test as Trait>::MultiCurrency::deposit_creating(&buyer, Some(payment_asset), sale_price);
		assert_ok!(Nft::buy(Some(buyer).into(), collection_id.clone(), token_id));
		let presale_issuance = GenericAsset::total_issuance(payment_asset);
		// royalties distributed according to `entitlements` map
		assert_eq!(
			GenericAsset::free_balance(payment_asset, &collection_owner),
			royalties_schedule.entitlements[0].1 * sale_price
		);
		assert_eq!(
			GenericAsset::free_balance(payment_asset, &beneficiary_1),
			royalties_schedule.entitlements[1].1 * sale_price
		);
		assert_eq!(
			GenericAsset::free_balance(payment_asset, &beneficiary_2),
			royalties_schedule.entitlements[2].1 * sale_price
		);
		// token owner gets sale price less royalties
		assert_eq!(
			GenericAsset::free_balance(payment_asset, &token_owner),
			sale_price
				- royalties_schedule
					.entitlements
					.into_iter()
					.map(|(_, e)| e * sale_price)
					.sum::<Balance>()
		);
		assert_eq!(GenericAsset::total_issuance(payment_asset), presale_issuance);

		// listing removed
		assert!(Nft::listings(&collection_id, token_id).is_none());
		assert!(Nft::listing_end_schedule(
			System::block_number() + <Test as Trait>::DefaultListingDuration::get(),
			(collection_id.clone(), token_id)
		)
		.is_none());

		// ownership changed
		assert_eq!(Nft::token_owner(&collection_id, token_id), buyer);
		assert_eq!(Nft::collected_tokens(&collection_id, &buyer), vec![token_id]);
	});
}

#[test]
fn buy_with_collection_royalties() {
	ExtBuilder::default().build().execute_with(|| {
		let beneficiary_1 = 11;
		let beneficiary_2 = 12;
		let collection_owner = 1;
		let royalties_schedule = RoyaltiesSchedule {
			entitlements: vec![
				(collection_owner, Permill::from_fraction(0.125)),
				(beneficiary_1, Permill::from_fraction(0.05)),
				(beneficiary_2, Permill::from_fraction(0.3)),
			],
		};
		let (collection_id, token_id, token_owner) = setup_token_with_royalties(royalties_schedule.clone());
		let buyer = 5;
		let payment_asset = 16_000;
		let sale_price = 1_000;

		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			collection_id.clone(),
			token_id,
			Some(buyer),
			payment_asset,
			sale_price,
			None,
		));

		let _ = <Test as Trait>::MultiCurrency::deposit_creating(&buyer, Some(payment_asset), sale_price);
		assert_ok!(Nft::buy(Some(buyer).into(), collection_id.clone(), token_id));
		let presale_issuance = GenericAsset::total_issuance(payment_asset);
		let for_royalties = royalties_schedule.calculate_total_entitlement() * sale_price;
		// token owner gets sale price less royalties
		assert_eq!(
			GenericAsset::free_balance(payment_asset, &token_owner),
			sale_price - for_royalties
		);
		// royalties distributed according to `entitlements` map
		assert_eq!(
			GenericAsset::free_balance(payment_asset, &collection_owner),
			royalties_schedule.entitlements[0].1 * sale_price
		);
		assert_eq!(
			GenericAsset::free_balance(payment_asset, &beneficiary_1),
			royalties_schedule.entitlements[1].1 * sale_price
		);
		assert_eq!(
			GenericAsset::free_balance(payment_asset, &beneficiary_2),
			royalties_schedule.entitlements[2].1 * sale_price
		);
		assert_eq!(GenericAsset::total_issuance(payment_asset), presale_issuance);

		// listing removed
		assert!(Nft::listings(&collection_id, token_id).is_none());
		assert!(Nft::listing_end_schedule(
			System::block_number() + <Test as Trait>::DefaultListingDuration::get(),
			(collection_id.clone(), token_id)
		)
		.is_none());

		// ownership changed
		assert_eq!(Nft::token_owner(&collection_id, token_id), buyer);
		assert_eq!(Nft::collected_tokens(&collection_id, &buyer), vec![token_id]);
	});
}

#[test]
fn buy_fails_prechecks() {
	ExtBuilder::default().build().execute_with(|| {
		let (collection_id, token_id, token_owner) = setup_token();
		let buyer = 5;
		let payment_asset = 16_000;
		let price = 1_000;

		// not for sale
		assert_noop!(
			Nft::buy(Some(buyer).into(), collection_id.clone(), token_id),
			Error::<Test>::NotForFixedPriceSale,
		);

		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			collection_id.clone(),
			token_id,
			Some(buyer),
			payment_asset,
			price,
			None,
		));

		// no permission
		assert_noop!(
			Nft::buy(Some(buyer + 1).into(), collection_id.clone(), token_id),
			Error::<Test>::NoPermission,
		);

		// fund the buyer with not quite enough
		let _ = <Test as Trait>::MultiCurrency::deposit_creating(&buyer, Some(payment_asset), price - 1);
		assert_noop!(
			Nft::buy(Some(buyer).into(), collection_id.clone(), token_id),
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

		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			collection_id.clone(),
			token_id,
			None,
			payment_asset,
			price,
			None,
		));

		let buyer = 11;
		let _ = <Test as Trait>::MultiCurrency::deposit_creating(&buyer, Some(payment_asset), price);
		assert_ok!(Nft::buy(Some(buyer).into(), collection_id.clone(), token_id));

		// paid
		assert!(GenericAsset::free_balance(payment_asset, &buyer).is_zero());

		// listing removed
		assert!(Nft::listings(&collection_id, token_id).is_none());
		assert!(Nft::listing_end_schedule(
			System::block_number() + <Test as Trait>::DefaultListingDuration::get(),
			(collection_id.clone(), token_id)
		)
		.is_none());

		// ownership changed
		assert_eq!(Nft::token_owner(&collection_id, token_id), buyer);
		assert_eq!(Nft::collected_tokens(&collection_id, &buyer), vec![token_id]);
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
		TokenRoyalties::<Test>::insert(collection_id.clone(), token_id, bad_schedule.clone());

		let buyer = 5;
		let payment_asset = 16_000;
		let price = 1_000;
		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			collection_id.clone(),
			token_id,
			Some(buyer),
			payment_asset,
			price,
			None,
		));

		let _ = <Test as Trait>::MultiCurrency::deposit_creating(&buyer, Some(payment_asset), price);
		let presale_issuance = GenericAsset::total_issuance(payment_asset);

		assert_ok!(Nft::buy(Some(buyer).into(), collection_id.clone(), token_id));

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
		let payment_asset = 16_000;
		let reserve_price = 100_000;

		assert_ok!(Nft::auction(
			Some(token_owner).into(),
			collection_id.clone(),
			token_id,
			payment_asset,
			reserve_price,
			Some(System::block_number() + 1),
		));

		assert_noop!(
			Nft::cancel_sale(Some(token_owner + 1).into(), collection_id.clone(), token_id,),
			Error::<Test>::NoPermission
		);

		assert_ok!(Nft::cancel_sale(
			Some(token_owner).into(),
			collection_id.clone(),
			token_id,
		));

		assert!(has_event(RawEvent::AuctionClosed(
			collection_id.clone(),
			token_id,
			AuctionClosureReason::VendorCancelled
		)));

		// storage cleared up
		assert!(Nft::listings(&collection_id, token_id).is_none());
		assert!(Nft::listing_end_schedule(System::block_number() + 1, (collection_id.clone(), token_id)).is_none());

		// it should be free to operate on the token
		assert_ok!(Nft::transfer(
			Some(token_owner).into(),
			collection_id.clone(),
			token_id,
			token_owner + 1,
		));
	});
}

#[test]
fn auction() {
	ExtBuilder::default().build().execute_with(|| {
		let (collection_id, token_id, token_owner) = setup_token();
		let payment_asset = 16_000;
		let reserve_price = 100_000;

		assert_ok!(Nft::auction(
			Some(token_owner).into(),
			collection_id.clone(),
			token_id,
			payment_asset,
			reserve_price,
			Some(1),
		));

		// first bidder at reserve price
		let bidder_1 = 10;
		let _ = <Test as Trait>::MultiCurrency::deposit_creating(&bidder_1, Some(payment_asset), reserve_price);
		assert_ok!(Nft::bid(
			Some(bidder_1).into(),
			collection_id.clone(),
			token_id,
			reserve_price,
		));
		assert_eq!(GenericAsset::reserved_balance(payment_asset, &bidder_1), reserve_price);

		// second bidder raises bid
		let bidder_2 = 11;
		let _ = <Test as Trait>::MultiCurrency::deposit_creating(&bidder_2, Some(payment_asset), reserve_price + 1);
		assert_ok!(Nft::bid(
			Some(bidder_2).into(),
			collection_id.clone(),
			token_id,
			reserve_price + 1,
		));
		assert!(GenericAsset::reserved_balance(payment_asset, &bidder_1).is_zero()); // bidder_1 funds released
		assert_eq!(
			GenericAsset::reserved_balance(payment_asset, &bidder_2),
			reserve_price + 1
		);

		// end auction
		let _ = Nft::on_initialize(System::block_number() + 1);

		// no royalties, all proceeds to token owner
		assert_eq!(
			GenericAsset::free_balance(payment_asset, &token_owner),
			reserve_price + 1
		);
		// bidder2 funds should be all gone (unreserved and transferred)
		assert!(GenericAsset::free_balance(payment_asset, &bidder_2).is_zero());
		assert!(GenericAsset::reserved_balance(payment_asset, &bidder_2).is_zero());

		// listing metadata removed
		assert!(Nft::listings(&collection_id, token_id).is_none());
		assert!(Nft::listing_end_schedule(System::block_number() + 1, (collection_id.clone(), token_id)).is_none());

		// ownership changed
		assert_eq!(Nft::token_owner(&collection_id, token_id), bidder_2);
		assert_eq!(Nft::collected_tokens(&collection_id, &bidder_2), vec![token_id]);

		// event logged
		assert!(has_event(RawEvent::AuctionSold(
			collection_id.clone(),
			token_id,
			payment_asset,
			reserve_price + 1,
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
		let (collection_id, token_id, token_owner) = setup_token_with_royalties(royalties_schedule.clone());

		assert_ok!(Nft::auction(
			Some(token_owner).into(),
			collection_id.clone(),
			token_id,
			payment_asset,
			reserve_price,
			Some(1),
		));

		// first bidder at reserve price
		let bidder = 10;
		let _ = <Test as Trait>::MultiCurrency::deposit_creating(&bidder, Some(payment_asset), reserve_price);
		assert_ok!(Nft::bid(
			Some(bidder).into(),
			collection_id.clone(),
			token_id,
			reserve_price,
		));

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
		assert!(!Listings::<Test>::contains_key(&collection_id, token_id));
		assert!(!ListingEndSchedule::<Test>::contains_key(
			System::block_number() + 1,
			(&collection_id, token_id)
		));

		// ownership changed
		assert_eq!(Nft::token_owner(&collection_id, token_id), bidder);
		assert_eq!(Nft::collected_tokens(&collection_id, &bidder), vec![token_id]);
	});
}

#[test]
fn close_listings_at_removes_listing_data() {
	ExtBuilder::default().build().execute_with(|| {
		let collection_id = b"test-collection".to_vec();
		let payment_asset = 16_000;
		let price = 123_456;
		let listings = vec![
			// an open sale which won't be bought before closing
			Listing::<Test>::FixedPrice(FixedPriceListing::<Test> {
				payment_asset,
				fixed_price: price,
				buyer: None,
				close: System::block_number() + 1,
			}),
			// an open auction which has no bids before closing
			Listing::<Test>::Auction(AuctionListing::<Test> {
				payment_asset,
				reserve_price: price,
				close: System::block_number() + 1,
			}),
			// an open auction which has a winning bid before closing
			Listing::<Test>::Auction(AuctionListing::<Test> {
				payment_asset,
				reserve_price: price,
				close: System::block_number() + 1,
			}),
		];

		// setup listings storage
		for i in 0..listings.len() {
			Listings::<Test>::insert(&collection_id, i as u32, listings[i].clone());
			ListingEndSchedule::<Test>::insert(System::block_number() + 1, (collection_id.clone(), i as u32), ());
		}
		// winning bidder has no funds, this should cause settlement failure
		ListingWinningBid::<Test>::insert(&collection_id, 2, (11u64, 100u128));

		// Close the listings
		Nft::close_listings_at(System::block_number() + 1);

		// Storage clear
		assert!(
			ListingEndSchedule::<Test>::iter_prefix_values(System::block_number() + 1)
				.count()
				.is_zero()
		);
		for i in 0..listings.len() {
			assert!(Nft::listings(&collection_id, i as u32).is_none());
			assert!(Nft::listing_winning_bid(&collection_id, i as u32).is_none());
			assert!(Nft::listing_end_schedule(System::block_number() + 1, (collection_id.clone(), i as u32)).is_none());
		}

		assert!(has_event(RawEvent::FixedPriceSaleClosed(collection_id.clone(), 0)));
		assert!(has_event(RawEvent::AuctionClosed(
			collection_id.clone(),
			1,
			AuctionClosureReason::ExpiredNoBids
		)));
		assert!(has_event(RawEvent::AuctionClosed(
			collection_id.clone(),
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

		// collection doesn't exist
		assert_noop!(
			Nft::auction(
				Some(token_owner + 1).into(),
				b"no-collection".to_vec(),
				token_id,
				payment_asset,
				reserve_price,
				Some(1),
			),
			Error::<Test>::NoPermission
		);

		// token doesn't exist
		assert_noop!(
			Nft::auction(
				Some(token_owner).into(),
				collection_id.clone(),
				2,
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
				collection_id.clone(),
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
			collection_id.clone(),
			token_id,
			payment_asset,
			reserve_price,
			Some(1),
		));
		// already listed
		assert_noop!(
			Nft::auction(
				Some(token_owner).into(),
				collection_id.clone(),
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
				collection_id.clone(),
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
		assert_noop!(
			Nft::bid(Some(1).into(), b"not-for-sale".to_vec(), 1, 100,),
			Error::<Test>::NotForAuction
		);

		let (collection_id, token_id, token_owner) = setup_token();
		let payment_asset = 16_000;
		let reserve_price = 100_000;

		assert_ok!(Nft::auction(
			Some(token_owner).into(),
			collection_id.clone(),
			token_id,
			payment_asset,
			reserve_price,
			Some(1),
		));

		let bidder = 5;
		// < reserve
		assert_noop!(
			Nft::bid(Some(bidder).into(), collection_id.clone(), token_id, reserve_price - 1,),
			Error::<Test>::BidTooLow
		);

		// no free balance
		assert_noop!(
			Nft::bid(Some(bidder).into(), collection_id.clone(), token_id, reserve_price,),
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
			Nft::bid(Some(bidder).into(), collection_id.clone(), token_id, reserve_price,),
			prml_generic_asset::Error::<Test>::InsufficientBalance
		);
		let _ = <<Test as Trait>::MultiCurrency as MultiCurrencyAccounting>::unreserve(
			&bidder,
			Some(payment_asset),
			reserve_price,
		);

		// <= current bid
		assert_ok!(Nft::bid(
			Some(bidder).into(),
			collection_id.clone(),
			token_id,
			reserve_price,
		));
		assert_noop!(
			Nft::bid(Some(bidder).into(), collection_id.clone(), token_id, reserve_price,),
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
		// mint token Ids 0,1,2
		assert_ok!(Nft::batch_create_token(
			Some(collection_owner).into(),
			collection_id.clone(),
			3,
			token_owner,
			vec![],
			None,
		));

		// test
		let transferred = vec![0_u32, 1, 2];
		let new_owner = 3_u64;
		assert_ok!(Nft::batch_transfer(
			Some(token_owner).into(),
			collection_id.clone(),
			transferred.clone(),
			new_owner,
		));
		assert!(has_event(RawEvent::TransferBatch(
			collection_id.clone(),
			transferred.clone(),
			new_owner
		)));

		for token_id in transferred.clone() {
			assert_eq!(Nft::token_owner(&collection_id, token_id), new_owner);
		}
		assert_eq!(Nft::collected_tokens(&collection_id, &new_owner), transferred);
		assert!(Nft::collected_tokens(&collection_id, &token_owner).is_empty());
		assert_eq!(Nft::next_token_id(&collection_id), 3);
	});
}

#[test]
fn batch_transfer_fails() {
	ExtBuilder::default().build().execute_with(|| {
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner, vec![]);
		let token_owner = 2_u64;
		// mint token Ids 0,1,2
		assert_ok!(Nft::batch_create_token(
			Some(collection_owner).into(),
			collection_id.clone(),
			3,
			token_owner,
			vec![],
			None,
		));

		// token 5 doesn't exist
		let transfer_ids = vec![1_u32, 5];
		let new_owner = 3_u64;
		assert_noop!(
			Nft::batch_transfer(
				Some(token_owner).into(),
				collection_id.clone(),
				transfer_ids.clone(),
				new_owner,
			),
			Error::<Test>::NoToken
		);

		// not owner
		assert_noop!(
			Nft::batch_transfer(
				Some(token_owner + 1).into(),
				collection_id.clone(),
				transfer_ids.clone(),
				new_owner,
			),
			Error::<Test>::NoPermission
		);

		// transfer empty ids should fail
		assert_noop!(
			Nft::batch_transfer(Some(token_owner).into(), collection_id.clone(), vec![], new_owner,),
			Error::<Test>::NoToken
		);

		// token is for sale
		<Listings<Test>>::insert(
			&collection_id,
			1,
			Listing::<Test>::FixedPrice(FixedPriceListing {
				payment_asset: 1,
				fixed_price: 1,
				close: 1,
				buyer: None,
			}),
		);
		assert_noop!(
			Nft::batch_transfer(
				Some(token_owner + 1).into(),
				collection_id.clone(),
				transfer_ids,
				new_owner,
			),
			Error::<Test>::NoPermission
		);
	});
}

#[test]
fn batch_create() {
	ExtBuilder::default().build().execute_with(|| {
		let schema = vec![
			(
				b"test-attribute".to_vec(),
				NFTAttributeValue::I32(Default::default()).type_id(),
			),
			(
				b"test-attribute-2".to_vec(),
				NFTAttributeValue::String(Default::default()).type_id(),
			),
		];
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner, schema);
		let attributes = vec![
			NFTAttributeValue::I32(123),
			NFTAttributeValue::String(b"foobar".to_owned().to_vec()),
		];
		let token_owner = 2_u64;
		// mint token Ids 0-4
		assert_ok!(Nft::batch_create_token(
			Some(collection_owner).into(),
			collection_id.clone(),
			5,
			token_owner,
			attributes.clone(),
			None,
		));

		let edition_id = Nft::next_edition_id(&collection_id);
		let created = vec![0_u32, 1, 2, 3, 4];
		assert!(has_event(RawEvent::CreateBatch(
			collection_id.clone(),
			edition_id,
			token_owner
		)));

		// check token ownership and attributes correct
		for token_id in created.clone() {
			assert_eq!(Nft::token_owner(&collection_id, token_id), token_owner);
			assert_eq!(Nft::token_attributes(&collection_id, token_id), attributes.clone());
		}

		assert_eq!(Nft::collected_tokens(&collection_id, &token_owner), created);
		assert_eq!(
			Nft::editions(&collection_id, edition_id).expect("edition created"),
			Edition::<Test> {
				id: edition_id,
				start_token_id: created[0],
				count: created.len() as u32,
			}
		);
	});
}

#[test]
fn batch_create_fails() {
	ExtBuilder::default().build().execute_with(|| {
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner, vec![]);

		// no edition Ids remain
		assert_noop!(
			Nft::batch_create_token(
				Some(collection_owner).into(),
				collection_id.clone(),
				5,
				collection_owner,
				vec![],
				None,
			),
			Error::<Test>::NoAvailableIds
		);

		// create 0 should fail
		assert_noop!(
			Nft::batch_create_token(
				Some(collection_owner).into(),
				collection_id.clone(),
				0,
				collection_owner,
				vec![],
				None,
			),
			Error::<Test>::NoToken
		);
	});
}
