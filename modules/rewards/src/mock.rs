//! Test utilities

#![cfg(test)]

use runtime_primitives::BuildStorage;
use runtime_primitives::{
	traits::{IdentityLookup, BlakeTwo256},
	testing::{Digest, DigestItem, Header},
};
use primitives::{H256, Blake2Hasher};
use runtime_io;
use staking;
use generic_asset;
use support::{impl_outer_origin};
use crate::{GenesisConfig, Module, Trait, system};

impl_outer_origin! {
	pub enum Origin for Test {}
}

// Workaround for https://github.com/rust-lang/rust/issues/26925 . Remove when sorted.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Test;

impl system::Trait for Test {
	type Origin = Origin;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = ::primitives::traits::BlakeTwo256;
	type Digest = Digest;
	type AccountId = u64;
	type Lookup = IdentityLookup<u64>;
	type Header = Header;
	type Event = ();
	type Log = DigestItem;
}

impl generic_asset::Trait for Test {
	type Balance = u64;
	type AssetId = u32;
	type Event = ();
}

impl staking::Trait for Test {
	type Currency = generic_assets::Module<Test>;
	type OnRewardMinted = ();
	type Event = ();
}

impl Trait for Test {}
