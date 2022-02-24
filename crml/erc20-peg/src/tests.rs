use super::*;
use crate::mock::{DepositEventSignature, Erc20Peg, ExtBuilder, GenericAsset, PegPalletId, Test};
use cennznet_primitives::types::AccountId;
use crml_support::MultiCurrency;
use frame_support::assert_ok;
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
		Erc20Peg::on_success(event_claim_id, &contract_address, &event_type, &claim.encode());

		let beneficiary: AccountId = AccountId::decode(&mut &beneficiary.0[..]).unwrap();
		let expected_asset_id = 0;
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
		let cennz_eth_address: EthAddress = H160::from_slice(&hex!("0000000000000000000000000000000000000000"));
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
		Erc20Peg::on_success(event_claim_id, &contract_address, &event_type, &claim.encode());

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
		let cennz_eth_address: EthAddress = H160::from_slice(&hex!("0000000000000000000000000000000000000000"));
		<AssetIdToErc20>::insert(cennz_asset_id, cennz_eth_address);

		let amount: Balance = 100;
		let _ = <Test as Config>::MultiCurrency::deposit_creating(&origin, cennz_asset_id, amount);
		let beneficiary: H160 = H160::from_slice(&hex!("a86e122EdbDcBA4bF24a2Abf89F5C230b37DF49d"));

		assert_ok!(Erc20Peg::activate_withdrawals(
			frame_system::RawOrigin::Root.into(),
			true
		));
		assert_ok!(Erc20Peg::activate_cennz_deposits(frame_system::RawOrigin::Root.into()));
		assert_eq!(Erc20Peg::cennz_deposit_active(), true);
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
	});
}

#[test]
fn withdraw() {
	ExtBuilder::default().build().execute_with(|| {
		let origin: AccountId =
			AccountId::from(hex!("0000000000000000000000a86e122edbdcba4bf24a2abf89f5c230b37df49d4a"));
		let asset_id: AssetId = 1;
		let cennz_eth_address: EthAddress = H160::from_slice(&hex!("0000000000000000000000000000000000000000"));
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
	});
}
