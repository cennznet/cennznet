// Copyright 2019 Centrality Investments Limited
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
extern crate serde;

extern crate parity_codec as codec;
extern crate primitives;
extern crate sr_io as runtime_io;
extern crate sr_std as rstd;
#[macro_use]
extern crate srml_support;
extern crate runtime_primitives;
extern crate srml_balances as balances;
extern crate srml_system as system;

pub mod device;
pub mod e2ee;
pub mod groups;
pub mod inbox;
pub mod response;
pub mod vault;

#[cfg(test)]
pub(crate) mod mock;

pub use rstd::vec;
