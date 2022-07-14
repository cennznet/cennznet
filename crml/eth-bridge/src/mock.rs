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

use crate::{
	self as crml_eth_bridge,
	sp_api_hidden_includes_decl_storage::hidden_include::{IterableStorageMap, StorageMap},
	types::{
		BridgeEthereumRpcApi, BridgeRpcError, CheckedEthCallRequest, CheckedEthCallResult, EthAddress, EthBlock,
		EthCallId, EthHash, LatestOrNumber, Log, TransactionReceipt,
	},
	Config,
};
use cennznet_primitives::eth::crypto::AuthorityId;
use codec::{Decode, Encode};
use crml_support::{
	EthAbiCodec, EthCallFailure, EthCallOracleSubscriber, EventClaimSubscriber, FinalSessionTracker,
	NotarizationRewardHandler, H160, H256 as H256Crml, U256,
};
use ethereum_types::U64;
use frame_support::{
	parameter_types,
	storage::{StorageDoubleMap, StorageValue},
	traits::{UnixTime, ValidatorSet as ValidatorSetT},
};
use scale_info::TypeInfo;
use sp_core::{ecdsa::Signature, ByteArray, H256};
use sp_runtime::{
	testing::{Header, TestXt},
	traits::{BlakeTwo256, Convert, Extrinsic as ExtrinsicT, IdentifyAccount, IdentityLookup, Verify},
	Percent,
};
use std::{
	marker::PhantomData,
	time::{SystemTime, UNIX_EPOCH},
};

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
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

parameter_types! {
	pub const DefaultListingDuration: u64 = 5;
	pub const MaxAttributeLength: u8 = 140;
	pub const NotarizationThreshold: Percent = Percent::from_parts(66_u8);
}
impl Config for TestRuntime {
	type AuthoritySet = MockValidatorSet;
	type EthCallSubscribers = MockEthCallSubscriber;
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

/// Values in EthBlock that we store in mock storage
#[derive(PartialEq, Eq, Encode, Decode, Debug, Clone, Default, TypeInfo)]
pub struct MockBlockResponse {
	pub block_hash: H256,
	pub block_number: u64,
	pub timestamp: U256,
}

/// Values in TransactionReceipt that we store in mock storage
#[derive(PartialEq, Eq, Encode, Decode, Clone, Default, TypeInfo)]
pub struct MockReceiptResponse {
	pub block_hash: H256,
	pub block_number: u64,
	pub transaction_hash: H256,
	pub status: u64,
	pub contract_address: Option<EthAddress>,
}

/// Builder for creating EthBlocks
pub(crate) struct MockBlockBuilder(EthBlock);

impl MockBlockBuilder {
	pub fn new() -> Self {
		MockBlockBuilder(EthBlock::default())
	}
	pub fn build(&self) -> EthBlock {
		self.0.clone()
	}
	pub fn block_hash(&mut self, block_hash: H256) -> &mut Self {
		self.0.hash = Some(block_hash);
		self
	}
	pub fn block_number(&mut self, block_number: u64) -> &mut Self {
		self.0.number = Some(U64::from(block_number));
		self
	}
	pub fn timestamp(&mut self, timestamp: U256) -> &mut Self {
		self.0.timestamp = timestamp;
		self
	}
}

/// Builder for creating TransactionReceipts
pub(crate) struct MockReceiptBuilder(TransactionReceipt);

impl MockReceiptBuilder {
	pub fn new() -> Self {
		MockReceiptBuilder(TransactionReceipt {
			status: Some(U64::from(1)),
			..Default::default()
		})
	}
	pub fn build(&self) -> TransactionReceipt {
		self.0.clone()
	}
	pub fn block_number(&mut self, block_number: u64) -> &mut Self {
		self.0.block_number = U64::from(block_number);
		self
	}
	pub fn status(&mut self, status: u64) -> &mut Self {
		self.0.status = Some(U64::from(status));
		self
	}
	pub fn transaction_hash(&mut self, tx_hash: H256) -> &mut Self {
		self.0.transaction_hash = tx_hash;
		self
	}
	pub fn contract_address(&mut self, contract_address: EthAddress) -> &mut Self {
		self.0.contract_address = Some(contract_address);
		self
	}
}

pub(crate) mod test_storage {
	//! storage used by tests to store mock EthBlocks and TransactionReceipts
	use super::{AccountId, MockBlockResponse, MockReceiptResponse};
	use crate::{
		types::{CheckedEthCallResult, EthAddress, EthCallId, EthHash},
		Config,
	};
	use crml_support::EthCallFailure;
	use frame_support::decl_storage;
	pub struct Module<T>(sp_std::marker::PhantomData<T>);
	decl_storage! {
		trait Store for Module<T: Config> as EthBridgeTest {
			pub BlockResponseAt: map hasher(identity) u64 => Option<MockBlockResponse>;
			pub CallAt: double_map hasher(twox_64_concat) u64, hasher(twox_64_concat) EthAddress => Option<Vec<u8>>;
			pub TransactionReceiptFor: map hasher(twox_64_concat) EthHash => Option<MockReceiptResponse>;
			pub Timestamp: Option<u64>;
			pub Validators: Vec<AccountId>;
			pub LastCallResult: Option<(EthCallId, CheckedEthCallResult)>;
			pub LastCallFailure: Option<(EthCallId, EthCallFailure)>;
		}
	}
}

/// set the block timestamp
pub fn mock_timestamp(now: u64) {
	test_storage::Timestamp::put(now);
}

// get the system unix timestamp in seconds
pub fn now() -> u64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.expect("after unix epoch")
		.as_secs()
}

/// Builder for `CheckedEthCallRequest`
pub struct CheckedEthCallRequestBuilder(CheckedEthCallRequest);

impl CheckedEthCallRequestBuilder {
	pub fn new() -> Self {
		Self(CheckedEthCallRequest {
			max_block_look_behind: 3_u64,
			target: EthAddress::from_low_u64_be(1),
			timestamp: now(),
			check_timestamp: now() + 3 * 5, // 3 blocks
			..Default::default()
		})
	}
	pub fn build(self) -> CheckedEthCallRequest {
		self.0
	}
	pub fn target(mut self, target: EthAddress) -> Self {
		self.0.target = target;
		self
	}
	pub fn try_block_number(mut self, try_block_number: u64) -> Self {
		self.0.try_block_number = try_block_number;
		self
	}
	pub fn max_block_look_behind(mut self, max_block_look_behind: u64) -> Self {
		self.0.max_block_look_behind = max_block_look_behind;
		self
	}
	pub fn check_timestamp(mut self, check_timestamp: u64) -> Self {
		self.0.check_timestamp = check_timestamp;
		self
	}
	pub fn timestamp(mut self, timestamp: u64) -> Self {
		self.0.timestamp = timestamp;
		self
	}
}

/// Mock ethereum rpc client
pub struct MockEthereumRpcClient;

impl MockEthereumRpcClient {
	/// store given block as the next response
	pub fn mock_block_response_at(block_number: u64, mock_block: EthBlock) {
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
			status: mock_tx_receipt.status.unwrap_or_default().as_u64(),
			contract_address: mock_tx_receipt.contract_address,
		};
		test_storage::TransactionReceiptFor::insert(tx_hash, mock_receipt_response);
	}
	/// setup a mock returndata for an `eth_call` at `block` and `contract` address
	pub fn mock_call_at(block_number: u64, contract: H160, return_data: &[u8]) {
		test_storage::CallAt::insert(block_number, contract, return_data.to_vec())
	}
}

impl BridgeEthereumRpcApi for MockEthereumRpcClient {
	/// Returns an ethereum block given a block height
	fn get_block_by_number(block_number: LatestOrNumber) -> Result<Option<EthBlock>, BridgeRpcError> {
		let mock_block_response = match block_number {
			LatestOrNumber::Latest => test_storage::BlockResponseAt::iter().last().map(|x| x.1).or(None),
			LatestOrNumber::Number(block) => test_storage::BlockResponseAt::get(block),
		};
		println!("get_block_by_number at: {:?}", mock_block_response);
		if mock_block_response.is_none() {
			return Ok(None);
		}
		let mock_block_response = mock_block_response.unwrap();

		let eth_block = EthBlock {
			number: Some(U64::from(mock_block_response.block_number)),
			hash: Some(mock_block_response.block_hash),
			timestamp: U256::from(mock_block_response.timestamp),
			..Default::default()
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
		// Inject a default Log with some default topics
		let default_log: Log = Log {
			address: mock_receipt.contract_address.unwrap(),
			topics: vec![Default::default()],
			transaction_hash: Some(hash),
			..Default::default()
		};
		let transaction_receipt = TransactionReceipt {
			block_hash: mock_receipt.block_hash,
			block_number: U64::from(mock_receipt.block_number),
			contract_address: mock_receipt.contract_address,
			to: mock_receipt.contract_address,
			status: Some(U64::from(mock_receipt.status)),
			transaction_hash: mock_receipt.transaction_hash,
			logs: vec![default_log],
			..Default::default()
		};
		Ok(Some(transaction_receipt))
	}
	fn eth_call(target: EthAddress, _input: &[u8], at_block: LatestOrNumber) -> Result<Vec<u8>, BridgeRpcError> {
		let block_number = match at_block {
			LatestOrNumber::Number(n) => n,
			LatestOrNumber::Latest => test_storage::BlockResponseAt::iter().last().unwrap().1.block_number,
		};
		println!("eth_call at: {:?}", block_number);
		test_storage::CallAt::get(block_number, target).ok_or(BridgeRpcError::HttpFetch)
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
		test_storage::Validators::get()
	}
}
impl MockValidatorSet {
	/// Mock n validator stashes
	pub fn mock_n_validators(n: u8) {
		let validators: Vec<AccountId> = (1..=n).map(|i| AccountId::from_slice(&[i; 33]).unwrap()).collect();
		test_storage::Validators::put(validators);
	}
}

pub struct MockClaimSubscriber;
impl EventClaimSubscriber for MockClaimSubscriber {
	/// Notify subscriber about a successful event claim for the given event data
	fn on_success(_event_claim_id: u64, _contract_address: &H160, _event_signature: &H256Crml, _event_data: &[u8]) {}
	/// Notify subscriber about a failed event claim for the given event data
	fn on_failure(_event_claim_id: u64, _contract_address: &H160, _event_signature: &H256Crml, _event_data: &[u8]) {}
}

pub struct MockEthCallSubscriber;
impl EthCallOracleSubscriber for MockEthCallSubscriber {
	type CallId = EthCallId;
	/// Stores the successful call info
	/// Available via `Self::success_result_for()`
	fn on_eth_call_complete(call_id: Self::CallId, return_data: &[u8; 32], block_number: u64, block_timestamp: u64) {
		test_storage::LastCallResult::put((
			call_id,
			CheckedEthCallResult::Ok(*return_data, block_number, block_timestamp),
		));
	}
	/// Stores the failed call info
	/// Available via `Self::failed_call_for()`
	fn on_eth_call_failed(call_id: Self::CallId, reason: EthCallFailure) {
		test_storage::LastCallFailure::put((call_id, reason));
	}
}

impl MockEthCallSubscriber {
	/// Returns last known successful call, if any
	pub fn success_result() -> Option<(EthCallId, CheckedEthCallResult)> {
		test_storage::LastCallResult::get()
	}
	/// Returns last known failed call, if any
	pub fn failed_result() -> Option<(EthCallId, EthCallFailure)> {
		test_storage::LastCallFailure::get()
	}
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

/// Returns a fake timestamp based on the current block number
pub struct MockUnixTime;
impl UnixTime for MockUnixTime {
	fn now() -> core::time::Duration {
		match test_storage::Timestamp::get() {
			// Use configured value for tests requiring precise timestamps
			Some(s) => core::time::Duration::new(s, 0),
			// fallback, use block number to derive timestamp for tests that only care abut block progression
			None => core::time::Duration::new(System::block_number() * 5, 0),
		}
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
		let block_number: u64 = 120;
		let block_hash: H256 = H256::from_low_u64_be(121);
		let timestamp: U256 = U256::from(122);

		let mock_block = EthBlock {
			number: Some(U64::from(block_number)),
			hash: Some(block_hash),
			timestamp,
			..Default::default()
		};

		MockEthereumRpcClient::mock_block_response_at(block_number, mock_block.clone());

		let result =
			<MockEthereumRpcClient as BridgeEthereumRpcApi>::get_block_by_number(LatestOrNumber::Number(block_number))
				.unwrap();
		assert_eq!(Some(mock_block), result);
	});
}

#[test]
fn mock_eth_call_at_latest_block() {
	ExtBuilder::default().build().execute_with(|| {
		for i in 0..10_u64 {
			let mock_block = EthBlock {
				number: Some(U64::from(i)),
				hash: Some(H256::from_low_u64_be(i)),
				..Default::default()
			};
			MockEthereumRpcClient::mock_block_response_at(i, mock_block.clone());
		}
		// checking this returns latest block
		MockEthereumRpcClient::mock_call_at(9, EthAddress::from_low_u64_be(1), &[1_u8, 2, 3]);

		assert_eq!(
			MockEthereumRpcClient::eth_call(EthAddress::from_low_u64_be(1), &[4_u8, 5, 6], LatestOrNumber::Latest),
			Ok(vec![1_u8, 2, 3])
		);
	});
}

#[test]
fn get_latest_block_by_number_mock_works() {
	ExtBuilder::default().build().execute_with(|| {
		let block_number = 12;

		let mock_block = EthBlock {
			number: Some(U64::from(block_number)),
			hash: Some(H256::default()),
			timestamp: U256::default(),
			..Default::default()
		};
		MockEthereumRpcClient::mock_block_response_at(block_number, mock_block.clone());

		let result =
			<MockEthereumRpcClient as BridgeEthereumRpcApi>::get_block_by_number(LatestOrNumber::Latest).unwrap();
		assert_eq!(Some(mock_block), result);
	});
}

#[test]
fn get_transaction_receipt_mock_works() {
	ExtBuilder::default().build().execute_with(|| {
		let block_number: u64 = 120;
		let block_hash: H256 = H256::from_low_u64_be(121);
		let tx_hash: EthHash = H256::from_low_u64_be(122);
		let status: U64 = U64::from(1);
		let contract_address: EthAddress = H160::from_low_u64_be(123);
		let default_log: Log = Log {
			address: contract_address,
			topics: vec![Default::default()],
			transaction_hash: Some(tx_hash),
			..Default::default()
		};

		let mock_tx_receipt = TransactionReceipt {
			block_hash,
			block_number: U64::from(block_number),
			contract_address: Some(contract_address),
			logs: vec![default_log],
			status: Some(status),
			to: Some(contract_address),
			transaction_hash: tx_hash,
			..Default::default()
		};

		MockEthereumRpcClient::mock_transaction_receipt_for(tx_hash, mock_tx_receipt.clone());

		let result = <MockEthereumRpcClient as BridgeEthereumRpcApi>::get_transaction_receipt(tx_hash).unwrap();
		assert_eq!(Some(mock_tx_receipt), result);
	});
}
