//! Weights for crml_nft
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0
//! DATE: 2021-04-19, STEPS: [50], REPEAT: 20, LOW RANGE: [], HIGH RANGE: []
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Compiled, CHAIN: Some("dev"), DB CACHE: 128
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::weights::{constants::RocksDbWeight as DbWeight, Weight};
use sp_std::marker::PhantomData;

impl crate::WeightInfo for () {
	fn set_owner() -> Weight {
		(1_000 as Weight)
			.saturating_add(DbWeight::get().reads(1 as Weight))
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn create_collection() -> Weight {
		(72_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(1 as Weight))
			.saturating_add(DbWeight::get().writes(3 as Weight))
	}
	fn mint_series(_q: u32) -> Weight {
		(133_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(5 as Weight))
			.saturating_add(DbWeight::get().writes(6 as Weight))
	}
	fn mint_additional(_q: u32) -> Weight {
		(133_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(5 as Weight))
			.saturating_add(DbWeight::get().writes(6 as Weight))
	}
	fn transfer() -> Weight {
		(96_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(6 as Weight))
			.saturating_add(DbWeight::get().writes(3 as Weight))
	}
	fn burn() -> Weight {
		(119_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(6 as Weight))
			.saturating_add(DbWeight::get().writes(4 as Weight))
	}
	fn sell() -> Weight {
		(69_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(3 as Weight))
			.saturating_add(DbWeight::get().writes(2 as Weight))
	}
	fn buy() -> Weight {
		(329_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(12 as Weight))
			.saturating_add(DbWeight::get().writes(10 as Weight))
	}
	fn bid() -> Weight {
		(117_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(6 as Weight))
			.saturating_add(DbWeight::get().writes(3 as Weight))
	}
	fn cancel_sale() -> Weight {
		(74_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(4 as Weight))
			.saturating_add(DbWeight::get().writes(2 as Weight))
	}
}
