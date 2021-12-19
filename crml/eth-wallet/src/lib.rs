//! Kudos: https://github.com/PlasmNetwork/Astar/blob/08c4a9211836b929abcbad4ed33ede0f616a6423/frame/custom-signatures/
//! Provides shims for Ethereum wallets (e.g. metamask) to interact with CENNZnet
#![cfg_attr(not(feature = "std"), no_std)]

use crate::ethereum::{ecrecover, EthereumSignature};
use codec::Encode;
use crml_support::{TransactionFeeHandler, H160 as EthAddress};
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage,
	dispatch::{DispatchInfo, Dispatchable},
	traits::{Get, UnfilteredDispatchable},
	weights::GetDispatchInfo,
	Parameter,
};
use frame_system::ensure_none;
use sp_runtime::{
	traits::IdentifyAccount,
	transaction_validity::{
		InvalidTransaction, TransactionPriority, TransactionSource, TransactionValidity, ValidTransaction,
	},
	DispatchResult, SaturatedConversion,
};
use sp_std::prelude::*;

/// Ethereum-compatible signatures (eth_sign API call).
pub mod ethereum;

/// The module's configuration trait.
pub trait Config: frame_system::Config {
	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;

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
		/// Signature decode fails.
		DecodeFailure,
		/// Signature & account mismatched.
		InvalidSignature,
		/// Nonce invalid
		InvalidNonce,
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

decl_storage! {
	trait Store for Module<T: Config> as EthWallet {
		/// Mapping from Ethereum address to a CENNZnet only transaction nonce
		/// It may be higher than the CENNZnet system nonce for the same address
		pub AddressNonce get(fn stored_address_nonce): map hasher(identity) EthAddress => u32
	}
}

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

			// check the known nonce for this ethereum address
			let address_nonce = Self::stored_address_nonce(eth_address);
			let message = &(&call, address_nonce).encode()[..];

			if let Some(public_key) = ecrecover(&signature, message, &eth_address) {
				let account = T::Signer::from(public_key).into_account(); // CENNZnet address

				// it's possible this account is used normally outside of eth signing-
				// ensure highest known nonce is used
				let system_nonce = <frame_system::Pallet<T>>::account_nonce(account.clone());
				let highest_nonce = sp_std::cmp::max(system_nonce.saturated_into(), address_nonce);
				let new_nonce = highest_nonce.checked_add(1).ok_or(Error::<T>::InvalidNonce)?;

				// Pay fee, increment nonce
				let _ = Self::pay_fee(&call, &account)?;
				AddressNonce::insert(eth_address, new_nonce);
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
	/// Return the known CENNZnet nonce for a given Ethereum address
	pub fn address_nonce(eth_address: &EthAddress) -> u32 {
		Self::stored_address_nonce(eth_address)
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
			let address_nonce = Self::stored_address_nonce(eth_address);
			let message = &(&call, address_nonce).encode()[..];
			if let Some(_public_key) = ecrecover(&signature, message, &eth_address) {
				return ValidTransaction::with_tag_prefix("EthWallet")
					.priority(T::UnsignedPriority::get())
					.and_provides((eth_address, address_nonce))
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
	use sp_core::{ecdsa, keccak_256, Pair};
	use sp_runtime::{
		testing::{Header, H256},
		traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
		transaction_validity::TransactionPriority,
		MultiSignature, MultiSigner,
	};
	use std::{convert::TryFrom, marker::PhantomData};

	pub const ECDSA_SEED: [u8; 32] = hex!("7e9c7ad85df5cdc88659f53e06fb2eb9bab3ebc59083a3190eaf2c730332529c");

	type BlockNumber = u64;
	type Signature = MultiSignature;
	type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;
	type Block = frame_system::mocking::MockBlock<Runtime>;
	type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;

	frame_support::construct_runtime!(
		pub enum Runtime where
			Block = Block,
			NodeBlock = Block,
			UncheckedExtrinsic = UncheckedExtrinsic,
		{
			System: frame_system::{Module, Call, Config, Storage, Event<T>},
			EthWallet: crml_eth_wallet::{Module, Call, Event<T>},
		}
	);

	parameter_types! {
		pub const BlockHashCount: u64 = 250;
	}
	impl frame_system::Config for Runtime {
		type Origin = Origin;
		type BaseCallFilter = ();
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
	}

	parameter_types! {
		pub const Priority: TransactionPriority = TransactionPriority::max_value();
	}
	impl Config for Runtime {
		type Event = Event;
		type Call = Call;
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
		let storage = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();
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
			let call = frame_system::Call::<Runtime>::remark(b"hello world".to_vec()).into();
			let signature = EthereumSignature {
				0: hex!("dd0992d40e5cdf99db76bed162808508ac65acd7ae2fdc8573594f03ed9c939773e813181788fc02c3c68f3fdc592759b35f6354484343e18cb5317d34dab6c61b")
			};
			assert_err!(
				EthWallet::call(Origin::none(), Box::new(call), bob, signature),
				Error::<Runtime>::InvalidSignature,
			);
		});
	}

	#[test]
	fn simple_remark() {
		new_test_ext().execute_with(|| {
			let pair = ecdsa::Pair::from_seed(&ECDSA_SEED);
			let eth_address: EthAddress = hex!("420aC537F1a4f78d4Dfb3A71e902be0E3d480AFB").into();
			let cennznet_address = MultiSigner::from(pair.public()).into_account();

			let call: Call = frame_system::Call::<Runtime>::remark(b"hello world".to_vec()).into();
			let system_nonce = <frame_system::Module<Runtime>>::account_nonce(&cennznet_address);
			let module_nonce = EthWallet::address_nonce(&eth_address);
			assert_eq!(system_nonce as u32, module_nonce);
			let signature =
				EthereumSignature::try_from(eth_sign(&ECDSA_SEED, (call.clone(), module_nonce).encode().as_ref()))
					.expect("valid sig");

			// execute the call
			assert_ok!(EthWallet::call(Origin::none(), Box::new(call), eth_address, signature));

			// nonces incremented
			assert_eq!(EthWallet::address_nonce(&eth_address), module_nonce + 1,);
			assert_eq!(
				<frame_system::Module<Runtime>>::account_nonce(&cennznet_address),
				system_nonce + 1,
			);

			// fee payment triggered
			assert_eq!(storage::unhashed::get(&MOCK_FEE_PAID), Some(1u32),);
		})
	}
}
