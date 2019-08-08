#![cfg_attr(not(feature = "std"), no_std)]
use support::rstd::prelude::*;
use support::{
	decl_event, decl_module, decl_storage,
	dispatch::Result,
	StorageMap, StorageValue,
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
			if !<Writers<T>>::exists(&signer) {
				return Err("signer does not have write permission here");
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

		/// Set the admin user, a one-time operation
		fn bootstrap_admin(user: <T as system::Trait>::AccountId) -> Result {
			if Self::administrator() == Default::default() {
				<Administrator<T>>::mutate(|admin| *admin = user.clone());
				Self::deposit_event(RawEvent::AdminSet(user));
				return Ok(())
			}
			return Err("Administrator account is already set");
		}

		/// Add a `user` to the writer whitelist
		pub fn add_writer(origin, user: <T as system::Trait>::AccountId) -> Result {

			let signer = ensure_signed(origin)?;
			if Self::administrator() != signer {
				return Err("signer does not have permission here");
			}

			<Writers<T>>::insert(user.clone(), true);
			Self::deposit_event(RawEvent::WriterWhitelisted(user));
			Ok(())
		}

		/// Remove a `user` from the writer whitelist
		pub fn revoke_writer(origin, user: <T as system::Trait>::AccountId) -> Result {

			let signer = ensure_signed(origin)?;
			if Self::administrator() != signer {
				return Err("signer does not have permission here");
			}

			if <Writers<T>>::exists(&user) {
				<Writers<T>>::remove(user.clone());
				Self::deposit_event(RawEvent::WriterRevoked(user));
				return Ok(())
			}
			Err("user is not in the writers whitelist")
		}

	}
}

decl_event!(
	pub enum Event<T> where <T as system::Trait>::AccountId {
		// A new metadata,doughnut was stored by signer at key
		MetadataStored(AccountId, Vec<u8>, Vec<u8>, Vec<u8>),
		// The account was added to writers whitelist
		WriterWhitelisted(AccountId),
		// The account was removed from the writers whitelist
		WriterRevoked(AccountId),
		// The account was added to the administrators whitelist
		AdminSet(AccountId),
	}
);

decl_storage! {
	trait Store for Module<T: Trait> as TrustCodes {
		/// A set of whitelisted writer public keys which may store data in this module
		/// The `bool` is redundant and is a placeholder to emulate a set
		Writers: map T::AccountId => bool;
		/// The module admin public keys which may modify with the writers whitelist
		Administrator get(administrator): T::AccountId;
		/// A map of key -> metadata
		BatchMetadata: map Vec<u8> => Vec<u8>;
		/// A map of key -> doughnut
		Doughnuts: map Vec<u8> => Vec<u8>;
	}
}
