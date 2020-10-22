// Copyright 2019-2020 Plug New Zealand Limited
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

//! Attestation benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_system::RawOrigin;

use crate::device::Module as SyloDevice;
use crate::e2ee::Module as SyloE2EE;
use crate::groups::Module as SyloGroups;
use crate::inbox::Module as SyloInbox;
use crate::payment::Module as SyloPayment;
use crate::response::Module as SyloResponse;
use crate::vault::Module as SyloVault;

const SEED: u32 = 0;

benchmarks! {
	_{ }

}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{ExtBuilder, Test};
	use frame_support::assert_ok;
}
