// Copyright (C) 2019 Centrality Investments Limited
// This file is part of CENNZnet.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
extern crate serde;

extern crate parity_codec as codec;
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
