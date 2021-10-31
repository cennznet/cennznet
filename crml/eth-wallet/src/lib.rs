//! Kudos: https://github.com/PlasmNetwork/Astar/blob/08c4a9211836b929abcbad4ed33ede0f616a6423/frame/custom-signatures/
#![cfg_attr(not(feature = "std"), no_std)]

use crate::ethereum::{ecrecover, EthereumSignature};
use codec::Encode;
use crml_support::{TransactionFeeHandler, H160 as EthAddress};
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage,
	dispatch::{DispatchInfo, Dispatchable},
	log,
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

pub(crate) const LOG_TARGET: &str = "eth-wallet";

// syntactic sugar for logging.
#[macro_export]
macro_rules! log {
	($level:tt, $patter:expr $(, $values:expr)* $(,)?) => {
		log::$level!(
			target: crate::LOG_TARGET,
			$patter $(, $values)*
		)
	};
}

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
		/// A call just executed. \[result\]
		Execute(AccountId, DispatchResult),
	}
);

decl_storage! {
	trait Store for Module<T: Config> as EthWallet {
		/// Mapping from Ethereum address to a CENNZnet only transaction nonce
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
			address: EthAddress,
			signature: EthereumSignature,
		) -> DispatchResult {
			ensure_none(origin)?;

			// check the known nonce for this ethereum address
			let address_nonce = Self::stored_address_nonce(address);
			let message = &(&call, address_nonce).encode()[..];

			if let Some(public_key) = ecrecover(&signature, message, &address) {
				// TODO: convert ethereum address to CENNZnet account type
				log!(trace, "eth address: {:?}, public key: {:?}\n", address, public_key);
				// 032df450513c73c9ff7a6e4f677767bb02807a27e93b016cc90ce6fd69677bc939
				// eca51cdc998e42bfa50c6700804fd133fe87401512b9017dc304c4297776031f
				let account = T::Signer::from(public_key).into_account();

				// ensure system nonce and nonce known by this pallet are synched
				let system_nonce = <frame_system::Module<T>>::account_nonce(account.clone());
				let highest_nonce = sp_std::cmp::max(system_nonce.saturated_into(), address_nonce);
				let new_nonce = highest_nonce.checked_add(1).ok_or(Error::<T>::InvalidNonce)?;
				AddressNonce::insert(address, new_nonce);
				log!(trace, "eth address: {:?}, cennznet account: {:?}\n", address, account);

				// execute the call
				let _ = Self::pay_fee(&call, &account)?;
				let new_origin = frame_system::RawOrigin::Signed(account.clone()).into();
				let res = call.dispatch_bypass_filter(new_origin).map(|_| ());
				Self::deposit_event(RawEvent::Execute(account, res.map_err(|e| e.error)));

				Ok(())
			} else {
				Err(Error::<T>::InvalidSignature)?
			}
		}
	}
}

impl<T: Config> Module<T> {
	fn pay_fee(call: &<T as Config>::Call, account: &T::AccountId) -> DispatchResult {
		let info = call.get_dispatch_info();
		let len = call.clone().encode()[..].len() as u32 + 33 + 65 + 4; // call + account, signature, nonce bytes

		match T::TransactionFeeHandler::pay_fee(len, call, &info, account) {
			Ok(_) => Ok(()),
			Err(_) => Err(Error::<T>::CantPay)?,
		}
	}
	/// Return the known CENNZnet nonce for a given Ethereum address
	pub fn address_nonce(eth_address: &EthAddress) -> u32 {
		Self::stored_address_nonce(eth_address)
	}
}

impl<T: Config> frame_support::unsigned::ValidateUnsigned for Module<T> {
	type Call = Call<T>;

	fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
		if let Call::call(call, address, signature) = call {
			let address_nonce = Self::stored_address_nonce(address);
			let message = &(&call, address_nonce).encode()[..];
			if let Some(_public_key) = ecrecover(&signature, message, &address) {
				return ValidTransaction::with_tag_prefix("EthWallet")
					.priority(T::UnsignedPriority::get())
					.and_provides((address, address_nonce))
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
	use std::marker::PhantomData;
	use std::convert::TryInto;
	use crate as crml_eth_wallet;
	use libsecp256k1 as secp256k1;
	use crml_eth_wallet::*;
	use frame_support::{assert_err, assert_ok, parameter_types};
	use frame_system::mocking::MockBlock;
	use hex_literal::hex;
	use sp_core::{crypto::Ss58Codec, ecdsa, keccak_256, Pair};
	use sp_keyring::AccountKeyring as Keyring;
	use sp_runtime::{
		testing::{Header, H256},
		traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
		transaction_validity::TransactionPriority,
		MultiSignature, MultiSigner,
	};

	pub const ECDSA_SEED: [u8; 32] = hex!("7e9c7ad85df5cdc88659f53e06fb2eb9bab3ebc59083a3190eaf2c730332529c");

	type Balance = u128;
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

	struct MockTransactionFeeHandler<AccountId, Call: Dispatchable + GetDispatchInfo + Encode> {
		phantom: PhantomData<(AccountId, Call)>
	}
	impl<AccountId, Call: Dispatchable + GetDispatchInfo + Encode> TransactionFeeHandler for MockTransactionFeeHandler<AccountId, Call> {
		/// Ubiquitous account type
		type AccountId = AccountId;
		/// Runtime call type
		type Call = Call;
		/// pay fee for `call_info` from `account`
		fn pay_fee(
			len: u32,
			call: &Self::Call,
			info: &<Self::Call as Dispatchable>::Info,
			account: &Self::AccountId,
		) -> Result<(), ()> {
			Ok(())
		}
	}

	fn new_test_ext() -> sp_io::TestExternalities {
		let mut storage = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		let pair = ecdsa::Pair::from_seed(&ECDSA_SEED);
		let account = MultiSigner::from(pair.public()).into_account();

		storage.into()
	}

	// Simple `eth_sign` implementation, should be equal to exported by RPC
	fn eth_sign(seed: &[u8; 32], data: &[u8]) -> Vec<u8> {
		let call_msg = ethereum::signable_message(data);
		// TODO: derive ethereum address here
		let ecdsa_msg = secp256k1::Message::parse(&keccak_256(&call_msg));
		let secret = secp256k1::SecretKey::parse(&seed).expect("valid seed");
		let mut ecdsa: ecdsa::Signature = secp256k1::sign(&ecdsa_msg, &secret).try_into().unwrap;
		// Fix recovery ID: Ethereum uses 27/28 notation
		ecdsa.as_mut()[64] += 27;
		Vec::from(ecdsa.as_ref() as &[u8])
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
		let bob: <Runtime as frame_system::Config>::AccountId = Keyring::Bob.into();
		let alice: <Runtime as frame_system::Config>::AccountId = Keyring::Alice.into();
		let call = frame_system::Call::<Runtime>::remark(b"hello world".to_vec()).into();
		let signature = Vec::from(&hex!("dd0992d40e5cdf99db76bed162808508ac65acd7ae2fdc8573594f03ed9c939773e813181788fc02c3c68f3fdc592759b35f6354484343e18cb5317d34dab6c61b")[..]);
		assert_err!(
			EthWallet::call(Origin::none(), Box::new(call), bob, signature),
			Error::<Runtime>::InvalidSignature,
		);
	}

	#[test]
	fn simple_remark() {
		new_test_ext().execute_with(|| {
			let pair = ecdsa::Pair::from_seed(&ECDSA_SEED);
			let account = MultiSigner::from(pair.public()).into_account();

			let alice: <Runtime as frame_system::Config>::AccountId = Keyring::Alice.into();

			let call: Call = frame_system::Call::<Runtime>::remark(b"hello world".to_vec()).into();
			let nonce = <frame_system::Module<Runtime>>::account_nonce(&account);
			let signature = eth_sign(&ECDSA_SEED, (call, nonce).encode().as_ref()).into();

			assert_ok!(EthWallet::call(
				Origin::none(),
				Box::new(call),
				account,
				signature
			));
		})
	}

	#[test]
	fn call_fixtures() {
		let seed = hex!("7e9c7ad85df5cdc88659f53e06fb2eb9bab3ebc59083a3190eaf2c730332529c");
		let pair = ecdsa::Pair::from_seed(&seed);
		assert_eq!(
			MultiSigner::from(pair.public()).into_account().to_ss58check(),
			"5Geeci7qCoYHyg9z2AwfpiT4CDryvxYyD7SAUdfNBz9CyDSb",
		);

		let dest = AccountId::from_ss58check("5GVwcV6EzxxYbXBm7H6dtxc9TCgL4oepMXtgqWYEc3VXJoaf").unwrap();
		let call: Call = frame_system::Call::<Runtime>::remark(b"hello world".to_vec()).into();
		assert_eq!(
			call.encode(),
			hex!("0000c4305fb88b6ccb43d6552dc11d18e7b0ee3185247adcc6e885eb284adf6c563da10f"),
		);

		let signature = hex!("96cd8087ef720b0ec10d96996a8bbb45005ba3320d1dde38450a56f77dfd149720cc2e6dcc8f09963aad4cdf5ec15e103ce56d0f4c7a753840217ef1787467a01c");
		assert_eq!(eth_sign(&seed, call.encode().as_ref()), signature)
	}
}
