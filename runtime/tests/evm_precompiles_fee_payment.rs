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

use cennznet_primitives::types::AssetId;
use cennznet_runtime::{EthWallet, Runtime};
use frame_support::assert_ok;
use hex_literal::hex;
use pallet_evm_precompiles_fee_payment::{Action, Context, EvmDataWriter, FeePaymentPrecompile, Precompile, FEE_PROXY};
use sp_core::{H160, U256};

mod common;
use common::mock::ExtBuilder;

#[test]
fn set_payment_asset() {
	ExtBuilder::default().build().execute_with(|| {
		let caller = H160::from_slice(&hex!("0000022EdbDcBA4bF24a2Abf89F5C230b3700000"));
		let payment_asset: AssetId = 12;
		let address = H160::from_low_u64_be(FEE_PROXY);
		let context = Context {
			address,
			caller,
			apparent_value: U256::default(),
		};
		let input_data = EvmDataWriter::new_with_selector(Action::SetFeeAsset)
			.write::<U256>(payment_asset.into())
			.build();

		assert_ok!(<FeePaymentPrecompile<Runtime> as Precompile>::execute(
			&input_data,
			None,
			&context,
			false
		));

		// Check payment asset has been set in eth-wallet pallet
		assert_eq!(EthWallet::evm_payment_asset(caller), Some(payment_asset));
	})
}
