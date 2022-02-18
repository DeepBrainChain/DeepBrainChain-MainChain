//! Autogenerated weights for committee
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 3.0.0
//! DATE: 2022-02-18, STEPS: [10, ], REPEAT: 20, LOW RANGE: [], HIGH RANGE: []
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Interpreted, CHAIN: Some("dev"), DB CACHE: 128

// Executed Command:
// ./target/release/dbc-chain
// benchmark
// --chain
// dev
// --execution=wasm
// --pallet
// committee
// --extrinsic
// add_committee
// --steps
// 10
// --repeat
// 20
// --output
// ./pallets/committee/src/weights.rs
// --template=./scripts/frame-weight-template.hbs


#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use sp_std::marker::PhantomData;

/// Weight functions needed for committee.
pub trait WeightInfo {
	fn add_committee(u: u32, ) -> Weight;
}

/// Weights for committee using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn add_committee(u: u32, ) -> Weight {
		(45_961_000 as Weight)
			// Standard Error: 1_000
			.saturating_add((3_000 as Weight).saturating_mul(u as Weight))
			.saturating_add(T::DbWeight::get().reads(1 as Weight))
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	fn add_committee(u: u32, ) -> Weight {
		(45_961_000 as Weight)
			// Standard Error: 1_000
			.saturating_add((3_000 as Weight).saturating_mul(u as Weight))
			.saturating_add(RocksDbWeight::get().reads(1 as Weight))
			.saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
}
