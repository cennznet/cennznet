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

use cennznet_runtime::{
	constants::{asset::*, currency::*, fee::MAX_WEIGHT},
	Call, CheckedExtrinsic, Runtime, TransactionBaseFee, TransactionMaxWeightFee, TransactionMinWeightFee,
	UncheckedExtrinsic,
};
use cennznet_testing::keyring::{alice, bob, sign, signed_extra};
use codec::Encode;
use crml_transaction_payment::ChargeTransactionPayment;
use frame_support::weights::{DispatchClass, DispatchInfo, GetDispatchInfo};
mod mock;
use mock::ExtBuilder;

// Make signed transaction given a `Call`
fn signed_tx(call: Call) -> UncheckedExtrinsic {
	sign(
		CheckedExtrinsic {
			signed: Some((alice(), signed_extra(0, 0, None, None))),
			function: call,
		},
		4,                  // tx version
		Default::default(), // genesis hash
	)
}

#[test]
fn fee_scaling_max_weight() {
	ExtBuilder::default().build().execute_with(|| {
		let dispatch_info = DispatchInfo {
			weight: MAX_WEIGHT as u32,
			class: DispatchClass::Operational,
			pays_fee: true,
		};
		// Scales to maximum weight fee + transaction base fee
		assert_eq!(
			TransactionMaxWeightFee::get() + TransactionBaseFee::get(),
			ChargeTransactionPayment::<Runtime>::compute_fee(0, dispatch_info, 0)
		);
	});
}

#[test]
fn fee_scaling_min_weight() {
	ExtBuilder::default().build().execute_with(|| {
		let dispatch_info = DispatchInfo {
			weight: 1,
			class: DispatchClass::Operational,
			pays_fee: true,
		};
		// Scales to minimum weight fee + transaction base fee
		assert_eq!(
			TransactionMinWeightFee::get() + TransactionBaseFee::get(),
			ChargeTransactionPayment::<Runtime>::compute_fee(0, dispatch_info, 0)
		);
	});
}

// These following tests may be used to inspect transaction fee values.
// They are not required to assert correctness.
#[test]
#[ignore]
fn fee_components_ga() {
	ExtBuilder::default().build().execute_with(|| {
		for amount in &[
			1,
			1 * DOLLARS,
			100 * DOLLARS,
			1000 * DOLLARS,
			10_000 * DOLLARS,
			100_000 * DOLLARS,
		] {
			let call = Call::GenericAsset(pallet_generic_asset::Call::transfer(CENTRAPAY_ASSET_ID, bob(), *amount));
			let tx = signed_tx(call);

			let tx_fee = ChargeTransactionPayment::<Runtime>::compute_fee_parts(
				Encode::encode(&tx).len() as u32,
				tx.get_dispatch_info(),
			);
			println!("{:?}", tx_fee);
		}

		assert!(false);
	});
}

#[test]
#[ignore]
fn fee_components_sylo_e2ee_call() {
	ExtBuilder::default().build().execute_with(|| {
		let sylo_call = Call::SyloE2EE(crml_sylo::e2ee::Call::register_device(
			100_000,
			// 8 pkbs (SHA-256 hash)
			vec![
				b"0xB94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EFCDE9".to_vec(),
				b"0xB94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EFCDE9".to_vec(),
				b"0xB94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EFCDE9".to_vec(),
				b"0xB94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EFCDE9".to_vec(),
				b"0xB94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EFCDE9".to_vec(),
				b"0xB94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EFCDE9".to_vec(),
				b"0xB94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EFCDE9".to_vec(),
				b"0xB94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EFCDE9".to_vec(),
				b"0xB94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EFCDE9".to_vec(),
			],
		));
		let tx = signed_tx(sylo_call);

		let tx_fee = ChargeTransactionPayment::<Runtime>::compute_fee_parts(
			Encode::encode(&tx).len() as u32,
			tx.get_dispatch_info(),
		);
		println!("{:?}", tx_fee.total());

		assert!(false);
	});
}

#[test]
#[ignore]
fn fee_components_sylo_group_update_member() {
	ExtBuilder::default().build().execute_with(|| {
		let sylo_call = Call::SyloGroups(crml_sylo::groups::Call::update_member(
			Default::default(),
			vec![(
				b"some metadata".to_vec(),
				b"some very long meta data which is similar in size to what would be sent from the normal sylo app"
					.to_vec(),
			)],
		));
		let tx = signed_tx(sylo_call);

		let tx_fee = ChargeTransactionPayment::<Runtime>::compute_fee_parts(
			Encode::encode(&tx).len() as u32,
			tx.get_dispatch_info(),
		);
		println!("{:?}", tx_fee.total());

		assert!(false);
	});
}

#[test]
#[ignore]
fn fee_components_sylo_vault_replenish_pkbs() {
	ExtBuilder::default().build().execute_with(|| {
		let sylo_call = Call::SyloE2EE(crml_sylo::e2ee::Call::replenish_pkbs(
			1_000_000_u32,
			vec![
				b"0xB94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EFCDE9".to_vec(),
				b"0xB94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EFCDE9".to_vec(),
				b"0xB94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EFCDE9".to_vec(),
				b"0xB94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EFCDE9".to_vec(),
				b"0xB94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EFCDE9".to_vec(),
				b"0xB94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EFCDE9".to_vec(),
				b"0xB94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EFCDE9".to_vec(),
				b"0xB94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EFCDE9".to_vec(),
				b"0xB94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EFCDE9".to_vec(),
				b"0xB94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EFCDE9".to_vec(),
				b"0xB94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EFCDE9".to_vec(),
				b"0xB94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EFCDE9".to_vec(),
			],
		));
		let tx = signed_tx(sylo_call);

		let tx_fee = ChargeTransactionPayment::<Runtime>::compute_fee_parts(
			Encode::encode(&tx).len() as u32,
			tx.get_dispatch_info(),
		);
		println!("{:?}", tx_fee.total());

		assert!(false);
	});
}

#[test]
#[ignore]
fn fee_components_sylo_vault_upsert_value() {
	ExtBuilder::default().build().execute_with(|| {
		let sylo_call = Call::SyloVault(crml_sylo::vault::Call::upsert_value(
			[1_u8; 64].to_vec(),
			[2_u8; 64].to_vec(),
		));
		let tx = signed_tx(sylo_call);

		let tx_fee = ChargeTransactionPayment::<Runtime>::compute_fee_parts(
			Encode::encode(&tx).len() as u32,
			tx.get_dispatch_info(),
		);
		println!("{:?}", tx_fee.total());

		assert!(false);
	});
}
