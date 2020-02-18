#![cfg(test)]
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

mod mock;
use mock::ExtBuilder;

const GENESIS_HASH: [u8; 32] = [69u8; 32];
const VERSION: u32 = cennznet_runtime::VERSION.spec_version;

fn sign(xt: CheckedExtrinsic) -> UncheckedExtrinsic {
	cennznet_testing::keyring::sign(xt, VERSION, GENESIS_HASH)
}

fn transfer_fee<E: Encode>(extrinsic: &E, fee_multiplier: Fixed64, contract_call: &Call) -> Balance {
	let length_fee = TransactionByteFee::get() * (extrinsic.encode().len() as Balance);

	let weight = contract_call.get_dispatch_info().weight;
	let weight_fee = <Runtime as crml_transaction_payment::Trait>::WeightToFee::convert(weight);

	let base_fee = TransactionBaseFee::get();
	base_fee + fee_multiplier.saturated_multiply_accumulate(length_fee + weight_fee)
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

		Executive::initialize_block(&Header::new(
			1,                        // block number
			sp_core::H256::default(), // extrinsics_root
			sp_core::H256::default(), // state_root
			GENESIS_HASH.into(),      // parent_hash
			Digest::default(),        // digest
		));
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
