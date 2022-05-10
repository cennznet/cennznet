//! Kudos: https://github.com/PlasmNetwork/Astar/blob/08c4a9211836b929abcbad4ed33ede0f616a6423/frame/custom-signatures/
//! Provides shims for Ethereum wallets (e.g. metamask) to sign transactions compatible with the CENNZnet runtime
#![cfg_attr(not(feature = "std"), no_std)]

use crate::ethereum::{ecrecover, EthereumSignature};
use codec::Encode;
use crml_support::{TransactionFeeHandler, H160 as EthAddress};
use frame_support::{
	decl_error, decl_event, decl_module,
	dispatch::{DispatchInfo, Dispatchable},
	traits::{Get, UnfilteredDispatchable},
	weights::GetDispatchInfo,
	Parameter,
};
use frame_system::ensure_none;
use pallet_evm::AddressMapping;
use sp_runtime::{
	traits::IdentifyAccount,
	transaction_validity::{
		InvalidTransaction, TransactionPriority, TransactionSource, TransactionValidity, ValidTransaction,
	},
	DispatchResult,
};
use sp_std::prelude::*;

/// Ethereum-compatible signatures (eth_sign API call).
pub mod ethereum;

/// The module's configuration trait.
pub trait Config: frame_system::Config {
	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;

	/// Maps Ethereum address to the CENNZnet format
	type AddressMapping: AddressMapping<Self::AccountId>;

	/// A signable call.
	type Call: Parameter
		+ Dispatchable<Info = DispatchInfo>
		+ UnfilteredDispatchable<Origin = Self::Origin>
		+ GetDispatchInfo;

	/// Provides transaction fee handling
	type TransactionFeeHandler: TransactionFeeHandler<AccountId = Self::AccountId, Call = <Self as Config>::Call>;

	type Signer: IdentifyAccount<AccountId = Self::AccountId> + From<sp_core::ecdsa::Public>;

	/// A configuration for base priority of unsigned transactions.
	///
	/// This is exposed so that it can be tuned for particular runtime, when
	/// multiple pallets send unsigned transactions.
	type UnsignedPriority: Get<TransactionPriority>;
}

decl_error! {
	pub enum Error for Module<T: Config> {
		/// Signature & account mismatched.
		InvalidSignature,
		/// Can't pay fees
		CantPay,
	}
}

decl_event!(
	pub enum Event<T>
	where
		AccountId = <T as frame_system::Config>::AccountId,
	{
		/// A call just executed. (Ethereum Address, CENNZnet Address, Result)
		Execute(EthAddress, AccountId, DispatchResult),
	}
);

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		#[weight = {
			let dispatch_info = call.get_dispatch_info();
			(dispatch_info.weight + 10_000, dispatch_info.class)
		}]
		/// Execute a runtime `call` signed using `eth_sign`
		/// Expects signature validates the payload `(call, nonce)`
		/// 'nonce' is the value known by the cennznet mapped system pallet for `eth_address`
		///
		/// origin must be `none`
		/// call - runtime call to execute
		/// account - signed by this account
		fn call(
			origin,
			call: Box<<T as Config>::Call> ,
			eth_address: EthAddress,
			signature: EthereumSignature,
		) -> DispatchResult {
			ensure_none(origin)?;

			// convert to CENNZnet address
			let account = T::AddressMapping::into_account_id(eth_address);
			let nonce = <frame_system::Pallet<T>>::account_nonce(account.clone());
			let prefix = "data:application-octet;base64,";
			let msg =  base64::encode(&(&call, nonce).encode()[..]);
			let full_msg = &[prefix.as_bytes(), msg.as_bytes()].concat()[..];
			if let Some(_public_key) = ecrecover(&signature, full_msg, &eth_address) {
				// Pay fee, increment nonce
				let _ = Self::pay_fee(&call, &account)?;
				<frame_system::Pallet<T>>::inc_account_nonce(&account);

				// execute the call
				let new_origin = frame_system::RawOrigin::Signed(account.clone()).into();
				let res = call.dispatch_bypass_filter(new_origin).map(|_| ());
				Self::deposit_event(RawEvent::Execute(eth_address, account, res.map_err(|e| e.error)));

				Ok(())
			} else {
				Err(Error::<T>::InvalidSignature)?
			}
		}
	}
}

impl<T: Config> Module<T> {
	/// Take required fees from `account` to dispatch `call`
	fn pay_fee(call: &<T as Config>::Call, account: &T::AccountId) -> DispatchResult {
		let info = call.get_dispatch_info();
		let len = call.clone().encode()[..].len() as u32 + 33 + 65 + 4; // call + account, signature, nonce bytes

		T::TransactionFeeHandler::pay_fee(len, call, &info, account)
			.map(|_| ())
			.map_err(|_| Error::<T>::CantPay.into())
	}
}

impl<T: Config> frame_support::unsigned::ValidateUnsigned for Module<T> {
	type Call = Call<T>;

	fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
		if let Call::call {
			call,
			eth_address,
			signature,
		} = call
		{
			let account = T::AddressMapping::into_account_id(*eth_address);
			let nonce = <frame_system::Pallet<T>>::account_nonce(account.clone());
			let prefix = "data:application-octet;base64,";
			let msg = base64::encode(&(&call, nonce).encode()[..]);
			let full_msg = &[prefix.as_bytes(), msg.as_bytes()].concat()[..];

			if let Some(_public_key) = ecrecover(&signature, full_msg, &eth_address) {
				return ValidTransaction::with_tag_prefix("EthWallet")
					.priority(T::UnsignedPriority::get())
					.and_provides((eth_address, nonce))
					.longevity(64_u64)
					.propagate(true)
					.build();
			} else {
				InvalidTransaction::BadProof.into()
			}
		} else {
			InvalidTransaction::Call.into()
		}
	}
}

#[cfg(test)]
mod tests {
	use crate as crml_eth_wallet;
	use crml_eth_wallet::*;
	use frame_support::{assert_err, assert_ok, parameter_types, storage};
	use hex_literal::hex;
	use libsecp256k1 as secp256k1;
	use pallet_evm::AddressMapping;
	use sp_core::{ecdsa, keccak_256, Pair};
	use sp_runtime::{
		testing::{Header, H256},
		traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
		transaction_validity::TransactionPriority,
		MultiSignature,
	};
	use std::{convert::TryFrom, marker::PhantomData};

	pub const ECDSA_SEED: [u8; 32] = hex!("7e9c7ad85df5cdc88659f53e06fb2eb9bab3ebc59083a3190eaf2c730332529c");

	type BlockNumber = u64;
	type Signature = MultiSignature;
	type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;
	type Block = frame_system::mocking::MockBlock<Test>;
	type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;

	frame_support::construct_runtime!(
		pub enum Test where
			Block = Block,
			NodeBlock = Block,
			UncheckedExtrinsic = UncheckedExtrinsic,
		{
			System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
			EthWallet: crml_eth_wallet::{Pallet, Call, Event<T>},
		}
	);

	parameter_types! {
		pub const BlockHashCount: u64 = 250;
	}
	impl frame_system::Config for Test {
		type Origin = Origin;
		type BaseCallFilter = frame_support::traits::Everything;
		type Index = u64;
		type BlockNumber = BlockNumber;
		type Call = Call;
		type Hash = H256;
		type Hashing = BlakeTwo256;
		type AccountId = AccountId;
		type Lookup = IdentityLookup<Self::AccountId>;
		type Header = Header;
		type Event = Event;
		type BlockHashCount = BlockHashCount;
		type Version = ();
		type PalletInfo = PalletInfo;
		type AccountData = ();
		type OnNewAccount = ();
		type OnKilledAccount = ();
		type DbWeight = ();
		type SystemWeightInfo = ();
		type BlockWeights = ();
		type BlockLength = ();
		type SS58Prefix = ();
		type OnSetCode = ();
	}

	parameter_types! {
		pub const Priority: TransactionPriority = TransactionPriority::max_value();
	}
	impl Config for Test {
		type Event = Event;
		type Call = Call;
		type AddressMapping = crml_support::PrefixedAddressMapping<AccountId>;
		type Signer = <Signature as Verify>::Signer;
		type TransactionFeeHandler = MockTransactionFeeHandler<AccountId, Call>;
		type UnsignedPriority = Priority;
	}

	/// temp storage key for testing fee payment
	const MOCK_FEE_PAID: [u8; 13] = *b"MOCK_FEE_PAID";

	/// Mock transaction handler (noop)
	pub struct MockTransactionFeeHandler<AccountId, Call: Dispatchable + GetDispatchInfo + Encode> {
		phantom: PhantomData<(AccountId, Call)>,
	}
	impl<AccountId, Call: Dispatchable + GetDispatchInfo + Encode> TransactionFeeHandler
		for MockTransactionFeeHandler<AccountId, Call>
	{
		/// Ubiquitous account type
		type AccountId = AccountId;
		/// Runtime call type
		type Call = Call;
		/// pay fee for `call_info` from `account`
		fn pay_fee(
			_len: u32,
			_call: &Self::Call,
			_info: &<Self::Call as Dispatchable>::Info,
			_account: &Self::AccountId,
		) -> Result<(), ()> {
			storage::unhashed::put(&MOCK_FEE_PAID, &1u32);
			Ok(())
		}
	}

	fn new_test_ext() -> sp_io::TestExternalities {
		let storage = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();
		storage.into()
	}

	// Simple `eth_sign` implementation, should be equal to exported by RPC
	fn eth_sign(seed: &[u8; 32], data: &[u8]) -> Vec<u8> {
		let call_msg = ethereum::signable_message(data);
		let ecdsa_msg = secp256k1::Message::parse(&keccak_256(&call_msg));
		let secret = secp256k1::SecretKey::parse(&seed).expect("valid seed");
		let (signature, recovery_id) = libsecp256k1::sign(&ecdsa_msg, &secret);
		let mut out = Vec::with_capacity(65);
		out.extend_from_slice(&signature.serialize()[..]);
		// Fix recovery ID: Ethereum uses 27/28 notation
		out.push(recovery_id.serialize() + 27);
		out
	}

	#[test]
	fn eth_sign_works() {
		let seed = hex!("7e9c7ad85df5cdc88659f53e06fb2eb9bab3ebc59083a3190eaf2c730332529c");
		let text = b"Hello Plasm";
		let signature = hex!("79eec99d7f5b321c1b75d2fc044b555f9afdbc4f9b43a011085f575b216f85c452a04373d487671852dca4be4fe5fd90836560afe709d1dab45ab18bc936c2111c");
		assert_eq!(eth_sign(&seed, &text[..]), signature);
	}

	#[test]
	fn invalid_signature() {
		new_test_ext().execute_with(|| {
			let bob = EthAddress::from_low_u64_be(555);
			let call = frame_system::Call::<Test>::remark{remark: b"hello world".to_vec()}.into();
			let signature = EthereumSignature {
				0: hex!("dd0992d40e5cdf99db76bed162808508ac65acd7ae2fdc8573594f03ed9c939773e813181788fc02c3c68f3fdc592759b35f6354484343e18cb5317d34dab6c61b")
			};
			assert_err!(
				EthWallet::call(Origin::none(), Box::new(call), bob, signature),
				Error::<Test>::InvalidSignature,
			);
		});
	}

	#[test]
	fn simple_remark() {
		new_test_ext().execute_with(|| {
			let pair = ecdsa::Pair::from_seed(&ECDSA_SEED);
			let eth_address: EthAddress = hex!("420aC537F1a4f78d4Dfb3A71e902be0E3d480AFB").into();
			let cennznet_address = <Test as Config>::AddressMapping::into_account_id(eth_address);
			let call: Call = frame_system::Call::<Test>::remark {
				remark: b"hello world".to_vec(),
			}
			.into();
			let nonce = <frame_system::Pallet<Test>>::account_nonce(&cennznet_address);
			let msg = base64::encode((call.clone(), nonce).encode());
			let prefix = "data:application-octet;base64,";
			let full_msg = &[prefix.as_bytes(), msg.as_bytes()].concat()[..];
			let signature = EthereumSignature::try_from(eth_sign(&ECDSA_SEED, full_msg.as_ref())).expect("valid sig");

			// execute the call
			assert_ok!(EthWallet::call(Origin::none(), Box::new(call), eth_address, signature));

			// nonce incremented
			assert_eq!(
				<frame_system::Pallet<Test>>::account_nonce(&cennznet_address),
				nonce + 1,
			);

			// fee payment triggered
			assert_eq!(storage::unhashed::get(&MOCK_FEE_PAID), Some(1u32),);
		})
	}
}
