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

			if !<BatchMetadata<T>>::exists(&key) {
				<BatchMetadata<T>>::insert(key.clone(), metadata.clone());
			}

			if !<Doughnuts<T>>::exists(&key) {
				<Doughnuts<T>>::insert(key.clone(), doughnut.clone());
				Self::deposit_event(RawEvent::MetadataStored(signer, key, metadata, doughnut));
			}

			Ok(())
		}
	}
}

decl_event!(
	pub enum Event<T> where <T as system::Trait>::AccountId {
		MetadataStored(AccountId, Vec<u8>, Vec<u8>, Vec<u8>),
	}
);

decl_storage! {
	trait Store for Module<T: Trait> as TrustCodes {
		/// A map of key -> metadata
		BatchMetadata: map Vec<u8> => Vec<u8>;
		/// A map of key -> doughnut
		Doughnuts: map Vec<u8> => Vec<u8>;
	}
}
