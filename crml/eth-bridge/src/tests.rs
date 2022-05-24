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
	mock::*,
	types::{EthAddress, EthBlock, EthHash, EventClaim, EventClaimResult, EventProofId, TransactionReceipt},
	BridgePaused, Error, Module, ProcessedTxBuckets, ProcessedTxHashes, BUCKET_FACTOR_S, CLAIM_PRUNING_INTERVAL,
};
use cennznet_primitives::eth::crypto::AuthorityId;
use crml_support::{EthAbiCodec, EventClaimVerifier, H160, U256};
use frame_support::{
	assert_noop, assert_ok,
	dispatch::DispatchError,
	storage::{IterableStorageDoubleMap, StorageDoubleMap, StorageMap, StorageValue},
	traits::{OnInitialize, OneSessionHandler, UnixTime},
	weights::{constants::RocksDbWeight as DbWeight, Weight},
};
use sp_core::{
	offchain::{testing, OffchainDbExt, OffchainWorkerExt},
	Public, H256,
};
use sp_runtime::{offchain::StorageKind, traits::Zero, SaturatedConversion};

/// Mocks an Eth block for when get_block_by_number is called
/// Adds this to the mock storage
/// The latest block will be the highest block stored
fn mock_block_response(block_number: u64, block_hash: H256, timestamp: U256) -> EthBlock {
	let mock_block = MockBlockBuilder::new()
		.block_number(block_number)
		.block_hash(block_hash)
		.timestamp(timestamp)
		.build();

	MockEthereumRpcClient::mock_block_response_at(block_number.saturated_into(), mock_block.clone());
	mock_block
}

/// Mocks a TransactionReceipt for when get_transaction_receipt is called
/// Adds this to the mock storage
fn create_transaction_receipt_mock(
	block_number: u64,
	block_hash: H256,
	tx_hash: EthHash,
	contract_address: EthAddress,
) -> TransactionReceipt {
	let mock_tx_receipt = MockReceiptBuilder::new()
		.block_number(block_number)
		.block_hash(block_hash)
		.transaction_hash(tx_hash)
		.contract_address(contract_address)
		.build();

	MockEthereumRpcClient::mock_transaction_receipt_for(tx_hash, mock_tx_receipt.clone());
	mock_tx_receipt
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
		// On initialize does up to 2 reads to check for delayed proofs
		assert_eq!(
			Module::<TestRuntime>::on_initialize(frame_system::Pallet::<TestRuntime>::block_number() + 1),
			DbWeight::get().reads(2 as Weight)
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
fn on_initialize_prunes_expired_tx_hashes() {
	ExtBuilder::default().build().execute_with(|| {
		// setup 2 buckets of txs
		// check the buckets are removed only when block + timestamp says they're expired
		// it prunes correct tx bucket
		// it prunes correct tx hashes
		let bucket_0 = 0;
		let tx_1_bucket_0 = EthHash::from_low_u64_be(1u64);
		let tx_2_bucket_0 = EthHash::from_low_u64_be(2u64);
		ProcessedTxBuckets::insert(bucket_0, tx_1_bucket_0, ());
		ProcessedTxBuckets::insert(bucket_0, tx_2_bucket_0, ());
		ProcessedTxHashes::insert(tx_1_bucket_0, ());
		ProcessedTxHashes::insert(tx_2_bucket_0, ());

		let bucket_1 = 1;
		let tx_1_bucket_1 = EthHash::from_low_u64_be(1_1u64);
		let tx_2_bucket_1 = EthHash::from_low_u64_be(1_2u64);
		ProcessedTxBuckets::insert(bucket_1, tx_1_bucket_1, ());
		ProcessedTxBuckets::insert(bucket_1, tx_2_bucket_1, ());
		ProcessedTxHashes::insert(tx_1_bucket_1, ());
		ProcessedTxHashes::insert(tx_2_bucket_1, ());

		let bucket_2 = 2;
		let tx_1_bucket_2 = EthHash::from_low_u64_be(2_1u64);
		ProcessedTxBuckets::insert(bucket_2, tx_1_bucket_2, ());
		ProcessedTxHashes::insert(tx_1_bucket_2, ());

		// configure tx expiry after 24 hours
		let event_deadline = 60 * 60 * 24;
		let _ = EthBridge::set_event_deadline(Origin::root(), event_deadline);
		// this will trigger 1st tx hash pruning
		let n_buckets = event_deadline / BUCKET_FACTOR_S;
		// blocks per event deadline / block time / n_buckets
		let blocks_per_bucket = event_deadline / 5 / n_buckets;

		// Test
		// prune bucket 0
		let expire_bucket_0_block = 0_u64;
		System::set_block_number(expire_bucket_0_block); // set block number updates the block timestamp provided by `MockUnixTime`
		let _ = EthBridge::on_initialize(expire_bucket_0_block);

		assert!(ProcessedTxBuckets::iter_prefix(bucket_0).count().is_zero());
		assert!(!ProcessedTxHashes::contains_key(tx_1_bucket_0));
		assert!(!ProcessedTxHashes::contains_key(tx_1_bucket_0));
		// other buckets untouched
		assert_eq!(ProcessedTxBuckets::iter_prefix(bucket_1).count(), 2_usize);
		assert!(ProcessedTxHashes::contains_key(tx_1_bucket_1));
		assert!(ProcessedTxHashes::contains_key(tx_2_bucket_1));
		assert_eq!(ProcessedTxBuckets::iter_prefix(bucket_2).count(), 1_usize);
		assert!(ProcessedTxHashes::contains_key(tx_1_bucket_2));

		// prune bucket 1
		let expire_bucket_1_block = CLAIM_PRUNING_INTERVAL as u64;
		System::set_block_number(expire_bucket_1_block);
		EthBridge::on_initialize(expire_bucket_1_block);

		assert!(ProcessedTxBuckets::iter_prefix(bucket_1).count().is_zero());
		assert!(!ProcessedTxHashes::contains_key(tx_1_bucket_1));
		assert!(!ProcessedTxHashes::contains_key(tx_2_bucket_1));
		// other buckets untouched
		assert_eq!(ProcessedTxBuckets::iter_prefix(bucket_2).count(), 1_usize);
		assert!(ProcessedTxHashes::contains_key(tx_1_bucket_2));

		// prune bucket 2
		let expire_bucket_2_block = blocks_per_bucket + CLAIM_PRUNING_INTERVAL as u64;
		System::set_block_number(expire_bucket_2_block);
		EthBridge::on_initialize(expire_bucket_2_block);

		assert!(ProcessedTxBuckets::iter_prefix(bucket_2).count().is_zero());
		assert!(!ProcessedTxHashes::contains_key(tx_1_bucket_2));

		// after `event_deadline` has elapsed, bucket index resets to 0
		let tx_1_bucket_0 = EthHash::from_low_u64_be(1u64);
		let tx_2_bucket_0 = EthHash::from_low_u64_be(2u64);
		ProcessedTxBuckets::insert(bucket_0, tx_1_bucket_0, ());
		ProcessedTxBuckets::insert(bucket_0, tx_2_bucket_0, ());
		ProcessedTxHashes::insert(tx_1_bucket_0, ());
		ProcessedTxHashes::insert(tx_2_bucket_0, ());

		// prune bucket 0 again
		let expire_bucket_0_again_block = blocks_per_bucket * n_buckets;
		System::set_block_number(expire_bucket_0_again_block);
		EthBridge::on_initialize(expire_bucket_0_again_block);
		assert!(ProcessedTxBuckets::iter_prefix(bucket_0).count().is_zero());
		assert!(!ProcessedTxHashes::contains_key(tx_1_bucket_0));
		assert!(!ProcessedTxHashes::contains_key(tx_2_bucket_0));
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
			if Module::<TestRuntime>::delayed_event_proofs(event_ids[i as usize]).is_none() {
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
			if Module::<TestRuntime>::delayed_event_proofs(event_ids[i as usize]).is_none() {
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
fn offchain_try_notarize_event() {
	ExtBuilder::default().build().execute_with(|| {
		// Mock block response and transaction receipt
		let block_number = 10;
		let block_hash: H256 = H256::from_low_u64_be(111);
		let timestamp: U256 = U256::from(<MockUnixTime as UnixTime>::now().as_secs().saturated_into::<u64>());
		let tx_hash: EthHash = H256::from_low_u64_be(222);
		let contract_address: EthAddress = H160::from_low_u64_be(333);

		// Create block info for both the transaction block and a later block
		let _mock_block_1 = mock_block_response(block_number, block_hash, timestamp);
		let _mock_block_2 = mock_block_response(block_number + 5, block_hash, timestamp);
		let _mock_tx_receipt = create_transaction_receipt_mock(block_number, block_hash, tx_hash, contract_address);

		let event_claim = EventClaim {
			tx_hash,
			data: vec![],
			contract_address,
			event_signature: Default::default(),
		};

		assert_eq!(
			Module::<TestRuntime>::offchain_try_notarize_event(event_claim),
			EventClaimResult::Valid
		);
	});
}

#[test]
fn offchain_try_notarize_event_no_tx_receipt_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let event_claim = EventClaim {
			tx_hash: H256::from_low_u64_be(222),
			data: vec![],
			contract_address: H160::from_low_u64_be(333),
			event_signature: Default::default(),
		};
		assert_eq!(
			Module::<TestRuntime>::offchain_try_notarize_event(event_claim),
			EventClaimResult::NoTxLogs
		);
	});
}

#[test]
fn offchain_try_notarize_event_no_status_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		// Mock transaction receipt
		let tx_hash: EthHash = H256::from_low_u64_be(222);
		let contract_address: EthAddress = H160::from_low_u64_be(333);
		let mock_tx_receipt = MockReceiptBuilder::new()
			.block_number(10)
			.block_hash(H256::from_low_u64_be(111))
			.transaction_hash(tx_hash)
			.contract_address(contract_address)
			.status(0)
			.build();

		// Create mock info for transaction receipt
		MockEthereumRpcClient::mock_transaction_receipt_for(tx_hash, mock_tx_receipt.clone());

		let event_claim = EventClaim {
			tx_hash,
			data: vec![],
			contract_address,
			event_signature: Default::default(),
		};

		assert_eq!(
			Module::<TestRuntime>::offchain_try_notarize_event(event_claim),
			EventClaimResult::TxStatusFailed
		);
	});
}

#[test]
fn offchain_try_notarize_event_unexpected_contract_address_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		// Mock transaction receipt
		let block_number = 10;
		let block_hash: H256 = H256::from_low_u64_be(111);
		let tx_hash: EthHash = H256::from_low_u64_be(222);
		let contract_address: EthAddress = H160::from_low_u64_be(333);

		// Create mock info for transaction receipt
		let _mock_tx_receipt = create_transaction_receipt_mock(block_number, block_hash, tx_hash, contract_address);

		// Create event claim with different contract address to tx_receipt
		let event_claim = EventClaim {
			tx_hash,
			data: vec![],
			contract_address: H160::from_low_u64_be(444),
			event_signature: Default::default(),
		};

		assert_eq!(
			Module::<TestRuntime>::offchain_try_notarize_event(event_claim),
			EventClaimResult::UnexpectedContractAddress
		);
	});
}

#[test]
fn offchain_try_notarize_event_no_block_number_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		// Mock transaction receipt
		let block_number = 10;
		let block_hash: H256 = H256::from_low_u64_be(111);
		let tx_hash: EthHash = H256::from_low_u64_be(222);
		let contract_address: EthAddress = H160::from_low_u64_be(333);

		// Create mock info for transaction receipt
		let _mock_tx_receipt = create_transaction_receipt_mock(block_number, block_hash, tx_hash, contract_address);

		let event_claim = EventClaim {
			tx_hash,
			data: vec![],
			contract_address,
			event_signature: Default::default(),
		};

		assert_eq!(
			Module::<TestRuntime>::offchain_try_notarize_event(event_claim),
			EventClaimResult::DataProviderErr
		);
	});
}

#[test]
fn offchain_try_notarize_event_no_confirmations_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		// Mock block response and transaction receipt
		let block_number = 10;
		let block_hash: H256 = H256::from_low_u64_be(111);
		let timestamp: U256 = U256::from(<MockUnixTime as UnixTime>::now().as_secs().saturated_into::<u64>());
		let tx_hash: EthHash = H256::from_low_u64_be(222);
		let contract_address: EthAddress = H160::from_low_u64_be(333);

		// Create block info for both the transaction block and a later block
		let _mock_block_1 = mock_block_response(block_number, block_hash, timestamp);
		let _mock_block_2 = mock_block_response(block_number, block_hash, timestamp);
		let _mock_tx_receipt = create_transaction_receipt_mock(block_number, block_hash, tx_hash, contract_address);

		let event_claim = EventClaim {
			tx_hash,
			data: vec![],
			contract_address,
			event_signature: Default::default(),
		};

		assert_eq!(
			Module::<TestRuntime>::offchain_try_notarize_event(event_claim),
			EventClaimResult::NotEnoughConfirmations
		);
	});
}

#[test]
fn offchain_try_notarize_event_expired_confirmation_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		// Mock block response and transaction receipt
		let block_number = 10;
		let block_hash: H256 = H256::from_low_u64_be(111);
		let timestamp: U256 = U256::from(0);
		let tx_hash: EthHash = H256::from_low_u64_be(222);
		let contract_address: EthAddress = H160::from_low_u64_be(333);

		// Create block info for both the transaction block and a later block
		let _mock_block_1 = mock_block_response(block_number, block_hash, timestamp);
		let _mock_block_2 = mock_block_response(block_number + 5, block_hash, timestamp);
		let _mock_tx_receipt = create_transaction_receipt_mock(block_number, block_hash, tx_hash, contract_address);

		let event_claim = EventClaim {
			tx_hash,
			data: vec![],
			contract_address,
			event_signature: Default::default(),
		};

		assert_eq!(
			Module::<TestRuntime>::offchain_try_notarize_event(event_claim),
			EventClaimResult::Expired
		);
	});
}

#[test]
fn offchain_try_notarize_event_no_observed_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		// Mock block response and transaction receipt
		let block_number = 10;
		let block_hash: H256 = H256::from_low_u64_be(111);
		let timestamp: U256 = U256::from(<MockUnixTime as UnixTime>::now().as_secs().saturated_into::<u64>());
		let tx_hash: EthHash = H256::from_low_u64_be(222);
		let contract_address: EthAddress = H160::from_low_u64_be(333);

		// Create block info for both the transaction block and a later block
		let _mock_block_1 = mock_block_response(block_number, block_hash, timestamp);
		let _mock_tx_receipt = create_transaction_receipt_mock(block_number + 1, block_hash, tx_hash, contract_address);

		let event_claim = EventClaim {
			tx_hash,
			data: vec![],
			contract_address,
			event_signature: Default::default(),
		};

		// Set event confirmations to 0 so it doesn't fail early
		let _ = Module::<TestRuntime>::set_event_confirmations(frame_system::RawOrigin::Root.into(), 0);
		assert_eq!(
			Module::<TestRuntime>::offchain_try_notarize_event(event_claim),
			EventClaimResult::DataProviderErr
		);
	});
}

#[test]
fn prunes_expired_events() {}

#[test]
fn double_claim_fails() {}

#[test]
fn invalid_notarization_fails() {}
