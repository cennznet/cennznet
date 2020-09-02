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

use cennznet_primitives::types::{AccountId, Balance, BlockNumber, DigestItem, FeeExchange, FeeExchangeV1};
use cennznet_runtime::{
	constants::{asset::*, currency::*},
	sylo_e2ee, sylo_groups, sylo_inbox, sylo_response, sylo_vault, Babe, Call, CennzxSpot, CheckedExtrinsic,
	ContractTransactionBaseFee, EpochDuration, Event, Executive, GenericAsset, Header, ImOnline, Origin, Runtime,
	Session, SessionsPerEra, Staking, SyloPayment, System, Timestamp, TransactionBaseFee, TransactionByteFee,
	TransactionMaxWeightFee, TransactionPayment, UncheckedExtrinsic,
};
use cennznet_testing::keyring::*;
use codec::Encode;
use crml_staking::{EraIndex, RewardDestination, StakingLedger};
use crml_transaction_payment::{constants::error_code::*, ChargeTransactionPayment};
use frame_support::{
	additional_traits::MultiCurrencyAccounting as MultiCurrency,
	assert_ok,
	storage::StorageValue,
	traits::OnInitialize,
	weights::{DispatchClass, DispatchInfo, GetDispatchInfo},
};
use frame_system::{EventRecord, Phase, RawOrigin};
use pallet_contracts::{ContractAddressFor, RawEvent, Schedule};
use sp_consensus_babe::{digests, AuthorityIndex, BABE_ENGINE_ID};
use sp_runtime::{
	testing::Digest,
	traits::{Convert, Hash, Header as HeaderT},
	transaction_validity::{InvalidTransaction, TransactionValidityError},
	DispatchError, Perbill,
};
use sp_staking::{offence::OnOffenceHandler, SessionIndex};

mod doughnut;
mod mock;
use mock::{validators, ExtBuilder};

const GENESIS_HASH: [u8; 32] = [69u8; 32];
const VERSION: u32 = cennznet_runtime::VERSION.spec_version;

fn header_for_block_number(n: BlockNumber) -> Header {
	HeaderT::new(
		n,                        // block number
		sp_core::H256::default(), // extrinsics_root
		sp_core::H256::default(), // state_root
		GENESIS_HASH.into(),      // parent_hash
		Digest::default(),        // digest
	)
}

fn header() -> Header {
	header_for_block_number(1)
}

/// Get a block header and set the author of that block in a way that is recognisable by BABE.
/// The author will be specified by its index in the Session::validators() list. So the author
/// should be a current validator. Return the modified header.
fn set_author(mut header: Header, author_index: AuthorityIndex) -> Header {
	use digests::{RawPreDigest, SecondaryPreDigest};

	let digest_data = RawPreDigest::<(), ()>::Secondary(SecondaryPreDigest {
		authority_index: author_index,
		slot_number: Babe::current_slot(),
	});

	let digest = header.digest_mut();
	digest
		.logs
		.push(DigestItem::PreRuntime(BABE_ENGINE_ID, digest_data.encode()));

	header
}

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

/// Send heartbeats for the current authorities
fn send_heartbeats() {
	for i in 0..Session::validators().len() {
		let heartbeat_data = pallet_im_online::Heartbeat {
			block_number: System::block_number(),
			network_state: Default::default(),
			session_index: Session::current_index(),
			authority_index: i as u32,
		};
		let call = pallet_im_online::Call::heartbeat(heartbeat_data, Default::default());
		ImOnline::dispatch(call, RawOrigin::None.into()).unwrap();
	}
}

/// Prior to rotating to a new session, we should make sure the authority heartbeats are sent to the
/// ImOnline module, time is set accordingly and the babe's current slot is adjusted
fn pre_rotate_session() {
	send_heartbeats();
	Timestamp::set_timestamp(Timestamp::now() + 1000);
	pallet_babe::CurrentSlot::put(Babe::current_slot() + EpochDuration::get());
}

fn rotate_to_session(index: SessionIndex) {
	assert!(Session::current_index() <= index);
	Session::on_initialize(System::block_number());

	let rotations = index - Session::current_index();
	for _i in 0..rotations {
		pre_rotate_session();
		Session::rotate_session();
	}
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
		// TODO Untie the block number from the session index as they are independet concepts.
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

/// Calculate the transaction fees of `xt` according to the current runtime implementation.
/// Ignores tip.
fn get_extrinsic_fee(xt: &UncheckedExtrinsic) -> Balance {
	ChargeTransactionPayment::<Runtime>::compute_fee(xt.encode().len() as u32, xt.get_dispatch_info(), 0)
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
	let initial_balance = 10_000 * DOLLARS;
	let mut total_transfer_fee: Balance = 0;

	let runtime_call_1 = Call::GenericAsset(pallet_generic_asset::Call::transfer(CENTRAPAY_ASSET_ID, bob(), 123));
	let runtime_call_2 = Call::GenericAsset(pallet_generic_asset::Call::transfer(CENTRAPAY_ASSET_ID, charlie(), 456));

	ExtBuilder::default()
		.initial_balance(initial_balance)
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
				// Check validator is included in current elected accounts
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
	let balance_amount = 100_000_000 * DOLLARS;
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
			assert_eq!(total_payout, 27_900);
			assert_eq!(inflation_era_1, 74_400);

			// Compute staking reward for each validator
			let validator_len = validators.len() as Balance;
			let per_staking_reward = total_payout / validator_len;

			// validators should receive staking reward after new era
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
			assert_eq!(total_payout, 71_100);
			assert_eq!(inflation_era_2, 189_600);

			// validators should receive staking reward after new era
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
	let balance_amount = 100_000_000 * DOLLARS;
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
			let issued_fee_reward = per_fee_reward * validator_len; // Don't use "fee" itself directly
			let (staking_payout, max_payout) = Staking::current_total_payout(total_issuance + issued_fee_reward);
			let per_staking_reward = staking_payout / validator_len;

			// Check total issuance of Spending Asset updated after new era
			assert_eq!(
				GenericAsset::total_issuance(CENTRAPAY_ASSET_ID),
				total_issuance + max_payout + issued_fee_reward,
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
/// This tests if authorship reward of the last block in an era is factored in.
fn authorship_reward_of_last_block_in_an_era() {
	let validator_count = 6;
	let initial_balance = 1_000 * DOLLARS;

	ExtBuilder::default()
		.validator_count(validator_count)
		.initial_balance(initial_balance)
		.stash(initial_balance)
		.build()
		.execute_with(|| {
			let final_session_of_era_index = SessionsPerEra::get() - 1;
			rotate_to_session(final_session_of_era_index);

			// The final session falls in the era 0
			assert_eq!(Staking::current_era(), 0);

			// Make sure we have the correct number of validators elected
			assert_eq!(Staking::current_elected().len(), validator_count);

			// Make a block header whose author is specified as below
			let author_index = 0; // index 0 of validators
			let first_block_of_era_1 = System::block_number() + 1;
			let header_of_last_block = header_for_block_number(first_block_of_era_1.into());
			let header = set_author(header_of_last_block, author_index.clone());

			let author_stash_id = Session::validators()[(author_index as usize)].clone();

			// The previous session should come to its end
			pallet_babe::CurrentSlot::put(Babe::current_slot() + EpochDuration::get());

			send_heartbeats();

			let author_stash_balance_before_adding_block =
				GenericAsset::free_balance(&SPENDING_ASSET_ID, &author_stash_id);

			// Let's go through the first stage of executing the block
			Executive::initialize_block(&header);

			// initializing the last block should have advanced the session and thus changed the era
			assert_eq!(Staking::current_era(), 1);

			// No offences should happened. Thus the number of validators shouldn't have changed
			assert_eq!(Staking::current_elected().len(), validator_count);

			// There should be a reward calculated for the author
			assert!(
				GenericAsset::free_balance(&SPENDING_ASSET_ID, &author_stash_id)
					> author_stash_balance_before_adding_block
			);
		});
}

#[test]
/// This tests if authorship reward of the last block in an era is factored in, even when the author
/// is chilled and thus not going to be an authority in the next era.
fn authorship_reward_of_a_chilled_validator() {
	let validator_count = 6;
	let initial_balance = 1_000 * DOLLARS;

	ExtBuilder::default()
		.validator_count(validator_count)
		.initial_balance(initial_balance)
		.stash(initial_balance)
		.build()
		.execute_with(|| {
			let final_session_of_era_index = SessionsPerEra::get() - 1;
			rotate_to_session(final_session_of_era_index);

			// The last session falls in the era 0
			assert_eq!(Staking::current_era(), 0);

			// make sure we have the correct number of validators elected
			assert_eq!(Staking::current_elected().len(), validator_count);

			// Make a block header whose author is specified as below
			let author_index = 0; // index 0 of validators
			let first_block_of_era_1 = System::block_number() + 1;
			let header_of_last_block = header_for_block_number(first_block_of_era_1.into());
			let header = set_author(header_of_last_block, author_index.clone());

			let author_stash_id = Session::validators()[(author_index as usize)].clone();

			// Report an offence for the author of the block that is going to be initialised
			<Runtime as pallet_offences::Trait>::OnOffenceHandler::on_offence(
				&[sp_staking::offence::OffenceDetails {
					offender: (author_stash_id.clone(), Staking::stakers(&author_stash_id)),
					reporters: vec![],
				}],
				&[Perbill::from_percent(0)],
				Session::current_index(),
			);

			// The previous session should come to its end
			pallet_babe::CurrentSlot::put(Babe::current_slot() + EpochDuration::get());

			send_heartbeats();

			let author_stash_balance_before_adding_block =
				GenericAsset::free_balance(&SPENDING_ASSET_ID, &author_stash_id);

			// Let's go through the first stage of executing the block
			Executive::initialize_block(&header);

			// initializing the last block should have advanced the session and thus changed the era
			assert_eq!(Staking::current_era(), 1);

			// If the offended validator is chilled, in the new era, there should be one less elected validators than before
			assert_eq!(Staking::current_elected().len(), validator_count - 1);

			// There should be a reward calculated for the author even though the author is chilled
			assert!(
				GenericAsset::free_balance(&SPENDING_ASSET_ID, &author_stash_id)
					> author_stash_balance_before_adding_block
			);
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

fn apply_extrinsic(origin: AccountId, call: Call) -> Balance {
	let xt = sign(CheckedExtrinsic {
		signed: Some((origin, signed_extra(0, 0, None, None))),
		function: call.clone(),
	});

	let fee = transfer_fee(&xt, &call);

	Executive::initialize_block(&header());
	let r = Executive::apply_extrinsic(xt);
	assert!(r.is_ok());

	fee
}

#[test]
fn non_sylo_call_is_not_paid_by_payment_account() {
	let call = Call::GenericAsset(pallet_generic_asset::Call::transfer(CENTRAPAY_ASSET_ID, dave(), 100));

	ExtBuilder::default()
		.initial_balance(TransactionMaxWeightFee::get())
		.build()
		.execute_with(|| {
			assert_ok!(SyloPayment::set_payment_account(Origin::ROOT, bob()));

			let fee_asset_id = Some(GenericAsset::spending_asset_id());
			let bob_balance = <GenericAsset as MultiCurrency>::free_balance(&bob(), fee_asset_id.clone());

			let _ = apply_extrinsic(charlie(), call);

			let bob_balance_after_calls = <GenericAsset as MultiCurrency>::free_balance(&bob(), fee_asset_id);
			assert_eq!(bob_balance_after_calls, bob_balance);
		});
}

#[test]
fn sylo_e2ee_call_is_paid_by_payment_account() {
	let call = Call::SyloE2EE(sylo_e2ee::Call::register_device(1, vec![]));

	ExtBuilder::default()
		.initial_balance(TransactionMaxWeightFee::get())
		.build()
		.execute_with(|| {
			assert_ok!(SyloPayment::set_payment_account(Origin::ROOT, bob()));

			let fee_asset_id = Some(GenericAsset::spending_asset_id());
			let bob_balance = <GenericAsset as MultiCurrency>::free_balance(&bob(), fee_asset_id.clone());

			let call_fee = apply_extrinsic(charlie(), call);

			let bob_balance_after_calls = <GenericAsset as MultiCurrency>::free_balance(&bob(), fee_asset_id);
			assert_eq!(bob_balance_after_calls, bob_balance - call_fee);
		});
}

#[test]
fn sylo_inbox_call_is_paid_by_payment_account() {
	let call = Call::SyloInbox(sylo_inbox::Call::add_value(dave(), b"dude!".to_vec()));

	ExtBuilder::default()
		.initial_balance(TransactionMaxWeightFee::get())
		.build()
		.execute_with(|| {
			assert_ok!(SyloPayment::set_payment_account(Origin::ROOT, bob()));

			let fee_asset_id = Some(GenericAsset::spending_asset_id());
			let bob_balance = <GenericAsset as MultiCurrency>::free_balance(&bob(), fee_asset_id.clone());

			let call_fee = apply_extrinsic(charlie(), call);

			let bob_balance_after_calls = <GenericAsset as MultiCurrency>::free_balance(&bob(), fee_asset_id);
			assert_eq!(bob_balance_after_calls, bob_balance - call_fee);
		});
}

#[test]
fn sylo_vault_call_is_paid_by_payment_account() {
	let call = Call::SyloVault(sylo_vault::Call::upsert_value(b"key".to_vec(), b"value".to_vec()));

	ExtBuilder::default()
		.initial_balance(TransactionMaxWeightFee::get())
		.build()
		.execute_with(|| {
			assert_ok!(SyloPayment::set_payment_account(Origin::ROOT, bob()));

			let fee_asset_id = Some(GenericAsset::spending_asset_id());
			let bob_balance = <GenericAsset as MultiCurrency>::free_balance(&bob(), fee_asset_id.clone());

			let call_fee = apply_extrinsic(charlie(), call);

			let bob_balance_after_calls = <GenericAsset as MultiCurrency>::free_balance(&bob(), fee_asset_id);
			assert_eq!(bob_balance_after_calls, bob_balance - call_fee);
		});
}

#[test]
fn sylo_response_call_is_paid_by_payment_account() {
	let call = Call::SyloResponse(sylo_response::Call::remove_response([0u8; 32].into()));

	ExtBuilder::default()
		.initial_balance(TransactionMaxWeightFee::get())
		.build()
		.execute_with(|| {
			assert_ok!(SyloPayment::set_payment_account(Origin::ROOT, bob()));

			let fee_asset_id = Some(GenericAsset::spending_asset_id());
			let bob_balance = <GenericAsset as MultiCurrency>::free_balance(&bob(), fee_asset_id.clone());

			let call_fee = apply_extrinsic(charlie(), call);

			let bob_balance_after_calls = <GenericAsset as MultiCurrency>::free_balance(&bob(), fee_asset_id);
			assert_eq!(bob_balance_after_calls, bob_balance - call_fee);
		});
}

#[test]
fn sylo_groups_call_is_paid_by_payment_account() {
	let meta = vec![(b"key".to_vec(), b"value".to_vec())];
	let call = Call::SyloGroups(sylo_groups::Call::create_group(
		[1u8; 32].into(),
		meta,
		vec![],
		(b"group".to_vec(), b"data".to_vec()),
	));

	ExtBuilder::default()
		.initial_balance(TransactionMaxWeightFee::get())
		.build()
		.execute_with(|| {
			assert_ok!(SyloPayment::set_payment_account(Origin::ROOT, bob()));

			let fee_asset_id = Some(GenericAsset::spending_asset_id());
			let bob_balance = <GenericAsset as MultiCurrency>::free_balance(&bob(), fee_asset_id.clone());

			let call_fee = apply_extrinsic(charlie(), call);

			let bob_balance_after_calls = <GenericAsset as MultiCurrency>::free_balance(&bob(), fee_asset_id);
			assert_eq!(bob_balance_after_calls, bob_balance - call_fee);
		});
}

#[test]
fn generic_asset_transfer_works_without_fee_exchange() {
	let initial_balance = 5 * DOLLARS;
	let transfer_amount = 7777 * MICROS;
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
				function: runtime_call.clone(),
			});

			Executive::initialize_block(&header());
			let r = Executive::apply_extrinsic(xt.clone());
			assert!(r.is_ok());

			assert_eq!(
				<GenericAsset as MultiCurrency>::free_balance(&alice(), Some(CENTRAPAY_ASSET_ID)),
				initial_balance - transfer_amount - get_extrinsic_fee(&xt)
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
				function: runtime_call.clone(),
			});

			// Calculate how much CENNZ should be sold to make the above extrinsic
			let cennz_sold_amount =
				CennzxSpot::get_asset_to_core_buy_price(&CENNZ_ASSET_ID, get_extrinsic_fee(&xt)).unwrap();
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
			let (contract_address, _) = setup_contract(mock::contracts::CONTRACT_WITH_TRAP, dave());

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
			let (contract_address, code_hash) = setup_contract(mock::contracts::CONTRACT_WITH_GA_TRANSFER, alice());

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
						get_extrinsic_fee(&contract_call_extrinsic),
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
					- get_extrinsic_fee(&xt)
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
				CennzxSpot::get_asset_to_core_buy_price(&CENNZ_ASSET_ID, get_extrinsic_fee(&xt)).unwrap();

			Executive::initialize_block(&header());
			let r = Executive::apply_extrinsic(xt.clone());
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
	let cennznut = doughnut::make_runtime_cennznut("generic-asset", "transfer");
	let doughnut = doughnut::make_doughnut("cennznet", cennznut.encode());

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
				function: runtime_call.clone(),
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
				balance_amount - get_extrinsic_fee(&xt), // Bob pays transaction fees
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
	let cennznut = doughnut::make_runtime_cennznut("attestation", "attest");
	let doughnut = doughnut::make_doughnut("cennznet", cennznut.encode());

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
				function: runtime_call.clone(),
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
				initial_balance - get_extrinsic_fee(&xt), // Bob pays transaction fees
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
	let cennznut = doughnut::make_runtime_cennznut("generic-asset", "transfer");
	let doughnut = doughnut::make_doughnut("cennznet", cennznut.encode());

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
				function: runtime_call.clone(),
			});

			// Check CENNZ fee price
			let cennz_sold_amount =
				CennzxSpot::get_asset_to_core_buy_price(&CENNZ_ASSET_ID, get_extrinsic_fee(&xt)).unwrap();
			assert_eq!(cennz_sold_amount, 14_161 * MICROS); // 1.4161 CPAY

			// Initialise block and apply the extrinsic
			Executive::initialize_block(&header());
			let r = Executive::apply_extrinsic(xt.clone());
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
	let cennznut = doughnut::make_contract_cennznut(&charlie());
	let doughnut = doughnut::make_doughnut("cennznet", cennznut.encode());

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
				initial_balance - get_extrinsic_fee(&xt), // Bob pays transaction fees
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
	let cennznut = doughnut::make_contract_cennznut(&charlie());
	let doughnut = doughnut::make_doughnut("cennznet", cennznut.encode());

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
	let cennznut = doughnut::make_contract_cennznut(&charlie());
	let doughnut = doughnut::make_doughnut("cennznet", cennznut.encode());

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
				initial_balance - get_extrinsic_fee(&xt),
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
