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

//! Fee integration tests

use cennznet_runtime::{
	constants::{asset::*, currency::*},
	Call, CheckedExtrinsic, TransactionPayment, UncheckedExtrinsic,
};
use codec::Encode;
use frame_support::weights::GetDispatchInfo;

mod common;
use common::helpers::sign;
use common::keyring::{alice, bob, signed_extra};
use common::mock::ExtBuilder;

// Make signed transaction given a `Call`
fn signed_tx(call: Call) -> UncheckedExtrinsic {
	sign(CheckedExtrinsic {
		signed: Some((alice(), signed_extra(0, 0, None))),
		function: call,
	})
}

// These following tests may be used to inspect transaction fee values.
// They are not required to assert correctness.
// last run range:
// ```ignore
// FeeParts { base_fee: 187, length_fee: 1490, weight_fee: 8760, peak_adjustment_fee: 0 }
// ```
#[test]
#[ignore]
fn fee_components_ga() {
	ExtBuilder::default().build().execute_with(|| {
		for amount in &[
			1 * DOLLARS,
			100 * DOLLARS,
			1000 * DOLLARS,
			10_000 * DOLLARS,
			100_000 * DOLLARS,
			1_000_000 * DOLLARS,
		] {
			let call = Call::GenericAsset(prml_generic_asset::Call::transfer(CENTRAPAY_ASSET_ID, bob(), *amount));
			let tx = signed_tx(call);
			let tx_fee =
				TransactionPayment::compute_fee_parts(Encode::encode(&tx).len() as u32, &tx.get_dispatch_info());
			println!("{:#?}", tx_fee);
			// optimising for a GA transfer fee of ~1.0000 CPAY
			assert!(1 * DOLLARS < tx_fee.total() && tx_fee.total() <= 2 * DOLLARS);
		}
	});
}
