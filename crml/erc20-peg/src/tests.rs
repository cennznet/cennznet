use super::*;
use crate::mock::{ExtBuilder, Erc20Peg, Test};
use frame_support::{assert_noop, assert_ok, traits::OnInitialize};
use sp_runtime::DispatchError;

#[test]
fn deposit_claim() {
    // address is valid
}

#[test]
fn on_deposit_fail() {
    // tokens not paid
}

#[test]
fn on_deposit_success() {
    // tokens paid
}

#[test]
fn withdraw() {
    // fails if token address is not mapped or withdrawals paused

    // tokens taken
    // withdrawal hash saved correctly
}