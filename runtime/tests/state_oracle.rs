use cennznet_runtime::{
	constants::{asset::*, currency::*},
	precompiles::StateOracleCallbackExecutor,
	Ethereum, Event, GenericAsset, Runtime, System,
};
use crml_support::{ContractExecutor, MultiCurrency, H160, U256};
use fp_rpc::runtime_decl_for_EthereumRuntimeRPCApi::EthereumRuntimeRPCApi;
use frame_support::{assert_ok, traits::OnFinalize};
use frame_system::EventRecord;
use pallet_evm::AddressMapping;

mod common;
use common::mock::ExtBuilder;

#[test]
fn callback_execution() {
	ExtBuilder::default().build().execute_with(|| {
		// allows events to be stored
		System::set_block_number(1);

		let caller = H160::from_low_u64_be(1_u64);
		let caller_ss58 = <Runtime as pallet_evm::Config>::AddressMapping::into_account_id(caller);
		let _ = GenericAsset::deposit_creating(&caller_ss58, CPAY_ASSET_ID, 1000 * DOLLARS);

		// Test
		assert_ok!(StateOracleCallbackExecutor::<Runtime>::execute(
			&caller,
			&H160::from_low_u64_be(2_u64),
			Default::default(),
			200_000_u64,
			Runtime::gas_price(),
			U256::zero(),
		));
		Ethereum::on_finalize(System::block_number());

		// system events has the executed event
		println!("{:?}", System::events());
		if let EventRecord {
			event: Event::Ethereum(pallet_ethereum::Event::Executed(caller_, _, _, _)),
			phase: _,
			topics: _,
		} = System::events().last().unwrap()
		{
			assert_eq!(caller, *caller_);
			// ethereum pallet current receipts contains the callback tx
			assert_eq!(Ethereum::current_receipts().expect("event exists").len(), 1_usize);
		} else {
			assert!(false, "expected evm execution event")
		}
	});
}
