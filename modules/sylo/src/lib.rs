#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
extern crate serde;

extern crate parity_codec as codec;
#[macro_use] extern crate parity_codec_derive;
extern crate substrate_primitives;
#[cfg_attr(not(feature = "std"), macro_use)]
extern crate sr_std as rstd;
extern crate sr_io as runtime_io;
#[macro_use] extern crate srml_support;
extern crate sr_primitives as primitives;
extern crate srml_balances as balances;
extern crate srml_system as system;

pub mod device;
pub mod inbox;
pub mod groups;

pub use groups::{Trait, Module, RawEvent, Event};