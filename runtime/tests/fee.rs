//!
//! Fee integration tests
//!
use cennznet_primitives::CheckedCennznetExtrinsic;
use cennznet_runtime::{Call, ExtrinsicFeePayment, Fee, Runtime};
use runtime_io::with_externalities;
use runtime_primitives::BuildStorage;
use substrate_primitives::{sr25519::Public, Blake2Hasher};
use support::{additional_traits::ChargeExtrinsicFee, assert_err, assert_ok};

// A default address for ChargeExtrinsicFee `transactor`
const DEFAULT_TRANSACTOR: Public = Public([0u8; 32]);

type MockCheckedExtrinsic = CheckedCennznetExtrinsic<substrate_primitives::sr25519::Public, u64, Call, u128>;
type System = system::Module<Runtime>;
type Fees = fees::Module<Runtime>;

// Nice aliases
const BASE_FEE: Fee = Fee::fees(fees::Fee::Base);
const BYTE_FEE: Fee = Fee::fees(fees::Fee::Bytes);
const CREATE_ACCOUNT_FEE: Fee = Fee::generic_asset(generic_asset::Fee::Transfer);

#[test]
fn charge_extrinsic_fee_works() {
	with_externalities(
		&mut ExtBuilder::default().set_fee(BASE_FEE, 3).set_fee(BYTE_FEE, 5).build(),
		|| {
			let xt = MockCheckedExtrinsic {
				signed: None,
				function: Call::Timestamp(timestamp::Call::<Runtime>::set(0)), // An arbitrarily chosen Runtime call
				fee_exchange: None,
			};

			System::set_extrinsic_index(0);
			assert_ok!(ExtrinsicFeePayment::charge_extrinsic_fee(&DEFAULT_TRANSACTOR, 7, &xt));
			assert_eq!(
				Fees::current_transaction_fee(0),
				Fees::fee_registry(BASE_FEE) + Fees::fee_registry(BYTE_FEE) * 7
			);

			System::set_extrinsic_index(1);
			assert_ok!(ExtrinsicFeePayment::charge_extrinsic_fee(&DEFAULT_TRANSACTOR, 11, &xt));
			assert_eq!(
				Fees::current_transaction_fee(1),
				Fees::fee_registry(BASE_FEE) + Fees::fee_registry(BYTE_FEE) * 11
			);

			System::set_extrinsic_index(3);
			assert_ok!(ExtrinsicFeePayment::charge_extrinsic_fee(&DEFAULT_TRANSACTOR, 13, &xt));
			assert_eq!(
				Fees::current_transaction_fee(3),
				Fees::fee_registry(BASE_FEE) + Fees::fee_registry(BYTE_FEE) * 13
			);
		},
	);
}

#[test]
fn charge_extrinsic_fee_for_generic_asset_transfer() {
	with_externalities(
		&mut ExtBuilder::default()
			.set_fee(BASE_FEE, 3)
			.set_fee(BYTE_FEE, 5)
			.set_fee(CREATE_ACCOUNT_FEE, 20)
			.build(),
		|| {
			let xt = MockCheckedExtrinsic {
				signed: None,
				function: Call::GenericAsset(generic_asset::Call::<Runtime>::transfer(0, DEFAULT_TRANSACTOR, 10)),
				fee_exchange: None,
			};

			System::set_extrinsic_index(0);
			assert_ok!(ExtrinsicFeePayment::charge_extrinsic_fee(&DEFAULT_TRANSACTOR, 7, &xt));
			assert_eq!(
				Fees::current_transaction_fee(0),
				Fees::fee_registry(BASE_FEE)
					+ Fees::fee_registry(CREATE_ACCOUNT_FEE)
					+ Fees::fee_registry(BYTE_FEE) * 7
			);
		},
	);
}

#[test]
fn charge_extrinsic_fee_for_generic_asset_transfer_overflow() {
	with_externalities(
		&mut ExtBuilder::default()
			.set_fee(BASE_FEE, 3)
			.set_fee(BYTE_FEE, 5)
			.set_fee(CREATE_ACCOUNT_FEE, u128::max_value())
			.build(),
		|| {
			let xt = MockCheckedExtrinsic {
				signed: None,
				function: Call::GenericAsset(generic_asset::Call::<Runtime>::transfer(0, DEFAULT_TRANSACTOR, 10)),
				fee_exchange: None,
			};

			System::set_extrinsic_index(0);
			assert_err!(
				ExtrinsicFeePayment::charge_extrinsic_fee(&DEFAULT_TRANSACTOR, 7, &xt),
				"extrinsic fee overflow (base + bytes + call)"
			);
		},
	);
}

#[test]
fn charge_extrinsic_fee_fails_with_bytes_fee_overflow() {
	let xt = MockCheckedExtrinsic {
		signed: None,
		function: Call::Timestamp(timestamp::Call::<Runtime>::set(0)),
		fee_exchange: None,
	};

	// bytes fee overflows.
	with_externalities(
		&mut ExtBuilder::default()
			.set_fee(BASE_FEE, 0)
			.set_fee(BYTE_FEE, u128::max_value())
			.build(),
		|| {
			System::set_extrinsic_index(0);
			assert_err!(
				ExtrinsicFeePayment::charge_extrinsic_fee(&DEFAULT_TRANSACTOR, 2, &xt),
				"extrinsic fee overflow (bytes)"
			);
		},
	);
}

#[test]
fn charge_extrinsic_fee_fails_with_total_fee_overflow() {
	let xt = MockCheckedExtrinsic {
		signed: None,
		function: Call::Timestamp(timestamp::Call::<Runtime>::set(0)),
		fee_exchange: None,
	};

	// bytes fee doesn't overflow, but total fee (bytes_fee + BASE_FEE) does
	with_externalities(
		&mut ExtBuilder::default()
			.set_fee(BASE_FEE, u128::max_value())
			.set_fee(BYTE_FEE, 1)
			.build(),
		|| {
			System::set_extrinsic_index(0);
			assert_err!(
				ExtrinsicFeePayment::charge_extrinsic_fee(&DEFAULT_TRANSACTOR, 1, &xt),
				"extrinsic fee overflow (base + bytes)"
			);
		},
	);
}

// Lifted from `prml-fees`, importing doesn't work...
pub struct ExtBuilder {
	transaction_base_fee: u128,
	transaction_byte_fee: u128,
	create_account_fee: u128,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			transaction_base_fee: 0,
			transaction_byte_fee: 0,
			create_account_fee: 0,
		}
	}
}

impl ExtBuilder {
	pub fn set_fee(mut self, fee: Fee, amount: u128) -> Self {
		match fee {
			Fee::fees(fees::Fee::Base) => self.transaction_base_fee = amount,
			Fee::fees(fees::Fee::Bytes) => self.transaction_byte_fee = amount,
			Fee::generic_asset(generic_asset::Fee::Transfer) => self.create_account_fee = amount,
		};

		self
	}
	pub fn build(self) -> runtime_io::TestExternalities<Blake2Hasher> {
		let (mut t, mut c) = system::GenesisConfig::<Runtime>::default().build_storage().unwrap();
		let _ = generic_asset::GenesisConfig::<Runtime> {
			staking_asset_id: 16_000,
			spending_asset_id: 16_001,
			assets: vec![16_001],
			endowed_accounts: vec![DEFAULT_TRANSACTOR],
			create_asset_stake: 10,
			initial_balance: u128::max_value(),
			next_asset_id: 10_000,
			transfer_fee: 0,
		}
		.assimilate_storage(&mut t, &mut c);
		let _ = fees::GenesisConfig::<Runtime> {
			_genesis_phantom_data: rstd::marker::PhantomData {},
			fee_registry: vec![
				(BASE_FEE, self.transaction_base_fee),
				(BYTE_FEE, self.transaction_byte_fee),
				(CREATE_ACCOUNT_FEE, self.create_account_fee),
			],
		}
		.assimilate_storage(&mut t, &mut c);

		t.into()
	}
}
