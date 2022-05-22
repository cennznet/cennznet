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

use crate as crml_eth_bridge;
use crate::sp_api_hidden_includes_decl_storage::hidden_include::StorageMap;
use crate::types::{BridgeRpcError, EthBlock, EthHash, LatestOrNumber, TransactionReceipt};
use crate::{types::BridgeEthereumRpcApi, Config};
use cennznet_primitives::eth::crypto::AuthorityId;
use codec::{Decode, Encode};
use crml_support::{
	EthAbiCodec, EventClaimSubscriber, FinalSessionTracker, NotarizationRewardHandler, H160, H256 as H256Crml, U256,
};
use ethereum_types::U64;
use frame_support::{
	parameter_types,
	traits::{UnixTime, ValidatorSet as ValidatorSetT},
};
use scale_info::TypeInfo;
use sp_core::{ecdsa::Signature, H256};
use sp_runtime::{
	testing::{Header, TestXt},
	traits::{BlakeTwo256, Convert, Extrinsic as ExtrinsicT, IdentifyAccount, IdentityLookup, Verify},
	Percent,
};
use std::marker::PhantomData;

pub type SessionIndex = u32;
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

pub type Extrinsic = TestXt<Call, ()>;
pub type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;
pub type Block = frame_system::mocking::MockBlock<TestRuntime>;

frame_support::construct_runtime!(
	pub enum TestRuntime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		EthBridge: crml_eth_bridge::{Pallet, Call, Storage, Event, ValidateUnsigned},
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
	pub const DefaultListingDuration: u64 = 5;
	pub const MaxAttributeLength: u8 = 140;
	pub const NotarizationThreshold: Percent = Percent::from_parts(66_u8);
}
impl Config for TestRuntime {
	type AuthoritySet = MockValidatorSet;
	type EthyId = AuthorityId;
	type EthereumRpcClient = MockEthereumRpcClient;
	type FinalSessionTracker = MockFinalSessionTracker;
	type NotarizationThreshold = NotarizationThreshold;
	type RewardHandler = MockRewardHandler;
	type Subscribers = MockClaimSubscriber;
	type UnixTime = MockUnixTime;
	type Call = Call;
	type Event = Event;
}

/// Mock eth-http endpoint
pub const MOCK_ETH_HTTP_URI: [u8; 31] = *b"http://ethereum-rpc.example.com";

#[derive(PartialEq, Eq, Encode, Decode, Clone, Default, TypeInfo)]
pub struct MockBlockResponse {
	pub block_hash: H256,
	pub block_number: u64,
	pub timestamp: U256,
}

#[derive(PartialEq, Eq, Encode, Decode, Clone, Default, TypeInfo)]
pub struct MockReceiptResponse {
	pub block_hash: H256,
	pub block_number: u64,
	pub transaction_hash: H256,
}

pub(crate) mod test_storage {
	//! storage used by tests
	use super::{MockBlockResponse, MockReceiptResponse};
	use crate::{types::EthHash, Config};
	use frame_support::decl_storage;
	pub struct Module<T>(sp_std::marker::PhantomData<T>);
	decl_storage! {
		trait Store for Module<T: Config> as EthBridgeTest {
			pub BlockResponseAt: map hasher(blake2_128_concat) u32 => Option<MockBlockResponse>;
			pub TransactionReceiptFor: map hasher(blake2_128_concat) EthHash => Option<MockReceiptResponse>;
		}
	}
}

/// Test request
#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
pub struct TestRequest {
	pub message: String,
}

/// Mock ethereum rpc client
pub struct MockEthereumRpcClient();

impl MockEthereumRpcClient {
	/// store given block as the next response
	pub fn mock_block_response_at(block_number: u32, mock_block: EthBlock) {
		let mock_block_response = MockBlockResponse {
			block_hash: mock_block.hash.unwrap(),
			block_number: mock_block.number.unwrap().as_u64(),
			timestamp: mock_block.timestamp,
		};
		test_storage::BlockResponseAt::insert(block_number, mock_block_response);
	}
	pub fn mock_transaction_receipt_for(tx_hash: EthHash, mock_tx_receipt: TransactionReceipt) {
		let mock_receipt_response = MockReceiptResponse {
			block_hash: mock_tx_receipt.block_hash,
			block_number: mock_tx_receipt.block_number.as_u64(),
			transaction_hash: mock_tx_receipt.transaction_hash,
		};
		test_storage::TransactionReceiptFor::insert(tx_hash, mock_receipt_response);
	}
}

impl BridgeEthereumRpcApi for MockEthereumRpcClient {
	/// Returns an ethereum block given a block height
	fn get_block_by_number(block_number: LatestOrNumber) -> Result<Option<EthBlock>, BridgeRpcError> {
		let (block_number, block_hash, timestamp): (u64, H256, U256) = match block_number {
			LatestOrNumber::Latest => {
				let block = System::block_number();
				(block, H256::default(), U256::default())
			}
			LatestOrNumber::Number(block) => {
				let mock_block_response = test_storage::BlockResponseAt::get(block);
				if mock_block_response.is_none() {
					return Ok(None);
				}
				let mock_block_response = mock_block_response.unwrap();
				(
					mock_block_response.block_number,
					mock_block_response.block_hash,
					U256::from(mock_block_response.timestamp),
				)
			}
		};

		let eth_block = EthBlock {
			number: Some(U64::from(block_number)),
			hash: Some(block_hash),
			timestamp: U256::from(timestamp),
			parent_hash: Default::default(),
			nonce: None,
			sha3_uncles: Default::default(),
			logs_bloom: None,
			transactions_root: Default::default(),
			state_root: Default::default(),
			receipts_root: Default::default(),
			miner: Default::default(),
			difficulty: Default::default(),
			total_difficulty: Default::default(),
			extra_data: vec![],
			size: Default::default(),
			gas_limit: Default::default(),
			gas_used: Default::default(),
			transactions: vec![],
			uncles: vec![],
		};
		Ok(Some(eth_block))
	}
	/// Returns an ethereum transaction receipt given a tx hash
	fn get_transaction_receipt(hash: EthHash) -> Result<Option<TransactionReceipt>, BridgeRpcError> {
		let mock_receipt: Option<MockReceiptResponse> = test_storage::TransactionReceiptFor::get(hash);
		if mock_receipt.is_none() {
			return Ok(None);
		}
		let mock_receipt = mock_receipt.unwrap();
		let transaction_receipt = TransactionReceipt {
			block_hash: mock_receipt.block_hash,
			block_number: U64::from(mock_receipt.block_number),
			contract_address: None,
			cumulative_gas_used: Default::default(),
			effective_gas_price: None,
			from: Default::default(),
			gas_used: None,
			logs: vec![],
			status: None,
			to: None,
			transaction_hash: mock_receipt.transaction_hash,
			transaction_index: Default::default(),
			root: None,
			logs_bloom: Default::default(),
			transaction_type: None,
			removed: false,
		};
		Ok(Some(transaction_receipt))
	}
}

pub struct NoopConverter<T>(sp_std::marker::PhantomData<T>);
impl<T: Config> Convert<T::AccountId, Option<T::AccountId>> for NoopConverter<T> {
	fn convert(address: T::AccountId) -> Option<T::AccountId> {
		Some(address)
	}
}

pub struct MockValidatorSet;
impl ValidatorSetT<AccountId> for MockValidatorSet {
	type ValidatorId = AccountId;
	type ValidatorIdOf = NoopConverter<TestRuntime>;
	/// Returns current session index.
	fn session_index() -> SessionIndex {
		1
	}
	/// Returns the active set of validators.
	fn validators() -> Vec<Self::ValidatorId> {
		Default::default()
	}
}

pub struct MockClaimSubscriber;
impl EventClaimSubscriber for MockClaimSubscriber {
	/// Notify subscriber about a successful event claim for the given event data
	fn on_success(_event_claim_id: u64, _contract_address: &H160, _event_signature: &H256Crml, _event_data: &[u8]) {}
	/// Notify subscriber about a failed event claim for the given event data
	fn on_failure(_event_claim_id: u64, _contract_address: &H160, _event_signature: &H256Crml, _event_data: &[u8]) {}
}

/// Mock final session tracker
pub struct MockFinalSessionTracker;
impl FinalSessionTracker for MockFinalSessionTracker {
	fn is_next_session_final() -> (bool, bool) {
		// at block 1, next session is final
		(frame_system::Pallet::<TestRuntime>::block_number() == 1, false)
	}
	fn is_active_session_final() -> bool {
		// at block 2, the active session is final
		frame_system::Pallet::<TestRuntime>::block_number() == 2
	}
}

/// Returns the current system time
pub struct MockRewardHandler;
impl NotarizationRewardHandler for MockRewardHandler {
	type AccountId = AccountId;
	fn reward_notary(_notary: &Self::AccountId) {
		// Do nothing
	}
}

/// Returns the current system time
pub struct MockUnixTime;
impl UnixTime for MockUnixTime {
	fn now() -> core::time::Duration {
		std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.unwrap()
	}
}

impl frame_system::offchain::SigningTypes for TestRuntime {
	type Public = <Signature as Verify>::Signer;
	type Signature = Signature;
}

impl<C> frame_system::offchain::SendTransactionTypes<C> for TestRuntime
where
	Call: From<C>,
{
	type OverarchingCall = Call;
	type Extrinsic = Extrinsic;
}

impl<LocalCall> frame_system::offchain::CreateSignedTransaction<LocalCall> for TestRuntime
where
	Call: From<LocalCall>,
{
	fn create_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
		call: Call,
		_public: <Signature as Verify>::Signer,
		_account: AccountId,
		nonce: u64,
	) -> Option<(Call, <Extrinsic as ExtrinsicT>::SignaturePayload)> {
		Some((call, (nonce, ())))
	}
}

// Mock withdraw message from ERC20-Peg
pub struct MockWithdrawMessage<TestRuntime>(pub PhantomData<TestRuntime>);

impl EthAbiCodec for MockWithdrawMessage<TestRuntime> {
	fn encode(&self) -> Vec<u8> {
		[0_u8; 32 * 3].to_vec()
	}

	fn decode(_data: &[u8]) -> Option<Self> {
		unimplemented!();
	}
}

#[derive(Clone, Copy, Default)]
pub struct ExtBuilder {
	next_session_final: bool,
	active_session_final: bool,
}

impl ExtBuilder {
	pub fn active_session_final(&mut self) -> &mut Self {
		self.active_session_final = true;
		self
	}
	pub fn next_session_final(&mut self) -> &mut Self {
		self.next_session_final = true;
		self
	}
	pub fn build(self) -> sp_io::TestExternalities {
		let mut ext: sp_io::TestExternalities = frame_system::GenesisConfig::default()
			.build_storage::<TestRuntime>()
			.unwrap()
			.into();
		if self.next_session_final {
			ext.execute_with(|| frame_system::Pallet::<TestRuntime>::set_block_number(1));
		} else if self.active_session_final {
			ext.execute_with(|| frame_system::Pallet::<TestRuntime>::set_block_number(2));
		}

		ext
	}
}

#[test]
fn get_block_by_number_mock_works() {
	ExtBuilder::default().build().execute_with(|| {
		let block_number: u32 = 120;
		let block_hash: H256 = H256::from_low_u64_be(121);
		let timestamp: U256 = U256::from(122);

		let mock_block = EthBlock {
			number: Some(U64::from(block_number)),
			hash: Some(block_hash),
			timestamp,
			parent_hash: Default::default(),
			nonce: None,
			sha3_uncles: Default::default(),
			logs_bloom: None,
			transactions_root: Default::default(),
			state_root: Default::default(),
			receipts_root: Default::default(),
			miner: Default::default(),
			difficulty: Default::default(),
			total_difficulty: Default::default(),
			extra_data: vec![],
			size: Default::default(),
			gas_limit: Default::default(),
			gas_used: Default::default(),
			transactions: vec![],
			uncles: vec![],
		};

		MockEthereumRpcClient::mock_block_response_at(block_number, mock_block.clone());

		let result =
			<MockEthereumRpcClient as BridgeEthereumRpcApi>::get_block_by_number(LatestOrNumber::Number(block_number))
				.unwrap();
		assert_eq!(Some(mock_block), result);
	});
}

#[test]
fn get_latest_block_by_number_mock_works() {
	ExtBuilder::default().build().execute_with(|| {
		let block_number = System::block_number();

		let mock_block = EthBlock {
			number: Some(U64::from(block_number)),
			hash: Some(H256::default()),
			timestamp: U256::default(),
			parent_hash: Default::default(),
			nonce: None,
			sha3_uncles: Default::default(),
			logs_bloom: None,
			transactions_root: Default::default(),
			state_root: Default::default(),
			receipts_root: Default::default(),
			miner: Default::default(),
			difficulty: Default::default(),
			total_difficulty: Default::default(),
			extra_data: vec![],
			size: Default::default(),
			gas_limit: Default::default(),
			gas_used: Default::default(),
			transactions: vec![],
			uncles: vec![],
		};

		let result =
			<MockEthereumRpcClient as BridgeEthereumRpcApi>::get_block_by_number(LatestOrNumber::Latest).unwrap();
		assert_eq!(Some(mock_block), result);
	});
}

#[test]
fn get_transaction_receipt_mock_works() {
	ExtBuilder::default().build().execute_with(|| {
		let block_number: u32 = 120;
		let block_hash: H256 = H256::from_low_u64_be(121);
		let tx_hash: EthHash = H256::from_low_u64_be(122);

		let mock_tx_receipt = TransactionReceipt {
			block_hash,
			block_number: U64::from(block_number),
			contract_address: None,
			cumulative_gas_used: Default::default(),
			effective_gas_price: None,
			from: Default::default(),
			gas_used: None,
			logs: vec![],
			status: None,
			to: None,
			transaction_hash: tx_hash,
			transaction_index: Default::default(),
			root: None,
			logs_bloom: Default::default(),
			transaction_type: None,
			removed: false,
		};

		MockEthereumRpcClient::mock_transaction_receipt_for(tx_hash, mock_tx_receipt.clone());

		let result = <MockEthereumRpcClient as BridgeEthereumRpcApi>::get_transaction_receipt(tx_hash).unwrap();
		assert_eq!(Some(mock_tx_receipt), result);
	});
}
