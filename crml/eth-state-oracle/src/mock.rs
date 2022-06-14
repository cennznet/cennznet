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

use crate::{self as crml_eth_state_oracle, CallRequest, CallResponse, Config, ReturnDataClaim};
use cennznet_primitives::{
	traits::BuyFeeAsset,
	types::{FeeExchange, FeePreferences},
};
use crml_support::{ContractExecutor, MultiCurrency, H160, H256, U256};
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

type AssetId = u32;
type Balance = u128;
/// nb: substrate testing crates define this as `u64`, cennznet runtime uses `u32`
type BlockNumber = u64;
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
		EthStateOracle: crml_eth_state_oracle::{Pallet, Call, Storage, Event<T>},
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
	type BlockNumber = BlockNumber;
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
	pub const ChallengePeriod: BlockNumber = 5;
	pub const MinGasPrice: u64 = 1;
	pub StateOraclePrecompileAddress: H160 = H160::from_low_u64_be(27572);
	pub RelayerBondAmount: Balance = 1_000_000_000;
	pub MaxRequestsPerBlock: u32 = 30;
	pub MaxRelayerCount: u32 = 1;
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
	type BuyFeeAsset = MockBuyFeeAsset;
	type RelayerBondAmount = RelayerBondAmount;
	type MaxRequestsPerBlock = MaxRequestsPerBlock;
	type MaxRelayerCount = MaxRelayerCount;
}

pub struct MockBuyFeeAsset;

impl BuyFeeAsset for MockBuyFeeAsset {
	type AccountId = AccountId;
	type Balance = Balance;
	type FeeExchange = FeeExchange<AssetId, Self::Balance>;

	fn buy_fee_asset(
		who: &Self::AccountId,
		amount: Self::Balance,
		fee_exchange: &Self::FeeExchange,
	) -> Result<Self::Balance, DispatchError> {
		let new_balance = GenericAsset::free_balance(fee_exchange.asset_id(), &who)
			.checked_sub(amount)
			.ok_or(DispatchError::Other("No Balance"))?;
		GenericAsset::make_free_balance_be(&who, fee_exchange.asset_id(), new_balance);
		let _ = GenericAsset::deposit_into_existing(&who, GenericAsset::fee_currency(), amount)?;

		Ok(amount)
	}

	fn buy_fee_weight() -> Weight {
		unimplemented!()
	}
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
			input_data: Vec::<u8>::default(),
			expiry_block: 0,
		})
	}
	pub fn build(self) -> CallRequest {
		self.0
	}
	pub fn expiry_block(mut self, expiry_block: BlockNumber) -> Self {
		self.0.expiry_block = expiry_block as u32;
		self
	}
	pub fn bounty(mut self, bounty: Balance) -> Self {
		self.0.bounty = bounty;
		self
	}
	pub fn caller(mut self, caller: u64) -> Self {
		self.0.caller = H160::from_low_u64_be(caller);
		self
	}
	pub fn destination(mut self, destination: u64) -> Self {
		self.0.destination = H160::from_low_u64_be(destination);
		self
	}
	pub fn fee_preferences(mut self, fee_preferences: Option<FeePreferences>) -> Self {
		self.0.fee_preferences = fee_preferences;
		self
	}
	pub fn callback_gas_limit(mut self, callback_gas_limit: u64) -> Self {
		self.0.callback_gas_limit = callback_gas_limit;
		self
	}
	pub fn callback_signature(mut self, callback_signature: [u8; 4]) -> Self {
		self.0.callback_signature = callback_signature;
		self
	}
}

pub(crate) struct CallResponseBuilder(CallResponse<AccountId>);

impl CallResponseBuilder {
	/// initialize a new CallResponseBuilder
	pub fn new() -> Self {
		CallResponseBuilder(CallResponse {
			return_data: ReturnDataClaim::Ok([1_u8; 32]),
			relayer: Default::default(),
			eth_block_number: 5,
			eth_block_timestamp: <TestRuntime as Config>::UnixTime::now().as_secs(),
		})
	}
	/// Return the built CallResponse
	pub fn build(self) -> CallResponse<AccountId> {
		self.0
	}
	pub fn eth_block_timestamp(mut self, eth_block_timestamp: u64) -> Self {
		self.0.eth_block_timestamp = eth_block_timestamp;
		self
	}
	pub fn relayer(mut self, relayer: AccountId) -> Self {
		self.0.relayer = relayer;
		self
	}
	pub fn return_data(mut self, return_data: ReturnDataClaim) -> Self {
		self.0.return_data = return_data;
		self
	}
}
