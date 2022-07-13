use std::sync::Arc;

use sc_keystore::LocalKeystore;
use sp_application_crypto::Pair as _PairT;
use sp_core::{ecdsa, keccak_256};
use sp_keystore::SyncCryptoStorePtr;

use cennznet_primitives::eth::crypto::{AuthorityId as Public, AuthorityPair as Pair, AuthoritySignature as Signature};

/// Set of test accounts using [`cennznet_primitives::eth::crypto`] types.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, strum::EnumIter)]
pub(crate) enum Keyring {
	Alice,
	Bob,
	Charlie,
	Dave,
	Eve,
	Ferdie,
	One,
	Two,
}

impl Keyring {
	/// Sign `msg`.
	pub fn sign(self, msg: &[u8]) -> Signature {
		let msg = keccak_256(msg);
		ecdsa::Pair::from(self).sign_prehashed(&msg).into()
	}

	/// Return key pair.
	pub fn pair(self) -> Pair {
		ecdsa::Pair::from_string(self.to_seed().as_str(), None).unwrap().into()
	}

	/// Return public key.
	pub fn public(self) -> Public {
		self.pair().public()
	}

	/// Return seed string.
	pub fn to_seed(self) -> String {
		format!("//{}", self)
	}
}

impl From<Keyring> for Pair {
	fn from(k: Keyring) -> Self {
		k.pair()
	}
}

impl From<Keyring> for ecdsa::Pair {
	fn from(k: Keyring) -> Self {
		k.pair().into()
	}
}

pub fn keystore() -> SyncCryptoStorePtr {
	Arc::new(LocalKeystore::in_memory())
}
