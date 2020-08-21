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
	constants::{asset::*, currency::*},
	Call, CheckedExtrinsic, Runtime, UncheckedExtrinsic,
};
use cennznet_testing::keyring::{alice, bob, sign, signed_extra};
use codec::Encode;
use crml_transaction_payment::ChargeTransactionPayment;
use frame_support::weights::GetDispatchInfo;
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
fn fee_components_ga() {
	ExtBuilder::default().build().execute_with(|| {

        for amount in &[1, 1 * DOLLARS, 100 * DOLLARS, 1000 * DOLLARS, 10_000 * DOLLARS, 100_000 * DOLLARS] {
	        let call = Call::GenericAsset(
                pallet_generic_asset::Call::transfer(CENTRAPAY_ASSET_ID, bob(), *amount)
            );
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
fn fee_components_sylo() {
	ExtBuilder::default().build().execute_with(|| {
		let sylo_call = Call::SyloE2EE(
			crml_sylo::e2ee::Call::register_device(
				100_000,
				// 12 pkbs (SHA-256 hash)
				vec![
					b"0xB94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EFCDE9".to_vec(),
				],
			)
		);
		let tx = signed_tx(sylo_call);

		let tx_fee = ChargeTransactionPayment::<Runtime>::compute_fee_parts(
			Encode::encode(&tx).len() as u32,
			tx.get_dispatch_info(),
		);
		println!("{:?}", tx_fee);
		println!("{:?}", <Runtime as crml_transaction_payment::Trait>::TransactionBaseFee::get());
		println!("{:?}", <Runtime as crml_transaction_payment::Trait>::TransactionByteFee::get());

		assert!(false);
	});
}
