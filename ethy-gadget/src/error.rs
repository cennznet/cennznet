// Copyright (C) 2020-2021 Parity Technologies (UK) Ltd. & Centrality Investments Ltd
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! ETHY gadget specific errors
//!
//! Used for ETHY gadget interal error handling only

use std::fmt::Debug;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum Error {
	#[error("Keystore error: {0}")]
	Keystore(String),
	#[error("Signature error: {0}")]
	Signature(String),
}
