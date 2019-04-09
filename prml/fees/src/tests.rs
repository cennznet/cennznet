// Copyright 2019 Parity Technologies (UK) Ltd.
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

//! Tests for the module.

#![cfg(test)]

use super::*;
use runtime_io::with_externalities;
use runtime_primitives::traits::OnFinalize;
use system::{EventRecord, Phase};

use mock::{Call, ExtBuilder, Fees, OnFeeChargedMock, System, Test};
use support::{additional_traits::ChargeFee, assert_err, assert_ok};

type MockCheckedExtrinsic = CheckedCennznetExtrinsic<u64, u64, Call<Test>, u128>;

#[test]
fn make_payment_should_work() {
	with_externalities(
		&mut ExtBuilder::default()
			.transaction_base_fee(3)
			.transaction_byte_fee(5)
			.build(),
		|| {
			let xt = MockCheckedExtrinsic {
				signed: None,
				function: tests::Call::do_nothing(),
				fee_exchange: None,
			};

			System::set_extrinsic_index(0);
			assert_ok!(Fees::make_payment(&0, 7, &xt));
			assert_eq!(Fees::current_transaction_fee(0), 3 + 5 * 7);

			System::set_extrinsic_index(1);
			assert_ok!(Fees::make_payment(&0, 11, &xt));
			assert_eq!(Fees::current_transaction_fee(1), 3 + 5 * 11);

			System::set_extrinsic_index(3);
			assert_ok!(Fees::make_payment(&0, 13, &xt));
			assert_eq!(Fees::current_transaction_fee(3), 3 + 5 * 13);
		},
	);
}

#[test]
fn make_payment_should_not_work_if_bytes_fee_overflow() {
	let xt = MockCheckedExtrinsic {
		signed: None,
		function: tests::Call::do_nothing(),
		fee_exchange: None,
	};

	// bytes fee overflows.
	with_externalities(
		&mut ExtBuilder::default()
			.transaction_base_fee(0)
			.transaction_byte_fee(u64::max_value())
			.build(),
		|| {
			System::set_extrinsic_index(0);
			assert_err!(Fees::make_payment(&0, 2, &xt), "bytes fee overflow");
		},
	);
}

#[test]
fn make_payment_should_not_work_if_overall_fee_overflow() {
	let xt = MockCheckedExtrinsic {
		signed: None,
		function: tests::Call::do_nothing(),
		fee_exchange: None,
	};

	// bytes fee doesn't overflow, but overall fee (bytes_fee + base_fee) does
	with_externalities(
		&mut ExtBuilder::default()
			.transaction_base_fee(u64::max_value())
			.transaction_byte_fee(1)
			.build(),
		|| {
			System::set_extrinsic_index(0);
			assert_err!(Fees::make_payment(&0, 1, &xt), "bytes fee overflow");
		},
	);
}

#[test]
fn charge_fee_should_work() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		System::set_extrinsic_index(0);
		assert_ok!(Fees::charge_fee(&0, 2));
		assert_ok!(Fees::charge_fee(&0, 3));
		assert_eq!(Fees::current_transaction_fee(0), 2 + 3);

		System::set_extrinsic_index(2);
		assert_ok!(Fees::charge_fee(&0, 5));
		assert_ok!(Fees::charge_fee(&0, 7));
		assert_eq!(Fees::current_transaction_fee(2), 5 + 7);
	});
}

#[test]
fn charge_fee_when_overflow_should_not_work() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		System::set_extrinsic_index(0);
		assert_ok!(Fees::charge_fee(&0, u64::max_value()));
		assert_err!(Fees::charge_fee(&0, 1), "fee got overflow after charge");
	});
}

#[test]
fn refund_fee_should_work() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		System::set_extrinsic_index(0);
		assert_ok!(Fees::charge_fee(&0, 5));
		assert_ok!(Fees::refund_fee(&0, 3));
		assert_eq!(Fees::current_transaction_fee(0), 5 - 3);
	});
}

#[test]
fn refund_fee_when_underflow_should_not_work() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		System::set_extrinsic_index(0);
		assert_err!(Fees::refund_fee(&0, 1), "fee got underflow after refund");
	});
}

#[test]
fn on_finalize_should_work() {
	with_externalities(&mut ExtBuilder::default().build(), || {
		// charge fees in extrinsic index 3
		System::set_extrinsic_index(3);
		assert_ok!(Fees::charge_fee(&0, 1));
		System::note_applied_extrinsic(&Ok(()), 1);
		// charge fees in extrinsic index 5
		System::set_extrinsic_index(5);
		assert_ok!(Fees::charge_fee(&0, 1));
		System::note_applied_extrinsic(&Ok(()), 1);
		System::note_finished_extrinsics();

		// `current_transaction_fee`, `extrinsic_count` should be as expected.
		assert_eq!(Fees::current_transaction_fee(3), 1);
		assert_eq!(Fees::current_transaction_fee(5), 1);
		assert_eq!(System::extrinsic_count(), 5 + 1);

		<Fees as OnFinalize<u64>>::on_finalize(1);

		// When finalised, `CurrentTransactionFee` records should be cleared.
		assert_eq!(Fees::current_transaction_fee(3), 0);
		assert_eq!(Fees::current_transaction_fee(5), 0);

		// When finalised, if any fee charged in a extrinsic, a `Charged` event should be deposited
		// for it.
		let fee_charged_events: Vec<EventRecord<mock::TestEvent>> = System::events()
			.into_iter()
			.filter(|e| match e.event {
				mock::TestEvent::fees(RawEvent::Charged(_, _)) => return true,
				_ => return false,
			})
			.collect();
		assert_eq!(
			fee_charged_events,
			vec![
				EventRecord {
					phase: Phase::Finalization,
					event: RawEvent::Charged(3, 1).into(),
				},
				EventRecord {
					phase: Phase::Finalization,
					event: RawEvent::Charged(5, 1).into(),
				},
			]
		);

		// Block fee should match.
		assert_eq!(OnFeeChargedMock::amount(), 2);
	});
}

#[test]
fn make_payment_should_work_with_fee_exchange() {
	with_externalities(
		&mut ExtBuilder::default()
			.transaction_base_fee(3)
			.transaction_byte_fee(5)
			.build(),
		|| {
			let xt = MockCheckedExtrinsic {
				signed: None,
				function: tests::Call::do_nothing(),
				fee_exchange: Some(FeeExchange::new(0, 1_000_000)),
			};

			System::set_extrinsic_index(0);
			assert_ok!(Fees::make_payment(&0, 7, &xt));
		},
	);
}
