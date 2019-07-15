use primitives::{Blake2Hasher, H256};
// The testing primitives are very useful for avoiding having to work with signatures
// or public keys. `u64` is used as the `AccountId` and no `Signature`s are required.
use runtime_primitives::{
	testing::{Digest, DigestItem, Header},
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage,
};

// For testing the module, we construct most of a mock runtime. This means
// first constructing a configuration type (`Test`) which `impl`s each of the
// configuration traits of modules we want to use.
#[derive(Clone, Eq, PartialEq)]
pub struct Test;
impl system::Trait for Test {
	type Origin = Origin;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type Digest = Digest;
	type AccountId = H256;
	type Lookup = IdentityLookup<H256>;
	type Header = Header;
	type Event = ();
	type Log = DigestItem;
	type Doughnut = ();
	type DispatchVerifier = ();
}

impl_outer_origin! {
	pub enum Origin for Test {}
}

// This function basically just builds a genesis storage key/value store according to
// our desired mockup.
pub fn new_test_ext() -> sr_io::TestExternalities<Blake2Hasher> {
	system::GenesisConfig::<Test>::default()
		.build_storage()
		.unwrap()
		.0
		.into()
}
