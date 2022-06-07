#![cfg(test)]

use super::*;
use crate::mock::{
	test_storage, AccountId, CallRequestBuilder, CallResponseBuilder, EthStateOracle, ExtBuilder, GenericAsset, System,
	TestRuntime,
};
use frame_support::{
	assert_err, assert_noop, assert_ok,
	traits::{OnIdle, OnInitialize, UnixTime},
};
use frame_system::RawOrigin;
use hex_literal::hex;
use sp_runtime::Permill;

fn state_oracle_ss58_address() -> AccountId {
	let state_oracle_precompile = <TestRuntime as Config>::StateOraclePrecompileAddress::get();
	<TestRuntime as Config>::AddressMapping::into_account_id(state_oracle_precompile)
}

fn deposit_bond_relayer(origin: u64) {
	let initial_balance = 100_000_000_000_000 as Balance;
	assert_ok!(GenericAsset::deposit_into_existing(
		&origin,
		GenericAsset::fee_currency(),
		initial_balance
	));
	assert_ok!(EthStateOracle::deposit_relayer_bond(RawOrigin::Signed(origin).into()));
}

fn deposit_bond_challenger(origin: u64) {
	let initial_balance = 100_000_000_000_000 as Balance;
	assert_ok!(GenericAsset::deposit_into_existing(
		&origin,
		GenericAsset::fee_currency(),
		initial_balance
	));
	assert_ok!(EthStateOracle::deposit_challenger_bond(
		RawOrigin::Signed(origin).into()
	));
}

#[test]
fn new_request() {
	ExtBuilder::default().build().execute_with(|| {
		let caller = H160::from_low_u64_be(123_u64);
		let destination = H160::from_low_u64_be(456_u64);
		let input_data = vec![0u8, 55, 66, 77, 88];
		let callback_signature = [0u8, 35, 45, 55];
		let callback_gas_limit = 200_000_u64;
		let fee_preferences = FeePreferences {
			asset_id: 12,
			slippage: Permill::from_percent(5),
		};
		let bounty = 10_000;
		let request_id = NextRequestId::get();
		// Submit challenger bond
		deposit_bond_challenger(1_u64);
		// Test
		assert_ok!(EthStateOracle::new_request(
			&caller,
			&destination,
			input_data.as_ref(),
			&callback_signature,
			callback_gas_limit,
			Some(fee_preferences.clone()),
			bounty,
		));

		let expected_request_info = CallRequest {
			caller,
			destination,
			callback_signature,
			callback_gas_limit,
			fee_preferences: Some(fee_preferences),
			bounty,
			timestamp: <TestRuntime as Config>::UnixTime::now().as_secs(),
			input_data,
			expiry_block: (System::block_number() + <TestRuntime as Config>::ChallengePeriod::get()) as u32,
		};
		assert_eq!(Requests::get(request_id), Some(expected_request_info));
		assert_eq!(NextRequestId::get(), request_id + U256::from(1_u32));
	});
}

#[test]
fn new_request_no_challenger_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let caller = H160::from_low_u64_be(123_u64);
		let destination = H160::from_low_u64_be(456_u64);
		let input_data = vec![0u8, 55, 66, 77, 88];
		let callback_signature = [0u8, 35, 45, 55];
		let callback_gas_limit = 200_000_u64;
		let fee_preferences = FeePreferences {
			asset_id: 12,
			slippage: Permill::from_percent(5),
		};
		let bounty = 10_000;
		// Test
		assert_err!(
			EthStateOracle::new_request(
				&caller,
				&destination,
				input_data.as_ref(),
				&callback_signature,
				callback_gas_limit,
				Some(fee_preferences.clone()),
				bounty,
			),
			Error::<TestRuntime>::NoAvailableResponses
		);
	});
}

#[test]
fn try_callback() {
	ExtBuilder::default().build().execute_with(|| {
		let caller = 111_u64;
		let bounty = 88 as Balance;
		let request_id = RequestId::from(123_u32);

		let request = CallRequestBuilder::new()
			.caller(caller)
			.destination(2_u64)
			.bounty(bounty)
			.callback_gas_limit(200_000_u64)
			// selector for 'testCallback'
			.callback_signature(hex!("0c43949d"))
			.build();

		let relayer = 3_u64;
		let response = CallResponseBuilder::new()
			.eth_block_timestamp(5_u64)
			.relayer(relayer)
			.build();

		// fund the caller
		let initial_caller_balance = 100_000_000_000_000 as Balance;
		assert!(
			GenericAsset::deposit_into_existing(&caller, GenericAsset::fee_currency(), initial_caller_balance).is_ok()
		);

		// Test
		assert!(
			EthStateOracle::try_callback(request_id, &request, &response).is_ok()
		);

		// bounty to relayer
		assert_eq!(
			GenericAsset::free_balance(GenericAsset::fee_currency(), &relayer),
			bounty,
		);

		// callback gas fees paid to state oracle address
		let max_fee_per_gas = U256::from(<TestRuntime as Config>::MinGasPrice::get());
		let max_priority_fee_per_gas = U256::one();
		let total_fee: Balance =
			scale_wei_to_4dp((max_fee_per_gas * request.callback_gas_limit + max_priority_fee_per_gas).saturated_into());
		// test is only valid if `total_fee` is non-zero
		assert!(total_fee > Zero::zero());

		assert_eq!(
			GenericAsset::free_balance(GenericAsset::fee_currency(), &state_oracle_ss58_address()),
			total_fee,
		);

		// contract executor receives correct values
		assert_eq!(
			test_storage::CurrentExecutionParameters::get().expect("parameters are set"),
			(
				<TestRuntime as Config>::StateOraclePrecompileAddress::get(),
				request.caller,
				// input is an abi encoded call `testCallback(123, 5, 0x0101010101010101010101010101010101010101010101010101010101010101)`
				// signature: `testCallback(uint256, uint256, bytes32)`
				hex!("0c43949d000000000000000000000000000000000000000000000000000000000000007b00000000000000000000000000000000000000000000000000000000000000050101010101010101010101010101010101010101010101010101010101010101").to_vec(),
				request.callback_gas_limit,
				max_fee_per_gas,
				max_priority_fee_per_gas,
				None,
			)
		);

		// total payment by caller
		assert_eq!(
			GenericAsset::free_balance(GenericAsset::fee_currency(), &caller),
			initial_caller_balance - bounty - total_fee,
		)
	});
}

#[test]
fn try_callback_with_fee_preferences() {
	ExtBuilder::default().build().execute_with(|| {
		let caller = 111_u64;
		let bounty = 88 as Balance;
		let request_id = RequestId::from(123_u32);
		let payment_asset_id: AssetId = 1;
		let fee_preferences = FeePreferences { asset_id: payment_asset_id, slippage: Default::default() };
		let request = CallRequestBuilder::new()
			.caller(caller)
			.destination(2_u64)
			.fee_preferences(Some(fee_preferences))
			.bounty(bounty)
			.callback_gas_limit(200_000_u64)
			// selector for 'testCallback'
			.callback_signature(hex!("0c43949d"))
			.build();
		let relayer = 3_u64;
		let response = CallResponseBuilder::new()
			.eth_block_timestamp(5_u64)
			.relayer(relayer)
			.build();
		// fund the caller
		let initial_caller_balance = 100_000_000_000_000 as Balance;
		// Asset used for payment
		assert!(
			GenericAsset::deposit_into_existing(&caller, payment_asset_id, initial_caller_balance).is_ok()
		);

		// Test
		assert!(
			EthStateOracle::try_callback(request_id, &request, &response).is_ok()
		);

		// bounty to relayer
		assert_eq!(
			GenericAsset::free_balance(GenericAsset::fee_currency(), &relayer),
			bounty,
		);

		// callback gas fees paid to state oracle address
		let max_fee_per_gas = U256::from(<TestRuntime as Config>::MinGasPrice::get());
		let max_priority_fee_per_gas = U256::one();
		let total_fee: Balance =
			scale_wei_to_4dp((max_fee_per_gas * request.callback_gas_limit + max_priority_fee_per_gas).saturated_into());
		// test is only valid if `total_fee` is non-zero
		assert!(total_fee > Zero::zero());

		assert_eq!(
			GenericAsset::free_balance(GenericAsset::fee_currency(), &state_oracle_ss58_address()),
			total_fee,
		);

		// contract executor receives correct values
		assert_eq!(
			test_storage::CurrentExecutionParameters::get().expect("parameters are set"),
			(
				<TestRuntime as Config>::StateOraclePrecompileAddress::get(),
				request.caller,
				// input is an abi encoded call `testCallback(123, 5, 0x0101010101010101010101010101010101010101010101010101010101010101)`
				// signature: `testCallback(uint256, uint256, bytes32)`
				hex!("0c43949d000000000000000000000000000000000000000000000000000000000000007b00000000000000000000000000000000000000000000000000000000000000050101010101010101010101010101010101010101010101010101010101010101").to_vec(),
				request.callback_gas_limit,
				max_fee_per_gas,
				max_priority_fee_per_gas,
				None,
			)
		);

		// caller fee asset balance should remain 0
		assert_eq!(
			GenericAsset::free_balance(GenericAsset::fee_currency(), &caller),
			0,
		);
		// caller fee asset should be reduced
		assert_eq!(
			GenericAsset::free_balance(payment_asset_id, &caller),
			initial_caller_balance - bounty - total_fee,
		);
	});
}

#[test]
fn try_callback_cannot_pay_bounty() {
	ExtBuilder::default().build().execute_with(|| {
		let request = CallRequestBuilder::new().caller(1_u64).bounty(88 as Balance).build();
		let response = CallResponseBuilder::new().relayer(555 as AccountId).build();

		assert_noop!(
			EthStateOracle::try_callback(RequestId::from(1_u64), &request, &response),
			Error::<TestRuntime>::InsufficientFundsBounty,
		);
	});
}

#[test]
fn try_callback_cannot_pay_gas() {
	ExtBuilder::default().build().execute_with(|| {
		let caller = 1_u64;
		let bounty = 1_234 as Balance;
		let request = CallRequestBuilder::new()
			.caller(caller)
			.bounty(bounty)
			.callback_gas_limit(100_000_u64 * 100_000_000_000_000_u64)
			.build();
		let relayer = 555 as AccountId;
		let response = CallResponseBuilder::new().relayer(relayer).build();
		// fund the caller for bounty payment only
		assert!(GenericAsset::deposit_into_existing(&caller, GenericAsset::fee_currency(), bounty).is_ok());
		assert!(<TestRuntime as Config>::MinGasPrice::get() > 0);

		// Test
		assert_err!(
			EthStateOracle::try_callback(RequestId::from(1_u64), &request, &response),
			Error::<TestRuntime>::InsufficientFundsGas,
		);
		// Bounty retained
		assert_eq!(
			GenericAsset::free_balance(GenericAsset::fee_currency(), &relayer),
			bounty,
		);
	});
}

#[test]
fn submit_call_response() {
	ExtBuilder::default().build().execute_with(|| {
		// setup request
		let relayer = 1_u64;
		let origin = RawOrigin::Signed(relayer);
		let request_id = RequestId::from(1_u64);
		let eth_block_number = 100_u64;
		let eth_block_timestamp = <TestRuntime as Config>::UnixTime::now().as_secs();
		let expiry_block = 5_u64;
		let return_data = ReturnDataClaim::Ok([1_u8; 32]);
		Requests::insert(request_id, CallRequestBuilder::new().expiry_block(expiry_block).build());
		RequestsExpiredAtBlock::<TestRuntime>::insert(expiry_block, vec![request_id]);
		deposit_bond_relayer(relayer);

		// Test
		assert!(EthStateOracle::submit_call_response(
			origin.into(),
			request_id,
			return_data.clone(),
			eth_block_number,
			eth_block_timestamp,
		)
		.is_ok());

		assert_eq!(
			EthStateOracle::responses(request_id).unwrap(),
			CallResponse {
				return_data,
				eth_block_number,
				eth_block_timestamp,
				relayer,
			}
		);
		// request is no longer marked for expiry
		assert!(!RequestsExpiredAtBlock::<TestRuntime>::get(expiry_block).contains(&request_id));

		// Scheduled as valid after `ChallengePeriod` blocks (i.e. the optimistic timeframe)
		let execute_block = System::block_number() + <TestRuntime as Config>::ChallengePeriod::get();
		let valid_at = <ResponsesValidAtBlock<TestRuntime>>::get(execute_block);
		assert!(valid_at.contains(&request_id));
	});
}

#[test]
fn submit_call_response_request_should_exist() {
	ExtBuilder::default().build().execute_with(|| {
		let relayer: u64 = 1;
		deposit_bond_relayer(relayer);

		assert_noop!(
			EthStateOracle::submit_call_response(
				RawOrigin::Signed(relayer).into(),
				RequestId::from(1_u64),
				ReturnDataClaim::Ok([0_u8; 32]),
				100_u64,
				<TestRuntime as Config>::UnixTime::now().as_secs(),
			),
			Error::<TestRuntime>::NoRequest,
		);
	});
}

#[test]
fn submit_call_response_accepts_first() {
	ExtBuilder::default().build().execute_with(|| {
		let relayer: u64 = 1;
		deposit_bond_relayer(relayer);

		// setup request
		let request_id = RequestId::from(1_u64);
		let request = CallRequestBuilder::new().build();
		Requests::insert(request_id, request);
		// first submission ok
		assert!(EthStateOracle::submit_call_response(
			RawOrigin::Signed(relayer).into(),
			request_id,
			ReturnDataClaim::Ok([1_u8; 32]),
			100_u64,
			<TestRuntime as Config>::UnixTime::now().as_secs(),
		)
		.is_ok());

		// Test
		// only one response can be submitted
		assert_noop!(
			EthStateOracle::submit_call_response(
				RawOrigin::Signed(1_u64).into(),
				request_id,
				ReturnDataClaim::Ok([1_u8; 32]),
				100_u64,
				<TestRuntime as Config>::UnixTime::now().as_secs(),
			),
			Error::<TestRuntime>::ResponseExists,
		);
	});
}

#[test]
fn submit_call_response_invalid_timestamps() {
	ExtBuilder::default().build().execute_with(|| {
		let relayer: u64 = 1;
		deposit_bond_relayer(relayer);

		// setup request
		let request_id = RequestId::from(1_u64);
		let request = CallRequestBuilder::new().build();
		Requests::insert(request_id, request);
		// ensures timestamp is non_zero
		System::set_block_number(25);

		// Test
		assert_err!(
			EthStateOracle::submit_call_response(
				RawOrigin::Signed(relayer).into(),
				request_id,
				ReturnDataClaim::Ok([1_u8; 32]),
				100_u64,
				<TestRuntime as Config>::UnixTime::now().as_secs() - (2 * 15) - 1, // block timestamp stale
			),
			Error::<TestRuntime>::InvalidResponseTimestamp,
		);
		assert_err!(
			EthStateOracle::submit_call_response(
				RawOrigin::Signed(1_u64).into(),
				request_id,
				ReturnDataClaim::Ok([1_u8; 32]),
				100_u64,
				<TestRuntime as Config>::UnixTime::now().as_secs() + (2 * 15) + 1, // block timestamp future
			),
			Error::<TestRuntime>::InvalidResponseTimestamp,
		);
	});
}

#[test]
fn response_progresses_to_callback() {
	ExtBuilder::default().build().execute_with(|| {
		// on initialize moves callbacks at their scheduled block into a ready state for handling by on_idle
		let ready_block = 1_u64;
		let responses = vec![1_u64, 2, 3].iter().map(|x| (*x).into()).collect::<Vec<U256>>();
		for r in responses.iter() {
			<ResponsesValidAtBlock<TestRuntime>>::append(ready_block, *r);
		}

		// Test
		let consumed_weight = EthStateOracle::on_initialize(1);

		assert!(!<ResponsesValidAtBlock<TestRuntime>>::contains_key(ready_block));
		assert_eq!(ResponsesForCallback::get(), responses);
		// atleast consumes the weight to remove and rewrite the values
		assert!(consumed_weight > DbWeight::get().writes(responses.len() as u64));
	});
}

#[test]
fn expired_requests_removed() {
	ExtBuilder::default().build().execute_with(|| {
		// on initialize moves callbacks at their scheduled block into a ready state for handling by on_idle
		let ready_block = 1_u64;
		let responses = vec![1_u64, 2, 3].iter().map(|x| (*x).into()).collect::<Vec<U256>>();
		for r in responses.iter() {
			Requests::insert(*r, CallRequestBuilder::new().build());
			<RequestsExpiredAtBlock<TestRuntime>>::append(ready_block, *r);
		}

		// Test
		let consumed_weight = EthStateOracle::on_initialize(1);

		assert!(!<RequestsExpiredAtBlock<TestRuntime>>::contains_key(ready_block));
		for r in responses.iter() {
			assert!(!Requests::contains_key(*r));
		}
		// atleast consumes the weight to remove and rewrite the values
		assert!(consumed_weight > DbWeight::get().writes(responses.len() as u64));
	});
}

#[test]
fn on_idle() {
	ExtBuilder::default().build().execute_with(|| {
		// Check
		// - response/request data removed from storage
		// - only process as many as weight permits
		// - the remaining callbacks are left for next on_idle block

		// Setup 4 requests and responses in storage
		let ready_callbacks = vec![
			ReturnDataClaim::Ok([1_u8; 32]),
			ReturnDataClaim::Ok([2_u8; 32]),
			ReturnDataClaim::ExceedsLengthLimit,
			ReturnDataClaim::Ok([3_u8; 32]),
		]
		.into_iter()
		.enumerate()
		.map(|(i, x)| (RequestId::from(i + 1), x))
		.collect::<Vec<(RequestId, ReturnDataClaim)>>();

		for (i, r) in ready_callbacks.iter() {
			<Responses<TestRuntime>>::insert(
				*i,
				CallResponseBuilder::new()
					.relayer(i.as_u64())
					.return_data(r.clone())
					.build(),
			);
			Requests::insert(*i, CallRequestBuilder::new().caller(i.as_u64()).build());
		}
		ResponsesForCallback::put(ready_callbacks.iter().map(|x| x.0).collect::<Vec<RequestId>>());

		// enough for 3 callbacks without considering overhead cost
		// should mean we only process 2 requests
		let per_callback_weight = EthStateOracle::per_callback_weight();
		let remaining_block_weight = 3 * per_callback_weight;

		// Test
		let consumed_weight = EthStateOracle::on_idle(1_u64, remaining_block_weight);

		// Storage cleared for fist 2 callbacks
		assert!(consumed_weight < remaining_block_weight);
		for (r, _) in &ready_callbacks[..2] {
			assert!(!<Responses<TestRuntime>>::contains_key(*r));
			assert!(!Requests::contains_key(*r));
		}

		// 3rd callback left for next time
		assert!(Requests::contains_key(RequestId::from(3_u64)));
		assert_eq!(ResponsesForCallback::get(), vec![3_u64.into(), 4_u64.into()]);

		// Clean up 3rd callback
		let consumed_weight = EthStateOracle::on_idle(2_u64, 2 * per_callback_weight);
		assert!(consumed_weight < remaining_block_weight);

		assert!(!<Responses<TestRuntime>>::contains_key(RequestId::from(3_u64)));
		assert!(!Requests::contains_key(RequestId::from(3_u64)));

		// 4th callback left for next time
		assert!(Requests::contains_key(RequestId::from(4_u64)));
		assert_eq!(ResponsesForCallback::get(), vec![4_u64.into()]);
	});
}

#[test]
fn deposit_relayer_bond() {
	ExtBuilder::default().build().execute_with(|| {
		let relayer = 1_u64;
		let origin = RawOrigin::Signed(relayer);
		let initial_balance = 100_000_000_000_000 as Balance;
		assert_ok!(GenericAsset::deposit_into_existing(
			&relayer,
			GenericAsset::fee_currency(),
			initial_balance
		));
		assert_ok!(EthStateOracle::deposit_relayer_bond(origin.into()));
		assert_eq!(
			<RelayerBonds<TestRuntime>>::get(relayer),
			<TestRuntime as Config>::RelayerBondAmount::get()
		);

		assert_eq!(
			GenericAsset::free_balance(GenericAsset::fee_currency(), &relayer),
			initial_balance - <TestRuntime as Config>::RelayerBondAmount::get(),
		);
	});
}

#[test]
fn deposit_relayer_bond_max_relayers_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let relayer = 1_u64;
		let origin = RawOrigin::Signed(relayer);
		let initial_balance = 100_000_000_000_000 as Balance;
		assert_ok!(GenericAsset::deposit_into_existing(
			&relayer,
			GenericAsset::fee_currency(),
			initial_balance
		));
		assert_ok!(EthStateOracle::deposit_relayer_bond(origin.into()));
		assert_noop!(
			EthStateOracle::deposit_relayer_bond(RawOrigin::Signed(2_u64).into()),
			Error::<TestRuntime>::MaxRelayersReached
		);
	});
}

#[test]
fn deposit_relayer_bond_already_bonded_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let relayer = 1_u64;
		let origin = RawOrigin::Signed(relayer);
		let initial_balance = 100_000_000_000_000 as Balance;
		assert_ok!(GenericAsset::deposit_into_existing(
			&relayer,
			GenericAsset::fee_currency(),
			initial_balance
		));
		assert_ok!(EthStateOracle::deposit_relayer_bond(origin.clone().into()));
		// Bond again should fail but not remove bond
		assert_noop!(
			EthStateOracle::deposit_relayer_bond(origin.into()),
			Error::<TestRuntime>::AlreadyBonded
		);

		assert_eq!(
			<RelayerBonds<TestRuntime>>::get(relayer),
			<TestRuntime as Config>::RelayerBondAmount::get()
		);
		assert_eq!(
			GenericAsset::free_balance(GenericAsset::fee_currency(), &relayer),
			initial_balance - <TestRuntime as Config>::RelayerBondAmount::get(),
		);
	});
}

#[test]
fn deposit_relayer_bond_not_enough_balance_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let relayer = 1_u64;
		let origin = RawOrigin::Signed(relayer);
		assert_noop!(
			EthStateOracle::deposit_relayer_bond(origin.into()),
			crml_generic_asset::Error::<TestRuntime>::InsufficientBalance
		);
		assert_eq!(<RelayerBonds<TestRuntime>>::get(relayer), 0);
	});
}

#[test]
fn unbond_relayer_bond() {
	ExtBuilder::default().build().execute_with(|| {
		let relayer = 1_u64;
		let origin = RawOrigin::Signed(relayer);
		let initial_balance = 100_000_000_000_000 as Balance;
		assert_ok!(GenericAsset::deposit_into_existing(
			&relayer,
			GenericAsset::fee_currency(),
			initial_balance
		));
		// Bond
		assert_ok!(EthStateOracle::deposit_relayer_bond(origin.clone().into()));
		assert_eq!(
			<RelayerBonds<TestRuntime>>::get(relayer),
			<TestRuntime as Config>::RelayerBondAmount::get()
		);

		// Unbond
		assert_ok!(EthStateOracle::unbond_relayer_bond(origin.into()));
		assert_eq!(<RelayerBonds<TestRuntime>>::get(relayer), 0);

		assert_eq!(
			GenericAsset::free_balance(GenericAsset::fee_currency(), &relayer),
			initial_balance,
		);
	});
}

#[test]
fn unbond_relayer_bond_active_response_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let relayer = 1_u64;
		let origin = RawOrigin::Signed(relayer);
		let initial_balance = 100_000_000_000_000 as Balance;
		assert_ok!(GenericAsset::deposit_into_existing(
			&relayer,
			GenericAsset::fee_currency(),
			initial_balance
		));
		// Bond
		assert_ok!(EthStateOracle::deposit_relayer_bond(origin.clone().into()));
		assert_eq!(
			<RelayerBonds<TestRuntime>>::get(relayer),
			<TestRuntime as Config>::RelayerBondAmount::get()
		);

		// Insert Response for account
		let call_response = CallResponseBuilder::new().relayer(relayer).build();
		let request_id: RequestId = RequestId::from(1);
		Responses::<TestRuntime>::insert(request_id, call_response);

		// Try unbond
		assert_noop!(
			EthStateOracle::unbond_relayer_bond(origin.into()),
			Error::<TestRuntime>::CantUnbond
		);

		// Make sure bond hasn't been removed
		assert_eq!(
			<RelayerBonds<TestRuntime>>::get(relayer),
			<TestRuntime as Config>::RelayerBondAmount::get()
		);
		assert_eq!(
			GenericAsset::free_balance(GenericAsset::fee_currency(), &relayer),
			initial_balance - <TestRuntime as Config>::RelayerBondAmount::get(),
		);
	});
}

#[test]
fn unbond_relayer_bond_no_bond_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let relayer = 1_u64;
		let origin = RawOrigin::Signed(relayer);

		// Unbond
		assert_noop!(
			EthStateOracle::unbond_relayer_bond(origin.into()),
			Error::<TestRuntime>::NotEnoughBonded
		);
	});
}

#[test]
fn deposit_challenger_bond() {
	ExtBuilder::default().build().execute_with(|| {
		let challenger = 1_u64;
		let origin = RawOrigin::Signed(challenger);
		let initial_balance = 100_000_000_000_000 as Balance;
		assert_ok!(GenericAsset::deposit_into_existing(
			&challenger,
			GenericAsset::fee_currency(),
			initial_balance
		));
		assert_ok!(EthStateOracle::deposit_challenger_bond(origin.into()));
		let challenger_bond_amount = <TestRuntime as Config>::ChallengerBondAmount::get();
		let total_challenger_bond: Balance = challenger_bond_amount.saturating_mul(MaxRelayerResponses::get().into());

		assert_eq!(<ChallengerBonds<TestRuntime>>::get(challenger), total_challenger_bond);

		assert_eq!(
			GenericAsset::free_balance(GenericAsset::fee_currency(), &challenger),
			initial_balance - total_challenger_bond,
		);
	});
}

#[test]
fn deposit_challenger_bond_already_bonded_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let challenger = 1_u64;
		let origin = RawOrigin::Signed(challenger);
		let initial_balance = 100_000_000_000_000 as Balance;
		assert_ok!(GenericAsset::deposit_into_existing(
			&challenger,
			GenericAsset::fee_currency(),
			initial_balance
		));
		// First bond should work
		assert_ok!(EthStateOracle::deposit_challenger_bond(origin.clone().into()));

		// Deposit again should fail but not remove bond
		assert_noop!(
			EthStateOracle::deposit_challenger_bond(origin.into()),
			Error::<TestRuntime>::AlreadyBonded
		);

		let challenger_bond_amount = <TestRuntime as Config>::ChallengerBondAmount::get();
		let total_challenger_bond: Balance = challenger_bond_amount.saturating_mul(MaxRelayerResponses::get().into());

		assert_eq!(<ChallengerBonds<TestRuntime>>::get(challenger), total_challenger_bond);
		assert_eq!(
			GenericAsset::free_balance(GenericAsset::fee_currency(), &challenger),
			initial_balance - total_challenger_bond,
		);
	});
}

#[test]
fn deposit_challenger_bond_not_enough_balance_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let challenger = 1_u64;
		let origin = RawOrigin::Signed(challenger);
		assert_noop!(
			EthStateOracle::deposit_challenger_bond(origin.into()),
			crml_generic_asset::Error::<TestRuntime>::InsufficientBalance
		);
		assert_eq!(<RelayerBonds<TestRuntime>>::get(challenger), 0);
	});
}

#[test]
fn unbond_challenger_bond() {
	ExtBuilder::default().build().execute_with(|| {
		let challenger = 1_u64;
		let origin = RawOrigin::Signed(challenger);
		let initial_balance = 100_000_000_000_000 as Balance;
		assert_ok!(GenericAsset::deposit_into_existing(
			&challenger,
			GenericAsset::fee_currency(),
			initial_balance
		));
		assert_ok!(EthStateOracle::deposit_challenger_bond(origin.clone().into()));
		let challenger_bond_amount = <TestRuntime as Config>::ChallengerBondAmount::get();
		let total_challenger_bond: Balance = challenger_bond_amount.saturating_mul(MaxRelayerResponses::get().into());

		assert_eq!(<ChallengerBonds<TestRuntime>>::get(challenger), total_challenger_bond);
		assert_eq!(
			GenericAsset::free_balance(GenericAsset::fee_currency(), &challenger),
			initial_balance - total_challenger_bond,
		);

		// Unbond
		assert_ok!(EthStateOracle::unbond_challenger_bond(origin.into()));
		// Check bond has been removed and funds returned
		assert_eq!(<ChallengerBonds<TestRuntime>>::get(challenger), 0);
		assert_eq!(
			GenericAsset::free_balance(GenericAsset::fee_currency(), &challenger),
			initial_balance,
		);
	});
}

#[test]
fn unbond_challenger_bond_active_request_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let challenger = 1_u64;
		let origin = RawOrigin::Signed(challenger);
		let initial_balance = 100_000_000_000_000 as Balance;
		assert_ok!(GenericAsset::deposit_into_existing(
			&challenger,
			GenericAsset::fee_currency(),
			initial_balance
		));
		assert_ok!(EthStateOracle::deposit_challenger_bond(origin.clone().into()));
		let challenger_bond_amount = <TestRuntime as Config>::ChallengerBondAmount::get();
		let total_challenger_bond: Balance = challenger_bond_amount.saturating_mul(MaxRelayerResponses::get().into());

		// Submit a request
		let request = CallRequestBuilder::new().build();
		let request_id: RequestId = RequestId::from(1);
		Requests::insert(request_id, request);

		// Try Unbond but fail as there is an active request
		assert_noop!(
			EthStateOracle::unbond_challenger_bond(origin.into()),
			Error::<TestRuntime>::CantUnbondChallenger
		);

		// Check bond has not been removed
		assert_eq!(<ChallengerBonds<TestRuntime>>::get(challenger), total_challenger_bond);
		assert_eq!(
			GenericAsset::free_balance(GenericAsset::fee_currency(), &challenger),
			initial_balance - total_challenger_bond,
		);
	});
}

#[test]
fn unbond_challenger_bond_no_bond_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let challenger = 1_u64;
		let origin = RawOrigin::Signed(challenger);

		// Unbond
		assert_noop!(
			EthStateOracle::unbond_challenger_bond(origin.into()),
			Error::<TestRuntime>::NotEnoughBonded
		);
	});
}
