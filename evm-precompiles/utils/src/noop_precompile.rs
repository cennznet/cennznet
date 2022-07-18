// Copyright 2019-2022 Centrality Investments Ltd.
// This file is part of CENNZnet.

// CENNZnet is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CENNZnet is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CENNZnet. If not, see <http://www.gnu.org/licenses/>.

use super::prelude::*;
use fp_evm::{PrecompileFailure, PrecompileOutput};
use pallet_evm::{ExitRevert, Precompile};
use sp_std::marker::PhantomData;

/// Provides a no operation precompile for reserving precompile addresses such as FEE_PROXY
pub struct NoopPrecompile<T>(PhantomData<T>);

impl<T> Precompile for NoopPrecompile<T> {
	fn execute(_handle: &mut impl PrecompileHandle) -> EvmResult<PrecompileOutput> {
		// Return an error
		Err(PrecompileFailure::Revert {
			exit_status: ExitRevert::Reverted,
			output: ("Precompile address is reserved").as_bytes().to_vec(),
		})
	}
}

impl<T> NoopPrecompile<T> {
	pub fn new() -> Self {
		Self(PhantomData)
	}
}
