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

use cennznet_primitives::types::{AccountId, Balance, FeeExchange, FeeExchangeV1};
use cennznet_runtime::{
	constants::{asset::*, currency::*},
	Babe, Call, CennzxSpot, CheckedExtrinsic, ContractTransactionBaseFee, EpochDuration, Event, Executive,
	GenericAsset, Header, Origin, Runtime, Session, SessionsPerEra, Staking, System, Timestamp, TransactionBaseFee,
	TransactionByteFee, TransactionPayment, UncheckedExtrinsic,
};
use cennznet_testing::keyring::*;
use codec::Encode;
use crml_staking::{EraIndex, RewardDestination, StakingLedger};
use crml_transaction_payment::constants::error_code::*;
use frame_support::{
	additional_traits::MultiCurrencyAccounting as MultiCurrency,
	storage::StorageValue,
	traits::{Imbalance, OnInitialize},
	weights::{DispatchClass, DispatchInfo, GetDispatchInfo},
};
use frame_system::{EventRecord, Phase};
use pallet_contracts::{ContractAddressFor, RawEvent};
use sp_runtime::{
	testing::Digest,
	traits::{Convert, Hash, Header as HeaderT},
	transaction_validity::InvalidTransaction,
};
use sp_staking::SessionIndex;

mod doughnut;
mod mock;
use mock::{validators, ExtBuilder};

const GENESIS_HASH: [u8; 32] = [69u8; 32];
const VERSION: u32 = cennznet_runtime::VERSION.spec_version;

fn header() -> Header {
	HeaderT::new(
		1,                        // block number
		sp_core::H256::default(), // extrinsics_root
		sp_core::H256::default(), // state_root
		GENESIS_HASH.into(),      // parent_hash
		Digest::default(),        // digest
	)
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

	Executive::initialize_block(&header());

	let put_code_call = Call::Contracts(pallet_contracts::Call::put_code(50_000_000, wasm));
	let put_code_extrinsic = sign(CheckedExtrinsic {
		signed: Some((contract_deployer.clone(), signed_extra(0, 0, None, None))),
		function: put_code_call,
	});
	let r = Executive::apply_extrinsic(put_code_extrinsic);
	println!(
		"{:?}, CPAY Balance: {:?}",
		r,
		<GenericAsset as MultiCurrency>::free_balance(&alice(), Some(CENTRAPAY_ASSET_ID))
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
		<GenericAsset as MultiCurrency>::free_balance(&alice(), Some(CENTRAPAY_ASSET_ID))
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

fn transfer_fee<E: Encode>(extrinsic: &E, runtime_call: &Call) -> Balance {
	let length_fee = TransactionByteFee::get() * (extrinsic.encode().len() as Balance);

	let weight = runtime_call.get_dispatch_info().weight;
	let weight_fee = <Runtime as crml_transaction_payment::Trait>::WeightToFee::convert(weight);

	let base_fee = TransactionBaseFee::get();
	let fee_multiplier = TransactionPayment::next_fee_multiplier();

	base_fee + fee_multiplier.saturated_multiply_accumulate(length_fee + weight_fee)
}

fn start_session(session_index: SessionIndex) {
	// If we run the function for the first time, block_number is 1, which won't
	// trigger Babe::should_end_session() so we have to run one extra loop. But
	// successive calls don't need to run one extra loop. See Babe::should_epoch_change()
	let up_to_session_index = if Session::current_index() == 0 {
		session_index + 1
	} else {
		session_index
	};
	for i in Session::current_index()..up_to_session_index {
		System::set_block_number((i + 1).into());
		pallet_babe::CurrentSlot::put(Babe::current_slot() + EpochDuration::get());
		Timestamp::set_timestamp((System::block_number() * 1000).into());
		Session::on_initialize(System::block_number()); // this ends session
	}
	assert_eq!(Session::current_index(), session_index);
}

fn advance_session() {
	let current_index = Session::current_index();
	start_session(current_index + 1);
}

// Starts all sessions up to `era_index` (eg, start_era(2) will start 14 sessions)
fn start_era(era_index: EraIndex) {
	start_session((era_index * SessionsPerEra::get()).into());
	assert_eq!(Staking::current_era(), era_index);
}

fn reward_validators(validators: &[(AccountId, AccountId)]) {
	let validators_points = validators
		.iter()
		.map(|v| (v.0.clone(), 1))
		.collect::<Vec<(AccountId, u32)>>();
	Staking::reward_by_ids(validators_points);
}

#[test]
fn start_session_works() {
	ExtBuilder::default().build().execute_with(|| {
		start_session(1);
		start_session(3);
		start_session(5);
	});
}

#[test]
fn advance_session_works() {
	ExtBuilder::default().build().execute_with(|| {
		let session_index = 12;
		start_session(session_index);
		advance_session();
		advance_session();
		advance_session();
		assert_eq!(Session::current_index(), 15);
	});
}

#[test]
fn start_era_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Staking::current_era(), 0);
		start_era(1);
		assert_eq!(Staking::current_era(), 1);
		start_era(10);
		assert_eq!(Staking::current_era(), 10);
	});
}

// Test to show that every extrinsic applied will add transfer fee to
// CurrentEraFeeRewards (until it's paid out at the end of an era)
#[test]
fn current_era_transaction_rewards_storage_update_works() {
	let balance_amount = 10_000 * TransactionBaseFee::get();
	let mut total_transfer_fee: Balance = 0;

	let runtime_call_1 = Call::GenericAsset(pallet_generic_asset::Call::transfer(CENTRAPAY_ASSET_ID, bob(), 123));
	let runtime_call_2 = Call::GenericAsset(pallet_generic_asset::Call::transfer(CENTRAPAY_ASSET_ID, charlie(), 456));

	ExtBuilder::default()
		.initial_balance(balance_amount)
		.build()
		.execute_with(|| {
			let xt_1 = sign(CheckedExtrinsic {
				signed: Some((alice(), signed_extra(0, 0, None, None))),
				function: runtime_call_1.clone(),
			});
			let xt_2 = sign(CheckedExtrinsic {
				signed: Some((bob(), signed_extra(0, 0, None, None))),
				function: runtime_call_2.clone(),
			});

			Executive::initialize_block(&header());
			start_era(1);
			advance_session(); // advance a session to trigger the beginning of era 2
			assert_eq!(Staking::current_era(), 2);

			// Start with 0 transaction rewards
			assert_eq!(Staking::current_era_transaction_fee_reward(), 0);

			// Apply first extrinsic and check transaction rewards
			assert!(Executive::apply_extrinsic(xt_1.clone()).is_ok());
			total_transfer_fee += transfer_fee(&xt_1, &runtime_call_1);
			assert_eq!(Staking::current_era_transaction_fee_reward(), total_transfer_fee);

			// Apply second extrinsic and check transaction rewards
			assert!(Executive::apply_extrinsic(xt_2.clone()).is_ok());
			total_transfer_fee += transfer_fee(&xt_2, &runtime_call_2);
			assert_eq!(Staking::current_era_transaction_fee_reward(), total_transfer_fee);

			// Advancing sessions shouldn't change transaction rewards storage
			advance_session();
			assert_eq!(Staking::current_era_transaction_fee_reward(), total_transfer_fee);
			advance_session();
			assert_eq!(Staking::current_era_transaction_fee_reward(), total_transfer_fee);

			// At the start of the next era (13th session), transaction rewards should be cleared (and paid out)
			start_era(2);
			advance_session();
			assert_eq!(Staking::current_era(), 3);
			assert_eq!(Staking::current_era_transaction_fee_reward(), 0);
		});
}

#[test]
fn staking_genesis_config_works() {
	let validators = validators(6);
	let balance_amount = 10_000 * TransactionBaseFee::get();
	let staked_amount = balance_amount / 6;
	ExtBuilder::default()
		.initial_balance(balance_amount)
		.stash(staked_amount)
		.validator_count(validators.len())
		.build()
		.execute_with(|| {
			for validator in validators {
				let (stash, controller) = validator;
				// Check validator is included in currect elelcted accounts
				assert!(Staking::current_elected().contains(&stash));
				// Check that RewardDestination is Stash (default)
				assert_eq!(Staking::payee(&stash), RewardDestination::Stash);
				// Check validator free balance
				assert_eq!(
					<GenericAsset as MultiCurrency>::free_balance(&stash, Some(CENTRAPAY_ASSET_ID)),
					balance_amount
				);
				// Check how much is at stake
				assert_eq!(
					Staking::ledger(controller),
					Some(StakingLedger {
						stash,
						total: staked_amount,
						active: staked_amount,
						unlocking: vec![],
					})
				);
			}
		});
}

#[test]
fn staking_inflation_and_reward_should_work() {
	let balance_amount = 10_000 * TransactionBaseFee::get();
	let total_issuance = balance_amount * 12; // 6 pre-configured + 6 stash accounts
	let staked_amount = balance_amount / 6;
	let validators = validators(6);

	ExtBuilder::default()
		.initial_balance(balance_amount)
		.stash(staked_amount)
		.validator_count(validators.len())
		.build()
		.execute_with(|| {
			// Total issuance remains unchanged at era 0.
			start_session(0);
			assert_eq!(Staking::current_era(), 0);
			assert_eq!(GenericAsset::total_issuance(CENNZ_ASSET_ID), total_issuance);
			assert_eq!(GenericAsset::total_issuance(CENTRAPAY_ASSET_ID), total_issuance);
			// Add points to each validator which use to allocate staking reward in the next new era
			reward_validators(&validators);

			// Total issuance for CPAY is inflated at the start of era 1, and that for CENNZ is unchanged.
			start_session(1);
			assert_eq!(Staking::current_era(), 1);
			reward_validators(&validators);

			// Compute total payout and inflation for new era
			let (total_payout, inflation_era_1) = Staking::current_total_payout(total_issuance);
			assert_eq!(total_payout, 279_000_000);
			assert_eq!(inflation_era_1, 744_000_000);

			// Compute staking reward for each validator
			let validator_len = validators.len() as Balance;
			let per_staking_reward = total_payout / validator_len;

			// validators should receive skaking reward after new era
			for (stash, _) in &validators {
				assert_eq!(
					<GenericAsset as MultiCurrency>::free_balance(&stash, Some(CENTRAPAY_ASSET_ID)),
					balance_amount + per_staking_reward
				);
			}

			let sessions_era_1 = vec![2, 3, 4, 5, 6];
			for session in sessions_era_1 {
				start_session(session);
				assert_eq!(Staking::current_era(), 1);
				// Total issuance for CENNZ is unchanged
				assert_eq!(GenericAsset::total_issuance(CENNZ_ASSET_ID), total_issuance);
				// Total issuance for CPAY remain the same within the same era
				assert_eq!(
					GenericAsset::total_issuance(CENTRAPAY_ASSET_ID),
					total_issuance + inflation_era_1
				);

				// The balance of stash accounts remain the same within the same era
				for (stash, _) in &validators {
					assert_eq!(
						<GenericAsset as MultiCurrency>::free_balance(&stash, Some(CENTRAPAY_ASSET_ID)),
						balance_amount + per_staking_reward
					);
				}
			}

			// Total issuance for CPAY is inflated at the start of era 2, and that for CENNZ is unchanged.
			start_session(7);
			assert_eq!(Staking::current_era(), 2);

			let (total_payout, inflation_era_2) = Staking::current_total_payout(total_issuance + inflation_era_1);
			assert_eq!(total_payout, 711_000_003);
			assert_eq!(inflation_era_2, 1_896_000_012);

			// validators should receive skaking reward after new era
			let per_staking_reward = total_payout / validator_len + per_staking_reward;
			for (stash, _) in &validators {
				assert_eq!(
					<GenericAsset as MultiCurrency>::free_balance(&stash, Some(CENTRAPAY_ASSET_ID)),
					balance_amount + per_staking_reward
				);
			}

			let sessions_era_2 = vec![8, 9, 10, 11, 12];
			for session in sessions_era_2 {
				start_session(session);
				assert_eq!(Staking::current_era(), 2);
				// Total issuance for CENNZ is unchanged
				assert_eq!(GenericAsset::total_issuance(CENNZ_ASSET_ID), total_issuance);
				// Total issuance for CPAY remain the same within the same era
				assert_eq!(
					GenericAsset::total_issuance(CENTRAPAY_ASSET_ID),
					total_issuance + inflation_era_1 + inflation_era_2
				);

				// The balance of stash accounts remain the same within the same era
				for (stash, _) in &validators {
					assert_eq!(
						<GenericAsset as MultiCurrency>::free_balance(&stash, Some(CENTRAPAY_ASSET_ID)),
						balance_amount + per_staking_reward
					);
				}
			}
		});
}

#[test]
fn staking_validators_should_receive_equal_transaction_fee_reward() {
	let validators = validators(6);
	let balance_amount = 10_000 * TransactionBaseFee::get();
	let staked_amount = balance_amount / 6;
	let transfer_amount = 50;
	let runtime_call = Call::GenericAsset(pallet_generic_asset::Call::transfer(
		CENTRAPAY_ASSET_ID,
		bob(),
		transfer_amount,
	));

	ExtBuilder::default()
		.initial_balance(balance_amount)
		.validator_count(validators.len())
		.stash(staked_amount)
		.build()
		.execute_with(|| {
			let xt = sign(CheckedExtrinsic {
				signed: Some((alice(), signed_extra(0, 0, None, None))),
				function: runtime_call.clone(),
			});

			let fee = transfer_fee(&xt, &runtime_call);
			let per_fee_reward = fee / validators.len() as Balance;

			start_era(1);
			let validator_len = validators.len() as Balance;
			reward_validators(&validators);

			let r = Executive::apply_extrinsic(xt);
			assert!(r.is_ok());

			// Check if the transfer is successful
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&alice(), Some(CENTRAPAY_ASSET_ID)),
				balance_amount - transfer_amount - fee
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&bob(), Some(CENTRAPAY_ASSET_ID)),
				balance_amount + transfer_amount
			);

			// Check if stash account balances are not yet changed
			for (stash, _) in &validators {
				assert_eq!(
					<GenericAsset as MultiCurrency>::free_balance(&stash, Some(CENTRAPAY_ASSET_ID)),
					balance_amount
				);
			}

			let total_issuance = GenericAsset::total_issuance(CENTRAPAY_ASSET_ID);
			start_era(2);
			let (staking_payout, max_payout) = Staking::current_total_payout(total_issuance);
			let per_staking_reward = staking_payout / validator_len;

			// Check total issuance of Spending Asset updatd after new era
			assert_eq!(
				GenericAsset::total_issuance(CENTRAPAY_ASSET_ID),
				total_issuance + max_payout,
			);

			// Check if validator balance changed correctly
			for validator in validators {
				let (stash, _) = validator;
				// Check tx fee reward went to the stash account of validator
				assert_eq!(
					<GenericAsset as MultiCurrency>::free_balance(&stash, Some(CENTRAPAY_ASSET_ID)),
					balance_amount + per_fee_reward + per_staking_reward
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
		for asset in &assets {
			for (account, balance) in &tests {
				assert_eq!(
					<GenericAsset as MultiCurrency>::free_balance(&account, Some(*asset)),
					*balance,
				);
				assert_eq!(<GenericAsset as MultiCurrency>::free_balance(&account, Some(123)), 0,)
			}
			// NOTE: 9 = 6 pre-configured accounts + 3 ExtBuilder.validator_count (to generate stash accounts)
			assert_eq!(GenericAsset::total_issuance(asset), amount * 9);
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
			<GenericAsset as MultiCurrency>::free_balance(&alice(), Some(CENTRAPAY_ASSET_ID)),
			balance_amount
		);

		let xt = sign(CheckedExtrinsic {
			signed: Some((alice(), signed_extra(0, 0, None, None))),
			function: runtime_call.clone(),
		});

		let fee = transfer_fee(&xt, &runtime_call);

		Executive::initialize_block(&header());
		let r = Executive::apply_extrinsic(xt);
		assert!(r.is_ok());

		assert_eq!(
			<GenericAsset as MultiCurrency>::free_balance(&alice(), Some(CENTRAPAY_ASSET_ID)),
			balance_amount - transfer_amount - fee
		);
		assert_eq!(
			<GenericAsset as MultiCurrency>::free_balance(&bob(), Some(CENTRAPAY_ASSET_ID)),
			transfer_amount
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
			assert_eq!(CennzxSpot::liquidity_balance(&ex_key, &alice()), liquidity_core_amount);

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
			let fee = transfer_fee(&xt, &runtime_call);

			// Calculate how much CENNZ should be sold to make the above extrinsic
			let cennz_sold_amount = CennzxSpot::get_asset_to_core_buy_price(&CENNZ_ASSET_ID, fee).unwrap();
			assert_eq!(cennz_sold_amount, 6);

			// Initialise block and apply the extrinsic
			Executive::initialize_block(&header());
			let r = Executive::apply_extrinsic(xt);
			assert!(r.is_ok());

			// Check remaining balances
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&alice(), Some(CENNZ_ASSET_ID)),
				balance_amount - liquidity_asset_amount - cennz_sold_amount, // transfer fee is charged in CENNZ
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&alice(), Some(CENTRAPAY_ASSET_ID)),
				balance_amount - liquidity_core_amount - transfer_amount // transfer fee is not charged in CPAY
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&bob(), Some(CENTRAPAY_ASSET_ID)),
				balance_amount + transfer_amount
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
					event: Event::frame_system(frame_system::Event::ExtrinsicSuccess(DispatchInfo {
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
					event: Event::frame_system(frame_system::Event::ExtrinsicSuccess(DispatchInfo {
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
					event: Event::frame_system(frame_system::Event::ExtrinsicSuccess(DispatchInfo {
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
			assert_eq!(
				CennzxSpot::liquidity_balance(&ex_key, &charlie()),
				liquidity_core_amount
			);

			let fee_exchange = FeeExchange::V1(FeeExchangeV1 {
				asset_id: CENNZ_ASSET_ID,
				max_payment: 10 * TransactionBaseFee::get(),
			});

			Executive::initialize_block(&header());
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
				Err(InvalidTransaction::Custom(INSUFFICIENT_BALANCE).into())
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
			Executive::initialize_block(&header());
			let r = Executive::apply_extrinsic(xt);
			assert!(r.is_ok());

			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&bob(), Some(CENTRAPAY_ASSET_ID)),
				balance_amount + transfer_amount,
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&alice(), Some(CENTRAPAY_ASSET_ID)),
				9997559989999715,
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
			assert_eq!(
				CennzxSpot::liquidity_balance(&ex_key, &charlie()),
				liquidity_core_amount
			);

			let fee_exchange = FeeExchange::V1(FeeExchangeV1 {
				asset_id: CENNZ_ASSET_ID,
				max_payment: 100_000_000 * gas_limit_amount,
			});

			let xt = sign(CheckedExtrinsic {
				signed: Some((alice(), signed_extra(0, 0, None, Some(fee_exchange)))),
				function: contract_call,
			});
			Executive::initialize_block(&header());
			let r = Executive::apply_extrinsic(xt);
			assert!(r.is_ok());

			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&bob(), Some(CENTRAPAY_ASSET_ID)),
				balance_amount + transfer_amount
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&alice(), Some(CENTRAPAY_ASSET_ID)),
				balance_amount - transfer_amount
			);
			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&alice(), Some(CENNZ_ASSET_ID)),
				9999739653196821,
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
			assert_eq!(
				CennzxSpot::liquidity_balance(&ex_key, &charlie()),
				liquidity_core_amount
			);

			let fee_exchange = FeeExchange::V1(FeeExchangeV1 {
				asset_id: CENNZ_ASSET_ID,
				max_payment: 1,
			});

			let xt = sign(CheckedExtrinsic {
				signed: Some((alice(), signed_extra(0, 0, None, Some(fee_exchange)))),
				function: contract_call,
			});
			Executive::initialize_block(&header());
			assert_eq!(
				Executive::apply_extrinsic(xt),
				Err(InvalidTransaction::Custom(PRICE_ABOVE_MAX_LIMIT).into())
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
			assert_eq!(
				CennzxSpot::liquidity_balance(&ex_key, &charlie()),
				liquidity_core_amount
			);

			let fee_exchange = FeeExchange::V1(FeeExchangeV1 {
				asset_id: CENNZ_ASSET_ID,
				max_payment: 100_000_000 * gas_limit_amount,
			});

			let xt = sign(CheckedExtrinsic {
				signed: Some((alice(), signed_extra(0, 0, None, Some(fee_exchange)))),
				function: contract_call,
			});
			Executive::initialize_block(&header());
			assert_eq!(
				Executive::apply_extrinsic(xt),
				Err(InvalidTransaction::Custom(INSUFFICIENT_ASSET_RESERVE).into())
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

			Executive::initialize_block(&header());
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
