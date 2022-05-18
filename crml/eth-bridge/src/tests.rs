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
use crate::mock::*;
use crate::{
	types::{BridgeEthereumRpcApi, EventProofId},
	Error, Module,
};
use cennznet_primitives::eth::crypto::AuthorityId;
use crml_support::{EthAbiCodec, EventClaimVerifier, H160};
use frame_support::{
	assert_noop, assert_ok,
	dispatch::DispatchError,
	storage::StorageValue,
	traits::{OnInitialize, OneSessionHandler},
	weights::{constants::RocksDbWeight as DbWeight, Weight},
};
use sp_core::{
	offchain::{testing, OffchainDbExt, OffchainWorkerExt},
	Public, H256,
};
use sp_runtime::offchain::StorageKind;

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
