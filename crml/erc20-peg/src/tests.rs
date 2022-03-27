use super::*;
use crate::mock::{DepositEventSignature, Erc20Peg, ExtBuilder, GenericAsset, PegPalletId, Test};
use cennznet_primitives::types::AccountId;
use crml_support::MultiCurrency;
use frame_support::{
	assert_noop, assert_ok,
	traits::{OnIdle, OnInitialize},
};
use hex_literal::hex;

#[test]
fn deposit_claim() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Erc20Peg::activate_deposits(frame_system::RawOrigin::Root.into(), true));
		let origin: AccountId =
			AccountId::from(hex!("0000000000000000000000a86e122edbdcba4bf24a2abf89f5c230b37df49d4a"));
		let token_address: H160 = H160::default();
		let amount: Balance = 100;
		let beneficiary: H256 = H256::default();
		let claim = Erc20DepositEvent {
			token_address,
			amount: amount.into(),
			beneficiary,
		};
		let tx_hash = H256::default();

		assert_ok!(Erc20Peg::deposit_claim(Some(origin).into(), tx_hash, claim));
	});
}

#[test]
fn on_deposit_mints() {
	ExtBuilder::default().build().execute_with(|| {
		let contract_address: EthAddress = EthAddress::from(Erc20Peg::contract_address());
		assert_ok!(Erc20Peg::set_contract_address(
			frame_system::RawOrigin::Root.into(),
			contract_address
		));

		let token_address: H160 = H160::default();
		let amount: Balance = 100;
		let beneficiary: H256 = H256::default();
		let claim = Erc20DepositEvent {
			token_address,
			amount: amount.into(),
			beneficiary,
		};
		let event_claim_id: u64 = 0;
		let event_type: H256 = DepositEventSignature::get().into();
		Erc20Peg::on_success(
			event_claim_id,
			&contract_address,
			&event_type,
			&crml_support::EthAbiCodec::encode(&claim),
		);

		let beneficiary: AccountId = AccountId::decode(&mut &beneficiary.0[..]).unwrap();
		let expected_asset_id = 17000;
		assert_eq!(GenericAsset::free_balance(expected_asset_id, &beneficiary), amount);
		assert_eq!(Erc20Peg::erc20_to_asset(contract_address), Some(expected_asset_id));
		assert_eq!(Erc20Peg::asset_to_erc20(expected_asset_id), Some(contract_address));
	});
}

#[test]
fn on_cennz_deposit_transfers() {
	ExtBuilder::default().build().execute_with(|| {
		let contract_address: EthAddress = EthAddress::from(Erc20Peg::contract_address());
		assert_ok!(Erc20Peg::set_contract_address(
			frame_system::RawOrigin::Root.into(),
			contract_address
		));
		let amount: Balance = 100;

		let cennz_asset_id: AssetId = <Test as Config>::MultiCurrency::staking_currency();
		let cennz_eth_address: EthAddress = H160::default();
		<Erc20ToAssetId>::insert(cennz_eth_address, cennz_asset_id);
		assert_ok!(Erc20Peg::activate_cennz_deposits(frame_system::RawOrigin::Root.into()));
		let _ = <Test as Config>::MultiCurrency::deposit_creating(
			&PegPalletId::get().into_account(),
			cennz_asset_id,
			amount,
		);

		let amount: Balance = 100;
		let beneficiary: H256 = H256::default();
		let claim = Erc20DepositEvent {
			token_address: cennz_eth_address,
			amount: amount.into(),
			beneficiary,
		};
		let event_claim_id: u64 = 0;
		let event_type: H256 = DepositEventSignature::get().into();
		Erc20Peg::on_success(
			event_claim_id,
			&contract_address,
			&event_type,
			&crml_support::EthAbiCodec::encode(&claim),
		);

		let beneficiary: AccountId = AccountId::decode(&mut &beneficiary.0[..]).unwrap();
		assert_eq!(GenericAsset::free_balance(cennz_asset_id, &beneficiary), amount);
		assert_eq!(
			GenericAsset::free_balance(cennz_asset_id, &PegPalletId::get().into_account()),
			0,
		);
	});
}

#[test]
fn cennz_withdraw_transfers() {
	// fails if token address is not mapped or withdrawals paused
	// tokens taken
	// withdrawal hash saved correctly
	ExtBuilder::default().build().execute_with(|| {
		let origin: AccountId =
			AccountId::from(hex!("0000000000000000000000a86e122edbdcba4bf24a2abf89f5c230b37df49d4a"));
		let cennz_asset_id: AssetId = <Test as Config>::MultiCurrency::staking_currency();
		let cennz_eth_address: EthAddress = H160::default();
		<AssetIdToErc20>::insert(cennz_asset_id, cennz_eth_address);

		let amount: Balance = 100;
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&origin, cennz_asset_id, amount);
		let beneficiary: H160 = H160::from_slice(&hex!("a86e122EdbDcBA4bF24a2Abf89F5C230b37DF49d"));

		assert_ok!(Erc20Peg::activate_withdrawals(
			frame_system::RawOrigin::Root.into(),
			true
		));
		assert_eq!(
			GenericAsset::free_balance(cennz_asset_id, &PegPalletId::get().into_account()),
			0,
		);
		assert_ok!(Erc20Peg::withdraw(
			Some(origin).into(),
			cennz_asset_id,
			amount,
			beneficiary
		));
		// Peg account should have cennz balance of amount
		assert_eq!(
			GenericAsset::free_balance(cennz_asset_id, &PegPalletId::get().into_account()),
			amount,
		);

		// Check withdrawal hash is stored correctly
		let message = WithdrawMessage {
			token_address: cennz_eth_address,
			amount: amount.into(),
			beneficiary,
		};
		let event_proof_id: u64 = <Test as Config>::EthBridge::generate_event_proof(&message).unwrap();
		let withdrawal_hash = <Test as frame_system::Config>::Hashing::hash(&mut (message, event_proof_id).encode());
		assert_eq!(Erc20Peg::withdrawal_digests(event_proof_id), withdrawal_hash);
	});
}

#[test]
fn withdraw() {
	ExtBuilder::default().build().execute_with(|| {
		let origin: AccountId =
			AccountId::from(hex!("0000000000000000000000a86e122edbdcba4bf24a2abf89f5c230b37df49d4a"));
		let asset_id: AssetId = 1;
		let cennz_eth_address: EthAddress = H160::default();
		<AssetIdToErc20>::insert(asset_id, cennz_eth_address);

		let amount: Balance = 100;
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&origin, asset_id, amount);
		let beneficiary: H160 = H160::from_slice(&hex!("a86e122EdbDcBA4bF24a2Abf89F5C230b37DF49d"));

		assert_ok!(Erc20Peg::activate_withdrawals(
			frame_system::RawOrigin::Root.into(),
			true
		));
		assert_eq!(GenericAsset::free_balance(asset_id, &origin), amount);
		assert_ok!(Erc20Peg::withdraw(
			Some(origin.clone()).into(),
			asset_id,
			amount,
			beneficiary
		));
		assert_eq!(GenericAsset::free_balance(asset_id, &origin), 0);

		// Check withdrawal hash is stored correctly
		let message = WithdrawMessage {
			token_address: cennz_eth_address,
			amount: amount.into(),
			beneficiary,
		};
		let event_proof_id: u64 = <Test as Config>::EthBridge::generate_event_proof(&message).unwrap();
		let withdrawal_hash = <Test as frame_system::Config>::Hashing::hash(&mut (message, event_proof_id).encode());
		assert_eq!(Erc20Peg::withdrawal_digests(event_proof_id), withdrawal_hash);
	});
}

#[test]
fn set_claim_delay() {
	ExtBuilder::default().build().execute_with(|| {
		let asset_id: AssetId = 1;
		let min_balance: Balance = 100;
		let delay: u64 = 1000;
		assert_ok!(Erc20Peg::set_claim_delay(
			frame_system::RawOrigin::Root.into(),
			asset_id,
			min_balance,
			delay
		));
		assert_eq!(Erc20Peg::claim_delay(asset_id), Some((min_balance, delay)));
	});
}

#[test]
fn deposit_claim_with_delay() {
	ExtBuilder::default().build().execute_with(|| {
		let origin: AccountId =
			AccountId::from(hex!("0000000000000000000000a86e122edbdcba4bf24a2abf89f5c230b37df49d4a"));
		let asset_id: AssetId = 1;
		let cennz_eth_address: EthAddress = H160::default();
		let amount: Balance = 100;
		let beneficiary: H256 = H256::default();
		let claim = Erc20DepositEvent {
			token_address: cennz_eth_address,
			amount: amount.into(),
			beneficiary,
		};
		let tx_hash = H256::default();
		let delay: u64 = 1000;

		assert_ok!(Erc20Peg::activate_deposits(frame_system::RawOrigin::Root.into(), true));
		<AssetIdToErc20>::insert(asset_id, cennz_eth_address);
		<Erc20ToAssetId>::insert(cennz_eth_address, asset_id);
		assert_ok!(Erc20Peg::set_claim_delay(
			frame_system::RawOrigin::Root.into(),
			asset_id,
			amount,
			delay
		));
		let claim_id = <NextClaimId>::get();
		assert_ok!(Erc20Peg::deposit_claim(Some(origin).into(), tx_hash, claim.clone()));
		let claim_block = <frame_system::Pallet<Test>>::block_number() + delay;
		// Check claim has been put into pending claims
		assert_eq!(Erc20Peg::claim_schedule(claim_block), Some(vec![claim_id]));
		assert_eq!(
			Erc20Peg::pending_claims(claim_id),
			Some(PendingClaim::Deposit((claim.clone(), tx_hash)))
		);
		// Check claim id has been increased
		assert_eq!(<NextClaimId>::get(), claim_id + 1);
		// Simulating block before with enough weight, claim shouldn't be removed
		assert_eq!(Erc20Peg::on_initialize(claim_block - 1), 10_000_000);
		assert_eq!(Erc20Peg::on_idle(claim_block - 1, 100_000_000), 10_000_000);
		// Simulating not enough weight left in block, claim shouldn't be removed
		assert_eq!(Erc20Peg::on_initialize(claim_block), 20_000_000);
		assert_eq!(Erc20Peg::ready_blocks(), Some(vec![claim_block]));

		assert_eq!(Erc20Peg::on_idle(claim_block, 40_000_000), 10_000_000);
		assert_eq!(Erc20Peg::claim_schedule(claim_block), Some(vec![claim_id]));
		assert_eq!(
			Erc20Peg::pending_claims(claim_id),
			Some(PendingClaim::Deposit((claim.clone(), tx_hash)))
		);

		// Try again next block with enough weight
		assert_eq!(Erc20Peg::on_initialize(claim_block + 1), 10_000_000);
		assert_eq!(Erc20Peg::on_idle(claim_block + 1, 55_000_000), 60_000_000);
		// Claim should be removed from storage
		assert_eq!(Erc20Peg::claim_schedule(claim_block), None);
		assert_eq!(Erc20Peg::pending_claims(claim_id), None);
	});
}

#[test]
fn multiple_deposit_claims_with_delay() {
	ExtBuilder::default().build().execute_with(|| {
		let origin: AccountId =
			AccountId::from(hex!("0000000000000000000000a86e122edbdcba4bf24a2abf89f5c230b37df49d4a"));
		let asset_id: AssetId = 1;
		let cennz_eth_address: EthAddress = H160::default();
		let amount: Balance = 100;
		let beneficiary: H256 = H256::default();
		let claim = Erc20DepositEvent {
			token_address: cennz_eth_address,
			amount: amount.into(),
			beneficiary,
		};
		let tx_hash = H256::default();
		let delay: u64 = 1000;

		assert_ok!(Erc20Peg::activate_deposits(frame_system::RawOrigin::Root.into(), true));
		<AssetIdToErc20>::insert(asset_id, cennz_eth_address);
		<Erc20ToAssetId>::insert(cennz_eth_address, asset_id);
		assert_ok!(Erc20Peg::set_claim_delay(
			frame_system::RawOrigin::Root.into(),
			asset_id,
			amount,
			delay
		));
		let mut claim_ids: Vec<ClaimId> = vec![];
		let num_claims = 10;
		let claim_block = <frame_system::Pallet<Test>>::block_number() + delay;

		for _ in 0..num_claims {
			let claim_id = <NextClaimId>::get();
			claim_ids.push(claim_id);
			assert_ok!(Erc20Peg::deposit_claim(
				Some(origin.clone()).into(),
				tx_hash,
				claim.clone()
			));
			// Check claim has been put into pending claims
			assert_eq!(
				Erc20Peg::pending_claims(claim_id),
				Some(PendingClaim::Deposit((claim.clone(), tx_hash)))
			);
		}
		assert_eq!(Erc20Peg::claim_schedule(claim_block), Some(claim_ids.clone()));

		// Call on_idle with room for only 5 claims
		// Weight in on_idle for one claim is 2_000_000
		assert_eq!(Erc20Peg::on_initialize(claim_block), 20_000_000);
		assert_eq!(Erc20Peg::on_idle(claim_block, 290_000_000), 260_000_000);
		let mut changed_count = 0;
		for i in 0..num_claims {
			if Erc20Peg::pending_claims(claim_ids[i]) == None {
				changed_count += 1;
			}
		}
		assert_eq!(changed_count, 5);

		assert_eq!(Erc20Peg::on_initialize(claim_block + 1), 10_000_000);
		assert_eq!(Erc20Peg::on_idle(claim_block + 1, 500_000_000), 260_000_000);
		for i in 0..num_claims {
			assert_eq!(Erc20Peg::pending_claims(claim_ids[i]), None);
		}
		assert_eq!(Erc20Peg::claim_schedule(claim_block), None);
	});
}

#[test]
fn deposit_claim_less_than_delay_goes_through() {
	ExtBuilder::default().build().execute_with(|| {
		let origin: AccountId =
			AccountId::from(hex!("0000000000000000000000a86e122edbdcba4bf24a2abf89f5c230b37df49d4a"));
		let asset_id: AssetId = 1;
		let cennz_eth_address: EthAddress = H160::default();
		let amount: Balance = 100;
		let beneficiary: H256 = H256::default();
		let claim = Erc20DepositEvent {
			token_address: cennz_eth_address,
			amount: (amount - 1).into(),
			beneficiary,
		};
		let tx_hash = H256::default();
		let delay: u64 = 1000;

		assert_ok!(Erc20Peg::activate_deposits(frame_system::RawOrigin::Root.into(), true));
		<AssetIdToErc20>::insert(asset_id, cennz_eth_address);
		<Erc20ToAssetId>::insert(cennz_eth_address, asset_id);
		assert_ok!(Erc20Peg::set_claim_delay(
			frame_system::RawOrigin::Root.into(),
			asset_id,
			amount,
			delay
		));
		let claim_id = <NextClaimId>::get();
		assert_ok!(Erc20Peg::deposit_claim(Some(origin).into(), tx_hash, claim.clone()));
		let claim_block = <frame_system::Pallet<Test>>::block_number() + delay;
		// Check claim has not been put into pending claims
		assert_eq!(Erc20Peg::claim_schedule(claim_block), None);
		assert_eq!(Erc20Peg::pending_claims(claim_id), None);
	});
}

#[test]
fn withdraw_with_delay() {
	ExtBuilder::default().build().execute_with(|| {
		let origin: AccountId =
			AccountId::from(hex!("0000000000000000000000a86e122edbdcba4bf24a2abf89f5c230b37df49d4a"));
		let asset_id: AssetId = 1;
		let cennz_eth_address: EthAddress = H160::default();
		let amount: Balance = 100;
		let beneficiary: H160 = H160::from_slice(&hex!("a86e122EdbDcBA4bF24a2Abf89F5C230b37DF49d"));
		let delay: u64 = 1000;
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&origin, asset_id, amount);

		<AssetIdToErc20>::insert(asset_id, cennz_eth_address);
		<Erc20ToAssetId>::insert(cennz_eth_address, asset_id);
		assert_ok!(Erc20Peg::activate_withdrawals(
			frame_system::RawOrigin::Root.into(),
			true
		));

		assert_ok!(Erc20Peg::set_claim_delay(
			frame_system::RawOrigin::Root.into(),
			asset_id,
			amount,
			delay
		));

		let claim_id = <NextClaimId>::get();
		let claim_block = <frame_system::Pallet<Test>>::block_number() + delay;
		assert_ok!(Erc20Peg::withdraw(
			Some(origin.clone()).into(),
			asset_id,
			amount,
			beneficiary
		));

		// Balance should be withdrawn straight away
		assert_eq!(GenericAsset::free_balance(asset_id, &origin), 0);
		let message = WithdrawMessage {
			token_address: cennz_eth_address,
			amount: amount.into(),
			beneficiary,
		};

		assert_eq!(Erc20Peg::claim_schedule(claim_block), Some(vec![claim_id]));
		assert_eq!(
			Erc20Peg::pending_claims(claim_id),
			Some(PendingClaim::Withdrawal(message))
		);
		// Check claim id has been increased
		assert_eq!(<NextClaimId>::get(), claim_id + 1);
		assert_eq!(Erc20Peg::on_initialize(claim_block), 20_000_000);
		assert_eq!(Erc20Peg::on_idle(claim_block, 900_000_000), 60_000_000);
		// Claim should be removed from storage
		assert_eq!(Erc20Peg::claim_schedule(claim_block), None);
		assert_eq!(Erc20Peg::pending_claims(claim_id), None);
	});
}

#[test]
fn withdraw_less_than_delay_goes_through() {
	ExtBuilder::default().build().execute_with(|| {
		let origin: AccountId =
			AccountId::from(hex!("0000000000000000000000a86e122edbdcba4bf24a2abf89f5c230b37df49d4a"));
		let asset_id: AssetId = 1;
		let cennz_eth_address: EthAddress = H160::default();
		let amount: Balance = 100;
		let beneficiary: H160 = H160::from_slice(&hex!("a86e122EdbDcBA4bF24a2Abf89F5C230b37DF49d"));
		let delay: u64 = 1000;
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&origin, asset_id, amount);

		<AssetIdToErc20>::insert(asset_id, cennz_eth_address);
		<Erc20ToAssetId>::insert(cennz_eth_address, asset_id);
		assert_ok!(Erc20Peg::activate_withdrawals(
			frame_system::RawOrigin::Root.into(),
			true
		));

		assert_ok!(Erc20Peg::set_claim_delay(
			frame_system::RawOrigin::Root.into(),
			asset_id,
			amount,
			delay
		));

		let claim_id = <NextClaimId>::get();
		let claim_block = <frame_system::Pallet<Test>>::block_number() + delay;
		assert_ok!(Erc20Peg::withdraw(
			Some(origin.clone()).into(),
			asset_id,
			amount - 1,
			beneficiary
		));
		assert_eq!(Erc20Peg::claim_schedule(claim_block), None);
		assert_eq!(Erc20Peg::pending_claims(claim_id), None);
	});
}

#[test]
fn withdraw_unsupported_asset_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let origin: AccountId =
			AccountId::from(hex!("0000000000000000000000a86e122edbdcba4bf24a2abf89f5c230b37df49d4a"));
		let asset_id: AssetId = 1;
		let amount: Balance = 100;
		let beneficiary: H160 = H160::from_slice(&hex!("a86e122EdbDcBA4bF24a2Abf89F5C230b37DF49d"));

		assert_ok!(Erc20Peg::activate_withdrawals(
			frame_system::RawOrigin::Root.into(),
			true
		));

		assert_noop!(
			Erc20Peg::withdraw(Some(origin.clone()).into(), asset_id, amount, beneficiary),
			Error::<Test>::UnsupportedAsset
		);
	});
}

#[test]
fn withdraw_not_active_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let origin: AccountId =
			AccountId::from(hex!("0000000000000000000000a86e122edbdcba4bf24a2abf89f5c230b37df49d4a"));
		let asset_id: AssetId = 1;
		let amount: Balance = 100;
		let beneficiary: H160 = H160::from_slice(&hex!("a86e122EdbDcBA4bF24a2Abf89F5C230b37DF49d"));

		assert_noop!(
			Erc20Peg::withdraw(Some(origin.clone()).into(), asset_id, amount, beneficiary),
			Error::<Test>::WithdrawalsPaused
		);
	});
}
