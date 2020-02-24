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

use cennznet_primitives::types::{Balance, FeeExchange, FeeExchangeV1};
use cennznet_runtime::{
	constants::asset::*, Call, CennzxSpot, CheckedExtrinsic, Executive, GenericAsset, Origin, Runtime,
	TransactionBaseFee, TransactionByteFee, TransactionPayment, UncheckedExtrinsic,
};
use cennznet_testing::keyring::*;
use codec::Encode;
use frame_support::{additional_traits::MultiCurrencyAccounting, traits::Imbalance, weights::GetDispatchInfo};
use sp_runtime::{
	testing::Digest,
	traits::{Convert, Header},
	transaction_validity::InvalidTransaction,
	Fixed64,
};

mod doughnut;
mod mock;
use mock::ExtBuilder;

const GENESIS_HASH: [u8; 32] = [69u8; 32];
const VERSION: u32 = cennznet_runtime::VERSION.spec_version;

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

fn initialize_block() {
	Executive::initialize_block(&Header::new(
		1,                        // block number
		sp_core::H256::default(), // extrinsics_root
		sp_core::H256::default(), // state_root
		GENESIS_HASH.into(),      // parent_hash
		Digest::default(),        // digest
	));
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
		for (account, balance) in tests.clone() {
			for asset in assets.clone() {
				assert_eq!(
					<GenericAsset as MultiCurrencyAccounting>::free_balance(&account, Some(asset)),
					balance,
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
	// Contract itself fails
}

#[test]
fn contract_fails_with_insufficient_gas() {
	// Not enough gas to run contract
}

#[test]
fn contract_call_works_without_fee_exchange() {
	// Happy case with no fee exchange
	// Contract changes users account assets
}

#[test]
fn contract_call_works_with_fee_exchange() {
	// Happy case with fee exchange (with/without excess funds)
	// Fee exchange is asking for CPay
	// Contract makes an extrinsic to the exchange
	// Contract changes users account assets
}

#[test]
fn contract_call_fails_when_fee_exchange_is_not_enough_for_gas() {
	// Fee exchange not enough to pay for gas
	// validate() should early terminate?
}

#[test]
fn contract_call_fails_when_exchange_liquidity_is_low() {
	// Exchange doesn’t have sufficient liquidity
}
