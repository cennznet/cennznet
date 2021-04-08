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
use sp_runtime::Percent;

type Nft = Module<Test>;
type GenericAsset = prml_generic_asset::Module<Test>;
type System = frame_system::Module<Test>;
type TokenId = u32;

parameter_types! {
	pub const DefaultListingDuration: u64 = 5;
}
impl Trait for Test {
	type Event = Event;
	type TokenId = TokenId;
	type MultiCurrency = prml_generic_asset::Module<Test>;
	type DefaultListingDuration = DefaultListingDuration;
	type WeightInfo = ();
}

// Check the test system contains an event record `event`
fn has_event(event: RawEvent<CollectionId, TokenId, AccountId, AssetId, Balance>) -> bool {
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
		vec![Some(NFTAttributeValue::I32(500))],
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
		vec![Some(NFTAttributeValue::I32(500))],
		Some(token_royalties),
	));

	(collection_id, token_id, token_owner)
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
		assert_eq!(Nft::collection_royalties(collection_id), None);
	});
}

#[test]
fn create_collection_invalid_schema() {
	ExtBuilder::default().build().execute_with(|| {
		let collection_id = b"test-collection".to_vec();
		assert_noop!(
			Nft::create_collection(Some(1_u64).into(), collection_id.clone(), vec![], None,),
			Error::<Test>::SchemaEmpty
		);

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
			),
			Error::<Test>::SchenmaDuplicateAttribute
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
			Nft::create_collection(Some(1_u64).into(), bad_collection_id, vec![], None),
			Error::<Test>::CollectionIdInvalid
		);

		// empty id
		assert_noop!(
			Nft::create_collection(Some(1_u64).into(), vec![], vec![], None),
			Error::<Test>::CollectionIdInvalid
		);

		// non UTF-8 chars
		// kudos: https://www.cl.cam.ac.uk/~mgk25/ucs/examples/UTF-8-test.txt
		let bad_collection_id = vec![0xfe, 0xff];
		assert_noop!(
			Nft::create_collection(Some(1_u64).into(), bad_collection_id, vec![], None),
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
				Some(RoyaltiesSchedule::<AccountId> {
					entitlements: vec![
						(3_u64, Percent::from_fraction(1.2)),
						(4_u64, Percent::from_fraction(3.3))
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
			entitlements: vec![(collection_owner, Percent::from_percent(10))],
		};
		assert_eq!(Nft::next_token_id(&collection_id), token_id);
		assert_ok!(Nft::create_token(
			Some(collection_owner).into(),
			collection_id.clone(),
			token_owner,
			vec![
				Some(NFTAttributeValue::I32(-33)),
				None,
				Some(NFTAttributeValue::Bytes32([1_u8; 32]))
			],
			Some(royalties_schedule.clone()),
		));
		assert!(has_event(RawEvent::CreateToken(
			collection_id.clone(),
			token_id,
			token_owner.clone()
		)));

		let token = Nft::tokens(&collection_id, token_id);
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
		assert_eq!(Nft::collected_tokens(&collection_id, token_owner), vec![token_id]);
		assert_eq!(
			Nft::next_token_id(&collection_id),
			token_id.checked_add(One::one()).unwrap()
		);
		assert_eq!(Nft::token_issuance(&collection_id), 1);
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
				Some(NFTAttributeValue::I32(-33)),
				None,
				Some(NFTAttributeValue::Bytes32([1_u8; 32]))
			],
			None,
		));

		assert_ok!(Nft::create_token(
			Some(collection_owner).into(),
			collection_id.clone(),
			token_owner,
			vec![
				Some(NFTAttributeValue::I32(33)),
				None,
				Some(NFTAttributeValue::Bytes32([2_u8; 32]))
			],
			None,
		));
		assert!(has_event(RawEvent::CreateToken(
			collection_id.clone(),
			1,
			token_owner.clone()
		)));

		assert_eq!(Nft::token_owner(collection_id.clone(), 1), token_owner);
		assert_eq!(Nft::collected_tokens(collection_id.clone(), token_owner), vec![0, 1]);
		assert_eq!(Nft::next_token_id(&collection_id), 2);
		assert_eq!(Nft::token_issuance(collection_id), 2);
	});
}

#[test]
fn create_token_fails_prechecks() {
	ExtBuilder::default().build().execute_with(|| {
		let schema = vec![(
			b"test-attribute-1".to_vec(),
			NFTAttributeValue::I32(Default::default()).type_id(),
		),
		(
			b"test-attribute-2".to_vec(),
			NFTAttributeValue::Url(Default::default()).type_id(),
		)
		];
		let collection_owner = 1_u64;
		let collection_id = setup_collection(collection_owner, schema);

		assert_noop!(
			Nft::create_token(
				Some(2_u64).into(),
				collection_id.clone(),
				collection_owner,
				vec![None, None],
				None
			),
			Error::<Test>::NoPermission
		);

		assert_noop!(
			Nft::create_token(
				Some(collection_owner).into(),
				b"this-collection-doesn't-exist".to_vec(),
				collection_owner,
				vec![None, None],
				None
			),
			Error::<Test>::NoCollection
		);

		assert_noop!(
			Nft::create_token(
				Some(collection_owner).into(),
				collection_id.clone(),
				collection_owner,
				vec![],
				None
			),
			Error::<Test>::SchemaEmpty
		);

		// additional attribute vs. schema
		assert_noop!(
			Nft::create_token(
				Some(collection_owner).into(),
				collection_id.clone(),
				collection_owner,
				vec![None, None, None],
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
				vec![Some(NFTAttributeValue::U32(404)), None],
				None,
			),
			Error::<Test>::SchemaMismatch
		);

		let too_many_attributes: [Option<NFTAttributeValue>; (MAX_SCHEMA_FIELDS + 1_u32) as usize] =
			Default::default();
		assert_noop!(
			Nft::create_token(
				Some(collection_owner).into(),
				collection_id.clone(),
				collection_owner,
				too_many_attributes.to_owned().to_vec(),
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
				vec![None, None],
				Some(RoyaltiesSchedule::<AccountId> {
					entitlements: vec![
						(3_u64, Percent::from_fraction(1.2)),
						(3_u64, Percent::from_fraction(1.2))
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
					None,
					Some(NFTAttributeValue::Url([1_u8; MAX_ATTRIBUTE_LENGTH + 1].to_vec()))
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
			vec![Some(NFTAttributeValue::I32(500))],
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
		assert!(Nft::collected_tokens(&collection_id, token_owner).is_empty());
		assert_eq!(Nft::collected_tokens(&collection_id, new_owner), vec![token_id]);
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
			vec![Some(NFTAttributeValue::I32(500))],
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

		assert_ok!(Nft::direct_sale(
			Some(token_owner).into(),
			collection_id.clone(),
			token_id,
			5,
			16_000,
			1_000
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
			vec![Some(NFTAttributeValue::I32(500))],
			None,
		));

		// test
		assert_ok!(Nft::burn(Some(token_owner).into(), collection_id.clone(), token_id));
		assert!(has_event(RawEvent::Burn(collection_id.clone(), token_id)));

		assert!(!<Tokens<Test>>::contains_key(&collection_id, token_id));
		assert!(!<TokenOwner<Test>>::contains_key(&collection_id, token_id));
		assert!(Nft::collected_tokens(&collection_id, token_owner).is_empty());
		assert_eq!(Nft::tokens_burnt(&collection_id), 1);
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
			vec![Some(NFTAttributeValue::I32(500))],
			None,
		));

		assert_noop!(
			Nft::burn(Some(3_u64).into(), collection_id.clone(), token_id),
			Error::<Test>::NoPermission,
		);

		assert_ok!(Nft::direct_sale(
			Some(token_owner).into(),
			collection_id.clone(),
			token_id,
			5,
			16_000,
			1_000
		));
		// cannot transfer while listed
		assert_noop!(
			Nft::transfer(Some(token_owner).into(), collection_id, token_id, token_owner),
			Error::<Test>::TokenListingProtection,
		);
	});
}

#[test]
fn direct_sale() {
	ExtBuilder::default().build().execute_with(|| {
		let (collection_id, token_id, token_owner) = setup_token();
		assert_ok!(Nft::direct_sale(
			Some(token_owner).into(),
			collection_id.clone(),
			token_id,
			5,
			16_000,
			1_000
		));

		let expected = Listing::<Test>::DirectSale(DirectSaleListing::<Test> {
			payment_asset: 16_000,
			fixed_price: 1_000,
			close: System::block_number() + <Test as Trait>::DefaultListingDuration::get(),
			buyer: 5,
		});

		let listing = Nft::listings(&collection_id, token_id).expect("token is listed");
		assert_eq!(listing, expected);

		// current block is 1 + duration
		assert_eq!(
			Nft::listing_end_blocks(System::block_number() + <Test as Trait>::DefaultListingDuration::get()),
			vec![(collection_id.clone(), token_id)]
		);

		assert!(has_event(RawEvent::DirectSaleListed(
			collection_id,
			token_id,
			5,
			16_000,
			1_000
		)));
	});
}

#[test]
fn direct_sale_prechecks() {
	ExtBuilder::default().build().execute_with(|| {
		let (collection_id, token_id, token_owner) = setup_token();
		// no permission
		assert_noop!(
			Nft::direct_sale(
				Some(token_owner + 1).into(),
				collection_id.clone(),
				token_id,
				5,
				16_000,
				1_000
			),
			Error::<Test>::NoPermission
		);
		// token listed already
		assert_ok!(Nft::direct_sale(
			Some(token_owner).into(),
			collection_id.clone(),
			token_id,
			5,
			16_000,
			1_000
		));
		assert_noop!(
			Nft::direct_sale(
				Some(token_owner).into(),
				collection_id.clone(),
				token_id,
				5,
				16_000,
				1_000
			),
			Error::<Test>::TokenListingProtection
		);
		// TODO: listed for auction should fail here
	});
}

#[test]
fn direct_sale_closes_on_schedule() {
	ExtBuilder::default().build().execute_with(|| {
		let (collection_id, token_id, token_owner) = setup_token();
		assert_ok!(Nft::direct_sale(
			Some(token_owner).into(),
			collection_id.clone(),
			token_id,
			5,
			16_000,
			1_000
		));

		Nft::on_initialize(System::block_number() + <Test as Trait>::DefaultListingDuration::get());

		assert_eq!(Nft::token_owner(&collection_id, token_id), token_owner);
		assert!(!Listings::<Test>::contains_key(&collection_id, token_id));
		assert!(!ListingEndSchedule::<Test>::contains_key(
			System::block_number() + <Test as Trait>::DefaultListingDuration::get()
		));

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
fn direct_purchase() {
	ExtBuilder::default().build().execute_with(|| {
		let (collection_id, token_id, token_owner) = setup_token();
		let buyer = 5;
		let payment_asset = 16_000;
		let price = 1_000;

		assert_ok!(Nft::direct_sale(
			Some(token_owner).into(),
			collection_id.clone(),
			token_id,
			buyer,
			payment_asset,
			price
		));

		let _ = <Test as Trait>::MultiCurrency::deposit_creating(&buyer, Some(payment_asset), price);
		assert_ok!(Nft::direct_purchase(
			Some(buyer).into(),
			collection_id.clone(),
			token_id
		));
		// no royalties, all proceeds to token owner
		assert_eq!(GenericAsset::free_balance(payment_asset, &token_owner), price,);

		// listing removed
		assert!(!Listings::<Test>::contains_key(&collection_id, token_id));
		assert!(ListingEndSchedule::<Test>::get(
			System::block_number() + <Test as Trait>::DefaultListingDuration::get()
		)
		.is_empty());

		// ownership changed
		assert_eq!(<TokenOwner<Test>>::get(&collection_id, token_id), buyer);
		assert_eq!(<CollectedTokens<Test>>::get(&collection_id, buyer), vec![token_id]);
	});
}

#[test]
fn direct_purchase_with_bespoke_token_royalties() {
	ExtBuilder::default().build().execute_with(|| {
		let collection_owner = 1;
		let beneficiary_1 = 11;
		let beneficiary_2 = 12;
		let royalties_schedule = RoyaltiesSchedule {
			entitlements: vec![
				(collection_owner, Percent::from_fraction(0.125)),
				(beneficiary_1, Percent::from_fraction(0.05)),
				(beneficiary_2, Percent::from_fraction(0.03)),
			],
		};
		let (collection_id, token_id, token_owner) = setup_token_with_royalties(royalties_schedule.clone());
		let buyer = 5;
		let payment_asset = 16_000;
		let sale_price = 1_000;

		assert_ok!(Nft::direct_sale(
			Some(token_owner).into(),
			collection_id.clone(),
			token_id,
			buyer,
			payment_asset,
			sale_price
		));

		let _ = <Test as Trait>::MultiCurrency::deposit_creating(&buyer, Some(payment_asset), sale_price);
		assert_ok!(Nft::direct_purchase(
			Some(buyer).into(),
			collection_id.clone(),
			token_id
		));
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
			royalties_schedule.entitlements[0].1 * for_royalties
		);
		assert_eq!(
			GenericAsset::free_balance(payment_asset, &beneficiary_1),
			royalties_schedule.entitlements[1].1 * for_royalties
		);
		assert_eq!(
			GenericAsset::free_balance(payment_asset, &beneficiary_2),
			royalties_schedule.entitlements[2].1 * for_royalties
		);
		assert_eq!(GenericAsset::total_issuance(payment_asset), presale_issuance,);

		// listing removed
		assert!(!Listings::<Test>::contains_key(&collection_id, token_id));
		assert!(ListingEndSchedule::<Test>::get(
			System::block_number() + <Test as Trait>::DefaultListingDuration::get()
		)
		.is_empty());

		// ownership changed
		assert_eq!(<TokenOwner<Test>>::get(&collection_id, token_id), buyer);
		assert_eq!(<CollectedTokens<Test>>::get(&collection_id, buyer), vec![token_id]);
	});
}

#[test]
fn direct_purchase_with_collection_royalties() {
	ExtBuilder::default().build().execute_with(|| {
		let beneficiary_1 = 11;
		let beneficiary_2 = 12;
		let collection_owner = 1;
		let royalties_schedule = RoyaltiesSchedule {
			entitlements: vec![
				(collection_owner, Percent::from_fraction(0.125)),
				(beneficiary_1, Percent::from_fraction(0.05)),
				(beneficiary_2, Percent::from_fraction(0.3)),
			],
		};
		let (collection_id, token_id, token_owner) = setup_token_with_royalties(royalties_schedule.clone());
		let buyer = 5;
		let payment_asset = 16_000;
		let sale_price = 1_000;

		assert_ok!(Nft::direct_sale(
			Some(token_owner).into(),
			collection_id.clone(),
			token_id,
			buyer,
			payment_asset,
			sale_price
		));

		let _ = <Test as Trait>::MultiCurrency::deposit_creating(&buyer, Some(payment_asset), sale_price);
		assert_ok!(Nft::direct_purchase(
			Some(buyer).into(),
			collection_id.clone(),
			token_id
		));
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
			royalties_schedule.entitlements[0].1 * for_royalties
		);
		assert_eq!(
			GenericAsset::free_balance(payment_asset, &beneficiary_1),
			royalties_schedule.entitlements[1].1 * for_royalties
		);
		assert_eq!(
			GenericAsset::free_balance(payment_asset, &beneficiary_2),
			royalties_schedule.entitlements[2].1 * for_royalties
		);
		assert_eq!(GenericAsset::total_issuance(payment_asset), presale_issuance,);

		// listing removed
		assert!(!Listings::<Test>::contains_key(&collection_id, token_id));
		assert!(ListingEndSchedule::<Test>::get(
			System::block_number() + <Test as Trait>::DefaultListingDuration::get()
		)
		.is_empty());

		// ownership changed
		assert_eq!(<TokenOwner<Test>>::get(&collection_id, token_id), buyer);
		assert_eq!(<CollectedTokens<Test>>::get(&collection_id, buyer), vec![token_id]);
	});
}

#[test]
fn direct_purchase_fails_prechecks() {
	ExtBuilder::default().build().execute_with(|| {
		let (collection_id, token_id, token_owner) = setup_token();
		let buyer = 5;
		let payment_asset = 16_000;
		let price = 1_000;

		// not for sale
		assert_noop!(
			Nft::direct_purchase(Some(buyer).into(), collection_id.clone(), token_id),
			Error::<Test>::NotForDirectSale,
		);

		assert_ok!(Nft::direct_sale(
			Some(token_owner).into(),
			collection_id.clone(),
			token_id,
			buyer,
			payment_asset,
			price
		));

		// no permission
		assert_noop!(
			Nft::direct_purchase(Some(buyer + 1).into(), collection_id.clone(), token_id),
			Error::<Test>::NoPermission,
		);

		// fund the buyer with not quite enough
		let _ = <Test as Trait>::MultiCurrency::deposit_creating(&buyer, Some(payment_asset), price - 1);
		assert_noop!(
			Nft::direct_purchase(Some(buyer).into(), collection_id.clone(), token_id),
			prml_generic_asset::Error::<Test>::InsufficientBalance,
		);
		// TODO: listed for auction should fail here
	});
}

#[test]
fn direct_purchase_with_overcommitted_royalties() {
	ExtBuilder::default().build().execute_with(|| {
		// royalties are > 100% total which could create funds out of nothing
		// in this case, default to 0 royalties.
		// royalty schedules should not make it into storage but we protect against it anyway
		let (collection_id, token_id, token_owner) = setup_token();
		let bad_schedule = RoyaltiesSchedule {
			entitlements: vec![
				(11_u64, Percent::from_fraction(0.125)),
				(12_u64, Percent::from_fraction(0.9)),
			],
		};
		TokenRoyalties::<Test>::insert(collection_id.clone(), token_id, bad_schedule.clone());

		let buyer = 5;
		let payment_asset = 16_000;
		let price = 1_000;
		assert_ok!(Nft::direct_sale(
			Some(token_owner).into(),
			collection_id.clone(),
			token_id,
			buyer,
			payment_asset,
			price
		));

		let _ = <Test as Trait>::MultiCurrency::deposit_creating(&buyer, Some(payment_asset), price);
		let presale_issuance = GenericAsset::total_issuance(payment_asset);

		assert_ok!(Nft::direct_purchase(
			Some(buyer).into(),
			collection_id.clone(),
			token_id
		));

		assert!(bad_schedule.calculate_total_entitlement().is_zero());
		assert_eq!(GenericAsset::free_balance(payment_asset, &token_owner), price);
		assert!(GenericAsset::free_balance(payment_asset, &buyer).is_zero());
		assert_eq!(GenericAsset::total_issuance(payment_asset), presale_issuance);
	})
}
