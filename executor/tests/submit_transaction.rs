// Copyright 2018-2020 Parity Technologies (UK) Ltd.
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

use cennznet_runtime::{
	constants::asset::SPENDING_ASSET_ID, Call, Executive, GenericAsset, Runtime, SubmitTransaction, UncheckedExtrinsic,
};

use codec::Decode;
use frame_support::additional_traits::MultiCurrencyAccounting;
use frame_system::offchain::{SubmitSignedTransaction, SubmitUnsignedTransaction};
use pallet_im_online::sr25519::AuthorityPair as Key;
use sp_application_crypto::AppKey;
use sp_core::offchain::{testing::TestTransactionPoolExt, TransactionPoolExt};
use sp_core::testing::KeyStore;
use sp_core::traits::KeystoreExt;

pub mod common;
use self::common::*;

#[test]
fn should_submit_unsigned_transaction() {
	let mut t = new_test_ext(COMPACT_CODE, false);
	let (pool, state) = TestTransactionPoolExt::new();
	t.register_extension(TransactionPoolExt::new(pool));

	t.execute_with(|| {
		let signature = Default::default();
		let heartbeat_data = pallet_im_online::Heartbeat {
			block_number: 1,
			network_state: Default::default(),
			session_index: 1,
			authority_index: 0,
		};

		let call = pallet_im_online::Call::heartbeat(heartbeat_data, signature);
		<SubmitTransaction as SubmitUnsignedTransaction<Runtime, Call>>::submit_unsigned(call).unwrap();

		assert_eq!(state.read().transactions.len(), 1)
	});
}

const PHRASE: &str = "news slush supreme milk chapter athlete soap sausage put clutch what kitten";

#[test]
fn should_submit_signed_transaction() {
	let mut t = new_test_ext(COMPACT_CODE, false);
	let (pool, state) = TestTransactionPoolExt::new();
	t.register_extension(TransactionPoolExt::new(pool));

	let keystore = KeyStore::new();
	keystore
		.write()
		.sr25519_generate_new(Key::ID, Some(&format!("{}/hunter1", PHRASE)))
		.unwrap();
	keystore
		.write()
		.sr25519_generate_new(Key::ID, Some(&format!("{}/hunter2", PHRASE)))
		.unwrap();
	keystore
		.write()
		.sr25519_generate_new(Key::ID, Some(&format!("{}/hunter3", PHRASE)))
		.unwrap();
	t.register_extension(KeystoreExt(keystore));

	t.execute_with(|| {
		let keys = <SubmitTransaction as SubmitSignedTransaction<Runtime, Call>>::find_all_local_keys();
		assert_eq!(keys.len(), 3, "Missing keys: {:?}", keys);

		let can_sign = <SubmitTransaction as SubmitSignedTransaction<Runtime, Call>>::can_sign();
		assert!(can_sign, "Since there are keys, `can_sign` should return true");

		let call = pallet_generic_asset::Call::transfer(SPENDING_ASSET_ID, Default::default(), Default::default());
		let results = <SubmitTransaction as SubmitSignedTransaction<Runtime, Call>>::submit_signed(call);

		let len = results.len();
		assert_eq!(len, 3);
		assert_eq!(results.into_iter().filter_map(|x| x.1.ok()).count(), len);
		assert_eq!(state.read().transactions.len(), len);
	});
}

#[test]
fn should_submit_signed_twice_from_the_same_account() {
	let mut t = new_test_ext(COMPACT_CODE, false);
	let (pool, state) = TestTransactionPoolExt::new();
	t.register_extension(TransactionPoolExt::new(pool));

	let keystore = KeyStore::new();
	keystore
		.write()
		.sr25519_generate_new(Key::ID, Some(&format!("{}/hunter1", PHRASE)))
		.unwrap();
	t.register_extension(KeystoreExt(keystore));

	t.execute_with(|| {
		let call = pallet_generic_asset::Call::transfer(SPENDING_ASSET_ID, Default::default(), Default::default());
		let results = <SubmitTransaction as SubmitSignedTransaction<Runtime, Call>>::submit_signed(call);

		let len = results.len();
		assert_eq!(len, 1);
		assert_eq!(results.into_iter().filter_map(|x| x.1.ok()).count(), len);
		assert_eq!(state.read().transactions.len(), 1);

		// submit another one from the same account. The nonce should be incremented.
		let call = pallet_generic_asset::Call::transfer(SPENDING_ASSET_ID, Default::default(), Default::default());
		let results = <SubmitTransaction as SubmitSignedTransaction<Runtime, Call>>::submit_signed(call);

		let len = results.len();
		assert_eq!(len, 1);
		assert_eq!(results.into_iter().filter_map(|x| x.1.ok()).count(), len);
		assert_eq!(state.read().transactions.len(), 2);

		// now check that the transaction nonces are not equal
		let s = state.read();
		fn nonce(tx: UncheckedExtrinsic) -> frame_system::CheckNonce<Runtime> {
			let extra = tx.signature.unwrap().2;
			extra.4
		}
		let nonce1 = nonce(UncheckedExtrinsic::decode(&mut &*s.transactions[0]).unwrap());
		let nonce2 = nonce(UncheckedExtrinsic::decode(&mut &*s.transactions[1]).unwrap());
		assert!(
			nonce1 != nonce2,
			"Transactions should have different nonces. Got: {:?}",
			nonce1
		);
	});
}

#[test]
fn submitted_transaction_should_be_valid() {
	use codec::Encode;
	use sp_runtime::transaction_validity::ValidTransaction;

	let mut t = new_test_ext(COMPACT_CODE, false);
	let (pool, state) = TestTransactionPoolExt::new();
	t.register_extension(TransactionPoolExt::new(pool));

	let keystore = KeyStore::new();
	keystore
		.write()
		.sr25519_generate_new(Key::ID, Some(&format!("{}/hunter1", PHRASE)))
		.unwrap();
	t.register_extension(KeystoreExt(keystore));

	t.execute_with(|| {
		let call = pallet_generic_asset::Call::transfer(SPENDING_ASSET_ID, Default::default(), Default::default());
		let results = <SubmitTransaction as SubmitSignedTransaction<Runtime, Call>>::submit_signed(call);
		let len = results.len();
		assert_eq!(len, 1);
		assert_eq!(results.into_iter().filter_map(|x| x.1.ok()).count(), len);
	});

	// check that transaction is valid, but reset environment storage,
	// since CreateTransaction increments the nonce
	let tx0 = state.read().transactions[0].clone();
	let mut t = new_test_ext(COMPACT_CODE, false);
	t.execute_with(|| {
		let extrinsic = UncheckedExtrinsic::decode(&mut &*tx0).unwrap();
		// add balance to the account
		let author = extrinsic.signature.clone().unwrap().0;
		<GenericAsset as MultiCurrencyAccounting>::make_free_balance_be(
			&author,
			Some(SPENDING_ASSET_ID),
			5_000_000_000_000,
		);

		// check validity
		let res = Executive::validate_transaction(extrinsic);

		assert_eq!(
			res.unwrap(),
			ValidTransaction {
				// This has changed from the substrate value `2_411_002_000_000`
				// TRANSACTION_BYTE_FEE = 10_000_000_000
				// - 10_000_000_000, Indices byte removed from balances `dest` address
				// - 10_000_000_000, Indices byte removed from address
				// + 10_000_000_000, Add Doughnut to SignedExtra
				priority: 2_401_002_000_000,
				requires: vec![],
				provides: vec![(author, 0).encode()],
				longevity: 127,
				propagate: true,
			}
		);
	});
}
