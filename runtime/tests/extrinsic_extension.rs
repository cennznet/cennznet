/* Copyright 2019-2020 Centrality Investments Limited
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

//! Extrinsic extension integration tests (doughnut, fee exchange)

use cennznet_primitives::types::{AccountId, FeeExchange, FeeExchangeV1};
use cennznet_runtime::{
	constants::{asset::*, currency::*},
	Call, CennzxSpot, CheckedExtrinsic, ContractTransactionBaseFee, Event, Executive, GenericAsset, Origin, Runtime,
};
use cennznet_testing::keyring::{alice, bob, charlie, dave, ferdie, signed_extra};
use codec::Encode;
use crml_transaction_payment::constants::error_code::*;
use frame_support::{
	additional_traits::MultiCurrencyAccounting as MultiCurrency,
	weights::{DispatchClass, DispatchInfo},
};
use frame_system::{EventRecord, Phase};
use pallet_contracts::{ContractAddressFor, RawEvent, Schedule};
use sp_runtime::{
	traits::Hash,
	transaction_validity::{InvalidTransaction, TransactionValidityError},
	DispatchError,
};

mod common;
mod doughnut;

use common::helpers::{extrinsic_fee_for, header, sign};
use common::mock::{
	contracts::{CONTRACT_WITH_GA_TRANSFER, CONTRACT_WITH_TRAP},
	ExtBuilder,
};
use doughnut::{make_contract_cennznut, make_doughnut, make_runtime_cennznut};

/// Setup a contract on-chain, return it's deployed address
/// This does the `put_code` and `instantiate` steps
/// Note: It will also initialize the block and requires `TestExternalities` to succeed
/// `contract_wabt` is the contract WABT to be deployed
/// `contract_deployer` is the account which will send the extrinsic to deploy the contract (IRL the contract developer)-
/// the account should also have funds to pay for tx/gas costs of the deployment.
fn setup_contract(
	contract_wabt: &'static str,
	contract_deployer: AccountId,
) -> (AccountId, <<Runtime as frame_system::Trait>::Hashing as Hash>::Output) {
	let wasm = wabt::wat2wasm(contract_wabt).unwrap();
	let code_hash = <Runtime as frame_system::Trait>::Hashing::hash(&wasm);

	Executive::initialize_block(&header());

	// Put the contract on chain
	let put_code_call = Call::Contracts(pallet_contracts::Call::put_code(10 * DOLLARS as u64, wasm));
	let put_code_extrinsic = sign(CheckedExtrinsic {
		signed: Some((contract_deployer.clone(), signed_extra(0, 0, None, None))),
		function: put_code_call,
	});
	let put_code_result = Executive::apply_extrinsic(put_code_extrinsic);
	println!(
		"{:?}, CPAY Balance: {:?}",
		put_code_result,
		<GenericAsset as MultiCurrency>::free_balance(&alice(), Some(CENTRAPAY_ASSET_ID))
	);
	assert!(put_code_result.is_ok());

	// Call the contract constructor
	let instantiate_call = Call::Contracts(pallet_contracts::Call::instantiate(
		0,                   // endowment
		10 * DOLLARS as u64, // gas limit
		code_hash.into(),
		vec![], // data
	));
	let instantiate_extrinsic = sign(CheckedExtrinsic {
		signed: Some((contract_deployer, signed_extra(1, 0, None, None))),
		function: instantiate_call,
	});
	let instantiate_result = Executive::apply_extrinsic(instantiate_extrinsic);
	println!(
		"{:?}, CPAY Balance: {:?}",
		instantiate_result,
		<GenericAsset as MultiCurrency>::free_balance(&alice(), Some(CENTRAPAY_ASSET_ID))
	);
	assert!(instantiate_result.is_ok());

	(
		<Runtime as pallet_contracts::Trait>::DetermineContractAddress::contract_address_for(&code_hash, &[], &alice()),
		code_hash,
	)
}

#[test]
fn generic_asset_transfer_works_without_fee_exchange() {
	let initial_balance = 5 * DOLLARS;
	let transfer_amount = 7_777 * MICROS;
	let runtime_call = Call::GenericAsset(pallet_generic_asset::Call::transfer(
		CENTRAPAY_ASSET_ID,
		bob(),
		transfer_amount,
	));

	ExtBuilder::default()
		.initial_balance(initial_balance)
		.build()
		.execute_with(|| {
			let xt = sign(CheckedExtrinsic {
				signed: Some((alice(), signed_extra(0, 0, None, None))),
				function: runtime_call,
			});

			Executive::initialize_block(&header());
			let r = Executive::apply_extrinsic(xt.clone());
			assert!(r.is_ok());

			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&alice(), Some(CENTRAPAY_ASSET_ID)),
				initial_balance - transfer_amount - extrinsic_fee_for(&xt)
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&bob(), Some(CENTRAPAY_ASSET_ID)),
				initial_balance + transfer_amount
			);
		});
}

#[test]
fn generic_asset_transfer_works_with_fee_exchange() {
	let initial_balance = 100 * DOLLARS;
	let initial_liquidity = 50 * DOLLARS;
	let transfer_amount = 25 * MICROS;

	let runtime_call = Call::GenericAsset(pallet_generic_asset::Call::transfer(
		CENTRAPAY_ASSET_ID,
		bob(),
		transfer_amount,
	));

	ExtBuilder::default()
		.initial_balance(initial_balance)
		.build()
		.execute_with(|| {
			// Alice sets up CENNZ <> CPAY liquidity
			assert!(CennzxSpot::add_liquidity(
				Origin::signed(alice()),
				CENNZ_ASSET_ID,
				0,                 // min liquidity
				initial_liquidity, // liquidity CENNZ
				initial_liquidity, // liquidity CPAY
			)
			.is_ok());

			// Exchange CENNZ (sell) for CPAY (buy) to pay for transaction fee
			let fee_exchange = FeeExchange::V1(FeeExchangeV1 {
				asset_id: CENNZ_ASSET_ID,
				max_payment: 5 * DOLLARS,
			});
			// Create an extrinsic where the transaction fee is to be paid in CENNZ
			let xt = sign(CheckedExtrinsic {
				signed: Some((alice(), signed_extra(0, 0, None, Some(fee_exchange)))),
				function: runtime_call,
			});

			// Calculate how much CENNZ should be sold to make the above extrinsic
			let cennz_sold_amount =
				CennzxSpot::get_asset_to_core_buy_price(&CENNZ_ASSET_ID, extrinsic_fee_for(&xt)).unwrap();
			assert_eq!(cennz_sold_amount, 11_807 * MICROS); // 1.1807 CPAY

			// Initialise block and apply the extrinsic
			Executive::initialize_block(&header());
			let r = Executive::apply_extrinsic(xt);
			assert!(r.is_ok());

			// Check remaining balances
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&alice(), Some(CENNZ_ASSET_ID)),
				initial_balance - initial_liquidity - cennz_sold_amount, // transfer fee is charged in CENNZ
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&alice(), Some(CENTRAPAY_ASSET_ID)),
				initial_balance - initial_liquidity - transfer_amount // transfer fee is not charged in CPAY
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&bob(), Some(CENTRAPAY_ASSET_ID)),
				initial_balance + transfer_amount
			);
		});
}

#[test]
fn contract_fails() {
	ExtBuilder::default()
		.initial_balance(10 * DOLLARS)
		.gas_price(1)
		.build()
		.execute_with(|| {
			let (contract_address, _) = setup_contract(CONTRACT_WITH_TRAP, dave());

			// Call the newly instantiated contract. The contract is expected to dispatch a call
			// and then trap.
			let contract_call = Call::Contracts(pallet_contracts::Call::call(
				contract_address,   // newly created contract address
				0,                  // transfer value in
				5 * DOLLARS as u64, // gas limit
				vec![],
			));
			let contract_call_extrinsic = sign(CheckedExtrinsic {
				signed: Some((bob(), signed_extra(2, 0, None, None))),
				function: contract_call,
			});

			assert!(Executive::apply_extrinsic(contract_call_extrinsic).is_err());
		});
}

// Scenario:
// - Extrinsic made with a contract call and fee payment in CENNZ
// - Contract will dispatch a runtime call to move the callers CENNZ funds which should be used for payment
// - This must fail!
#[test]
fn contract_dispatches_runtime_call_funds_are_safu() {
	ExtBuilder::default()
		.initial_balance(1_000 * DOLLARS)
		.gas_price(1)
		.build()
		.execute_with(|| {
			// Setup CENNZ <> CPAY liquidity
			assert!(CennzxSpot::add_liquidity(
				Origin::signed(dave()),
				CENNZ_ASSET_ID,
				0,
				1_000 * DOLLARS, // CENNZ liquidity
				1_000 * DOLLARS, // CPAY liquidity
			)
			.is_ok());

			// The contract will attempt to transfer 99% of bobs funds.
			// which should not succeed as this is required for fee payment.
			let bob_funds_for_payment = 990 * DOLLARS;

			// We use an encoded call in the contract
			// if the test fails here then the runtime encoding has changed so the contract WABT needs an update
			let encoded_ga_transfer = vec![
				5, 1, 1, 250, 144, 181, 171, 32, 92, 105, 116, 201, 234, 132, 27, 230, 136, 134, 70, 51, 220, 156, 168,
				163, 87, 132, 62, 234, 207, 35, 20, 100, 153, 101, 254, 34, 130, 63, 92, 2,
			];
			assert_eq!(
				&Call::GenericAsset(pallet_generic_asset::Call::transfer(
					CENNZ_ASSET_ID,
					charlie(),
					bob_funds_for_payment
				))
				.encode(),
				&encoded_ga_transfer
			);
			let (contract_address, code_hash) = setup_contract(CONTRACT_WITH_GA_TRANSFER, alice());

			// Call the newly instantiated contract. The contract is expected to dispatch a call
			// and then trap.
			let contract_call = Call::Contracts(pallet_contracts::Call::call(
				contract_address.clone(), // newly created contract address
				0,                        // transfer value in
				5 * DOLLARS as u64,       // gas limit (it should be less then `max_payment` in the fee exchange)
				vec![],
			));
			let fee_exchange = FeeExchange::V1(FeeExchangeV1 {
				asset_id: CENNZ_ASSET_ID,
				max_payment: 10 * DOLLARS,
			});
			let contract_call_extrinsic = sign(CheckedExtrinsic {
				signed: Some((bob(), signed_extra(0, 0, None, Some(fee_exchange)))),
				function: contract_call,
			});

			// This only shows transaction fee payment success, not gas payment success
			let contract_call_result = Executive::apply_extrinsic(contract_call_extrinsic.clone());
			println!("{:?}", contract_call_result);
			assert!(contract_call_result.is_ok());

			let block_events = frame_system::Module::<Runtime>::events();
			let expected_events = vec![
				EventRecord {
					phase: Phase::ApplyExtrinsic(0),
					event: Event::pallet_contracts(RawEvent::CodeStored(code_hash.into())),
					topics: vec![],
				},
				EventRecord {
					phase: Phase::ApplyExtrinsic(0),
					event: Event::frame_system(frame_system::Event::ExtrinsicSuccess(DispatchInfo {
						weight: 10_000,
						class: DispatchClass::Normal,
						pays_fee: true,
					})),
					topics: vec![],
				},
				EventRecord {
					phase: Phase::ApplyExtrinsic(1),
					event: Event::pallet_contracts(RawEvent::Transfer(alice(), contract_address.clone(), 0)),
					topics: vec![],
				},
				EventRecord {
					phase: Phase::ApplyExtrinsic(1),
					event: Event::pallet_contracts(RawEvent::Instantiated(alice(), contract_address.clone())),
					topics: vec![],
				},
				EventRecord {
					phase: Phase::ApplyExtrinsic(1),
					event: Event::frame_system(frame_system::Event::ExtrinsicSuccess(DispatchInfo {
						weight: 10_000,
						class: DispatchClass::Normal,
						pays_fee: true,
					})),
					topics: vec![],
				},
				// Pays for transaction fees
				EventRecord {
					phase: Phase::ApplyExtrinsic(2),
					event: Event::crml_cennzx_spot(crml_cennzx_spot::RawEvent::AssetPurchase(
						CENNZ_ASSET_ID,
						CENTRAPAY_ASSET_ID,
						bob(),
						// CENNZ sold
						1636,
						// CPAY to buy
						extrinsic_fee_for(&contract_call_extrinsic),
					)),
					topics: vec![],
				},
				// Pays for gas fees
				EventRecord {
					phase: Phase::ApplyExtrinsic(2),
					event: Event::crml_cennzx_spot(crml_cennzx_spot::RawEvent::AssetPurchase(
						CENNZ_ASSET_ID,
						CENTRAPAY_ASSET_ID,
						bob(),
						// CENNZ sold
						538,
						// CPAY to buy
						// = contract execution gas cost:
						// base call fee + read encoded ga call + deposit event
						(Schedule::default().call_base_cost
							+ (Schedule::default().sandbox_data_read_cost * encoded_ga_transfer.len() as u64)
							+ Schedule::default().event_base_cost) as u128
							// it's not clear where this additional 360 gas cost comes from
							// prepare_code(CONTRACT_WITH_GA_TRANSFER) will show the implementation code...
							+ 360,
					)),
					topics: vec![],
				},
				// This event shows the generic asset transfer contract has failed with result = false
				EventRecord {
					phase: Phase::ApplyExtrinsic(2),
					event: Event::pallet_contracts(RawEvent::Dispatched(contract_address.clone(), false)),
					topics: vec![],
				},
				EventRecord {
					phase: Phase::ApplyExtrinsic(2),
					event: Event::frame_system(frame_system::Event::ExtrinsicSuccess(DispatchInfo {
						weight: 10_000,
						class: DispatchClass::Normal,
						pays_fee: true,
					})),
					topics: vec![],
				},
			];
			assert_eq!(block_events, expected_events);
		});
}

#[test]
fn contract_call_fails_with_insufficient_gas_without_fee_exchange() {
	ExtBuilder::default()
		.initial_balance(100)
		.gas_price(1)
		.build()
		.execute_with(|| {
			Executive::initialize_block(&header());
			let xt = sign(CheckedExtrinsic {
				signed: Some((alice(), signed_extra(0, 0, None, None))),
				function: Call::Contracts(pallet_contracts::Call::call::<Runtime>(
					bob(),
					10,
					10 * ContractTransactionBaseFee::get() as u64,
					vec![],
				)),
			});
			assert_eq!(
				Executive::apply_extrinsic(xt),
				Err(InvalidTransaction::Custom(INSUFFICIENT_FEE_ASSET_BALANCE).into())
			);
		});
}

#[test]
fn contract_call_fails_with_insufficient_gas_with_fee_exchange() {
	// Scenario:
	// - Alice makes a contract call opting to pay for gas fees in CENNZ
	// - She has insufficient CENNZ to pay for the gas (via CENNZ-X)
	// - It should fail
	let initial_balance = 10 * DOLLARS;
	ExtBuilder::default()
		.initial_balance(initial_balance)
		.gas_price(1)
		.build()
		.execute_with(|| {
			// Setup CENNZ <> CPAY liquidity
			assert!(CennzxSpot::add_liquidity(
				Origin::signed(charlie()),
				CENNZ_ASSET_ID,
				0,               // min. liquidity
				initial_balance, // liquidity CENNZ
				initial_balance, // liquidity CPAY
			)
			.is_ok());

			// Alice transfers all her CENNZ except `0.0100`
			assert!(GenericAsset::transfer(
				Origin::signed(alice()),
				CENNZ_ASSET_ID,
				bob(),
				initial_balance - 100 * MICROS,
			)
			.is_ok());

			// Setup extrinsic
			let fee_exchange = FeeExchange::V1(FeeExchangeV1 {
				asset_id: CENNZ_ASSET_ID,
				max_payment: 1 * DOLLARS,
			});
			let xt = sign(CheckedExtrinsic {
				signed: Some((alice(), signed_extra(0, 0, None, Some(fee_exchange)))),
				function: Call::Contracts(pallet_contracts::Call::call::<Runtime>(
					bob(),
					0,
					// gas limit
					Schedule::default().call_base_cost + Schedule::default().transfer_cost,
					vec![],
				)),
			});

			Executive::initialize_block(&header());
			assert_eq!(
				Executive::apply_extrinsic(xt),
				Err(InvalidTransaction::Custom(INSUFFICIENT_BALANCE).into())
			);
		});
}

#[test]
fn contract_call_works_without_fee_exchange() {
	let balance_amount = 100 * DOLLARS;
	let transfer_amount = 555 * MICROS;
	let gas_limit_amount = 5 * DOLLARS;

	let contract_call = Call::Contracts(pallet_contracts::Call::call::<Runtime>(
		bob(),
		transfer_amount,
		gas_limit_amount as u64,
		vec![],
	));

	ExtBuilder::default()
		.initial_balance(balance_amount)
		.gas_price(1)
		.build()
		.execute_with(|| {
			let xt = sign(CheckedExtrinsic {
				signed: Some((alice(), signed_extra(0, 0, None, None))),
				function: contract_call,
			});
			Executive::initialize_block(&header());
			let r = Executive::apply_extrinsic(xt.clone());
			assert!(r.is_ok());

			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&bob(), Some(CENTRAPAY_ASSET_ID)),
				balance_amount + transfer_amount,
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&alice(), Some(CENTRAPAY_ASSET_ID)),
				balance_amount
					- transfer_amount
					// transaction fees
					- extrinsic_fee_for(&xt)
					// contract gas fees (contract base fee + transfer fee)
					- (Schedule::default().call_base_cost + Schedule::default().transfer_cost) as u128,
			);
		});
}

#[test]
fn contract_call_works_with_fee_exchange() {
	let initial_balance = 1_000 * DOLLARS;
	let transfer_amount = 555 * MICROS;
	let gas_limit = 5 * DOLLARS;
	let contract_call = Call::Contracts(pallet_contracts::Call::call::<Runtime>(
		bob(),
		transfer_amount,
		gas_limit as u64,
		vec![],
	));

	ExtBuilder::default()
		.initial_balance(initial_balance)
		.gas_price(1)
		.build()
		.execute_with(|| {
			let initial_liquidity = 1_000 * DOLLARS;
			assert!(CennzxSpot::add_liquidity(
				Origin::signed(charlie()),
				CENNZ_ASSET_ID,
				0,
				initial_liquidity, // liquidity CENNZ
				initial_liquidity, // liquidity CPAY
			)
			.is_ok());

			// Setup the extrinsic
			let fee_exchange = FeeExchange::V1(FeeExchangeV1 {
				asset_id: CENNZ_ASSET_ID,
				max_payment: 10 * DOLLARS,
			});
			let xt = sign(CheckedExtrinsic {
				signed: Some((alice(), signed_extra(0, 0, None, Some(fee_exchange)))),
				function: contract_call,
			});

			// Calculate the expected gas cost of contract execution at the current CENNZ-X spot rate.
			let gas_cost = (Schedule::default().call_base_cost + Schedule::default().transfer_cost) as u128;
			// Check CENNZ price to buy gas (gas is 1:1 with CPAY)
			let cennz_for_gas_fees = CennzxSpot::get_asset_to_core_buy_price(&CENNZ_ASSET_ID, gas_cost).unwrap();
			// Check CENNZ price to buy tx fees in CPAY
			let cennz_for_tx_fees =
				CennzxSpot::get_asset_to_core_buy_price(&CENNZ_ASSET_ID, extrinsic_fee_for(&xt)).unwrap();

			Executive::initialize_block(&header());
			let r = Executive::apply_extrinsic(xt);
			assert!(r.is_ok());

			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&bob(), Some(CENTRAPAY_ASSET_ID)),
				initial_balance + transfer_amount
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&alice(), Some(CENTRAPAY_ASSET_ID)),
				initial_balance - transfer_amount
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&alice(), Some(CENNZ_ASSET_ID)),
				initial_balance - cennz_for_tx_fees - cennz_for_gas_fees,
			);
		});
}

#[test]
fn contract_call_fails_when_fee_exchange_is_not_enough_for_gas() {
	let contract_call = Call::Contracts(pallet_contracts::Call::call::<Runtime>(
		bob(),
		0,
		Schedule::default().call_base_cost,
		vec![],
	));

	ExtBuilder::default()
		.initial_balance(1 * DOLLARS)
		.gas_price(1)
		.build()
		.execute_with(|| {
			assert!(CennzxSpot::add_liquidity(
				Origin::signed(charlie()),
				CENNZ_ASSET_ID,
				0,           // min. liquidity
				1 * DOLLARS, // liquidity CENNZ
				1 * DOLLARS, // liquidity CPAY
			)
			.is_ok());

			let fee_exchange = FeeExchange::V1(FeeExchangeV1 {
				asset_id: CENNZ_ASSET_ID,
				max_payment: 1 * MICROS,
			});
			let xt = sign(CheckedExtrinsic {
				signed: Some((alice(), signed_extra(0, 0, None, Some(fee_exchange)))),
				function: contract_call,
			});
			Executive::initialize_block(&header());
			assert_eq!(
				Executive::apply_extrinsic(xt),
				Err(InvalidTransaction::Custom(MAXIMUM_SELL_REQUIREMENT_NOT_MET).into())
			);
		});
}

#[test]
fn contract_call_fails_when_exchange_liquidity_is_low() {
	let gas_limit = 10 * DOLLARS;
	let contract_call = Call::Contracts(pallet_contracts::Call::call::<Runtime>(
		bob(),
		50,
		gas_limit as u64,
		vec![],
	));

	ExtBuilder::default()
		.initial_balance(10 * DOLLARS)
		.gas_price(1)
		.build()
		.execute_with(|| {
			assert!(CennzxSpot::add_liquidity(
				Origin::signed(charlie()),
				CENNZ_ASSET_ID,
				0,            // min. liquidity
				100 * MICROS, // liquidity CENNZ
				100 * MICROS, // liquidity CPAY
			)
			.is_ok());

			let fee_exchange = FeeExchange::V1(FeeExchangeV1 {
				asset_id: CENNZ_ASSET_ID,
				max_payment: 100 * gas_limit,
			});
			let xt = sign(CheckedExtrinsic {
				signed: Some((alice(), signed_extra(0, 0, None, Some(fee_exchange)))),
				function: contract_call,
			});

			Executive::initialize_block(&header());
			assert_eq!(
				Executive::apply_extrinsic(xt),
				Err(InvalidTransaction::Custom(INSUFFICIENT_EXCHANGE_POOL_RESERVE).into())
			);
		});
}

#[test]
fn contract_call_fails_when_cpay_is_used_for_fee_exchange() {
	let contract_call = Call::Contracts(pallet_contracts::Call::call::<Runtime>(
		bob(),
		50,
		1 * DOLLARS as u64,
		vec![],
	));

	ExtBuilder::default()
		.initial_balance(10_000 * DOLLARS)
		.gas_price(1)
		.build()
		.execute_with(|| {
			let fee_exchange = FeeExchange::V1(FeeExchangeV1 {
				asset_id: CENTRAPAY_ASSET_ID,
				max_payment: 100 * MICROS,
			});
			let xt = sign(CheckedExtrinsic {
				signed: Some((alice(), signed_extra(0, 0, None, Some(fee_exchange)))),
				function: contract_call,
			});

			Executive::initialize_block(&header());
			assert_eq!(
				Executive::apply_extrinsic(xt),
				Err(InvalidTransaction::Custom(ASSET_CANNOT_SWAP_FOR_ITSELF).into())
			);
		});
}

#[test]
fn generic_asset_transfer_works_with_doughnut() {
	let cennznut = make_runtime_cennznut("generic-asset", "transfer");
	let doughnut = make_doughnut("cennznet", cennznut.encode());

	let balance_amount = 10 * DOLLARS;

	let transfer_amount = 50 * MICROS;
	let runtime_call = Call::GenericAsset(pallet_generic_asset::Call::transfer(
		CENNZ_ASSET_ID,
		charlie(),
		transfer_amount,
	));

	ExtBuilder::default()
		.initial_balance(balance_amount)
		.build()
		.execute_with(|| {
			// Create an extrinsic where the doughnut is passed
			let xt = sign(CheckedExtrinsic {
				signed: Some((bob(), signed_extra(0, 0, Some(doughnut), None))),
				function: runtime_call,
			});

			// Initialise block and apply the extrinsic
			Executive::initialize_block(&header());
			let r = Executive::apply_extrinsic(xt.clone());
			assert!(r.is_ok());

			// Check CENNZ balances
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&bob(), Some(CENNZ_ASSET_ID)),
				balance_amount, // Bob does not transfer CENNZ
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&alice(), Some(CENNZ_ASSET_ID)),
				balance_amount - transfer_amount // transfer from Alice
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&charlie(), Some(CENNZ_ASSET_ID)),
				balance_amount + transfer_amount // transfer to Charlie
			);
			// Check CPAY balances
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&bob(), Some(CENTRAPAY_ASSET_ID)),
				balance_amount - extrinsic_fee_for(&xt), // Bob pays transaction fees
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&alice(), Some(CENTRAPAY_ASSET_ID)),
				balance_amount, // Alice does not pay transaction fees
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&charlie(), Some(CENTRAPAY_ASSET_ID)),
				balance_amount // Charlie is not affected
			);
		});
}

#[test]
fn generic_asset_transfer_fails_with_bad_doughnut_permissions() {
	let cennznut = make_runtime_cennznut("attestation", "attest");
	let doughnut = make_doughnut("cennznet", cennznut.encode());

	let initial_balance = 100 * DOLLARS;
	let transfer_amount = 50 * MICROS;
	let runtime_call = Call::GenericAsset(pallet_generic_asset::Call::transfer(
		CENNZ_ASSET_ID,
		charlie(),
		transfer_amount,
	));

	ExtBuilder::default()
		.initial_balance(initial_balance)
		.build()
		.execute_with(|| {
			// Create an extrinsic where the doughnut is passed
			let xt = sign(CheckedExtrinsic {
				signed: Some((bob(), signed_extra(0, 0, Some(doughnut), None))),
				function: runtime_call,
			});

			// Initialise block and apply the extrinsic
			Executive::initialize_block(&header());
			assert_eq!(
				Executive::apply_extrinsic(xt.clone()),
				Ok(Err(DispatchError::Other(
					"CENNZnut does not grant permission for module"
				)))
			);

			// All CENNZ balances stay the same
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&bob(), Some(CENNZ_ASSET_ID)),
				initial_balance,
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&alice(), Some(CENNZ_ASSET_ID)),
				initial_balance,
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&charlie(), Some(CENNZ_ASSET_ID)),
				initial_balance,
			);
			// Check CPAY balances (all same, except bob who pays tx fees)
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&bob(), Some(CENTRAPAY_ASSET_ID)),
				initial_balance - extrinsic_fee_for(&xt), // Bob pays transaction fees
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&alice(), Some(CENTRAPAY_ASSET_ID)),
				initial_balance,
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&charlie(), Some(CENTRAPAY_ASSET_ID)),
				initial_balance
			);
		});
}

#[test]
fn generic_asset_transfer_works_with_doughnut_and_fee_exchange_combo() {
	let cennznut = make_runtime_cennznut("generic-asset", "transfer");
	let doughnut = make_doughnut("cennznet", cennznut.encode());

	let initial_balance = 100 * DOLLARS;
	let initial_liquidity = 50 * DOLLARS;
	let transfer_amount = 25 * MICROS;

	let runtime_call = Call::GenericAsset(pallet_generic_asset::Call::transfer(
		CENTRAPAY_ASSET_ID,
		charlie(),
		transfer_amount,
	));

	ExtBuilder::default()
		.initial_balance(initial_balance)
		.build()
		.execute_with(|| {
			// setup CENNZ <> CPAY liquidity
			assert!(CennzxSpot::add_liquidity(
				Origin::signed(ferdie()),
				CENNZ_ASSET_ID,
				0,                 // min. liquidity
				initial_liquidity, // liquidity CENNZ
				initial_liquidity, // liquidity CPAY
			)
			.is_ok());

			// Create an extrinsic where the doughnut and fee exchange is passed
			let fee_exchange = FeeExchange::V1(FeeExchangeV1 {
				asset_id: CENNZ_ASSET_ID,
				max_payment: 5 * DOLLARS,
			});
			let xt = sign(CheckedExtrinsic {
				signed: Some((bob(), signed_extra(0, 0, Some(doughnut), Some(fee_exchange)))),
				function: runtime_call,
			});

			// Check CENNZ fee price
			let cennz_sold_amount =
				CennzxSpot::get_asset_to_core_buy_price(&CENNZ_ASSET_ID, extrinsic_fee_for(&xt)).unwrap();
			assert_eq!(cennz_sold_amount, 14_161 * MICROS); // 1.4161 CPAY

			// Initialise block and apply the extrinsic
			Executive::initialize_block(&header());
			let r = Executive::apply_extrinsic(xt);
			assert!(r.is_ok());

			// Check CPAY balances
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&bob(), Some(CENTRAPAY_ASSET_ID)),
				initial_balance, // Bob does not transfer CPAY
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&alice(), Some(CENTRAPAY_ASSET_ID)),
				initial_balance - transfer_amount // transfer is paid by Alice
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&charlie(), Some(CENTRAPAY_ASSET_ID)),
				initial_balance + transfer_amount
			);
			// Check CENNZ balances
			// Calculate how much CENNZ should be sold to make the above extrinsic
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&bob(), Some(CENNZ_ASSET_ID)),
				initial_balance - cennz_sold_amount, // Bob pays fees (in CENNZ)
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&alice(), Some(CENNZ_ASSET_ID)),
				initial_balance, // Alice does not pay transaction fees
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&charlie(), Some(CENNZ_ASSET_ID)),
				initial_balance
			);
		});
}

#[test]
fn contract_call_works_with_doughnut() {
	let cennznut = make_contract_cennznut(&charlie());
	let doughnut = make_doughnut("cennznet", cennznut.encode());

	let initial_balance = 100 * DOLLARS;
	let transfer_amount = 50 * DOLLARS;
	let gas_limit_amount = 1 * DOLLARS;

	let contract_call = Call::Contracts(pallet_contracts::Call::call::<Runtime>(
		charlie(),
		transfer_amount,
		gas_limit_amount as u64,
		vec![],
	));

	ExtBuilder::default()
		.initial_balance(initial_balance)
		.gas_price(1)
		.build()
		.execute_with(|| {
			let xt = sign(CheckedExtrinsic {
				signed: Some((bob(), signed_extra(0, 0, Some(doughnut), None))),
				function: contract_call,
			});
			Executive::initialize_block(&header());
			let r = Executive::apply_extrinsic(xt.clone());
			assert!(r.is_ok());

			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&bob(), Some(CENTRAPAY_ASSET_ID)),
				initial_balance - extrinsic_fee_for(&xt), // Bob pays transaction fees
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&charlie(), Some(CENTRAPAY_ASSET_ID)),
				initial_balance + transfer_amount, // charlie receives transfer amount
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&alice(), Some(CENTRAPAY_ASSET_ID)),
				initial_balance
				- transfer_amount
				// contract gas fees (contract base fee + transfer fee)
				- (Schedule::default().call_base_cost + Schedule::default().transfer_cost) as u128,
			);
		});
}

#[test]
fn contract_call_fails_with_invalid_doughnut_holder() {
	let cennznut = make_contract_cennznut(&charlie());
	let doughnut = make_doughnut("cennznet", cennznut.encode());

	// defined in: prml_doughnut::constants::error_code::VALIDATION_HOLDER_SIGNER_IDENTITY_MISMATCH
	let validation_holder_signer_identity_mismatch = 180;

	let balance_amount = 10 * DOLLARS;
	let transfer_amount = 5 * MICROS;
	let gas_limit_amount = 1 * DOLLARS;
	let contract_call = Call::Contracts(pallet_contracts::Call::call::<Runtime>(
		charlie(),
		transfer_amount,
		gas_limit_amount as u64,
		vec![],
	));

	ExtBuilder::default()
		.initial_balance(balance_amount)
		.gas_price(1)
		.build()
		.execute_with(|| {
			let xt = sign(CheckedExtrinsic {
				signed: Some((dave(), signed_extra(0, 0, Some(doughnut), None))),
				function: contract_call,
			});
			Executive::initialize_block(&header());
			assert_eq!(
				Executive::apply_extrinsic(xt),
				Err(TransactionValidityError::Invalid(InvalidTransaction::Custom(
					validation_holder_signer_identity_mismatch
				)))
			);

			// All accounts stay the same
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&alice(), Some(CENTRAPAY_ASSET_ID)),
				balance_amount,
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&bob(), Some(CENTRAPAY_ASSET_ID)),
				balance_amount,
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&charlie(), Some(CENTRAPAY_ASSET_ID)),
				balance_amount,
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&dave(), Some(CENTRAPAY_ASSET_ID)),
				balance_amount,
			);
		});
}

#[test]
fn contract_call_with_doughnut_fails_with_invalid_contract_address() {
	let cennznut = make_contract_cennznut(&charlie());
	let doughnut = make_doughnut("cennznet", cennznut.encode());

	let initial_balance = 5 * DOLLARS;
	let transfer_amount = 1 * DOLLARS;
	let gas_limit_amount = 3 * DOLLARS;
	let contract_call = Call::Contracts(pallet_contracts::Call::call::<Runtime>(
		dave(), // cennznut permissions charlie, but we will try calling dave
		transfer_amount,
		gas_limit_amount as u64,
		vec![],
	));

	ExtBuilder::default()
		.initial_balance(initial_balance)
		.gas_price(1)
		.build()
		.execute_with(|| {
			let xt = sign(CheckedExtrinsic {
				signed: Some((bob(), signed_extra(0, 0, Some(doughnut), None))),
				function: contract_call,
			});
			Executive::initialize_block(&header());
			assert_eq!(
				Executive::apply_extrinsic(xt.clone()),
				Ok(Err(DispatchError::Other(
					"CENNZnut does not grant permission for contract"
				)))
			);

			// Bob pays transaction fees
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&bob(), Some(CENTRAPAY_ASSET_ID)),
				initial_balance - extrinsic_fee_for(&xt),
			);
			// All other accounts stay the same
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&alice(), Some(CENTRAPAY_ASSET_ID)),
				initial_balance,
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&charlie(), Some(CENTRAPAY_ASSET_ID)),
				initial_balance,
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&dave(), Some(CENTRAPAY_ASSET_ID)),
				initial_balance,
			);
		});
}
