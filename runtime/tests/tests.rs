// Copyright (C) 2020 Centrality Investments Limited
// This file is part of CENNZnet.
//
// CENNZnet is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// CENNZnet is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with CENNZnet.  If not, see <http://www.gnu.org/licenses/>.

use cennznet_primitives::types::{AccountId, Balance, FeeExchange, FeeExchangeV1};
use cennznet_runtime::{
	constants::{asset::*, currency::*},
	Call, CennzxSpot, CheckedExtrinsic, ContractTransactionBaseFee, Event, Executive, GenericAsset, Staking, Origin, Runtime,
	TransactionBaseFee, TransactionByteFee, TransactionPayment, UncheckedExtrinsic,
};
use cennznet_testing::keyring::*;
use codec::Encode;
use crml_transaction_payment::constants::error_code::*;
use frame_support::{
	additional_traits::MultiCurrencyAccounting,
	traits::Imbalance,
	weights::{DispatchClass, DispatchInfo, GetDispatchInfo},
};
use frame_system::{EventRecord, Phase};
use pallet_contracts::{ContractAddressFor, RawEvent};
use sp_runtime::{
	testing::Digest,
	traits::{Convert, Hash, Header},
	transaction_validity::InvalidTransaction,
	Fixed64,
};
use pallet_staking::{StakingLedger, RewardDestination};

mod doughnut;
mod mock;
use mock::{validators, ExtBuilder};

const GENESIS_HASH: [u8; 32] = [69u8; 32];
const VERSION: u32 = cennznet_runtime::VERSION.spec_version;

fn initialize_block() {
	Executive::initialize_block(&Header::new(
		1,                        // block number
		sp_core::H256::default(), // extrinsics_root
		sp_core::H256::default(), // state_root
		GENESIS_HASH.into(),      // parent_hash
		Digest::default(),        // digest
	));
}

/// Setup a contract on-chain, return it's deployed address
/// This does the `put_code` and `instantiate` steps
/// Note: It will also initialize the block and requires `TestExternalities` to succeed
/// `contract_wabt` is the contract WABT to be deployed
/// `contract_deployer` is the account which will send the extrinsic to deploy the contract (IRL the contract developer)
fn setup_contract(
	contract_wabt: &'static str,
	contract_deployer: AccountId,
) -> (AccountId, <<Runtime as frame_system::Trait>::Hashing as Hash>::Output) {
	// Contract itself fails
	let wasm = wabt::wat2wasm(contract_wabt).unwrap();
	let code_hash = <Runtime as frame_system::Trait>::Hashing::hash(&wasm);

	initialize_block();

	let put_code_call = Call::Contracts(pallet_contracts::Call::put_code(50_000_000, wasm));
	let put_code_extrinsic = sign(CheckedExtrinsic {
		signed: Some((contract_deployer.clone(), signed_extra(0, 0, None, None))),
		function: put_code_call,
	});
	let r = Executive::apply_extrinsic(put_code_extrinsic);
	println!(
		"{:?}, CPAY Balance: {:?}",
		r,
		<GenericAsset as MultiCurrencyAccounting>::free_balance(&alice(), Some(CENTRAPAY_ASSET_ID))
	);
	assert!(r.is_ok());

	let instantiate_call = Call::Contracts(pallet_contracts::Call::instantiate(
		0,                   // endowment
		100_000_000_000_000, // gas limit
		code_hash.into(),
		vec![], // data
	));
	let instantiate_extrinsic = sign(CheckedExtrinsic {
		signed: Some((contract_deployer, signed_extra(1, 0, None, None))),
		function: instantiate_call,
	});
	let r2 = Executive::apply_extrinsic(instantiate_extrinsic);
	println!(
		"{:?}, CPAY Balance: {:?}",
		r2,
		<GenericAsset as MultiCurrencyAccounting>::free_balance(&alice(), Some(CENTRAPAY_ASSET_ID))
	);
	assert!(r2.is_ok());

	(
		<Runtime as pallet_contracts::Trait>::DetermineContractAddress::contract_address_for(&code_hash, &[], &alice()),
		code_hash,
	)
}

fn sign(xt: CheckedExtrinsic) -> UncheckedExtrinsic {
	cennznet_testing::keyring::sign(xt, VERSION, GENESIS_HASH)
}

fn transfer_fee<E: Encode>(extrinsic: &E, fee_multiplier: Fixed64, runtime_call: &Call) -> Balance {
	let length_fee = TransactionByteFee::get() * (extrinsic.encode().len() as Balance);

	let weight = runtime_call.get_dispatch_info().weight;
	let weight_fee = <Runtime as crml_transaction_payment::Trait>::WeightToFee::convert(weight);

	let base_fee = TransactionBaseFee::get();
	base_fee + fee_multiplier.saturated_multiply_accumulate(length_fee + weight_fee)
}

#[test]
fn staking_genesis_config_works() {
	let balance_amount = 10_000 * TransactionBaseFee::get();
	let staked_amount = balance_amount / 5;
	ExtBuilder::default()
		.initial_balance(balance_amount)
		.stash(staked_amount)
		.build()
		.execute_with(|| {
			for validator in validators() {
				let (stash, controller) = validator;
				// Check validator is included in currect elelcted accounts
				assert!(Staking::current_elected().contains(&stash));
				// Check that RewardDestination is Staked (default)
				assert_eq!(Staking::payee(&stash), RewardDestination::Staked);				
				// Check validator free balance
				assert_eq!(
					<GenericAsset as MultiCurrencyAccounting>::free_balance(&stash, Some(CENTRAPAY_ASSET_ID)),
					balance_amount
				);
				// Check how much is at stake
				assert_eq!(Staking::ledger(controller), Some(StakingLedger {
					stash,
					total: staked_amount,
					active: staked_amount,
					unlocking: vec![],
				}));
			}
		});
}

#[test]
fn staking_validators_should_received_equal_transaction_fee_reward() {
	let transfer_amount = 50;
	let balance_amount = 10_000 * TransactionBaseFee::get();
	let runtime_call = Call::GenericAsset(pallet_generic_asset::Call::transfer(
		CENTRAPAY_ASSET_ID,
		bob(),
		transfer_amount,
	));

	ExtBuilder::default()
		.initial_balance(balance_amount)
		.build()
		.execute_with(|| {
			let xt = sign(CheckedExtrinsic {
				signed: Some((alice(), signed_extra(0, 0, None, None))),
				function: runtime_call.clone(),
			});
	
			let fm = TransactionPayment::next_fee_multiplier();
			let fee = transfer_fee(&xt, fm, &runtime_call);
			let validator_count = validators().len() as Balance;
			let fee_reward = fee / validator_count;
			let remainder = fee % validator_count;

			let previous_total_issuance = GenericAsset::total_issuance(&CENTRAPAY_ASSET_ID);

			initialize_block();
			let r = Executive::apply_extrinsic(xt);
			assert!(r.is_ok());

			// Check total_issurance is adjusted
			// FIXME: total_issurance is not adjusted
			assert_eq!(
				GenericAsset::total_issuance(&CENTRAPAY_ASSET_ID),
				previous_total_issuance - remainder
			);

			for validator in validators() {
				// Check tx fee reward went to the stash account of validator
				assert_eq!(
					<GenericAsset as MultiCurrencyAccounting>::free_balance(&validator.0, Some(CENTRAPAY_ASSET_ID)),
					balance_amount + fee_reward
				);
			}
		});
}

#[test]
fn runtime_mock_setup_works() {
	let amount = 100;
	ExtBuilder::default().initial_balance(amount).build().execute_with(|| {
		let tests = vec![
			(alice(), amount),
			(bob(), amount),
			(charlie(), amount),
			(dave(), amount),
			(eve(), amount),
			(ferdie(), amount),
		];
		let assets = vec![
			CENNZ_ASSET_ID,
			CENTRAPAY_ASSET_ID,
			PLUG_ASSET_ID,
			SYLO_ASSET_ID,
			CERTI_ASSET_ID,
			ARDA_ASSET_ID,
		];
		for (account, balance) in &tests {
			for asset in &assets {
				assert_eq!(
					<GenericAsset as MultiCurrencyAccounting>::free_balance(&account, Some(*asset)),
					*balance,
				);
				assert_eq!(
					<GenericAsset as MultiCurrencyAccounting>::free_balance(&account, Some(123)),
					0,
				)
			}
		}
	});
}

#[test]
fn generic_asset_transfer_works_without_fee_exchange() {
	let transfer_amount = 50;
	let runtime_call = Call::GenericAsset(pallet_generic_asset::Call::transfer(
		CENTRAPAY_ASSET_ID,
		bob(),
		transfer_amount,
	));
	let encoded = Encode::encode(&runtime_call);

	// First 2 bytes are module and method indices, respectively (NOTE: module index doesn't count modules
	// without Call in construct_runtime!). The next 2 bytes are 16_001 encoded using compact codec,
	// followed by 32 bytes of bob's account id. The last byte is 50 encoded using the compact codec as well.
	// For more info, see the method signature for generic_asset::transfer() and the use of #[compact] for args.
	let encoded_test_bytes: Vec<u8> = vec![
		6, 1, 5, 250, 142, 175, 4, 21, 22, 135, 115, 99, 38, 201, 254, 161, 126, 37, 252, 82, 135, 97, 54, 147, 201,
		18, 144, 156, 178, 38, 170, 71, 148, 242, 106, 72, 200,
	];
	assert_eq!(encoded, encoded_test_bytes);
	assert_eq!(
		hex::encode(encoded),
		"060105fa8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48c8"
	);

	ExtBuilder::default().build().execute_with(|| {
		let balance_amount = 10_000 * TransactionBaseFee::get(); // give enough to make a transaction
		let imbalance = GenericAsset::deposit_creating(&alice(), Some(CENTRAPAY_ASSET_ID), balance_amount);
		assert_eq!(imbalance.peek(), balance_amount);
		assert_eq!(
			<GenericAsset as MultiCurrencyAccounting>::free_balance(&alice(), Some(CENTRAPAY_ASSET_ID)),
			balance_amount,
		);

		let xt = sign(CheckedExtrinsic {
			signed: Some((alice(), signed_extra(0, 0, None, None))),
			function: runtime_call.clone(),
		});

		let fm = TransactionPayment::next_fee_multiplier();
		let fee = transfer_fee(&xt, fm, &runtime_call);

		initialize_block();
		let r = Executive::apply_extrinsic(xt);
		assert!(r.is_ok());

		assert_eq!(
			<GenericAsset as MultiCurrencyAccounting>::free_balance(&alice(), Some(CENTRAPAY_ASSET_ID)),
			balance_amount - transfer_amount - fee,
		);
		assert_eq!(
			<GenericAsset as MultiCurrencyAccounting>::free_balance(&bob(), Some(CENTRAPAY_ASSET_ID)),
			transfer_amount,
		);
	});
}

#[test]
fn generic_asset_transfer_works_with_fee_exchange() {
	let balance_amount = 1_000_000 * TransactionBaseFee::get();
	let liquidity_core_amount = 100 * TransactionBaseFee::get();
	let liquidity_asset_amount = 200;
	let transfer_amount = 50;
	let runtime_call = Call::GenericAsset(pallet_generic_asset::Call::transfer(
		CENTRAPAY_ASSET_ID,
		bob(),
		transfer_amount,
	));

	ExtBuilder::default()
		.initial_balance(balance_amount)
		.build()
		.execute_with(|| {
			// Alice adds initial liquidity to an exchange
			let _ = CennzxSpot::add_liquidity(
				Origin::signed(alice()),
				CENNZ_ASSET_ID,
				10, // min_liquidity
				liquidity_asset_amount,
				liquidity_core_amount,
			);
			let ex_key = (CENTRAPAY_ASSET_ID, CENNZ_ASSET_ID);
			assert_eq!(CennzxSpot::get_liquidity(&ex_key, &alice()), liquidity_core_amount);

			// Exchange CENNZ (sell) for CPAY (buy) to pay for transaction fee
			let fee_exchange = FeeExchange::V1(FeeExchangeV1 {
				asset_id: CENNZ_ASSET_ID,
				max_payment: 100_000_000,
			});

			// Create an extrinsic where the transaction fee is to be paid in CENNZ
			let xt = sign(CheckedExtrinsic {
				signed: Some((alice(), signed_extra(0, 0, None, Some(fee_exchange)))),
				function: runtime_call.clone(),
			});

			// Compute the transaction fee of the extrinsic
			let fm = TransactionPayment::next_fee_multiplier();
			let fee = transfer_fee(&xt, fm, &runtime_call);

			// Calculate how much CENNZ should be sold to make the above extrinsic
			let cennz_sold_amount =
				CennzxSpot::get_asset_to_core_output_price(&CENNZ_ASSET_ID, fee, CennzxSpot::fee_rate()).unwrap();
			assert_eq!(cennz_sold_amount, 6);

			// Initialise block and apply the extrinsic
			initialize_block();
			let r = Executive::apply_extrinsic(xt);
			assert!(r.is_ok());

			// Check remaining balances
			assert_eq!(
				<GenericAsset as MultiCurrencyAccounting>::free_balance(&alice(), Some(CENNZ_ASSET_ID)),
				balance_amount - liquidity_asset_amount - cennz_sold_amount, // transfer fee is charged in CENNZ
			);
			assert_eq!(
				<GenericAsset as MultiCurrencyAccounting>::free_balance(&alice(), Some(CENTRAPAY_ASSET_ID)),
				balance_amount - liquidity_core_amount - transfer_amount, // transfer fee is not charged in CPAY
			);
			assert_eq!(
				<GenericAsset as MultiCurrencyAccounting>::free_balance(&bob(), Some(CENTRAPAY_ASSET_ID)),
				balance_amount + transfer_amount,
			);
		});
}

#[test]
fn contract_fails() {
	ExtBuilder::default()
		.initial_balance(1_000_000 * TransactionBaseFee::get())
		.gas_price(1)
		.build()
		.execute_with(|| {
			let (contract_address, _) = setup_contract(mock::contracts::CONTRACT_WITH_TRAP, dave());

			// Call the newly instantiated contract. The contract is expected to dispatch a call
			// and then trap.
			let contract_call = Call::Contracts(pallet_contracts::Call::call(
				contract_address, // newly created contract address
				0,                // transfer value in
				1_000_000,        // gas limit
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
// This must fail!
#[test]
fn contract_dispatches_runtime_call_funds_are_safu() {
	ExtBuilder::default()
		.initial_balance(1_000_000_000_000 * DOLLARS)
		.gas_price(1)
		.build()
		.execute_with(|| {
			// Setup lots of CENNZ / CPAY liquidity
			assert!(CennzxSpot::add_liquidity(
				Origin::signed(dave()),
				CENNZ_ASSET_ID,
				1_000_000_000 * DOLLARS,
				1_000_000_000 * DOLLARS,
				1_000_000_000 * DOLLARS,
			)
			.is_ok());

			let bob_max_funds = 10 * CENTS;
			// We use an encoded call in the contract
			// if the test fails here the runtime encoding has changed so the contract WABT needs an update
			assert_eq!(
				Call::GenericAsset(pallet_generic_asset::Call::transfer(
					CENNZ_ASSET_ID,
					charlie(),
					bob_max_funds
				))
				.encode()
				.as_slice(),
				vec![
					6, 1, 1, 250, 144, 181, 171, 32, 92, 105, 116, 201, 234, 132, 27, 230, 136, 134, 70, 51, 220, 156,
					168, 163, 87, 132, 62, 234, 207, 35, 20, 100, 153, 101, 254, 34, 11, 0, 160, 114, 78, 24, 9
				]
				.as_slice()
			);
			let (contract_address, code_hash) = setup_contract(mock::contracts::CONTRACT_WITH_GA_TRANSFER, alice());

			// Call the newly instantiated contract. The contract is expected to dispatch a call
			// and then trap.
			let contract_call = Call::Contracts(pallet_contracts::Call::call(
				contract_address.clone(), // newly created contract address
				0,                        // transfer value in
				5_000_000_000,            // gas limit
				vec![],
			));
			let fee_exchange = FeeExchange::V1(FeeExchangeV1 {
				asset_id: CENNZ_ASSET_ID,
				max_payment: bob_max_funds,
			});
			let contract_call_extrinsic = sign(CheckedExtrinsic {
				signed: Some((bob(), signed_extra(0, 0, None, Some(fee_exchange)))),
				function: contract_call,
			});

			// This only shows transaction fee payment success, not gas payment success
			assert!(Executive::apply_extrinsic(contract_call_extrinsic).is_ok());

			let block_events = frame_system::Module::<Runtime>::events();
			let events = vec![
				EventRecord {
					phase: Phase::ApplyExtrinsic(0),
					event: Event::pallet_contracts(RawEvent::CodeStored(code_hash.into())),
					topics: vec![],
				},
				EventRecord {
					phase: Phase::ApplyExtrinsic(0),
					event: Event::system(frame_system::Event::ExtrinsicSuccess(DispatchInfo {
						weight: 10000,
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
					event: Event::system(frame_system::Event::ExtrinsicSuccess(DispatchInfo {
						weight: 10000,
						class: DispatchClass::Normal,
						pays_fee: true,
					})),
					topics: vec![],
				},
				EventRecord {
					phase: Phase::ApplyExtrinsic(2),
					event: Event::crml_cennzx_spot(crml_cennzx_spot::RawEvent::AssetPurchase(
						CENNZ_ASSET_ID,
						CENTRAPAY_ASSET_ID,
						bob(),
						2_587_750_030_067,
						2_580_010_000_000,
					)),
					topics: vec![],
				},
				EventRecord {
					phase: Phase::ApplyExtrinsic(2),
					event: Event::crml_cennzx_spot(crml_cennzx_spot::RawEvent::AssetPurchase(
						CENNZ_ASSET_ID,
						CENTRAPAY_ASSET_ID,
						bob(),
						421_261_139,
						420_001_135,
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
					event: Event::system(frame_system::Event::ExtrinsicSuccess(DispatchInfo {
						weight: 10000,
						class: DispatchClass::Normal,
						pays_fee: true,
					})),
					topics: vec![],
				},
			];
			assert_eq!(block_events, events);
		});
}

#[test]
fn contract_call_fails_with_insufficient_gas_without_fee_exchange() {
	ExtBuilder::default()
		.initial_balance(100)
		.gas_price(1)
		.build()
		.execute_with(|| {
			initialize_block();
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
	ExtBuilder::default()
		.initial_balance(100)
		.gas_price(1)
		.build()
		.execute_with(|| {
			// Add more funds to charlie's account so he can create an exchange
			let balance_amount = 10_000 * TransactionBaseFee::get();
			let _ = GenericAsset::deposit_creating(&charlie(), Some(CENTRAPAY_ASSET_ID), balance_amount);
			let _ = GenericAsset::deposit_creating(&charlie(), Some(CENNZ_ASSET_ID), balance_amount);
			assert_eq!(
				GenericAsset::free_balance(&CENTRAPAY_ASSET_ID, &charlie()),
				balance_amount + 100
			);
			assert_eq!(
				GenericAsset::free_balance(&CENNZ_ASSET_ID, &charlie()),
				balance_amount + 100
			);

			let liquidity_core_amount = 100 * TransactionBaseFee::get();
			let liquidity_asset_amount = 200 * TransactionBaseFee::get();

			let _ = CennzxSpot::add_liquidity(
				Origin::signed(charlie()),
				CENNZ_ASSET_ID,
				10, // min_liquidity
				liquidity_asset_amount,
				liquidity_core_amount,
			);
			let ex_key = (CENTRAPAY_ASSET_ID, CENNZ_ASSET_ID);
			assert_eq!(CennzxSpot::get_liquidity(&ex_key, &charlie()), liquidity_core_amount);

			let fee_exchange = FeeExchange::V1(FeeExchangeV1 {
				asset_id: CENNZ_ASSET_ID,
				max_payment: 10 * TransactionBaseFee::get(),
			});

			initialize_block();
			let xt = sign(CheckedExtrinsic {
				signed: Some((alice(), signed_extra(0, 0, None, Some(fee_exchange)))),
				function: Call::Contracts(pallet_contracts::Call::call::<Runtime>(
					bob(),
					10,
					10 * ContractTransactionBaseFee::get() as u64,
					vec![],
				)),
			});
			assert_eq!(
				Executive::apply_extrinsic(xt),
				Err(InvalidTransaction::Custom(INSUFFICIENT_BUYER_TRADE_ASSET_BALANCE).into())
			);
		});
}

#[test]
fn contract_call_works_without_fee_exchange() {
	let balance_amount = 10_000 * TransactionBaseFee::get();
	let transfer_amount = 50;
	let gas_limit_amount = 10 * ContractTransactionBaseFee::get();
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
			initialize_block();
			let r = Executive::apply_extrinsic(xt);
			assert!(r.is_ok());

			assert_eq!(
				<GenericAsset as MultiCurrencyAccounting>::free_balance(&bob(), Some(CENTRAPAY_ASSET_ID)),
				balance_amount + transfer_amount,
			);
			assert_eq!(
				<GenericAsset as MultiCurrencyAccounting>::free_balance(&alice(), Some(CENTRAPAY_ASSET_ID)),
				balance_amount - 2_440_010_001_185,
			);
		});
}

#[test]
fn contract_call_works_with_fee_exchange() {
	let balance_amount = 10_000 * TransactionBaseFee::get();
	let transfer_amount = 50;
	let gas_limit_amount = 10 * ContractTransactionBaseFee::get();
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
			let liquidity_core_amount = 100 * TransactionBaseFee::get();
			let liquidity_asset_amount = 10 * TransactionBaseFee::get();
			let _ = CennzxSpot::add_liquidity(
				Origin::signed(charlie()),
				CENNZ_ASSET_ID,
				10, // min_liquidity
				liquidity_asset_amount,
				liquidity_core_amount,
			);
			let ex_key = (CENTRAPAY_ASSET_ID, CENNZ_ASSET_ID);
			assert_eq!(CennzxSpot::get_liquidity(&ex_key, &charlie()), liquidity_core_amount);

			let fee_exchange = FeeExchange::V1(FeeExchangeV1 {
				asset_id: CENNZ_ASSET_ID,
				max_payment: 100_000_000 * gas_limit_amount,
			});

			let xt = sign(CheckedExtrinsic {
				signed: Some((alice(), signed_extra(0, 0, None, Some(fee_exchange)))),
				function: contract_call,
			});
			initialize_block();
			let r = Executive::apply_extrinsic(xt);
			assert!(r.is_ok());

			assert_eq!(
				<GenericAsset as MultiCurrencyAccounting>::free_balance(&bob(), Some(CENTRAPAY_ASSET_ID)),
				balance_amount + transfer_amount,
			);
			assert_eq!(
				<GenericAsset as MultiCurrencyAccounting>::free_balance(&alice(), Some(CENTRAPAY_ASSET_ID)),
				balance_amount - transfer_amount,
			);
			assert_eq!(
				<GenericAsset as MultiCurrencyAccounting>::free_balance(&alice(), Some(CENNZ_ASSET_ID)),
				balance_amount - 260_346_803_274,
			);
		});
}

#[test]
fn contract_call_fails_when_fee_exchange_is_not_enough_for_gas() {
	let contract_call = Call::Contracts(pallet_contracts::Call::call::<Runtime>(
		bob(),
		50,
		10 * ContractTransactionBaseFee::get() as u64,
		vec![],
	));

	ExtBuilder::default()
		.initial_balance(10_000 * TransactionBaseFee::get())
		.gas_price(1)
		.build()
		.execute_with(|| {
			let liquidity_core_amount = 100 * TransactionBaseFee::get();
			let liquidity_asset_amount = 10 * TransactionBaseFee::get();
			let _ = CennzxSpot::add_liquidity(
				Origin::signed(charlie()),
				CENNZ_ASSET_ID,
				10, // min_liquidity
				liquidity_asset_amount,
				liquidity_core_amount,
			);
			let ex_key = (CENTRAPAY_ASSET_ID, CENNZ_ASSET_ID);
			assert_eq!(CennzxSpot::get_liquidity(&ex_key, &charlie()), liquidity_core_amount);

			let fee_exchange = FeeExchange::V1(FeeExchangeV1 {
				asset_id: CENNZ_ASSET_ID,
				max_payment: 1,
			});

			let xt = sign(CheckedExtrinsic {
				signed: Some((alice(), signed_extra(0, 0, None, Some(fee_exchange)))),
				function: contract_call,
			});
			initialize_block();
			assert_eq!(
				Executive::apply_extrinsic(xt),
				Err(InvalidTransaction::Custom(ASSET_TO_CORE_PRICE_ABOVE_MAX_LIMIT).into())
			);
		});
}

#[test]
fn contract_call_fails_when_exchange_liquidity_is_low() {
	let gas_limit_amount = 10 * ContractTransactionBaseFee::get();
	let contract_call = Call::Contracts(pallet_contracts::Call::call::<Runtime>(
		bob(),
		50,
		gas_limit_amount as u64,
		vec![],
	));

	ExtBuilder::default()
		.initial_balance(10_000 * TransactionBaseFee::get())
		.gas_price(1)
		.build()
		.execute_with(|| {
			let liquidity_core_amount = 100;
			let liquidity_asset_amount = 100;
			let _ = CennzxSpot::add_liquidity(
				Origin::signed(charlie()),
				CENNZ_ASSET_ID,
				10, // min_liquidity
				liquidity_asset_amount,
				liquidity_core_amount,
			);
			let ex_key = (CENTRAPAY_ASSET_ID, CENNZ_ASSET_ID);
			assert_eq!(CennzxSpot::get_liquidity(&ex_key, &charlie()), liquidity_core_amount);

			let fee_exchange = FeeExchange::V1(FeeExchangeV1 {
				asset_id: CENNZ_ASSET_ID,
				max_payment: 100_000_000 * gas_limit_amount,
			});

			let xt = sign(CheckedExtrinsic {
				signed: Some((alice(), signed_extra(0, 0, None, Some(fee_exchange)))),
				function: contract_call,
			});
			initialize_block();
			assert_eq!(
				Executive::apply_extrinsic(xt),
				Err(InvalidTransaction::Custom(INSUFFICIENT_CORE_ASSET_RESERVE).into())
			);
		});
}

#[test]
fn contract_call_fails_when_cpay_is_used_for_fee_exchange() {
	let gas_limit_amount = 10 * ContractTransactionBaseFee::get();
	let contract_call = Call::Contracts(pallet_contracts::Call::call::<Runtime>(
		bob(),
		50,
		gas_limit_amount as u64,
		vec![],
	));

	ExtBuilder::default()
		.initial_balance(10_000 * TransactionBaseFee::get())
		.gas_price(1)
		.build()
		.execute_with(|| {
			let fee_exchange = FeeExchange::V1(FeeExchangeV1 {
				asset_id: CENTRAPAY_ASSET_ID,
				max_payment: 100 * gas_limit_amount,
			});

			initialize_block();
			let xt = sign(CheckedExtrinsic {
				signed: Some((alice(), signed_extra(0, 0, None, Some(fee_exchange)))),
				function: contract_call,
			});
			assert_eq!(
				Executive::apply_extrinsic(xt),
				Err(InvalidTransaction::Custom(ASSET_CANNOT_SWAP_FOR_ITSELF).into())
			);
		});
}
