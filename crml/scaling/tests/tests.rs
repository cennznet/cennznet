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

use cennznet_primitives::types::{AccountId, AssetId, Balance};
use cennznet_testing::keyring::{alice, bob};
use frame_support::{traits::OnRuntimeUpgrade, StorageDoubleMap, StorageMap};
use mock::{ExtBuilder, ScaleDownFactor, Scaling, Test, PLUG_ASSET_ID, SPENDING_ASSET_ID, STAKING_ASSET_ID};

mod mock;

#[test]
fn burn_asset_on_runtime_upgrade() {
	const INITIAL_BALANCE: Balance = 1000_000_000_000;
	const INITIAL_ISSUANCE: Balance = INITIAL_BALANCE * 1000;
	const ALICE_BALANCE: Balance = INITIAL_BALANCE * 11;
	const BOB_BALANCE: Balance = INITIAL_BALANCE * 23;

	ExtBuilder::default().sudoer(alice()).build().execute_with(|| {
		type TotalIssuance = pallet_generic_asset::TotalIssuance<Test>;
		TotalIssuance::insert(&STAKING_ASSET_ID, INITIAL_ISSUANCE);
		TotalIssuance::insert(&SPENDING_ASSET_ID, INITIAL_ISSUANCE);
		TotalIssuance::insert(&PLUG_ASSET_ID, INITIAL_ISSUANCE);

		type FreeBalance = pallet_generic_asset::FreeBalance<Test>;
		FreeBalance::insert::<AssetId, AccountId, Balance>(STAKING_ASSET_ID, alice(), ALICE_BALANCE);
		FreeBalance::insert::<AssetId, AccountId, Balance>(SPENDING_ASSET_ID, alice(), ALICE_BALANCE);
		FreeBalance::insert::<AssetId, AccountId, Balance>(PLUG_ASSET_ID, alice(), ALICE_BALANCE);
		FreeBalance::insert::<AssetId, AccountId, Balance>(STAKING_ASSET_ID, bob(), BOB_BALANCE);
		FreeBalance::insert::<AssetId, AccountId, Balance>(SPENDING_ASSET_ID, bob(), BOB_BALANCE);
		FreeBalance::insert::<AssetId, AccountId, Balance>(PLUG_ASSET_ID, bob(), BOB_BALANCE);

		Scaling::on_runtime_upgrade();

		type GenericAsset = pallet_generic_asset::Module<Test>;
		assert_eq!(
			GenericAsset::free_balance(&STAKING_ASSET_ID, &alice()),
			ALICE_BALANCE.checked_div(ScaleDownFactor::get()).unwrap()
		);
		assert_eq!(
			GenericAsset::free_balance(&SPENDING_ASSET_ID, &alice()),
			ALICE_BALANCE.checked_div(ScaleDownFactor::get()).unwrap()
		);
		assert_eq!(GenericAsset::free_balance(&PLUG_ASSET_ID, &alice()), ALICE_BALANCE);

		assert_eq!(
			GenericAsset::free_balance(&STAKING_ASSET_ID, &bob()),
			BOB_BALANCE.checked_div(ScaleDownFactor::get()).unwrap()
		);
		assert_eq!(
			GenericAsset::free_balance(&SPENDING_ASSET_ID, &bob()),
			BOB_BALANCE.checked_div(ScaleDownFactor::get()).unwrap()
		);
		assert_eq!(GenericAsset::free_balance(&PLUG_ASSET_ID, &bob()), BOB_BALANCE);
	});
}
