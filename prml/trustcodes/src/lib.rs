#![cfg_attr(not(feature = "std"), no_std)]
use support::rstd::prelude::*;
use support::{
	decl_event, decl_module, decl_storage,
	dispatch::Result,
	StorageMap,
};
use system::ensure_signed;

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event<T>() = default;

		/// Store some `metadata` and a `doughnut` against `key`
		pub fn store(origin, key: Vec<u8>, metadata: Vec<u8>, doughnut: Vec<u8>) -> Result {
			let signer = ensure_signed(origin)?;
			if !<Whitelist<T>>::exists(&signer) {
				return Err("signer is not trusted");
			}

			if !<BatchMetadata<T>>::exists(&key) {
				<BatchMetadata<T>>::insert(key.clone(), metadata.clone());
			}

			if !<Doughnuts<T>>::exists(&key) {
				<Doughnuts<T>>::insert(key.clone(), doughnut.clone());
				Self::deposit_event(RawEvent::MetadataStored(signer, key, metadata, doughnut));
			}

			Ok(())
		}

		/// Whitelist an account
		pub fn whitelist_add(user: <T as system::Trait>::AccountId) -> Result {
			<Whitelist<T>>::insert(user.clone(), true);
			Self::deposit_event(RawEvent::Whitelisted(user));
			Ok(())
		}

		/// Remove an account from the whitelist
		pub fn whitelist_remove(user: <T as system::Trait>::AccountId) -> Result {
			if <Whitelist<T>>::exists(&user) {
				<Whitelist<T>>::remove(user.clone());
				Self::deposit_event(RawEvent::WhitelistRevoked(user));
				return Ok(())
			}
			Err("user is not in the whitelist")
		}
	}
}

decl_event!(
	pub enum Event<T> where <T as system::Trait>::AccountId {
		// A new metadata,doughnut was stored by signer at key
		MetadataStored(AccountId, Vec<u8>, Vec<u8>, Vec<u8>),
		// The account was added to the whitelist
		Whitelisted(AccountId),
		// The account was removed from the whitelist
		WhitelistRevoked(AccountId),
	}
);

decl_storage! {
	trait Store for Module<T: Trait> as TrustCodes {
		/// A set of whitelisted public keys. The `bool` is an irrelevant placeholder value to emulate a set
		Whitelist: map T::AccountId => bool;
		/// A map of key -> metadata
		BatchMetadata: map Vec<u8> => Vec<u8>;
		/// A map of key -> doughnut
		Doughnuts: map Vec<u8> => Vec<u8>;
	}
}
