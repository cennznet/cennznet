//! The CENNZnet runtime reexported for WebAssembly compile.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate cennznet_runtime;
pub use cennznet_runtime::*;
