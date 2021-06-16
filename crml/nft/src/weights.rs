//! Weights for crml_nft
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0
//! DATE: 2021-04-19, STEPS: [50], REPEAT: 20, LOW RANGE: [], HIGH RANGE: []
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Compiled, CHAIN: Some("dev"), DB CACHE: 128
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::weights::{constants::RocksDbWeight as DbWeight, Weight};
use sp_std::marker::PhantomData;

/// NFT module weights
pub trait WeightInfo {
	fn set_owner() -> Weight;
	fn create_collection() -> Weight;
	fn mint_series(q: u32) -> Weight;
	fn mint_additional(q: u32) -> Weight;
	fn transfer() -> Weight;
	fn burn() -> Weight;
	fn sell() -> Weight;
	fn buy() -> Weight;
	fn bid() -> Weight;
	fn cancel_sale() -> Weight;
}

impl WeightInfo for () {
	fn set_owner() -> Weight {
		(16_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(1 as Weight))
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn create_collection() -> Weight {
		(51_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(1 as Weight))
			.saturating_add(DbWeight::get().writes(5 as Weight))
	}
	fn mint_additional(q: u32) -> Weight {
		(79_200_000 as Weight)
			// Standard Error: 2_166_000
			.saturating_add((3_536_000 as Weight).saturating_mul(q as Weight))
			.saturating_add(DbWeight::get().reads(4 as Weight))
			.saturating_add(DbWeight::get().writes(2 as Weight))
			.saturating_add(DbWeight::get().writes((1 as Weight).saturating_mul(q as Weight)))
	}
	fn mint_series(q: u32) -> Weight {
		(74_033_000 as Weight)
			// Standard Error: 58_000
			.saturating_add((4_321_000 as Weight).saturating_mul(q as Weight))
			.saturating_add(DbWeight::get().reads(2 as Weight))
			.saturating_add(DbWeight::get().writes(6 as Weight))
			.saturating_add(DbWeight::get().writes((1 as Weight).saturating_mul(q as Weight)))
	}
	fn transfer() -> Weight {
		(51_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(2 as Weight))
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn burn() -> Weight {
		(65_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(3 as Weight))
			.saturating_add(DbWeight::get().writes(2 as Weight))
	}
	fn sell() -> Weight {
		(93_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(5 as Weight))
			.saturating_add(DbWeight::get().writes(5 as Weight))
	}
	fn buy() -> Weight {
		(477_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(13 as Weight))
			.saturating_add(DbWeight::get().writes(11 as Weight))
	}
	fn bid() -> Weight {
		(165_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(8 as Weight))
			.saturating_add(DbWeight::get().writes(5 as Weight))
	}
	fn cancel_sale() -> Weight {
		(60_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(2 as Weight))
			.saturating_add(DbWeight::get().writes(4 as Weight))
	}
}
