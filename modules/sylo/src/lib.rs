#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
extern crate serde;

extern crate parity_codec as codec;
extern crate parity_codec_derive;
extern crate sr_io as runtime_io;
extern crate sr_std as rstd;
extern crate substrate_primitives;
#[macro_use]
extern crate srml_support;
extern crate sr_primitives as primitives;
extern crate srml_balances as balances;
extern crate srml_system as system;

pub mod device;
pub mod e2ee;
pub mod groups;
pub mod inbox;
pub mod response;
pub mod vault;

pub use rstd::vec;
