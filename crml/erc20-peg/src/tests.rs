use super::*;
use crate::mock::{Erc20Peg, ExtBuilder, Test};
use frame_support::{assert_noop, assert_ok, traits::OnInitialize};
use sp_runtime::DispatchError;

#[test]
fn deposit_claim() {
	ExtBuilder::default().execute_with(|ext| {
		// address is valid
	});
}

#[test]
fn on_cennz_deposit_transfers() {
	ExtBuilder::default().execute_with(|ext| {
		Erc20Peg::on_success(event_claim_id: u64, contract_address: &EthAddress, event_type: &H256, event_data: &[u8]);
	});
}

#[test]
fn on_deposit_mints() {
	ExtBuilder::default().execute_with(|ext| {
		Erc20Peg::on_success(event_claim_id: u64, contract_address: &EthAddress, event_type: &H256, event_data: &[u8]);
	});
}

#[test]
fn withdraw() {
	// fails if token address is not mapped or withdrawals paused
	// tokens taken
	// withdrawal hash saved correctly
	ExtBuilder::default().execute_with(|ext| {

	});
}

#[test]
fn cennz_withdraw_transfers() {
	ExtBuilder::default().execute_with(|ext| {
		// tokens not paid
	});
}
