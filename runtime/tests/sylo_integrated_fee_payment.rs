/* Copyright 2019-2020 Centrality Investments Limited
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

//! Sylo integrated fee payment tests

use cennznet_primitives::types::{AccountId, Balance};
use cennznet_runtime::{
	constants::asset::*, sylo_e2ee, sylo_groups, sylo_inbox, sylo_response, sylo_vault, Call, CheckedExtrinsic,
	Executive, GenericAsset, Origin, SyloPayment, TransactionMaxWeightFee,
};
use cennznet_testing::keyring::{bob, charlie, dave, signed_extra};
use frame_support::{additional_traits::MultiCurrencyAccounting as MultiCurrency, assert_ok};

mod common;

use common::helpers::{extrinsic_fee_for, header, sign};
use common::mock::ExtBuilder;

fn apply_extrinsic(origin: AccountId, call: Call) -> Balance {
	let xt = sign(CheckedExtrinsic {
		signed: Some((origin, signed_extra(0, 0, None, None))),
		function: call,
	});

	let fee = extrinsic_fee_for(&xt);

	Executive::initialize_block(&header());
	let r = Executive::apply_extrinsic(xt);
	assert!(r.is_ok());

	fee
}

#[test]
fn non_sylo_call_is_not_paid_by_payment_account() {
	let call = Call::GenericAsset(pallet_generic_asset::Call::transfer(CENTRAPAY_ASSET_ID, dave(), 100));

	ExtBuilder::default()
		.initial_balance(TransactionMaxWeightFee::get())
		.build()
		.execute_with(|| {
			assert_ok!(SyloPayment::set_payment_account(Origin::ROOT, bob()));

			let fee_asset_id = Some(GenericAsset::spending_asset_id());
			let bob_balance = <GenericAsset as MultiCurrency>::free_balance(&bob(), fee_asset_id.clone());

			let _ = apply_extrinsic(charlie(), call);

			let bob_balance_after_calls = <GenericAsset as MultiCurrency>::free_balance(&bob(), fee_asset_id);
			assert_eq!(bob_balance_after_calls, bob_balance);
		});
}

#[test]
fn sylo_e2ee_call_is_paid_by_payment_account() {
	let call = Call::SyloE2EE(sylo_e2ee::Call::register_device(1, vec![]));

	ExtBuilder::default()
		.initial_balance(TransactionMaxWeightFee::get())
		.build()
		.execute_with(|| {
			assert_ok!(SyloPayment::set_payment_account(Origin::ROOT, bob()));

			let fee_asset_id = Some(GenericAsset::spending_asset_id());
			let bob_balance = <GenericAsset as MultiCurrency>::free_balance(&bob(), fee_asset_id.clone());

			let call_fee = apply_extrinsic(charlie(), call);

			let bob_balance_after_calls = <GenericAsset as MultiCurrency>::free_balance(&bob(), fee_asset_id);
			assert_eq!(bob_balance_after_calls, bob_balance - call_fee);
		});
}

#[test]
fn sylo_inbox_call_is_paid_by_payment_account() {
	let call = Call::SyloInbox(sylo_inbox::Call::add_value(dave(), b"dude!".to_vec()));

	ExtBuilder::default()
		.initial_balance(TransactionMaxWeightFee::get())
		.build()
		.execute_with(|| {
			assert_ok!(SyloPayment::set_payment_account(Origin::ROOT, bob()));

			let fee_asset_id = Some(GenericAsset::spending_asset_id());
			let bob_balance = <GenericAsset as MultiCurrency>::free_balance(&bob(), fee_asset_id.clone());

			let call_fee = apply_extrinsic(charlie(), call);

			let bob_balance_after_calls = <GenericAsset as MultiCurrency>::free_balance(&bob(), fee_asset_id);
			assert_eq!(bob_balance_after_calls, bob_balance - call_fee);
		});
}

#[test]
fn sylo_vault_call_is_paid_by_payment_account() {
	let call = Call::SyloVault(sylo_vault::Call::upsert_value(b"key".to_vec(), b"value".to_vec()));

	ExtBuilder::default()
		.initial_balance(TransactionMaxWeightFee::get())
		.build()
		.execute_with(|| {
			assert_ok!(SyloPayment::set_payment_account(Origin::ROOT, bob()));

			let fee_asset_id = Some(GenericAsset::spending_asset_id());
			let bob_balance = <GenericAsset as MultiCurrency>::free_balance(&bob(), fee_asset_id.clone());

			let call_fee = apply_extrinsic(charlie(), call);

			let bob_balance_after_calls = <GenericAsset as MultiCurrency>::free_balance(&bob(), fee_asset_id);
			assert_eq!(bob_balance_after_calls, bob_balance - call_fee);
		});
}

#[test]
fn sylo_response_call_is_paid_by_payment_account() {
	let call = Call::SyloResponse(sylo_response::Call::remove_response([0u8; 32].into()));

	ExtBuilder::default()
		.initial_balance(TransactionMaxWeightFee::get())
		.build()
		.execute_with(|| {
			assert_ok!(SyloPayment::set_payment_account(Origin::ROOT, bob()));

			let fee_asset_id = Some(GenericAsset::spending_asset_id());
			let bob_balance = <GenericAsset as MultiCurrency>::free_balance(&bob(), fee_asset_id.clone());

			let call_fee = apply_extrinsic(charlie(), call);

			let bob_balance_after_calls = <GenericAsset as MultiCurrency>::free_balance(&bob(), fee_asset_id);
			assert_eq!(bob_balance_after_calls, bob_balance - call_fee);
		});
}

#[test]
fn sylo_groups_call_is_paid_by_payment_account() {
	let meta = vec![(b"key".to_vec(), b"value".to_vec())];
	let call = Call::SyloGroups(sylo_groups::Call::create_group(
		[1u8; 32].into(),
		meta,
		vec![],
		(b"group".to_vec(), b"data".to_vec()),
	));

	ExtBuilder::default()
		.initial_balance(TransactionMaxWeightFee::get())
		.build()
		.execute_with(|| {
			assert_ok!(SyloPayment::set_payment_account(Origin::ROOT, bob()));

			let fee_asset_id = Some(GenericAsset::spending_asset_id());
			let bob_balance = <GenericAsset as MultiCurrency>::free_balance(&bob(), fee_asset_id.clone());

			let call_fee = apply_extrinsic(charlie(), call);

			let bob_balance_after_calls = <GenericAsset as MultiCurrency>::free_balance(&bob(), fee_asset_id);
			assert_eq!(bob_balance_after_calls, bob_balance - call_fee);
		});
}
