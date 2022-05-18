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

use super::BridgePaused;
use crate as crml_eth_bridge;
use crate::types::{EthBlock, EthHash, LatestOrNumber, TransactionReceipt};
use crate::{
	types::{BridgeEthereumRpcApi, EventProofId},
	Config, Error, Module,
};
use cennznet_primitives::eth::crypto::AuthorityId;
use crml_support::{
	EthAbiCodec, EventClaimSubscriber, EventClaimVerifier, FinalSessionTracker, NotarizationRewardHandler, H160,
	H256 as H256Crml,
};
use frame_support::{
	assert_noop, assert_ok,
	dispatch::DispatchError,
	parameter_types,
	storage::StorageValue,
	traits::{OnInitialize, OneSessionHandler, UnixTime, ValidatorSet as ValidatorSetT},
	weights::{constants::RocksDbWeight as DbWeight, Weight},
};
use sp_core::{
	ecdsa::Signature,
	offchain::{testing, OffchainDbExt, OffchainWorkerExt},
	Public, H256,
};
use sp_runtime::offchain::StorageKind;
use sp_runtime::{
	testing::{Header, TestXt},
	traits::{BlakeTwo256, Convert, Extrinsic as ExtrinsicT, IdentifyAccount, IdentityLookup, Verify},
	Percent,
};
use std::marker::PhantomData;

type SessionIndex = u32;
type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

type Extrinsic = TestXt<Call, ()>;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;
type Block = frame_system::mocking::MockBlock<TestRuntime>;

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
	type EthereumRpcClient = MockEthereumRpcClient<Self>;
	type FinalSessionTracker = MockFinalSessionTracker;
	type NotarizationThreshold = NotarizationThreshold;
	type RewardHandler = MockRewardHandler;
	type Subscribers = MockClaimSubscriber;
	type UnixTime = MockUnixTime;
	type Call = Call;
	type Event = Event;
}

/// Mock ethereum rpc client
pub struct MockEthereumRpcClient<T: Config>(PhantomData<T>);

impl<T: Config> MockEthereumRpcClient<T> {
	/// store given block as the next response
	pub fn mock_block_response_at(block_number: u32, mock_block: EthBlock) {
		// TODO: implement
		unimplemented!();
	}
	pub fn mock_transaction_receipt_for(tx_hash: EthHash, mock_tx_receipt: TransactionReceipt) {
		// TODO: implement
		unimplemented!();
	}
}

impl<T: Config> BridgeEthereumRpcApi<T> for MockEthereumRpcClient<T> {
	/// Returns an ethereum block given a block height
	fn get_block_by_number(block_number: LatestOrNumber) -> Result<Option<EthBlock>, Error<T>> {
		// TODO: implement
		unimplemented!();
	}
	/// Returns an ethereum transaction receipt given a tx hash
	fn get_transaction_receipt(hash: EthHash) -> Result<Option<TransactionReceipt>, Error<T>> {
		// TODO: implement
		unimplemented!();
	}

	fn query_eth_client<R: serde::Serialize>(request_body: R) -> Result<Vec<u8>, Error<T>> {
		// TODO: Implement
		unimplemented!();
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
struct MockWithdrawMessage<TestRuntime>(PhantomData<TestRuntime>);

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

/// Mock eth-http endpoint
const MOCK_ETH_HTTP_URI: [u8; 31] = *b"http://ethereum-rpc.example.com";

/// Test request
#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
struct TestRequest {
	message: String,
}

#[test]
fn tracks_pending_claims() {
	ExtBuilder::default().build().execute_with(|| {
		let contract_address = H160::from_low_u64_be(11);
		let event_signature = H256::from_low_u64_be(22);
		let tx_hash = H256::from_low_u64_be(33);
		let event_data = [1u8, 2, 3, 4, 5];
		assert_ok!(Module::<TestRuntime>::submit_event_claim(
			&contract_address,
			&event_signature,
			&tx_hash,
			&event_data
		));
		assert_noop!(
			Module::<TestRuntime>::submit_event_claim(&contract_address, &event_signature, &tx_hash, &event_data),
			Error::<TestRuntime>::DuplicateClaim
		);
	});
}

#[test]
fn pre_last_session_change() {
	ExtBuilder::default().next_session_final().build().execute_with(|| {
		let current_keys = vec![
			AuthorityId::from_slice(&[1_u8; 33]),
			AuthorityId::from_slice(&[2_u8; 33]),
		];
		let next_keys = vec![
			AuthorityId::from_slice(&[3_u8; 33]),
			AuthorityId::from_slice(&[4_u8; 33]),
		];
		let event_proof_id = Module::<TestRuntime>::next_proof_id();

		Module::<TestRuntime>::handle_authorities_change(current_keys, next_keys.clone());

		assert_eq!(Module::<TestRuntime>::next_notary_keys(), next_keys);
		assert_eq!(Module::<TestRuntime>::notary_set_proof_id(), event_proof_id);
		assert_eq!(Module::<TestRuntime>::next_proof_id(), event_proof_id + 1);
	});
}

#[test]
fn last_session_change() {
	ExtBuilder::default().active_session_final().build().execute_with(|| {
		let current_set_id = Module::<TestRuntime>::notary_set_id();

		// setup storage
		let current_keys = vec![
			AuthorityId::from_slice(&[1_u8; 33]),
			AuthorityId::from_slice(&[2_u8; 33]),
		];
		crate::NotaryKeys::<TestRuntime>::put(&current_keys);
		let next_keys = vec![
			AuthorityId::from_slice(&[3_u8; 33]),
			AuthorityId::from_slice(&[4_u8; 33]),
		];
		crate::NextNotaryKeys::<TestRuntime>::put(&next_keys);

		// current session is last in era: starting
		Module::<TestRuntime>::handle_authorities_change(current_keys, next_keys.clone());
		assert!(Module::<TestRuntime>::bridge_paused());
		// current session is last in era: finishing
		<Module<TestRuntime> as OneSessionHandler<AccountId>>::on_before_session_ending();
		assert_eq!(Module::<TestRuntime>::notary_keys(), next_keys);
		assert_eq!(Module::<TestRuntime>::notary_set_id(), current_set_id + 1);
		assert!(!Module::<TestRuntime>::bridge_paused());
	});
}

#[test]
fn eth_client_http_request() {
	let (offchain, offchain_state) = testing::TestOffchainExt::new();
	let mut t = sp_io::TestExternalities::default();
	t.register_extension(OffchainDbExt::new(offchain.clone()));
	t.register_extension(OffchainWorkerExt::new(offchain));
	// Set the ethereum http endpoint for OCW queries
	t.execute_with(|| sp_io::offchain::local_storage_set(StorageKind::PERSISTENT, b"ETH_HTTP", &MOCK_ETH_HTTP_URI));

	// Setup
	// Mock an ethereum JSON-RPC response
	let request_body = TestRequest {
		message: "hello ethereum".to_string(),
	};
	let request_body_raw = serde_json::to_string(&request_body).unwrap();
	{
		let mut offchain_state = offchain_state.write();
		offchain_state.expect_request(testing::PendingRequest {
			method: "POST".into(),
			uri: core::str::from_utf8(&MOCK_ETH_HTTP_URI)
				.expect("valid utf8")
				.to_string(),
			body: request_body_raw.as_bytes().to_vec(),
			response: Some(br#"{"message":"hello cennznet"}"#.to_vec()),
			headers: vec![
				("Content-Type".to_string(), "application/json".to_string()),
				("Content-Length".to_string(), request_body_raw.len().to_string()),
			],
			sent: true,
			..Default::default()
		});
	}

	// Test
	t.execute_with(|| {
		let response =
			<MockEthereumRpcClient<TestRuntime> as BridgeEthereumRpcApi<TestRuntime>>::query_eth_client(request_body)
				.expect("got response");
		assert_eq!(
			serde_json::from_slice::<'_, TestRequest>(response.as_slice()).unwrap(),
			TestRequest {
				message: "hello cennznet".to_string()
			}
		);
	})
}

#[test]
fn generate_event_proof() {
	ExtBuilder::default().build().execute_with(|| {
		// Test generating event proof without delay
		let message = MockWithdrawMessage { 0: Default::default() };
		let event_proof_id = Module::<TestRuntime>::next_proof_id();

		// Generate event proof
		assert_ok!(Module::<TestRuntime>::generate_event_proof(&message));
		// Ensure event has not been added to delayed claims
		assert_eq!(Module::<TestRuntime>::delayed_event_proofs(event_proof_id), None);
		assert_eq!(Module::<TestRuntime>::next_proof_id(), event_proof_id + 1);
		// On initialize should return 0 weight as there are no pending proofs
		assert_eq!(
			Module::<TestRuntime>::on_initialize(frame_system::Pallet::<TestRuntime>::block_number() + 1),
			DbWeight::get().reads(1 as Weight)
		);
	});
}

#[test]
fn delayed_event_proof() {
	ExtBuilder::default().build().execute_with(|| {
		let message = MockWithdrawMessage { 0: Default::default() };
		BridgePaused::put(true);
		assert_eq!(Module::<TestRuntime>::bridge_paused(), true);

		let event_proof_id = Module::<TestRuntime>::next_proof_id();
		let packed_event_with_id = [
			&message.encode()[..],
			&EthAbiCodec::encode(&Module::<TestRuntime>::validator_set().id)[..],
			&EthAbiCodec::encode(&event_proof_id)[..],
		]
		.concat();

		// Generate event proof
		assert_ok!(Module::<TestRuntime>::generate_event_proof(&message));
		// Ensure event has been added to delayed claims
		assert_eq!(
			Module::<TestRuntime>::delayed_event_proofs(event_proof_id),
			Some(packed_event_with_id)
		);
		assert_eq!(Module::<TestRuntime>::next_proof_id(), event_proof_id + 1);

		// Re-enable bridge
		BridgePaused::put(false);
		// initialize pallet and initiate event proof
		let max_delayed_events = Module::<TestRuntime>::delayed_event_proofs_per_block() as u64;
		let expected_weight: Weight =
			DbWeight::get().reads(3 as Weight) + DbWeight::get().writes(2 as Weight) * max_delayed_events;
		assert_eq!(
			Module::<TestRuntime>::on_initialize(frame_system::Pallet::<TestRuntime>::block_number() + 1),
			expected_weight
		);
		// Ensure event has been removed from delayed claims
		assert_eq!(Module::<TestRuntime>::delayed_event_proofs(event_proof_id), None);
	});
}

#[test]
fn multiple_delayed_event_proof() {
	ExtBuilder::default().build().execute_with(|| {
		let message = MockWithdrawMessage { 0: Default::default() };
		BridgePaused::put(true);
		assert_eq!(Module::<TestRuntime>::bridge_paused(), true);

		let max_delayed_events = Module::<TestRuntime>::delayed_event_proofs_per_block();
		let event_count: u8 = max_delayed_events * 2;
		let mut event_ids: Vec<EventProofId> = vec![];
		let mut packed_event_with_ids = vec![];
		for _ in 0..event_count {
			let event_proof_id = Module::<TestRuntime>::next_proof_id();
			event_ids.push(event_proof_id);
			let packed_event_with_id = [
				&message.encode()[..],
				&EthAbiCodec::encode(&Module::<TestRuntime>::validator_set().id)[..],
				&EthAbiCodec::encode(&event_proof_id)[..],
			]
			.concat();
			packed_event_with_ids.push(packed_event_with_id.clone());
			// Generate event proof
			assert_ok!(Module::<TestRuntime>::generate_event_proof(&message));
			// Ensure event has been added to delayed claims
			assert_eq!(
				Module::<TestRuntime>::delayed_event_proofs(event_proof_id),
				Some(packed_event_with_id)
			);
			assert_eq!(Module::<TestRuntime>::next_proof_id(), event_proof_id + 1);
		}

		// Re-enable bridge
		BridgePaused::put(false);
		// initialize pallet and initiate event proof
		assert_eq!(
			Module::<TestRuntime>::on_initialize(frame_system::Pallet::<TestRuntime>::block_number() + 1),
			DbWeight::get().reads(3 as Weight) + DbWeight::get().writes(2 as Weight) * max_delayed_events as u64
		);

		let mut removed_count = 0;
		for i in 0..event_count {
			// Ensure event has been removed from delayed claims
			if Module::<TestRuntime>::delayed_event_proofs(event_ids[i as usize]) == None {
				removed_count += 1;
			} else {
				assert_eq!(
					Module::<TestRuntime>::delayed_event_proofs(event_ids[i as usize]),
					Some(packed_event_with_ids[i as usize].clone())
				)
			}
		}
		// Should have only processed max amount
		assert_eq!(removed_count, max_delayed_events);

		// Now initialize next block and process the rest
		assert_eq!(
			Module::<TestRuntime>::on_initialize(frame_system::Pallet::<TestRuntime>::block_number() + 2),
			DbWeight::get().reads(3 as Weight) + DbWeight::get().writes(2 as Weight) * max_delayed_events as u64
		);

		let mut removed_count = 0;
		for i in 0..event_count {
			// Ensure event has been removed from delayed claims
			if Module::<TestRuntime>::delayed_event_proofs(event_ids[i as usize]) == None {
				removed_count += 1;
			}
		}
		// All events should have now been processed
		assert_eq!(removed_count, event_count);
	});
}

#[test]
fn set_delayed_event_proofs_per_block() {
	ExtBuilder::default().build().execute_with(|| {
		// Check that it starts as default value
		assert_eq!(Module::<TestRuntime>::delayed_event_proofs_per_block(), 5);
		let new_max_delayed_events: u8 = 10;
		assert_ok!(Module::<TestRuntime>::set_delayed_event_proofs_per_block(
			frame_system::RawOrigin::Root.into(),
			new_max_delayed_events
		));
		assert_eq!(
			Module::<TestRuntime>::delayed_event_proofs_per_block(),
			new_max_delayed_events
		);

		let message = MockWithdrawMessage { 0: Default::default() };
		let mut event_ids: Vec<EventProofId> = vec![];
		BridgePaused::put(true);

		for _ in 0..new_max_delayed_events {
			let event_proof_id = Module::<TestRuntime>::next_proof_id();
			event_ids.push(event_proof_id);
			let packed_event_with_id = [
				&message.encode()[..],
				&EthAbiCodec::encode(&Module::<TestRuntime>::validator_set().id)[..],
				&EthAbiCodec::encode(&event_proof_id)[..],
			]
			.concat();
			// Generate event proof
			assert_ok!(Module::<TestRuntime>::generate_event_proof(&message));
			// Ensure event has been added to delayed claims
			assert_eq!(
				Module::<TestRuntime>::delayed_event_proofs(event_proof_id),
				Some(packed_event_with_id)
			);
			assert_eq!(Module::<TestRuntime>::next_proof_id(), event_proof_id + 1);
		}

		// Re-enable bridge
		BridgePaused::put(false);
		// initialize pallet and initiate event proof
		assert_eq!(
			Module::<TestRuntime>::on_initialize(frame_system::Pallet::<TestRuntime>::block_number() + 1),
			DbWeight::get().reads(3 as Weight) + DbWeight::get().writes(2 as Weight) * new_max_delayed_events as u64
		);

		for i in 0..new_max_delayed_events {
			// Ensure event has been removed from delayed claims
			assert_eq!(Module::<TestRuntime>::delayed_event_proofs(event_ids[i as usize]), None);
		}
	});
}

#[test]
fn set_delayed_event_proofs_per_block_not_root_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		// Check that it starts as default value
		assert_eq!(Module::<TestRuntime>::delayed_event_proofs_per_block(), 5);
		let new_value: u8 = 10;
		assert_noop!(
			Module::<TestRuntime>::set_delayed_event_proofs_per_block(frame_system::RawOrigin::None.into(), new_value),
			DispatchError::BadOrigin
		);
		assert_eq!(Module::<TestRuntime>::delayed_event_proofs_per_block(), 5);
	});
}

#[test]
fn prunes_expired_events() {}

#[test]
fn double_claim_fails() {}

#[test]
fn invalid_notarization_fails() {}
