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
use crate::mock::{AccountId, ExtBuilder, GenericAsset, Nft, System, Test};
use cennznet_primitives::types::{SeriesId, TokenId};
use frame_support::{assert_noop, assert_ok, traits::OnInitialize};
use sp_runtime::Permill;
use sp_std::collections::btree_map::BTreeMap;

/// The asset Id used for payment in these tests
const PAYMENT_ASSET: AssetId = 16_001;

// Check the test system contains an event record `event`
// fn has_event(event: Event<Test>) -> bool {
// 	System::events()
// 		.iter()
// 		.find(|e| e.event == Event::Nft(event.clone()))
// 		.is_some()
// }

// Create an NFT series
// Returns the created `series_id`
fn setup_series(owner: AccountId) -> SeriesId {
	let series_id = Nft::next_series_id();
	let series_name = b"test-series".to_vec();
	let metadata_scheme = MetadataScheme::IpfsDir(b"<CID>".to_vec());
	assert_ok!(Nft::create_series(
		Some(owner).into(),
		series_name,
		0,
		None,
		None,
		metadata_scheme,
		None
	));
	series_id
}

/// Setup a token, return series id, token id, token owner
fn setup_token() -> (SeriesId, TokenId, AccountId) {
	let series_owner = 1_u64;
	let series_id = setup_series(series_owner);
	let token_owner = 2_u64;
	let token_id = (series_id, 0);
	assert_ok!(Nft::mint(Some(series_owner).into(), series_id, 1, Some(token_owner),));

	(series_id, token_id, token_owner)
}

/// Setup a token, return series id, token id, token owner
fn setup_token_with_royalties(
	royalties_schedule: RoyaltiesSchedule<AccountId>,
	quantity: TokenCount,
) -> (SeriesId, TokenId, AccountId) {
	let series_owner = 1_u64;
	let series_id = Nft::next_series_id();
	let series_name = b"test-series".to_vec();
	let metadata_scheme = MetadataScheme::IpfsDir(b"<CID>".to_vec());
	assert_ok!(Nft::create_series(
		Some(series_owner).into(),
		series_name,
		0,
		None,
		None,
		metadata_scheme,
		Some(royalties_schedule),
	));

	let token_owner = 2_u64;
	let token_id = (series_id, 0);
	assert_ok!(Nft::mint(
		Some(series_owner).into(),
		series_id,
		quantity,
		Some(token_owner),
	));

	(series_id, token_id, token_owner)
}

/// Create an offer on a token. Return offer_id, offer
fn make_new_simple_offer(
	offer_amount: Balance,
	token_id: TokenId,
	buyer: AccountId,
	marketplace_id: Option<MarketplaceId>,
) -> (OfferId, SimpleOffer<AccountId>) {
	let next_offer_id = Nft::next_offer_id();

	assert_ok!(Nft::make_simple_offer(
		Some(buyer).into(),
		token_id,
		offer_amount,
		PAYMENT_ASSET,
		marketplace_id
	));
	let offer = SimpleOffer {
		token_id,
		asset_id: PAYMENT_ASSET,
		amount: offer_amount,
		buyer,
		marketplace_id,
	};

	// Check storage has been updated
	assert_eq!(Nft::next_offer_id(), next_offer_id + 1);
	assert_eq!(Nft::offers(next_offer_id), Some(OfferType::Simple(offer.clone())));
	// assert!(has_event(RawEvent::OfferMade(
	// 	next_offer_id,
	// 	offer_amount,
	// 	PAYMENT_ASSET,
	// 	marketplace_id,
	// 	buyer
	// )));

	(next_offer_id, offer)
}

#[test]
fn set_owner() {
	ExtBuilder::default().build().execute_with(|| {
		// setup token series + one token
		let series_owner = 1_u64;
		let series_id = setup_series(series_owner);
		let new_owner = 2_u64;

		assert_ok!(Nft::set_owner(Some(series_owner).into(), series_id, new_owner));
		assert_noop!(
			Nft::set_owner(Some(series_owner).into(), series_id, new_owner),
			Error::<Test>::NoPermission
		);
		assert_noop!(
			Nft::set_owner(Some(series_owner).into(), series_id + 1, new_owner),
			Error::<Test>::NoSeries
		);
	});
}

#[test]
fn create_series_works() {
	ExtBuilder::default().build().execute_with(|| {
		let owner = 1_u64;
		let series_id = setup_series(owner);
		let name = b"test-series".to_vec();
		// assert!(has_event(RawEvent::CreateCollection(
		// 	series_id,
		// 	name.clone(),
		// 	owner
		// )));
		assert_eq!(
			Nft::series_info(series_id).unwrap(),
			SeriesInformation {
				owner,
				name,
				metadata_scheme: MetadataScheme::IpfsDir(b"<CID>".to_vec()),
				royalties_schedule: None,
				max_issuance: None,
			}
		);
		assert_eq!(Nft::next_series_id(), series_id + 1);
	});
}

#[test]
fn create_series_invalid_name() {
	ExtBuilder::default().build().execute_with(|| {
		// too long
		let bad_series_name = b"someidentifierthatismuchlongerthanthe32bytelimitsoshouldfail".to_vec();
		let metadata_scheme = MetadataScheme::IpfsDir(b"<CID>".to_vec());
		assert_noop!(
			Nft::create_series(
				Some(1_u64).into(),
				bad_series_name,
				1,
				None,
				None,
				metadata_scheme.clone(),
				None
			),
			Error::<Test>::SeriesNameInvalid
		);

		// empty name
		assert_noop!(
			Nft::create_series(Some(1_u64).into(), vec![], 1, None, None, metadata_scheme.clone(), None),
			Error::<Test>::SeriesNameInvalid
		);

		// non UTF-8 chars
		// kudos: https://www.cl.cam.ac.uk/~mgk25/ucs/examples/UTF-8-test.txt
		let bad_series_name = vec![0xfe, 0xff];
		assert_noop!(
			Nft::create_series(
				Some(1_u64).into(),
				bad_series_name,
				1,
				None,
				None,
				metadata_scheme,
				None
			),
			Error::<Test>::SeriesNameInvalid
		);
	});
}

#[test]
fn create_series_royalties_invalid() {
	ExtBuilder::default().build().execute_with(|| {
		let owner = 1_u64;
		let name = b"test-series".to_vec();
		let metadata_scheme = MetadataScheme::IpfsDir(b"<CID>".to_vec());

		// Too big royalties should fail
		assert_noop!(
			Nft::create_series(
				Some(owner).into(),
				name.clone(),
				1,
				None,
				None,
				metadata_scheme.clone(),
				Some(RoyaltiesSchedule::<AccountId> {
					entitlements: vec![(3_u64, Permill::from_float(1.2)), (4_u64, Permill::from_float(3.3))]
				}),
			),
			Error::<Test>::RoyaltiesInvalid
		);

		// Empty vector should fail
		assert_noop!(
			Nft::create_series(
				Some(owner).into(),
				name,
				1,
				None,
				None,
				metadata_scheme,
				Some(RoyaltiesSchedule::<AccountId> { entitlements: vec![] }),
			),
			Error::<Test>::RoyaltiesInvalid
		);
	})
}

#[test]
fn transfer() {
	ExtBuilder::default().build().execute_with(|| {
		// setup token series + one token
		let series_owner = 1_u64;
		let series_id = Nft::next_series_id();
		let token_owner = 2_u64;
		let token_id = (series_id, 0);
		assert_ok!(Nft::create_series(
			Some(series_owner).into(),
			b"test-series".to_vec(),
			1,
			None,
			Some(token_owner),
			MetadataScheme::IpfsDir(b"<CID>".to_vec()),
			None,
		));

		// test
		let (series_id, _) = token_id;
		let new_owner = 3_u64;
		assert_ok!(Nft::transfer(Some(token_owner).into(), token_id, new_owner,));
		// assert!(has_event(RawEvent::Transfer(
		// 	token_owner,
		// 	series_id,
		// 	series_id,
		// 	vec![serial_number],
		// 	new_owner
		// )));

		assert!(Nft::collected_tokens(series_id, &token_owner).is_empty());
		assert_eq!(Nft::collected_tokens(series_id, &new_owner), vec![token_id]);
		assert_eq!(Nft::token_balance(&token_owner).unwrap().get(&series_id), None);
		assert_eq!(Nft::token_balance(&new_owner).unwrap().get(&series_id), Some(&1));
	});
}

#[test]
fn transfer_fails_prechecks() {
	ExtBuilder::default().build().execute_with(|| {
		// setup token series + one token
		let series_owner = 1_u64;
		let series_id = Nft::next_series_id();
		let token_owner = 2_u64;
		let token_id = (series_id, 0);

		// no token yet
		assert_noop!(
			Nft::transfer(Some(token_owner).into(), token_id, token_owner),
			Error::<Test>::NoPermission,
		);

		assert_ok!(Nft::create_series(
			Some(series_owner).into(),
			b"test-series".to_vec(),
			1,
			None,
			Some(token_owner),
			MetadataScheme::IpfsDir(b"<CID>".to_vec()),
			None,
		));

		let not_the_owner = 3_u64;
		assert_noop!(
			Nft::transfer(Some(not_the_owner).into(), token_id, not_the_owner),
			Error::<Test>::NoPermission,
		);

		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			vec![token_id],
			Some(5),
			PAYMENT_ASSET,
			1_000,
			None,
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
		// setup token series + one token
		let series_owner = 1_u64;
		let series_id = Nft::next_series_id();
		let token_owner = 2_u64;

		assert_ok!(Nft::create_series(
			Some(series_owner).into(),
			b"test-series".to_vec(),
			3,
			None,
			Some(token_owner),
			MetadataScheme::Https(b"example.com/metadata".to_vec()),
			None,
		));

		// test
		assert_ok!(Nft::burn(Some(token_owner).into(), (series_id, 0)));
		// assert!(has_event(RawEvent::Burn(series_id, vec![0])));
		assert_eq!(Nft::token_balance(&token_owner).unwrap().get(&series_id), Some(&2));

		assert_ok!(Nft::burn(Some(token_owner).into(), (series_id, 1)));
		assert_ok!(Nft::burn(Some(token_owner).into(), (series_id, 2)));
		// assert!(has_event(RawEvent::Burn(series_id, vec![1, 2])));

		assert!(!<SeriesIssuance<Test>>::contains_key(series_id));
		assert!(!<SeriesInfo<Test>>::contains_key(series_id));
		assert!(!<TokenOwner<Test>>::contains_key(series_id, 0));
		assert!(!<TokenOwner<Test>>::contains_key(series_id, 1));
		assert!(!<TokenOwner<Test>>::contains_key(series_id, 2));
		assert!(Nft::collected_tokens(series_id, &token_owner).is_empty());
		assert_eq!(Nft::token_balance(&token_owner).unwrap().get(&series_id), None);
	});
}

#[test]
fn burn_fails_prechecks() {
	ExtBuilder::default().build().execute_with(|| {
		// setup token series + one token
		let series_owner = 1_u64;
		let series_id = Nft::next_series_id();
		let token_owner = 2_u64;

		// token doesn't exist yet
		assert_noop!(
			Nft::burn(Some(token_owner).into(), (series_id, 0)),
			Error::<Test>::NoPermission
		);

		assert_ok!(Nft::create_series(
			Some(series_owner).into(),
			b"test-series".to_vec(),
			100,
			None,
			Some(token_owner),
			MetadataScheme::Https(b"example.com/metadata".to_vec()),
			None,
		));

		// Not owner
		assert_noop!(
			Nft::burn(Some(token_owner + 1).into(), (series_id, 0)),
			Error::<Test>::NoPermission,
		);

		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			vec![(series_id, 0)],
			None,
			PAYMENT_ASSET,
			1_000,
			None,
			None,
		));
		// cannot burn while listed
		assert_noop!(
			Nft::burn(Some(token_owner).into(), (series_id, 0)),
			Error::<Test>::TokenListingProtection,
		);
	});
}

#[test]
fn sell() {
	ExtBuilder::default().build().execute_with(|| {
		let series_owner = 1_u64;
		let quantity = 5;
		let series_id = Nft::next_series_id();

		assert_ok!(Nft::create_series(
			Some(series_owner).into(),
			b"test-series".to_vec(),
			quantity,
			None,
			None,
			MetadataScheme::Https(b"example.com/metadata".to_vec()),
			None,
		));

		let tokens = vec![(series_id, 1), (series_id, 3), (series_id, 4)];
		let listing_id = Nft::next_listing_id();

		assert_ok!(Nft::sell(
			Some(series_owner).into(),
			tokens.clone(),
			None,
			PAYMENT_ASSET,
			1_000,
			None,
			None,
		));

		for token in tokens.iter() {
			assert_eq!(Nft::token_locks(token).unwrap(), TokenLockReason::Listed(listing_id));
		}

		let buyer = 3;
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&buyer, PAYMENT_ASSET, 1_000);
		assert_ok!(Nft::buy(Some(buyer).into(), listing_id));
		assert_eq!(Nft::collected_tokens(series_id, &buyer), tokens);
		assert_eq!(Nft::token_balance(&series_owner).unwrap().get(&series_id), Some(&2));
		assert_eq!(
			Nft::token_balance(&buyer).unwrap().get(&series_id),
			Some(&(tokens.len() as TokenCount))
		);
	})
}

#[test]
fn sell_multiple_fails() {
	ExtBuilder::default().build().execute_with(|| {
		let series_owner = 1_u64;
		let series_id = setup_series(series_owner);
		let series_id_2 = setup_series(series_owner);
		// mint some fake tokens
		<TokenOwner<Test>>::insert(series_id, 1, series_owner);
		<TokenOwner<Test>>::insert(series_id, 2, series_owner);
		<TokenOwner<Test>>::insert(series_id_2, 1, series_owner);

		// empty tokens fails
		assert_noop!(
			Nft::sell(
				Some(series_owner).into(),
				vec![],
				None,
				PAYMENT_ASSET,
				1_000,
				None,
				None
			),
			Error::<Test>::NoToken
		);

		// cannot bundle sell tokens from different series
		assert_noop!(
			Nft::sell(
				Some(series_owner).into(),
				vec![(series_id, 1), (series_id_2, 1),],
				None,
				PAYMENT_ASSET,
				1_000,
				None,
				None,
			),
			Error::<Test>::MixedBundleSale
		);
	})
}

#[test]
fn sell_multiple() {
	ExtBuilder::default().build().execute_with(|| {
		let (series_id, token_id, token_owner) = setup_token();
		let listing_id = Nft::next_listing_id();

		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			vec![token_id],
			Some(5),
			PAYMENT_ASSET,
			1_000,
			None,
			None,
		));

		assert_eq!(Nft::token_locks(token_id).unwrap(), TokenLockReason::Listed(listing_id));
		assert!(Nft::open_series_listings(series_id, listing_id).unwrap());

		let expected = Listing::<Test>::FixedPrice(FixedPriceListing::<Test> {
			payment_asset: PAYMENT_ASSET,
			fixed_price: 1_000,
			close: System::block_number() + <Test as Config>::DefaultListingDuration::get(),
			buyer: Some(5),
			tokens: vec![token_id],
			seller: token_owner,
			royalties_schedule: Default::default(),
			marketplace_id: None,
		});

		let listing = Nft::listings(listing_id).expect("token is listed");
		assert_eq!(listing, expected);

		// current block is 1 + duration
		assert!(Nft::listing_end_schedule(
			System::block_number() + <Test as Config>::DefaultListingDuration::get(),
			listing_id
		)
		.unwrap());

		// Can't transfer while listed for sale
		assert_noop!(
			Nft::transfer(Some(token_owner).into(), token_id, token_owner + 1),
			Error::<Test>::TokenListingProtection
		);

		// assert!(has_event(RawEvent::FixedPriceSaleListed(
		// 	series_id,
		// 	listing_id,
		// 	None
		// )));
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
				vec![token_id],
				Some(5),
				PAYMENT_ASSET,
				1_000,
				None,
				None
			),
			Error::<Test>::NoPermission
		);

		// token listed already
		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			vec![token_id],
			Some(5),
			PAYMENT_ASSET,
			1_000,
			None,
			None,
		));
		assert_noop!(
			Nft::sell(
				Some(token_owner).into(),
				vec![token_id],
				Some(5),
				PAYMENT_ASSET,
				1_000,
				None,
				None
			),
			Error::<Test>::TokenListingProtection
		);

		// can't auction, listed for fixed price sale
		assert_noop!(
			Nft::auction(
				Some(token_owner).into(),
				vec![token_id],
				PAYMENT_ASSET,
				1_000,
				None,
				None
			),
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
			vec![token_id],
			Some(5),
			PAYMENT_ASSET,
			1_000,
			None,
			None
		));
		assert_ok!(Nft::cancel_sale(Some(token_owner).into(), listing_id));
		// assert!(has_event(RawEvent::FixedPriceSaleClosed(series_id, listing_id)));

		// storage cleared up
		assert!(Nft::listings(listing_id).is_none());
		assert!(Nft::listing_end_schedule(
			System::block_number() + <Test as Config>::DefaultListingDuration::get(),
			listing_id
		)
		.is_none());

		// it should be free to operate on the token
		assert_ok!(Nft::transfer(Some(token_owner).into(), token_id, token_owner + 1,));
	});
}

#[test]
fn sell_closes_on_schedule() {
	ExtBuilder::default().build().execute_with(|| {
		let (_, token_id, token_owner) = setup_token();
		let listing_duration = 100;
		let listing_id = Nft::next_listing_id();

		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			vec![token_id],
			Some(5),
			PAYMENT_ASSET,
			1_000,
			Some(listing_duration),
			None
		));

		// sale should close after the duration expires
		Nft::on_initialize(System::block_number() + listing_duration);

		// seller should have tokens
		assert!(Nft::listings(listing_id).is_none());
		assert!(Nft::listing_end_schedule(System::block_number() + listing_duration, listing_id).is_none());

		// should be free to transfer now
		let new_owner = 8;
		assert_ok!(Nft::transfer(Some(token_owner).into(), token_id, new_owner,));
	});
}

#[test]
fn updates_fixed_price() {
	ExtBuilder::default().build().execute_with(|| {
		let (_, token_id, token_owner) = setup_token();
		let listing_id = Nft::next_listing_id();
		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			vec![token_id],
			Some(5),
			PAYMENT_ASSET,
			1_000,
			None,
			None
		));
		assert_ok!(Nft::update_fixed_price(Some(token_owner).into(), listing_id, 1_500));
		// assert!(has_event(RawEvent::FixedPriceSalePriceUpdated(
		// 	series_id,
		// 	listing_id
		// )));

		let expected = Listing::<Test>::FixedPrice(FixedPriceListing::<Test> {
			payment_asset: PAYMENT_ASSET,
			fixed_price: 1_500,
			close: System::block_number() + <Test as Config>::DefaultListingDuration::get(),
			buyer: Some(5),
			seller: token_owner,
			tokens: vec![token_id],
			royalties_schedule: Default::default(),
			marketplace_id: None,
		});

		let listing = Nft::listings(listing_id).expect("token is listed");
		assert_eq!(listing, expected);
	});
}

#[test]
fn update_fixed_price_fails() {
	ExtBuilder::default().build().execute_with(|| {
		let (_, token_id, token_owner) = setup_token();
		let payment_asset = PAYMENT_ASSET;
		let reserve_price = 1_000;
		let listing_id = Nft::next_listing_id();

		// can't update, token not listed
		assert_noop!(
			Nft::update_fixed_price(Some(token_owner).into(), listing_id, 1_500),
			Error::<Test>::NotForFixedPriceSale
		);

		assert_ok!(Nft::auction(
			Some(token_owner).into(),
			vec![token_id],
			payment_asset,
			reserve_price,
			Some(System::block_number() + 1),
			None,
		));

		// can't update, listed for auction
		assert_noop!(
			Nft::update_fixed_price(Some(token_owner).into(), listing_id, 1_500),
			Error::<Test>::NotForFixedPriceSale
		);
	});
}

#[test]
fn update_fixed_price_fails_not_owner() {
	ExtBuilder::default().build().execute_with(|| {
		let (_, token_id, token_owner) = setup_token();
		let listing_id = Nft::next_listing_id();
		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			vec![token_id],
			Some(5),
			PAYMENT_ASSET,
			1_000,
			None,
			None
		));

		assert_noop!(
			Nft::update_fixed_price(Some(token_owner + 1).into(), listing_id, 1_500),
			Error::<Test>::NoPermission
		);
	});
}

#[test]
fn register_marketplace() {
	ExtBuilder::default().build().execute_with(|| {
		let account = 1;
		let entitlements: Permill = Permill::from_float(0.1);
		let marketplace_id = Nft::next_marketplace_id();
		assert_ok!(Nft::register_marketplace(Some(account).into(), None, entitlements));
		// assert!(has_event(RawEvent::RegisteredMarketplace(account, entitlements, 0)));
		assert_eq!(Nft::next_marketplace_id(), marketplace_id + 1);
	});
}

#[test]
fn register_marketplace_separate_account() {
	ExtBuilder::default().build().execute_with(|| {
		let account = 1;
		let marketplace_account = 2;
		let entitlements: Permill = Permill::from_float(0.1);
		assert_ok!(Nft::register_marketplace(
			Some(account).into(),
			Some(marketplace_account).into(),
			entitlements
		));
		// assert!(has_event(RawEvent::RegisteredMarketplace(
		// 	marketplace_account,
		// 	entitlements,
		// 	0
		// )));
	});
}

#[test]
fn buy_with_marketplace_royalties() {
	ExtBuilder::default().build().execute_with(|| {
		let series_owner = 1;
		let beneficiary_1 = 11;
		let royalties_schedule = RoyaltiesSchedule {
			entitlements: vec![(beneficiary_1, Permill::from_float(0.1111))],
		};
		let (series_id, _, token_owner) = setup_token_with_royalties(royalties_schedule.clone(), 2);

		let buyer = 5;
		let payment_asset = PAYMENT_ASSET;
		let sale_price = 1_000_008;
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&buyer, payment_asset, sale_price * 2);
		let token_id = (series_id, 0);

		let marketplace_account = 20;
		let initial_balance_marketplace = GenericAsset::free_balance(payment_asset, &marketplace_account);
		let marketplace_entitlement: Permill = Permill::from_float(0.5);
		assert_ok!(Nft::register_marketplace(
			Some(marketplace_account).into(),
			Some(marketplace_account).into(),
			marketplace_entitlement
		));
		let marketplace_id = 0;
		let listing_id = Nft::next_listing_id();
		assert_eq!(listing_id, 0);
		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			vec![token_id],
			Some(buyer),
			payment_asset,
			sale_price,
			None,
			Some(marketplace_id).into(),
		));

		let initial_balance_owner = GenericAsset::free_balance(payment_asset, &series_owner);
		let initial_balance_b1 = GenericAsset::free_balance(payment_asset, &beneficiary_1);

		assert_ok!(Nft::buy(Some(buyer).into(), listing_id));
		let presale_issuance = GenericAsset::total_issuance(payment_asset);
		assert_eq!(
			GenericAsset::free_balance(payment_asset, &marketplace_account),
			initial_balance_marketplace + marketplace_entitlement * sale_price
		);
		assert_eq!(
			GenericAsset::free_balance(payment_asset, &beneficiary_1),
			initial_balance_b1 + royalties_schedule.clone().entitlements[0].1 * sale_price
		);
		// token owner gets sale price less royalties
		assert_eq!(
			GenericAsset::free_balance(payment_asset, &token_owner),
			initial_balance_owner + sale_price
				- marketplace_entitlement * sale_price
				- royalties_schedule.clone().entitlements[0].1 * sale_price
		);
		assert_eq!(GenericAsset::total_issuance(payment_asset), presale_issuance);
	});
}

#[test]
fn list_with_invalid_marketplace_royalties_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let beneficiary_1 = 11;
		let royalties_schedule = RoyaltiesSchedule {
			entitlements: vec![(beneficiary_1, Permill::from_float(0.51))],
		};
		let (series_id, _, token_owner) = setup_token_with_royalties(royalties_schedule.clone(), 2);

		let buyer = 5;
		let payment_asset = PAYMENT_ASSET;
		let sale_price = 1_000_008;
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&buyer, payment_asset, sale_price * 2);
		let token_id = (series_id, 0);

		let marketplace_account = 20;
		let marketplace_entitlement: Permill = Permill::from_float(0.5);
		assert_ok!(Nft::register_marketplace(
			Some(marketplace_account).into(),
			Some(marketplace_account).into(),
			marketplace_entitlement
		));
		let marketplace_id = 0;
		assert_noop!(
			Nft::sell(
				Some(token_owner).into(),
				vec![token_id],
				Some(buyer),
				payment_asset,
				sale_price,
				None,
				Some(marketplace_id).into(),
			),
			Error::<Test>::RoyaltiesInvalid,
		);
	});
}

#[test]
fn buy() {
	ExtBuilder::default().build().execute_with(|| {
		let (series_id, token_id, token_owner) = setup_token();
		let buyer = 5;
		let payment_asset = PAYMENT_ASSET;
		let price = 1_000;
		let listing_id = Nft::next_listing_id();

		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			vec![token_id],
			Some(buyer),
			payment_asset,
			price,
			None,
			None
		));

		let _ = <Test as Config>::MultiCurrency::deposit_creating(&buyer, payment_asset, price);
		assert_ok!(Nft::buy(Some(buyer).into(), listing_id));
		// no royalties, all proceeds to token owner
		assert_eq!(GenericAsset::free_balance(payment_asset, &token_owner), price);

		// listing removed
		assert!(Nft::listings(listing_id).is_none());
		assert!(Nft::listing_end_schedule(
			System::block_number() + <Test as Config>::DefaultListingDuration::get(),
			listing_id
		)
		.is_none());

		// ownership changed
		assert!(Nft::token_locks(&token_id).is_none());
		assert!(Nft::open_series_listings(series_id, listing_id).is_none());
		assert_eq!(Nft::collected_tokens(series_id, &buyer), vec![token_id]);
	});
}

#[test]
fn buy_with_royalties() {
	ExtBuilder::default().build().execute_with(|| {
		let series_owner = 1;
		let beneficiary_1 = 11;
		let beneficiary_2 = 12;
		let royalties_schedule = RoyaltiesSchedule {
			entitlements: vec![
				(series_owner, Permill::from_float(0.111)),
				(beneficiary_1, Permill::from_float(0.1111)),
				(beneficiary_2, Permill::from_float(0.3333)),
			],
		};
		let (series_id, _, token_owner) = setup_token_with_royalties(royalties_schedule.clone(), 2);
		let buyer = 5;
		let payment_asset = PAYMENT_ASSET;
		let sale_price = 1_000_008;
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&buyer, payment_asset, sale_price * 2);
		let token_id = (series_id, 0);

		let listing_id = Nft::next_listing_id();
		assert_eq!(listing_id, 0);
		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			vec![token_id],
			Some(buyer),
			payment_asset,
			sale_price,
			None,
			None
		));

		let initial_balance_owner = GenericAsset::free_balance(payment_asset, &series_owner);
		let initial_balance_b1 = GenericAsset::free_balance(payment_asset, &beneficiary_1);
		let initial_balance_b2 = GenericAsset::free_balance(payment_asset, &beneficiary_2);
		let initial_balance_seller = GenericAsset::free_balance(payment_asset, &token_owner);

		assert_ok!(Nft::buy(Some(buyer).into(), listing_id));
		let presale_issuance = GenericAsset::total_issuance(payment_asset);
		// royalties distributed according to `entitlements` map
		assert_eq!(
			GenericAsset::free_balance(payment_asset, &series_owner),
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
		assert!(Nft::listing_end_schedule(
			System::block_number() + <Test as Config>::DefaultListingDuration::get(),
			listing_id
		)
		.is_none());

		// ownership changed
		assert!(Nft::collected_tokens(series_id, &buyer).contains(&token_id));
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
			vec![token_id],
			Some(buyer),
			payment_asset,
			price,
			None,
			None
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
		let (series_id, token_id, token_owner) = setup_token();
		let payment_asset = PAYMENT_ASSET;
		let price = 1_000;
		let listing_id = Nft::next_listing_id();

		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			vec![token_id],
			None,
			payment_asset,
			price,
			None,
			None
		));

		let buyer = 11;
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&buyer, payment_asset, price);
		assert_ok!(Nft::buy(Some(buyer).into(), listing_id));

		// paid
		assert!(GenericAsset::free_balance(payment_asset, &buyer).is_zero());

		// listing removed
		assert!(Nft::listings(listing_id).is_none());
		assert!(Nft::listing_end_schedule(
			System::block_number() + <Test as Config>::DefaultListingDuration::get(),
			listing_id
		)
		.is_none());

		// ownership changed
		assert_eq!(Nft::collected_tokens(series_id, &buyer), vec![token_id]);
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
			entitlements: vec![(11_u64, Permill::from_float(0.125)), (12_u64, Permill::from_float(0.9))],
		};
		let listing_id = Nft::next_listing_id();

		let buyer = 5;
		let payment_asset = PAYMENT_ASSET;
		let price = 1_000;
		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			vec![token_id],
			Some(buyer),
			payment_asset,
			price,
			None,
			None
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
		let (_, token_id, token_owner) = setup_token();
		let payment_asset = PAYMENT_ASSET;
		let reserve_price = 100_000;
		let listing_id = Nft::next_listing_id();

		assert_ok!(Nft::auction(
			Some(token_owner).into(),
			vec![token_id],
			payment_asset,
			reserve_price,
			Some(System::block_number() + 1),
			None,
		));

		assert_noop!(
			Nft::cancel_sale(Some(token_owner + 1).into(), listing_id),
			Error::<Test>::NoPermission
		);

		assert_ok!(Nft::cancel_sale(Some(token_owner).into(), listing_id,));

		// assert!(has_event(RawEvent::AuctionClosed(
		// 	series_id,
		// 	listing_id,
		// 	AuctionClosureReason::VendorCancelled
		// )));

		// storage cleared up
		assert!(Nft::listings(listing_id).is_none());
		assert!(Nft::listing_end_schedule(System::block_number() + 1, listing_id).is_none());

		// it should be free to operate on the token
		assert_ok!(Nft::transfer(Some(token_owner).into(), token_id, token_owner + 1,));
	});
}

#[test]
fn auction_bundle() {
	ExtBuilder::default().build().execute_with(|| {
		let series_owner = 1_u64;
		let series_id = Nft::next_series_id();
		let quantity = 5;

		assert_ok!(Nft::create_series(
			Some(series_owner).into(),
			b"test-series".to_vec(),
			quantity,
			None,
			None,
			MetadataScheme::Https(b"example.com/metadata".to_vec()),
			None,
		));
		assert_eq!(Nft::token_balance(&series_owner).unwrap().get(&series_id), Some(&(5)));

		let tokens = vec![(series_id, 1), (series_id, 3), (series_id, 4)];
		let listing_id = Nft::next_listing_id();

		assert_ok!(Nft::auction(
			Some(series_owner).into(),
			tokens.clone(),
			PAYMENT_ASSET,
			1_000,
			Some(1),
			None,
		));

		assert!(Nft::open_series_listings(series_id, listing_id).unwrap());
		for token in tokens.iter() {
			assert_eq!(Nft::token_locks(token).unwrap(), TokenLockReason::Listed(listing_id));
		}

		let buyer = 3;
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&buyer, PAYMENT_ASSET, 1_000);
		assert_ok!(Nft::bid(Some(buyer).into(), listing_id, 1_000));
		// end auction
		let _ = Nft::on_initialize(System::block_number() + AUCTION_EXTENSION_PERIOD as u64);

		assert_eq!(Nft::collected_tokens(series_id, &buyer), tokens);
		assert_eq!(Nft::token_balance(&series_owner).unwrap().get(&series_id), Some(&(2)));
		assert_eq!(
			Nft::token_balance(&buyer).unwrap().get(&series_id),
			Some(&(tokens.len() as TokenCount))
		);
	})
}

#[test]
fn auction_bundle_fails() {
	ExtBuilder::default().build().execute_with(|| {
		let series_owner = 1_u64;
		let series_id = setup_series(series_owner);
		let series_id_2 = setup_series(series_owner);
		// mint some fake tokens
		<TokenOwner<Test>>::insert(series_id, 1, series_owner);
		<TokenOwner<Test>>::insert(series_id, 2, series_owner);
		<TokenOwner<Test>>::insert(series_id_2, 1, series_owner);

		// empty tokens fails
		assert_noop!(
			Nft::auction(Some(series_owner).into(), vec![], PAYMENT_ASSET, 1_000, None, None),
			Error::<Test>::NoToken
		);

		// cannot bundle sell tokens from different series
		assert_noop!(
			Nft::auction(
				Some(series_owner).into(),
				vec![(series_id, 1), (series_id_2, 1),],
				PAYMENT_ASSET,
				1_000,
				None,
				None
			),
			Error::<Test>::MixedBundleSale
		);
	})
}

#[test]
fn auction() {
	ExtBuilder::default().build().execute_with(|| {
		let (series_id, token_id, token_owner) = setup_token();
		let payment_asset = PAYMENT_ASSET;
		let reserve_price = 100_000;

		let listing_id = Nft::next_listing_id();

		assert_ok!(Nft::auction(
			Some(token_owner).into(),
			vec![token_id],
			payment_asset,
			reserve_price,
			Some(1),
			None,
		));
		assert_eq!(
			Nft::token_locks(&token_id).unwrap(),
			TokenLockReason::Listed(listing_id)
		);
		assert_eq!(Nft::next_listing_id(), listing_id + 1);
		assert!(Nft::open_series_listings(series_id, listing_id).unwrap());

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
		let _ = Nft::on_initialize(System::block_number() + AUCTION_EXTENSION_PERIOD as u64);

		// no royalties, all proceeds to token owner
		assert_eq!(GenericAsset::free_balance(payment_asset, &token_owner), winning_bid);
		// bidder2 funds should be all gone (unreserved and transferred)
		assert!(GenericAsset::free_balance(payment_asset, &bidder_2).is_zero());
		assert!(GenericAsset::reserved_balance(payment_asset, &bidder_2).is_zero());

		// listing metadata removed
		assert!(Nft::listings(listing_id).is_none());
		assert!(Nft::listing_end_schedule(System::block_number() + 1, listing_id).is_none());

		// ownership changed
		assert!(Nft::token_locks(&token_id).is_none());
		assert_eq!(Nft::collected_tokens(series_id, &bidder_2), vec![token_id]);
		assert!(Nft::open_series_listings(series_id, listing_id).is_none());

		// event logged
		// assert!(has_event(RawEvent::AuctionSold(
		// 	series_id,
		// 	listing_id,
		// 	payment_asset,
		// 	winning_bid,
		// 	bidder_2
		// )));
	});
}

#[test]
fn bid_auto_extends() {
	ExtBuilder::default().build().execute_with(|| {
		let (_, token_id, token_owner) = setup_token();
		let payment_asset = PAYMENT_ASSET;
		let reserve_price = 100_000;

		let listing_id = Nft::next_listing_id();

		assert_ok!(Nft::auction(
			Some(token_owner).into(),
			vec![token_id],
			payment_asset,
			reserve_price,
			Some(2),
			None,
		));

		// Place bid
		let bidder_1 = 10;
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&bidder_1, payment_asset, reserve_price);
		assert_ok!(Nft::bid(Some(bidder_1).into(), listing_id, reserve_price,));

		if let Some(Listing::Auction(listing)) = Nft::listings(listing_id) {
			assert_eq!(listing.close, System::block_number() + AUCTION_EXTENSION_PERIOD as u64);
		}
		assert!(
			Nft::listing_end_schedule(System::block_number() + AUCTION_EXTENSION_PERIOD as u64, listing_id).unwrap()
		);
	});
}

#[test]
fn auction_royalty_payments() {
	ExtBuilder::default().build().execute_with(|| {
		let payment_asset = PAYMENT_ASSET;
		let reserve_price = 100_004;
		let beneficiary_1 = 11;
		let beneficiary_2 = 12;
		let series_owner = 1;
		let royalties_schedule = RoyaltiesSchedule {
			entitlements: vec![
				(series_owner, Permill::from_float(0.1111)),
				(beneficiary_1, Permill::from_float(0.1111)),
				(beneficiary_2, Permill::from_float(0.1111)),
			],
		};
		let (series_id, token_id, token_owner) = setup_token_with_royalties(royalties_schedule.clone(), 1);
		let listing_id = Nft::next_listing_id();

		assert_ok!(Nft::auction(
			Some(token_owner).into(),
			vec![token_id],
			payment_asset,
			reserve_price,
			Some(1),
			None,
		));

		// first bidder at reserve price
		let bidder = 10;
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&bidder, payment_asset, reserve_price);
		assert_ok!(Nft::bid(Some(bidder).into(), listing_id, reserve_price,));

		// end auction
		let _ = Nft::on_initialize(System::block_number() + AUCTION_EXTENSION_PERIOD as u64);

		// royalties paid out
		let presale_issuance = GenericAsset::total_issuance(payment_asset);
		// royalties distributed according to `entitlements` map
		assert_eq!(
			GenericAsset::free_balance(payment_asset, &series_owner),
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
		assert_eq!(Nft::collected_tokens(series_id, &bidder), vec![token_id]);
	});
}

#[test]
fn close_listings_at_removes_listing_data() {
	ExtBuilder::default().build().execute_with(|| {
		let series_id = Nft::next_series_id();
		let payment_asset = PAYMENT_ASSET;
		let price = 123_456;

		let token_1 = (series_id, 0);

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
				marketplace_id: None,
			}),
			// an open auction which has no bids before closing
			Listing::<Test>::Auction(AuctionListing::<Test> {
				payment_asset,
				reserve_price: price,
				close: System::block_number() + 1,
				seller: 1,
				tokens: vec![token_1],
				royalties_schedule: Default::default(),
				marketplace_id: None,
			}),
			// an open auction which has a winning bid before closing
			Listing::<Test>::Auction(AuctionListing::<Test> {
				payment_asset,
				reserve_price: price,
				close: System::block_number() + 1,
				seller: 1,
				tokens: vec![token_1],
				royalties_schedule: Default::default(),
				marketplace_id: None,
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
			assert!(Nft::listing_end_schedule(System::block_number() + 1, listing_id).is_none());
		}

		// assert!(has_event(RawEvent::FixedPriceSaleClosed(series_id, 0)));
		// assert!(has_event(RawEvent::AuctionClosed(
		// 	series_id,
		// 	1,
		// 	AuctionClosureReason::ExpiredNoBids
		// )));
		// assert!(has_event(RawEvent::AuctionClosed(
		// 	series_id,
		// 	2,
		// 	AuctionClosureReason::SettlementFailed
		// )));
	});
}

#[test]
fn auction_fails_prechecks() {
	ExtBuilder::default().build().execute_with(|| {
		let (series_id, token_id, token_owner) = setup_token();
		let payment_asset = PAYMENT_ASSET;
		let reserve_price = 100_000;

		let missing_token_id = (series_id, 2);

		// token doesn't exist
		assert_noop!(
			Nft::auction(
				Some(token_owner).into(),
				vec![missing_token_id],
				payment_asset,
				reserve_price,
				Some(1),
				None,
			),
			Error::<Test>::NoPermission
		);

		// not owner
		assert_noop!(
			Nft::auction(
				Some(token_owner + 1).into(),
				vec![token_id],
				payment_asset,
				reserve_price,
				Some(1),
				None,
			),
			Error::<Test>::NoPermission
		);

		// setup listed token, and try list it again
		assert_ok!(Nft::auction(
			Some(token_owner).into(),
			vec![token_id],
			payment_asset,
			reserve_price,
			Some(1),
			None,
		));
		// already listed
		assert_noop!(
			Nft::auction(
				Some(token_owner).into(),
				vec![token_id],
				payment_asset,
				reserve_price,
				Some(1),
				None,
			),
			Error::<Test>::TokenListingProtection
		);

		// listed for auction
		assert_noop!(
			Nft::sell(
				Some(token_owner).into(),
				vec![token_id],
				None,
				payment_asset,
				reserve_price,
				None,
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
			vec![token_id],
			payment_asset,
			reserve_price,
			Some(1),
			None,
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
fn create_series() {
	ExtBuilder::default().build().execute_with(|| {
		let series_owner = 1_u64;
		let token_owner = 2_u64;
		let quantity = 5;
		let series_id = Nft::next_series_id();
		let royalties_schedule = RoyaltiesSchedule {
			entitlements: vec![(series_owner, Permill::one())],
		};

		// mint token Ids 0-4
		assert_ok!(Nft::create_series(
			Some(series_owner).into(),
			b"test-series".to_vec(),
			quantity,
			None,
			Some(token_owner),
			MetadataScheme::Https(b"example.com/metadata".to_vec()),
			Some(royalties_schedule.clone()),
		));

		// assert!(has_event(RawEvent::CreateSeries(
		// 	series_id,
		// 	series_id,
		// 	quantity,
		// 	token_owner
		// )));

		// check token ownership
		assert_eq!(Nft::series_issuance(series_id).unwrap(), quantity);
		assert_eq!(
			Nft::series_info(series_id).unwrap().royalties_schedule,
			Some(royalties_schedule)
		);
		// We minted series token 0, next series token id is 1
		assert_eq!(Nft::next_series_id(), 1);
		assert_eq!(
			Nft::collected_tokens(series_id, &token_owner),
			vec![0, 1, 2, 3, 4]
				.into_iter()
				.map(|t| (series_id, t))
				.collect::<Vec<TokenId>>(),
		);
		assert_eq!(Nft::token_balance(&token_owner).unwrap().get(&series_id), Some(&(5)));

		// check we can mint some more
		// mint token Ids 5-7
		let additional_quantity = 3;
		assert_ok!(Nft::mint(
			Some(series_owner).into(),
			series_id,
			additional_quantity,
			Some(token_owner + 1), // new owner this time
		));
		assert_eq!(
			Nft::token_balance(&token_owner + 1).unwrap().get(&series_id),
			Some(&(3))
		);
		assert_eq!(
			Nft::next_serial_number(series_id).unwrap(),
			quantity + additional_quantity
		);

		assert_eq!(
			Nft::collected_tokens(series_id, &token_owner),
			vec![0, 1, 2, 3, 4]
				.into_iter()
				.map(|t| (series_id, t))
				.collect::<Vec<TokenId>>()
		);
		assert_eq!(
			Nft::collected_tokens(series_id, &(token_owner + 1)),
			vec![5, 6, 7]
				.into_iter()
				.map(|t| (series_id, t))
				.collect::<Vec<TokenId>>()
		);
		assert_eq!(Nft::series_issuance(series_id).unwrap(), quantity + additional_quantity);
	});
}

#[test]
fn mint_over_max_issuance_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let series_owner = 1_u64;
		let token_owner = 2_u64;
		let initial_issuance = 2;
		let max_issuance = 5;
		let series_id = Nft::next_series_id();

		// mint token Ids 0-1
		assert_ok!(Nft::create_series(
			Some(series_owner).into(),
			b"test-series".to_vec(),
			initial_issuance,
			Some(max_issuance),
			Some(token_owner),
			MetadataScheme::Https(b"example.com/metadata".to_vec()),
			None,
		));
		assert_eq!(Nft::series_issuance(series_id).unwrap(), initial_issuance);

		// Mint tokens 2-5
		assert_ok!(Nft::mint(Some(series_owner).into(), series_id, 3, Some(token_owner)));
		assert_eq!(Nft::series_issuance(series_id).unwrap(), initial_issuance + 3);

		// No more can be minted as max issuance has been reached
		assert_noop!(
			Nft::mint(Some(series_owner).into(), series_id, 1, Some(token_owner)),
			Error::<Test>::MaxIssuanceReached
		);

		// Even if tokens are burned, more can't be minted
		assert_ok!(Nft::burn(Some(token_owner).into(), (series_id, 0)));
		assert_noop!(
			Nft::mint(Some(series_owner).into(), series_id, 1, Some(token_owner)),
			Error::<Test>::MaxIssuanceReached
		);
	});
}

#[test]
fn invalid_max_issuance_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		// Max issuance of 0 should fail
		assert_noop!(
			Nft::create_series(
				Some(1_u64).into(),
				b"test-series".to_vec(),
				0,
				Some(0),
				None,
				MetadataScheme::Https(b"example.com/metadata".to_vec()),
				None,
			),
			Error::<Test>::InvalidMaxIssuance
		);

		// Max issuance lower than initial issuance should fail
		assert_noop!(
			Nft::create_series(
				Some(1_u64).into(),
				b"test-series".to_vec(),
				5,
				Some(2),
				None,
				MetadataScheme::Https(b"example.com/metadata".to_vec()),
				None,
			),
			Error::<Test>::InvalidMaxIssuance
		);
	});
}

#[test]
fn mint_fails() {
	ExtBuilder::default().build().execute_with(|| {
		let series_owner = 1_u64;
		let series_id = Nft::next_series_id();

		// mint token Ids 0-4
		assert_ok!(Nft::create_series(
			Some(series_owner).into(),
			b"test-series".to_vec(),
			5,
			None,
			None,
			MetadataScheme::Https(b"example.com/metadata".to_vec()),
			None,
		));

		// add 0 additional fails
		assert_noop!(
			Nft::mint(Some(series_owner).into(), series_id, 0, None),
			Error::<Test>::NoToken
		);

		// add to non-existing series fails
		assert_noop!(
			Nft::mint(Some(series_owner).into(), series_id + 1, 5, None),
			Error::<Test>::NoSeries
		);

		// not series owner
		assert_noop!(
			Nft::mint(Some(series_owner + 1).into(), series_id, 5, None),
			Error::<Test>::NoPermission
		);

		assert_ok!(Nft::create_series(
			Some(series_owner).into(),
			b"test-series".to_vec(),
			1,
			None,
			None,
			MetadataScheme::IpfsDir(b"<CID>".to_vec()),
			None,
		));
	});
}

#[test]
fn get_series_listings_on_no_active_listings() {
	ExtBuilder::default().build().execute_with(|| {
		let owner = 1_u64;
		let series_id = setup_series(owner);
		let cursor: u128 = 0;
		let limit: u16 = 100;

		// Should return an empty array as no NFTs have been listed
		let response = Nft::series_listings(series_id, cursor, limit);

		assert_eq!(response.0, None);
		assert_eq!(response.1, vec![]);
	});
}

#[test]
fn get_series_listings() {
	ExtBuilder::default().build().execute_with(|| {
		let owner = 1_u64;
		let cursor: u128 = 0;
		let limit: u16 = 100;
		let quantity = 200;

		let series_id = Nft::next_series_id();
		// mint token Ids
		assert_ok!(Nft::create_series(
			Some(owner).into(),
			b"test-series".to_vec(),
			quantity,
			None,
			None,
			MetadataScheme::Https(b"example.com/metadata".to_vec()),
			None,
		));
		// assert!(has_event(RawEvent::CreateSeries(
		// 	series_id,
		// 	quantity,
		// 	owner
		// )));

		let payment_asset = PAYMENT_ASSET;
		let price = 1_000;
		let close = 10;
		// List tokens for sale
		for serial_number in 0..quantity {
			let token_id: TokenId = (series_id, serial_number);
			assert_ok!(Nft::sell(
				Some(owner).into(),
				vec![token_id],
				None,
				payment_asset,
				price,
				Some(close),
				None,
			));
		}

		// Should return an empty array as no NFTs have been listed
		let (new_cursor, listings) = Nft::series_listings(series_id, cursor, limit);
		let royalties_schedule = RoyaltiesSchedule { entitlements: vec![] };
		assert_eq!(new_cursor, Some(limit as u128));

		// Check the response is as expected
		for id in 0..limit {
			let token_id: Vec<TokenId> = vec![(series_id, id as u32)];
			let expected_listing = FixedPriceListing {
				payment_asset,
				fixed_price: price,
				close: close + 1,
				buyer: None,
				seller: owner,
				tokens: token_id,
				royalties_schedule: royalties_schedule.clone(),
				marketplace_id: None,
			};
			let expected_listing = Listing::FixedPrice(expected_listing);
			assert_eq!(listings[id as usize], (id as u128, expected_listing));
		}
	});
}

#[test]
fn get_series_listings_over_limit() {
	ExtBuilder::default().build().execute_with(|| {
		let owner = 1_u64;
		let cursor: u128 = 0;
		let limit: u16 = 1000;

		let quantity = 200;
		let series_id = Nft::next_series_id();
		// mint token Ids
		assert_ok!(Nft::create_series(
			Some(owner).into(),
			b"test-series".to_vec(),
			quantity,
			None,
			None,
			MetadataScheme::Https(b"example.com/metadata".to_vec()),
			None,
		));
		// assert!(has_event(RawEvent::CreateSeries(
		// 	series_id,
		// 	quantity,
		// 	owner
		// )));

		let payment_asset = PAYMENT_ASSET;
		let price = 1_000;
		let close = 10;
		// List tokens for sale
		for serial_number in 0..quantity {
			let token_id: TokenId = (series_id, serial_number);
			assert_ok!(Nft::sell(
				Some(owner).into(),
				vec![token_id],
				None,
				payment_asset,
				price,
				Some(close),
				None,
			));
		}

		// Should return an empty array as no NFTs have been listed
		let (new_cursor, _listings) = Nft::series_listings(series_id, cursor, limit);
		assert_eq!(new_cursor, Some(100));
	});
}

#[test]
fn get_series_listings_cursor_too_high() {
	ExtBuilder::default().build().execute_with(|| {
		let owner = 1_u64;
		let cursor: u128 = 300;
		let limit: u16 = 1000;

		let quantity = 200;
		let series_id = Nft::next_series_id();
		// mint token Ids
		assert_ok!(Nft::create_series(
			Some(owner).into(),
			b"test-series".to_vec(),
			quantity,
			None,
			None,
			MetadataScheme::Https(b"example.com/metadata".to_vec()),
			None,
		));
		// assert!(has_event(RawEvent::CreateSeries(
		// 	series_id,
		// 	quantity,
		// 	owner
		// )));

		// Should return an empty array as no NFTs have been listed
		let (new_cursor, listings) = Nft::series_listings(series_id, cursor, limit);
		assert_eq!(listings, vec![]);
		assert_eq!(new_cursor, None);
	});
}

#[test]
fn token_uri_construction() {
	ExtBuilder::default().build().execute_with(|| {
		let owner = 1_u64;
		let quantity = 5;
		let series_id = Nft::next_series_id();
		// mint token Ids
		assert_ok!(Nft::create_series(
			Some(owner).into(),
			b"test-series".to_vec(),
			quantity,
			None,
			None,
			MetadataScheme::Https(b"example.com/metadata".to_vec()),
			None,
		));

		assert_eq!(
			Nft::token_uri((series_id, 0)),
			b"https://example.com/metadata/0.json".to_vec(),
		);
		assert_eq!(
			Nft::token_uri((series_id, 1)),
			b"https://example.com/metadata/1.json".to_vec(),
		);

		assert_ok!(Nft::create_series(
			Some(owner).into(),
			b"test-series".to_vec(),
			quantity,
			None,
			None,
			MetadataScheme::Http(b"test.example.com/metadata".to_vec()),
			None,
		));

		assert_eq!(
			Nft::token_uri((series_id + 1, 1)),
			b"http://test.example.com/metadata/1.json".to_vec(),
		);

		assert_ok!(Nft::create_series(
			Some(owner).into(),
			b"test-series".to_vec(),
			quantity,
			None,
			None,
			MetadataScheme::IpfsDir(b"bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi".to_vec()),
			None,
		));
		assert_eq!(
			Nft::token_uri((series_id + 2, 1)),
			b"ipfs://bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi/1.json".to_vec(),
		);

		assert_ok!(Nft::create_series(
			Some(owner).into(),
			b"test-series".to_vec(),
			quantity,
			None,
			None,
			MetadataScheme::IpfsShared(b"bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi".to_vec()),
			None,
		));
		assert_eq!(
			Nft::token_uri((series_id + 3, 1)),
			b"ipfs://bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi.json".to_vec(),
		);
	});
}

#[test]
fn make_simple_offer() {
	ExtBuilder::default().build().execute_with(|| {
		let (_, token_id, _) = setup_token();
		let buyer: u64 = 3;
		let offer_amount: Balance = 100;
		let initial_balance_buyer: Balance = 1000;
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&buyer, PAYMENT_ASSET, initial_balance_buyer);
		assert_eq!(GenericAsset::reserved_balance(PAYMENT_ASSET, &buyer), 0);

		let (offer_id, _) = make_new_simple_offer(offer_amount, token_id, buyer, None);
		assert_eq!(Nft::token_offers(token_id).unwrap(), vec![offer_id]);
		// Check funds have been locked
		assert_eq!(
			GenericAsset::free_balance(PAYMENT_ASSET, &buyer),
			initial_balance_buyer - offer_amount
		);
		assert_eq!(GenericAsset::reserved_balance(PAYMENT_ASSET, &buyer), offer_amount);
	});
}

#[test]
fn make_simple_offer_insufficient_funds_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let (_, token_id, _) = setup_token();
		let buyer: u64 = 3;
		let offer_amount: Balance = 100;
		let next_offer_id = Nft::next_offer_id();
		assert_eq!(GenericAsset::free_balance(PAYMENT_ASSET, &buyer), 0);

		assert_noop!(
			Nft::make_simple_offer(Some(buyer).into(), token_id, offer_amount, PAYMENT_ASSET, None),
			crml_generic_asset::Error::<Test>::InsufficientBalance
		);

		// Check storage has not been updated
		assert_eq!(Nft::next_offer_id(), next_offer_id);
		assert!(Nft::token_offers(token_id).is_none());
		assert_eq!(Nft::offers(next_offer_id), None);
		// Check funds have not been locked
		assert_eq!(GenericAsset::reserved_balance(PAYMENT_ASSET, &buyer), 0);
	});
}

#[test]
fn make_simple_offer_zero_amount_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let (_, token_id, _) = setup_token();
		let buyer: u64 = 3;
		let offer_amount: Balance = 0;
		let next_offer_id = Nft::next_offer_id();
		assert_eq!(GenericAsset::free_balance(PAYMENT_ASSET, &buyer), 0);

		assert_noop!(
			Nft::make_simple_offer(Some(buyer).into(), token_id, offer_amount, PAYMENT_ASSET, None),
			Error::<Test>::ZeroOffer
		);

		// Check storage has not been updated
		assert_eq!(Nft::next_offer_id(), next_offer_id);
		assert!(Nft::token_offers(token_id).is_none());
		assert_eq!(Nft::offers(next_offer_id), None);
		// Check funds have not been locked
		assert_eq!(GenericAsset::reserved_balance(PAYMENT_ASSET, &buyer), 0);
	});
}

#[test]
fn make_simple_offer_token_owner_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let (_, token_id, token_owner) = setup_token();
		let offer_amount: Balance = 100;
		let next_offer_id = Nft::next_offer_id();

		assert_noop!(
			Nft::make_simple_offer(Some(token_owner).into(), token_id, offer_amount, PAYMENT_ASSET, None),
			Error::<Test>::IsTokenOwner
		);

		// Check storage has not been updated
		assert_eq!(Nft::next_offer_id(), next_offer_id);
		assert!(Nft::token_offers(token_id).is_none());
		assert_eq!(Nft::offers(next_offer_id), None);
		// Check funds have not been locked
		assert_eq!(GenericAsset::reserved_balance(PAYMENT_ASSET, &token_owner), 0);
	});
}

#[test]
fn make_simple_offer_on_fixed_price_listing() {
	ExtBuilder::default().build().execute_with(|| {
		let (_, token_id, token_owner) = setup_token();
		let buyer: u64 = 3;
		let offer_amount: Balance = 100;
		let initial_balance_buyer: Balance = 1000;
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&buyer, PAYMENT_ASSET, initial_balance_buyer);
		assert_eq!(GenericAsset::reserved_balance(PAYMENT_ASSET, &buyer), 0);

		let sell_price = 100_000;
		assert_ok!(Nft::sell(
			Some(token_owner).into(),
			vec![token_id],
			None,
			PAYMENT_ASSET,
			sell_price,
			None,
			None,
		));

		make_new_simple_offer(offer_amount, token_id, buyer, None);
		// Check funds have been locked
		assert_eq!(
			GenericAsset::free_balance(PAYMENT_ASSET, &buyer),
			initial_balance_buyer - offer_amount
		);
		assert_eq!(GenericAsset::reserved_balance(PAYMENT_ASSET, &buyer), offer_amount);
	});
}

#[test]
fn make_simple_offer_on_auction_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let (_, token_id, token_owner) = setup_token();
		let buyer: u64 = 3;
		let offer_amount: Balance = 100;
		let initial_balance_buyer: Balance = 1000;
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&buyer, PAYMENT_ASSET, initial_balance_buyer);
		assert_eq!(GenericAsset::reserved_balance(PAYMENT_ASSET, &buyer), 0);

		let reserve_price = 100_000;

		assert_ok!(Nft::auction(
			Some(token_owner).into(),
			vec![token_id],
			PAYMENT_ASSET,
			reserve_price,
			Some(System::block_number() + 1),
			None,
		));

		let next_offer_id = Nft::next_offer_id();
		assert_noop!(
			Nft::make_simple_offer(Some(buyer).into(), token_id, offer_amount, PAYMENT_ASSET, None),
			Error::<Test>::TokenOnAuction
		);

		// Check storage has not been updated
		assert_eq!(Nft::next_offer_id(), next_offer_id);
		assert!(Nft::token_offers(token_id).is_none());
		assert_eq!(Nft::offers(next_offer_id), None);
		// Check funds have not been locked
		assert_eq!(GenericAsset::free_balance(PAYMENT_ASSET, &buyer), initial_balance_buyer);
		assert_eq!(GenericAsset::reserved_balance(PAYMENT_ASSET, &buyer), 0);
	});
}

#[test]
fn cancel_offer() {
	ExtBuilder::default().build().execute_with(|| {
		let (_, token_id, _) = setup_token();
		let buyer: u64 = 3;
		let offer_amount: Balance = 100;
		let initial_balance_buyer: Balance = 1000;
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&buyer, PAYMENT_ASSET, initial_balance_buyer);

		let (offer_id, _) = make_new_simple_offer(offer_amount, token_id, buyer, None);
		assert_ok!(Nft::cancel_offer(Some(buyer).into(), offer_id));

		// assert!(has_event(RawEvent::OfferCancelled(offer_id)));

		// Check storage has been removed
		let empty_offer_vector: Vec<OfferId> = vec![];
		assert_eq!(Nft::token_offers(token_id).unwrap(), empty_offer_vector);
		assert_eq!(Nft::offers(offer_id), None);
		// Check funds have been unlocked after offer cancelled
		assert_eq!(GenericAsset::free_balance(PAYMENT_ASSET, &buyer), initial_balance_buyer);
		assert_eq!(GenericAsset::reserved_balance(PAYMENT_ASSET, &buyer), 0);
	});
}

#[test]
fn cancel_offer_multiple_offers() {
	ExtBuilder::default().build().execute_with(|| {
		let (_, token_id, _) = setup_token();
		let buyer_1: u64 = 3;
		let buyer_2: u64 = 4;

		let offer_amount_1: Balance = 100;
		let offer_amount_2: Balance = 150;
		let initial_balance_buyer_1: Balance = 1000;
		let initial_balance_buyer_2: Balance = 1000;
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&buyer_1, PAYMENT_ASSET, initial_balance_buyer_1);
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&buyer_2, PAYMENT_ASSET, initial_balance_buyer_2);

		let (offer_id_1, _) = make_new_simple_offer(offer_amount_1, token_id, buyer_1, None);
		let (offer_id_2, offer_2) = make_new_simple_offer(offer_amount_2, token_id, buyer_2, None);

		// Can't cancel other offer
		assert_noop!(
			Nft::cancel_offer(Some(buyer_1).into(), offer_id_2),
			Error::<Test>::NotBuyer
		);
		// Can cancel their offer
		assert_ok!(Nft::cancel_offer(Some(buyer_1).into(), offer_id_1));
		// assert!(has_event(RawEvent::OfferCancelled(offer_id_1)));

		// Check storage has been removed
		let offer_vector: Vec<OfferId> = vec![offer_id_2];
		assert_eq!(Nft::token_offers(token_id).unwrap(), offer_vector);
		assert_eq!(Nft::offers(offer_id_2), Some(OfferType::Simple(offer_2.clone())));
		assert_eq!(Nft::offers(offer_id_1), None);

		// Check funds have been unlocked after offer cancelled
		assert_eq!(
			GenericAsset::free_balance(PAYMENT_ASSET, &buyer_1),
			initial_balance_buyer_1
		);
		assert_eq!(GenericAsset::reserved_balance(PAYMENT_ASSET, &buyer_1), 0);
		// Check buyer_2 funds have not been unlocked
		assert_eq!(
			GenericAsset::free_balance(PAYMENT_ASSET, &buyer_2),
			initial_balance_buyer_2 - offer_amount_2
		);
		assert_eq!(GenericAsset::reserved_balance(PAYMENT_ASSET, &buyer_2), offer_amount_2);
	});
}

#[test]
fn cancel_offer_not_buyer_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let (_, token_id, _) = setup_token();
		let buyer: u64 = 3;
		let offer_amount: Balance = 100;
		let initial_balance_buyer: Balance = 1000;
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&buyer, PAYMENT_ASSET, initial_balance_buyer);
		assert_eq!(GenericAsset::reserved_balance(PAYMENT_ASSET, &buyer), 0);

		let (offer_id, offer) = make_new_simple_offer(offer_amount, token_id, buyer, None);
		assert_noop!(Nft::cancel_offer(Some(4).into(), offer_id), Error::<Test>::NotBuyer);

		// Check storage has not been removed
		assert_eq!(Nft::token_offers(token_id).unwrap(), vec![offer_id]);
		assert_eq!(Nft::offers(offer_id), Some(OfferType::Simple(offer.clone())));
		// Check funds have not been unlocked
		assert_eq!(
			GenericAsset::free_balance(PAYMENT_ASSET, &buyer),
			initial_balance_buyer - offer_amount
		);
		assert_eq!(GenericAsset::reserved_balance(PAYMENT_ASSET, &buyer), offer_amount);
	});
}

#[test]
fn accept_offer() {
	ExtBuilder::default().build().execute_with(|| {
		let (_, token_id, token_owner) = setup_token();
		let buyer: u64 = 3;
		let offer_amount: Balance = 100;
		let initial_balance_buyer: Balance = 1000;
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&buyer, PAYMENT_ASSET, initial_balance_buyer);

		let (offer_id, _) = make_new_simple_offer(offer_amount, token_id, buyer, None);
		assert_ok!(Nft::accept_offer(Some(token_owner).into(), offer_id));
		// assert!(has_event(RawEvent::OfferAccepted(offer_id)));

		// Check storage has been removed
		let empty_offer_vector: Vec<OfferId> = vec![];
		assert_eq!(Nft::token_offers(token_id).unwrap(), empty_offer_vector);
		assert_eq!(Nft::offers(offer_id), None);
		// Check funds have been transferred
		assert_eq!(
			GenericAsset::free_balance(PAYMENT_ASSET, &buyer),
			initial_balance_buyer - offer_amount
		);
		assert_eq!(GenericAsset::reserved_balance(PAYMENT_ASSET, &buyer), 0);
		assert_eq!(GenericAsset::free_balance(PAYMENT_ASSET, &token_owner), offer_amount);
	});
}

#[test]
fn accept_offer_multiple_offers() {
	ExtBuilder::default().build().execute_with(|| {
		let (_, token_id, token_owner) = setup_token();
		let buyer_1: u64 = 3;
		let buyer_2: u64 = 4;

		let offer_amount_1: Balance = 100;
		let offer_amount_2: Balance = 150;
		let initial_balance_buyer_1: Balance = 1000;
		let initial_balance_buyer_2: Balance = 1000;
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&buyer_1, PAYMENT_ASSET, initial_balance_buyer_1);
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&buyer_2, PAYMENT_ASSET, initial_balance_buyer_2);

		let (offer_id_1, offer_1) = make_new_simple_offer(offer_amount_1, token_id, buyer_1, None);
		let (offer_id_2, _) = make_new_simple_offer(offer_amount_2, token_id, buyer_2, None);

		// Accept second offer
		assert_ok!(Nft::accept_offer(Some(token_owner).into(), offer_id_2));
		// assert!(has_event(RawEvent::OfferAccepted(offer_id_2)));

		// Check storage has been removed
		let offer_vector: Vec<OfferId> = vec![offer_id_1];
		assert_eq!(Nft::token_offers(token_id).unwrap(), offer_vector);
		assert_eq!(Nft::offers(offer_id_1), Some(OfferType::Simple(offer_1.clone())));
		assert_eq!(Nft::offers(offer_id_2), None);

		// Check funds have been transferred
		assert_eq!(
			GenericAsset::free_balance(PAYMENT_ASSET, &buyer_2),
			initial_balance_buyer_2 - offer_amount_2
		);
		assert_eq!(
			GenericAsset::free_balance(PAYMENT_ASSET, &buyer_1),
			initial_balance_buyer_1 - offer_amount_1
		);
		assert_eq!(GenericAsset::reserved_balance(PAYMENT_ASSET, &buyer_1), offer_amount_1);
		assert_eq!(GenericAsset::reserved_balance(PAYMENT_ASSET, &buyer_2), 0);
		assert_eq!(GenericAsset::free_balance(PAYMENT_ASSET, &token_owner), offer_amount_2);

		// Accept first offer should fail as token_owner is no longer owner
		assert_noop!(
			Nft::accept_offer(Some(token_owner).into(), offer_id_1),
			Error::<Test>::NoPermission
		);
	});
}

#[test]
fn accept_offer_pays_marketplace_royalties() {
	ExtBuilder::default().build().execute_with(|| {
		let (_, token_id, token_owner) = setup_token();
		let buyer: u64 = 3;
		let offer_amount: Balance = 100;
		let initial_balance_buyer: Balance = 1000;
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&buyer, PAYMENT_ASSET, initial_balance_buyer);

		let marketplace_account = 4;
		let entitlements: Permill = Permill::from_float(0.1);
		let marketplace_id = Nft::next_marketplace_id();
		assert_ok!(Nft::register_marketplace(
			Some(marketplace_account).into(),
			None,
			entitlements
		));

		let (offer_id, _) = make_new_simple_offer(offer_amount, token_id, buyer, Some(marketplace_id));
		assert_ok!(Nft::accept_offer(Some(token_owner).into(), offer_id));
		// assert!(has_event(RawEvent::OfferAccepted(offer_id)));

		// Check storage has been removed
		let empty_offer_vector: Vec<OfferId> = vec![];
		assert_eq!(Nft::token_offers(token_id).unwrap(), empty_offer_vector);
		assert_eq!(Nft::offers(offer_id), None);
		// Check funds have been transferred with royalties
		assert_eq!(
			GenericAsset::free_balance(PAYMENT_ASSET, &buyer),
			initial_balance_buyer - offer_amount
		);
		assert_eq!(
			GenericAsset::free_balance(PAYMENT_ASSET, &marketplace_account),
			entitlements * offer_amount
		);
		assert_eq!(GenericAsset::reserved_balance(PAYMENT_ASSET, &buyer), 0);
		assert_eq!(
			GenericAsset::free_balance(PAYMENT_ASSET, &token_owner),
			offer_amount - (entitlements * offer_amount)
		);
	});
}

#[test]
fn accept_offer_not_token_owner_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let (_, token_id, token_owner) = setup_token();
		let buyer: u64 = 3;
		let offer_amount: Balance = 100;
		let initial_balance_buyer: Balance = 1000;
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&buyer, PAYMENT_ASSET, initial_balance_buyer);

		let (offer_id, offer) = make_new_simple_offer(offer_amount, token_id, buyer, None);
		assert_noop!(Nft::accept_offer(Some(4).into(), offer_id), Error::<Test>::NoPermission);

		// Check storage has not been removed
		assert_eq!(Nft::token_offers(token_id).unwrap(), vec![offer_id]);
		assert_eq!(Nft::offers(offer_id), Some(OfferType::Simple(offer.clone())));
		// Check funds have not been transferred
		assert_eq!(
			GenericAsset::free_balance(PAYMENT_ASSET, &buyer),
			initial_balance_buyer - offer_amount
		);
		assert_eq!(GenericAsset::reserved_balance(PAYMENT_ASSET, &buyer), offer_amount);
		assert_eq!(GenericAsset::free_balance(PAYMENT_ASSET, &token_owner), 0);
	});
}

#[test]
fn transfer_changes_token_balance() {
	ExtBuilder::default().build().execute_with(|| {
		let series_owner = 1_u64;
		let series_id = Nft::next_series_id();
		let token_owner = 2_u64;
		let new_owner = 3_u64;
		let initial_quantity: u32 = 1;
		// Create BTreeMaps for both owners
		let mut owner_map = BTreeMap::new();
		let mut new_owner_map = BTreeMap::new();

		// Mint 1 token
		assert_ok!(Nft::create_series(
			Some(series_owner).into(),
			b"test-series".to_vec(),
			initial_quantity,
			None,
			Some(token_owner),
			MetadataScheme::IpfsDir(b"<CID>".to_vec()),
			None,
		));

		owner_map.insert(series_id, initial_quantity);
		assert_eq!(Nft::token_balance(token_owner).unwrap(), owner_map);
		assert!(Nft::token_balance(new_owner).is_none());

		// Mint an additional 2 tokens
		let additional_quantity: u32 = 2;
		assert_ok!(Nft::mint(
			Some(series_owner).into(),
			series_id,
			additional_quantity,
			Some(token_owner),
		));

		owner_map.insert(series_id, initial_quantity + additional_quantity);
		assert_eq!(Nft::token_balance(token_owner).unwrap(), owner_map);
		assert!(Nft::token_balance(new_owner).is_none());

		// Transfer 2 tokens
		let tokens = vec![(series_id, 0_u32), (series_id, 1_u32)];
		let transfer_quantity: u32 = tokens.len() as u32;
		assert_ok!(Nft::transfer(Some(token_owner).into(), tokens[0], new_owner,));
		assert_ok!(Nft::transfer(Some(token_owner).into(), tokens[1], new_owner,));

		owner_map.insert(series_id, initial_quantity + additional_quantity - transfer_quantity);
		new_owner_map.insert(series_id, transfer_quantity);
		assert_eq!(Nft::token_balance(token_owner).unwrap(), owner_map);
		assert_eq!(Nft::token_balance(new_owner).unwrap(), new_owner_map);
	});
}

#[test]
fn transfer_many_tokens_changes_token_balance() {
	ExtBuilder::default().build().execute_with(|| {
		let series_owner = 1_u64;
		let series_id = Nft::next_series_id();
		let token_owner = 2_u64;
		let new_owner = 3_u64;
		let initial_quantity: u32 = 100;
		// Create BTreeMaps for both owners
		let mut owner_map = BTreeMap::new();
		let mut new_owner_map = BTreeMap::new();

		// Mint 1 token
		assert_ok!(Nft::create_series(
			Some(series_owner).into(),
			b"test-series".to_vec(),
			initial_quantity,
			None,
			Some(token_owner),
			MetadataScheme::IpfsDir(b"<CID>".to_vec()),
			None,
		));

		owner_map.insert(series_id, initial_quantity);
		assert_eq!(Nft::token_balance(token_owner).unwrap(), owner_map);
		assert!(Nft::token_balance(new_owner).is_none());

		for i in 0_u32..initial_quantity {
			// Transfer token
			let token_id: TokenId = (series_id, i);
			assert_ok!(Nft::transfer(Some(token_owner).into(), token_id, new_owner,));

			// Check storage
			let changed_quantity = i + 1;
			if changed_quantity == initial_quantity {
				assert_eq!(Nft::token_balance(token_owner).unwrap(), BTreeMap::new());
			} else {
				owner_map.insert(series_id, initial_quantity - changed_quantity);
				assert_eq!(Nft::token_balance(token_owner).unwrap(), owner_map);
			}
			new_owner_map.insert(series_id, changed_quantity);
			assert_eq!(Nft::token_balance(new_owner).unwrap(), new_owner_map);
		}
	});
}
