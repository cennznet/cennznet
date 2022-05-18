use super::*;
use crate::mock::{
	test_storage, AccountId, CallRequestBuilder, EthStateOracle, ExtBuilder, GenericAsset, System, TestRuntime,
};
use frame_support::{
	assert_err, assert_noop,
	traits::{OnIdle, OnInitialize, UnixTime},
};
use frame_system::RawOrigin;
use hex_literal::hex;
use sp_runtime::Permill;

fn state_oracle_ss58_address() -> AccountId {
	let state_oracle_precompile = <TestRuntime as Config>::StateOraclePrecompileAddress::get();
	<TestRuntime as Config>::AddressMapping::into_account_id(state_oracle_precompile)
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

		// Test
		EthStateOracle::new_request(
			&caller,
			&destination,
			input_data.as_ref(),
			&callback_signature,
			callback_gas_limit,
			Some(fee_preferences.clone()),
			bounty,
		);

		let request_info = CallRequest {
			caller,
			destination,
			callback_signature,
			callback_gas_limit,
			fee_preferences: Some(fee_preferences),
			bounty,
			timestamp: <TestRuntime as Config>::UnixTime::now().as_secs(),
		};
		assert_eq!(Requests::get(request_id), Some(request_info),);
		assert_eq!(RequestInputData::get(request_id), input_data.to_vec());
		assert_eq!(NextRequestId::get(), request_id + U256::from(1_u32));
	});
}

#[test]
fn try_callback() {
	ExtBuilder::default().build().execute_with(|| {
		let caller = 111_u64;
		let relayer = 3_u64;
		let bounty = 88 as Balance;
		let request_id = RequestId::from(123_u32);
		let return_data = return_data_to_bytes32(vec![0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99]);
		let request = CallRequestBuilder::new()
			.caller(caller)
			.destination(2_u64)
			.bounty(bounty)
			.callback_gas_limit(200_000_u64)
			// selector for 'testCallback'
			.callback_signature(hex!("0c43949d"))
			.build();
		// fund the caller
		let initial_caller_balance = 100_000_000_000_000 as Balance;
		assert!(
			GenericAsset::deposit_into_existing(&caller, GenericAsset::fee_currency(), initial_caller_balance).is_ok()
		);

		// Test
		assert!(
			EthStateOracle::try_callback(request_id, &request, &relayer, &return_data).is_ok()
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
			scale_to_4dp((max_fee_per_gas * request.callback_gas_limit + max_priority_fee_per_gas).saturated_into());
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
				// input is an abi encoded call `testCallback(123, 0x0011223344556677889900000000000000000000000000000000000000000000)`
				// signature: `testCallback(uint256, bytes32)`
				hex!("0c43949d000000000000000000000000000000000000000000000000000000000000007b0011223344556677889900000000000000000000000000000000000000000000").to_vec(),
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
fn try_callback_cannot_pay_bounty() {
	ExtBuilder::default().build().execute_with(|| {
		let request = CallRequestBuilder::new().caller(1_u64).bounty(88 as Balance).build();
		let relayer = 555 as AccountId;

		assert_noop!(
			EthStateOracle::try_callback(RequestId::from(1_u64), &request, &relayer, &<[u8; 32]>::default()),
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
		// fund the caller for bounty payment only
		assert!(GenericAsset::deposit_into_existing(&caller, GenericAsset::fee_currency(), bounty).is_ok());
		assert!(<TestRuntime as Config>::MinGasPrice::get() > 0);

		// Test
		assert_err!(
			EthStateOracle::try_callback(RequestId::from(1_u64), &request, &relayer, &<[u8; 32]>::default()),
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
		let return_data = vec![0_u8, 1, 2, 3, 4, 5, 5, 6, 7, 8, 9];
		let request = CallRequestBuilder::new().build();
		Requests::insert(request_id, request);

		// Test
		assert!(EthStateOracle::submit_call_response(origin.into(), request_id, return_data.clone(), 100_u64).is_ok());

		let return_data_padded = return_data_to_bytes32(return_data);
		assert_eq!(
			EthStateOracle::responses(request_id).unwrap(),
			CallResponse {
				return_data: return_data_padded,
				eth_block_number,
				reporter: relayer,
			}
		);

		// Scheduled as valid after `ChallengePeriod` blocks (i.e. the optimistic timeframe)
		let execute_block = System::block_number() + <TestRuntime as Config>::ChallengePeriod::get();
		let valid_at = <ResponsesValidAtBlock<TestRuntime>>::get(execute_block);
		assert!(valid_at.contains(&request_id));
	});
}

#[test]
fn submit_call_response_request_should_exist() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			EthStateOracle::submit_call_response(
				RawOrigin::Signed(1_u64).into(),
				RequestId::from(1_u64),
				Default::default(),
				100_u64
			),
			Error::<TestRuntime>::NoRequest,
		);
	});
}

#[test]
fn submit_call_response_accepts_first() {
	ExtBuilder::default().build().execute_with(|| {
		// setup request
		let request_id = RequestId::from(1_u64);
		let request = CallRequestBuilder::new().build();
		Requests::insert(request_id, request);
		// first submission ok
		assert!(EthStateOracle::submit_call_response(
			RawOrigin::Signed(1_u64).into(),
			request_id,
			Default::default(),
			100_u64
		)
		.is_ok());

		// Test
		// only one response can be submitted
		assert_noop!(
			EthStateOracle::submit_call_response(
				RawOrigin::Signed(1_u64).into(),
				request_id,
				Default::default(),
				100_u64
			),
			Error::<TestRuntime>::ResponseExists,
		);
	});
}

#[test]
fn submit_call_response_truncates_to_32() {
	ExtBuilder::default().build().execute_with(|| {
		// setup request
		let request_id = RequestId::from(1_u64);
		Requests::insert(request_id, CallRequestBuilder::new().build());
		let return_data = vec![1_u8; 35]; // 32 is max length for returndata

		// Test
		assert!(EthStateOracle::submit_call_response(
			RawOrigin::Signed(1_u64).into(),
			request_id,
			return_data.clone(),
			100_u64
		)
		.is_ok());

		// truncated
		assert_eq!(
			&EthStateOracle::responses(request_id).unwrap().return_data[..],
			&return_data[..32],
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
fn on_idle() {
	ExtBuilder::default().build().execute_with(|| {
		// Check
		// - response/request data removed from storage
		// - only process as many as weight permits
		// - the remaining callbacks are left for next on_idle block

		// Setup 3 requests and responses in storage
		let ready_callbacks = vec![1_u64, 2, 3].iter().map(|x| (*x).into()).collect::<Vec<U256>>();
		for (idx, r) in ready_callbacks.iter().enumerate() {
			<Responses<TestRuntime>>::insert(
				*r,
				CallResponse {
					return_data: [idx as u8; 32],
					eth_block_number: r.as_u64(),
					reporter: r.as_u64(),
				},
			);
			Requests::insert(*r, CallRequestBuilder::new().caller(r.as_u64()).build());
			RequestInputData::insert(*r, vec![idx as u8, 0, 0]);
		}
		ResponsesForCallback::put(ready_callbacks.clone());

		// enough for 3 callbacks without considering overhead cost
		// should mean we only process 2 requests
		let per_callback_weight = EthStateOracle::per_callback_weight();
		let remaining_block_weight = 3 * per_callback_weight;

		// Test
		let consumed_weight = EthStateOracle::on_idle(1_u64, remaining_block_weight);

		// Storage cleared for fist 2 callbacks
		assert!(consumed_weight < remaining_block_weight);
		for r in &ready_callbacks[..2] {
			assert!(!<Responses<TestRuntime>>::contains_key(*r));
			assert!(!Requests::contains_key(*r));
			assert!(!RequestInputData::contains_key(*r));
		}

		// 3rd callback left for next time
		assert!(Requests::contains_key(RequestId::from(3_u64)));
		assert_eq!(ResponsesForCallback::get(), vec![RequestId::from(3_u64)]);
	});
}
