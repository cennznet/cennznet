// Copyright 2018-2020 Parity Technologies (UK) Ltd. and Centrality Investments Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

//! fee multiplier vs. block fullness integration tests

use codec::{Encode, Joiner};
use frame_support::{storage::StorageDoubleMap, weights::GetDispatchInfo, StorageMap, StorageValue};
use sp_core::{map, storage::Storage, NeverNativeValue};
use sp_runtime::{
	traits::{BlakeTwo256, Convert},
	Fixed64, Perbill,
};

use cennznet_primitives::types::Balance;
use cennznet_runtime::impls::LinearWeightToFee;
use cennznet_runtime::{
	constants::asset::SPENDING_ASSET_ID, constants::currency::*, Call, CheckedExtrinsic, GenericAsset, Runtime,
	TransactionBaseFee, TransactionByteFee, TransactionPayment, WeightFeeCoefficient,
};
use cennznet_testing::keyring::*;

pub mod common;
use self::common::{sign, *};

#[test]
fn fee_multiplier_increases_and_decreases_on_big_weight() {
	let mut t = new_test_ext(COMPACT_CODE, false);

	// initial fee multiplier must be zero
	let mut prev_multiplier = Fixed64::from_parts(0);

	t.execute_with(|| {
		assert_eq!(TransactionPayment::next_fee_multiplier(), prev_multiplier);
	});

	let mut tt = new_test_ext(COMPACT_CODE, false);

	// big one in terms of weight.
	let block1 = construct_block(
		&mut tt,
		1,
		GENESIS_HASH.into(),
		vec![
			CheckedExtrinsic {
				signed: None,
				function: Call::Timestamp(pallet_timestamp::Call::set(42 * 1000)),
			},
			CheckedExtrinsic {
				signed: Some((charlie(), signed_extra(0, 0, None, None))),
				function: Call::System(frame_system::Call::fill_block(Perbill::from_percent(90))),
			},
		],
	);

	// small one in terms of weight.
	let block2 = construct_block(
		&mut tt,
		2,
		block1.1.clone(),
		vec![
			CheckedExtrinsic {
				signed: None,
				function: Call::Timestamp(pallet_timestamp::Call::set(52 * 1000)),
			},
			CheckedExtrinsic {
				signed: Some((charlie(), signed_extra(1, 0, None, None))),
				function: Call::System(frame_system::Call::remark(vec![0; 1])),
			},
		],
	);

	println!(
		"++ Block 1 size: {} / Block 2 size {}",
		block1.0.encode().len(),
		block2.0.encode().len()
	);

	// execute a big block.
	executor_call::<NeverNativeValue, fn() -> _>(&mut t, "Core_execute_block", &block1.0, true, None)
		.0
		.unwrap();

	// weight multiplier is increased for next block.
	t.execute_with(|| {
		let fm = TransactionPayment::next_fee_multiplier();
		println!("After a big block: {:?} -> {:?}", prev_multiplier, fm);
		assert!(fm > prev_multiplier);
		prev_multiplier = fm;
	});

	// execute a big block.
	executor_call::<NeverNativeValue, fn() -> _>(&mut t, "Core_execute_block", &block2.0, true, None)
		.0
		.unwrap();

	// weight multiplier is increased for next block.
	t.execute_with(|| {
		let fm = TransactionPayment::next_fee_multiplier();
		println!("After a small block: {:?} -> {:?}", prev_multiplier, fm);
		assert!(fm < prev_multiplier);
	});
}

#[test]
fn transaction_fee_is_correct_ultimate() {
	// This uses the exact values of cennznet-node.
	//
	// weight of transfer call as of now: 1_000_000
	// if weight of the cheapest weight would be 10^7, this would be 10^9, which is:
	//   - 1 MILLICENTS in substrate node.
	//   - 1 milli-dot based on current polkadot runtime.
	// (this baed on assigning 0.1 CENT to the cheapest tx with `weight = 100`)
	let mut t = TestExternalities::<BlakeTwo256>::new_with_code(
		COMPACT_CODE,
		Storage {
			top: map![
				<pallet_generic_asset::SpendingAssetId<Runtime>>::hashed_key().to_vec() => {
					SPENDING_ASSET_ID.encode()
				},
				<pallet_generic_asset::FreeBalance<Runtime>>::hashed_key_for(&SPENDING_ASSET_ID, &alice()) => {
					(100 * DOLLARS, 0 * DOLLARS, 0 * DOLLARS, 0 * DOLLARS).encode()
				},
				<pallet_generic_asset::FreeBalance<Runtime>>::hashed_key_for(&SPENDING_ASSET_ID, &bob()) => {
					(10 * DOLLARS, 0 * DOLLARS, 0 * DOLLARS, 0 * DOLLARS).encode()
				},
				<pallet_generic_asset::TotalIssuance<Runtime>>::hashed_key_for(SPENDING_ASSET_ID) => {
					(110 * DOLLARS, 0 * DOLLARS, 0 * DOLLARS, 0 * DOLLARS).encode()
				},
				<frame_system::BlockHash<Runtime>>::hashed_key_for(0).to_vec() => vec![0u8; 32]
			],
			children: map![],
		},
	);

	let tip = 1_000_000;
	let xt = sign(CheckedExtrinsic {
		signed: Some((alice(), signed_extra(0, tip, None, None))),
		function: Call::GenericAsset(default_transfer_call()),
	});

	let r = executor_call::<NeverNativeValue, fn() -> _>(
		&mut t,
		"Core_initialize_block",
		&vec![].and(&from_block_number(1u32)),
		true,
		None,
	)
	.0;

	assert!(r.is_ok());
	let r = executor_call::<NeverNativeValue, fn() -> _>(
		&mut t,
		"BlockBuilder_apply_extrinsic",
		&vec![].and(&xt.clone()),
		true,
		None,
	)
	.0;
	assert!(r.is_ok());

	t.execute_with(|| {
		assert_eq!(
			GenericAsset::total_balance(&SPENDING_ASSET_ID, &bob()),
			(10 + 69) * DOLLARS
		);
		// Components deducted from alice's generic_asset:
		// - Weight fee
		// - Length fee
		// - Tip
		// - Creation-fee of bob's account.
		let mut balance_alice = (100 - 69) * DOLLARS;

		let length_fee = TransactionBaseFee::get() + TransactionByteFee::get() * (xt.clone().encode().len() as Balance);
		balance_alice -= length_fee;

		let weight = default_transfer_call().get_dispatch_info().weight;
		let weight_fee = LinearWeightToFee::<WeightFeeCoefficient>::convert(weight);

		// we know that weight to fee multiplier is effect-less in block 1.
		// generic assert uses default weight = 10_000, Balance set weight = 1_000_000
		// we can use #[weight = SimpleDispatchInfo::FixedNormal(1_000_000)] to config the weight
		assert_eq!(weight_fee as Balance, 10_000_000);
		balance_alice -= weight_fee;

		balance_alice -= tip;

		assert_eq!(GenericAsset::total_balance(&SPENDING_ASSET_ID, &alice()), balance_alice);
	});
}
