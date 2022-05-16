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

use crate::{self as crml_eth_state_oracle, CallRequest, Config};
use cennznet_primitives::types::FeePreferences;
use crml_support::{ContractExecutor, H160, H256, U256};
use frame_support::{
	dispatch::{DispatchResultWithPostInfo, PostDispatchInfo},
	pallet_prelude::*,
	parameter_types,
	storage::StorageValue,
	traits::UnixTime,
};
use pallet_evm::AddressMapping;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};
use sp_std::convert::TryFrom;

type AssetId = u32;
type Balance = u128;
pub type AccountId = u64;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;
type Block = frame_system::mocking::MockBlock<TestRuntime>;

frame_support::construct_runtime!(
	pub enum TestRuntime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		GenericAsset: crml_generic_asset::{Pallet, Call, Config<T>, Storage, Event<T>},
		EthStateOracle: crml_eth_state_oracle::{Pallet, Call, Storage, Event},
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

impl crml_generic_asset::Config for TestRuntime {
	type AssetId = AssetId;
	type Balance = Balance;
	type Event = Event;
	type OnDustImbalance = ();
	type WeightInfo = ();
}

/// Lifted from runtime/src/lib.rs
pub const GAS_PER_SECOND: u64 = 40_000_000;
pub const WEIGHT_PER_GAS: u64 = frame_support::weights::constants::WEIGHT_PER_SECOND / GAS_PER_SECOND;
pub struct MockGasWeightMapping;
impl pallet_evm::GasWeightMapping for MockGasWeightMapping {
	fn gas_to_weight(gas: u64) -> Weight {
		gas.saturating_mul(WEIGHT_PER_GAS)
	}
	fn weight_to_gas(weight: Weight) -> u64 {
		u64::try_from(weight.wrapping_div(WEIGHT_PER_GAS)).unwrap_or(u32::MAX as u64)
	}
}

parameter_types! {
	pub const ChallengePeriod: u64 = 5;
	pub const MinGasPrice: u64 = 1;
	pub StateOraclePrecompileAddress: H160 = H160::from_low_u64_be(27572);
}
impl Config for TestRuntime {
	type AddressMapping = SimpleAddressMapping<AccountId>;
	type ChallengePeriod = ChallengePeriod;
	type ContractExecutor = MockContractExecutor;
	type Event = Event;
	type EthCallOracle = ();
	type MultiCurrency = GenericAsset;
	type MinGasPrice = MinGasPrice;
	type GasWeightMapping = MockGasWeightMapping;
	type StateOraclePrecompileAddress = StateOraclePrecompileAddress;
	type UnixTime = MockTimestampGetter;
}

pub struct MockTimestampGetter;

impl UnixTime for MockTimestampGetter {
	fn now() -> core::time::Duration {
		core::time::Duration::new(System::block_number() * 5_u64, 0)
	}
}

pub struct SimpleAddressMapping<AccountId>(sp_std::marker::PhantomData<AccountId>);
impl AddressMapping<AccountId> for SimpleAddressMapping<AccountId> {
	fn into_account_id(address: H160) -> AccountId {
		address.to_low_u64_be()
	}
}

/// Callback execution parameters
type CallbackExecutionParameters = (H160, H160, Vec<u8>, u64, U256, U256, Option<FeePreferences>);

pub(crate) mod test_storage {
	//! storage used by tests
	use super::CallbackExecutionParameters;
	use crate::Config;
	use frame_support::decl_storage;
	pub struct Module<T>(sp_std::marker::PhantomData<T>);
	decl_storage! {
		trait Store for Module<T: Config> as EthStateOracleTest {
			pub CurrentExecutionParameters: Option<CallbackExecutionParameters>
		}
	}
}

pub struct MockContractExecutor;
impl ContractExecutor for MockContractExecutor {
	type Address = H160;
	/// Stores the contract execution parameters for inspection by tests
	fn execute(
		caller: &Self::Address,
		target: &Self::Address,
		input_data: &[u8],
		gas_limit: u64,
		max_fee_per_gas: U256,
		max_priority_fee_per_gas: U256,
	) -> DispatchResultWithPostInfo {
		let parameters: CallbackExecutionParameters = (
			*caller,
			*target,
			input_data.to_vec(),
			gas_limit,
			max_fee_per_gas,
			max_priority_fee_per_gas,
			None,
		);
		test_storage::CurrentExecutionParameters::put(parameters);

		Ok(PostDispatchInfo {
			actual_weight: Some(1_000_u64),
			pays_fee: Pays::No,
		})
	}
}

#[derive(Default)]
pub struct ExtBuilder;

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut ext: sp_io::TestExternalities = frame_system::GenesisConfig::default()
			.build_storage::<TestRuntime>()
			.unwrap()
			.into();

		ext.execute_with(|| {
			System::initialize(&1, &[0u8; 32].into(), &Default::default(), frame_system::InitKind::Full);
		});

		ext
	}
}

/// Builds a `CallRequest`
pub(crate) struct CallRequestBuilder(CallRequest);

impl CallRequestBuilder {
	pub fn new() -> Self {
		CallRequestBuilder(CallRequest {
			callback_signature: Default::default(),
			callback_gas_limit: 0,
			bounty: 0,
			timestamp: 0,
			caller: H160::default(),
			destination: H160::default(),
			fee_preferences: None,
		})
	}
	pub fn build(&self) -> CallRequest {
		self.0.clone()
	}
	pub fn bounty(&mut self, bounty: Balance) -> &mut Self {
		self.0.bounty = bounty;
		self
	}
	pub fn caller(&mut self, caller: u64) -> &mut Self {
		self.0.caller = H160::from_low_u64_be(caller);
		self
	}
	pub fn destination(&mut self, destination: u64) -> &mut Self {
		self.0.destination = H160::from_low_u64_be(destination);
		self
	}
	pub fn callback_gas_limit(&mut self, callback_gas_limit: u64) -> &mut Self {
		self.0.callback_gas_limit = callback_gas_limit;
		self
	}
	pub fn callback_signature(&mut self, callback_signature: [u8; 4]) -> &mut Self {
		self.0.callback_signature = callback_signature;
		self
	}
	pub fn timestamp(&mut self, timestamp: u64) -> &mut Self {
		self.0.timestamp = timestamp;
		self
	}
}
