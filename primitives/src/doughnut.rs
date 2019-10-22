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
use parity_codec::{Decode, Encode, Input};
use rstd::prelude::*;

use crate::util::encode_with_vec_prefix;
use runtime_primitives::doughnut::DoughnutV0;
use runtime_primitives::traits::DoughnutApi;

/// The CENNZnet doughnut type. It wraps an encoded v0 doughnut
/// Wrapping it like this provides length prefix support for the SCALE codec used by the extrinsic format
/// and type conversions into runtime data types
#[cfg_attr(feature = "std", derive(Debug))]
#[derive(Eq, PartialEq, Clone)]
pub struct CennznetDoughnut(DoughnutV0);

impl CennznetDoughnut {
	/// Create a new CennznetDoughnut
	pub fn new(doughnut: DoughnutV0) -> Self {
		Self(doughnut)
	}
}

impl Decode for CennznetDoughnut {
	fn decode<I: Input>(input: &mut I) -> Option<Self> {
		// This is a little more complicated than usual since the binary format must be compatible
		// with substrate's generic `Vec<u8>` type. Basically this just means accepting that there
		// will be a prefix of vector length (we don't need to use this).
		let _length_do_not_remove_me_see_above: Vec<()> = Decode::decode(input)?;
		let doughnut = DoughnutV0::decode(input)?;
		Some(CennznetDoughnut(doughnut))
	}
}

impl Encode for CennznetDoughnut {
	fn encode(&self) -> Vec<u8> {
		encode_with_vec_prefix::<Self, _>(|v| self.0.encode_to(v))
	}
}

// TODO: Convert doughnut fields to runtime types here, remove shim traits from executive
impl DoughnutApi for CennznetDoughnut {
	/// The holder and issuer account id type
	type AccountId = <DoughnutV0 as DoughnutApi>::AccountId;
	/// The expiry timestamp type
	type Timestamp = <DoughnutV0 as DoughnutApi>::Timestamp;
	/// The signature types
	type Signature = <DoughnutV0 as DoughnutApi>::Signature;
	/// Return the doughnut holder
	fn holder(&self) -> Self::AccountId {
		self.0.holder()
	}
	/// Return the doughnut issuer
	fn issuer(&self) -> Self::AccountId {
		self.0.issuer()
	}
	/// Return the doughnut expiry timestamp
	fn expiry(&self) -> Self::Timestamp {
		self.0.expiry().into()
	}
	/// Return the doughnut payload bytes
	fn payload(&self) -> Vec<u8> {
		self.0.payload()
	}
	/// Return the doughnut signature
	fn signature(&self) -> Self::Signature {
		self.0.signature()
	}
	/// Return the payload for domain, if it exists in the doughnut
	fn get_domain(&self, domain: &str) -> Option<&[u8]> {
		self.0.get_domain(domain)
	}
}
