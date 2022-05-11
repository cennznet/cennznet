/* Copyright 2019-2022 Centrality Investments Limited
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

use crate::{self as crml_eth_state_oracle, Config};
use crml_support::{
    ContractExecutor, H160, H256,
};
use pallet_evm::AddressMapping;
use frame_support::{
	assert_noop, assert_ok, pallet_prelude::*,
    dispatch::DispatchResultWithPostInfo,
};

type AssetId = u32;
type Balance = u128;
type AccountId = u64;

frame_support::construct_runtime!(
	pub enum TestRuntime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		GenericAsset: crml_generic_asset::{Pallet, Call, Config<T>, Storage, Event<T>},
		EthStateOracle: crml_eth_bridge::{Pallet, Call, Storage, Event},
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
}
impl frame_system::Config for TestRuntime {
	type BlockWeights = ();
	type BlockLength = ();
	type BaseCallFilter = frame_support::traits::Everything;
	type Origin = Origin;
	type Index = u64;
	type BlockNumber = u64;
	type Call = Call;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type BlockHashCount = BlockHashCount;
	type Event = Event;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
}

parameter_types! {
	pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
}
impl crml_generic_asset::Config for Test {
	type AssetId = AssetId;
	type Balance = Balance;
	type Event = Event;
	type OnDustImbalance = TransferDustImbalance<TreasuryPalletId>;
	type WeightInfo = ();
}

parameter_types! {
    pub const ChallengePeriod: u64 = 5;
    pub const MinGasPrice: u64 = 1;
}
impl Config for TestRuntime {
	type AddressMapping = SimpleAddressMapping<AccountId>;
	type ChallengePeriod = ChallengePeriod;
	type ContractExecutor = MockContractExecutor<Address = H160>;
	type Event = Event;
	type MultiCurrency = GenericAsset;
	type MinGasPrice = MinGasPrice;
}

struct SimpleAddressMapping<AccountId>(sp_std::marker::PhantomData<AccountId>);

impl AddressMapping<AccountId> for SimpleAddressMapping<AccountId> {
	fn into_account_id(address: H160) -> AccountId {
        address.to_low_u64_be()
    }
}

struct MockContractExecutor;

impl ContractExecutor for MockContractExecutor {
    fn execute(
        _caller: &Self::Address,
		_target: &Self::Address,
		_input_data: &[u8],
		_gas_limit: u64,
    ) -> DispatchResultWithPostInfo {

    }
}