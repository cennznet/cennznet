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
use parity_codec::{Codec, Decode, Encode};

trait TestCodec: std::fmt::Debug + PartialEq + Codec {}
impl<T: std::fmt::Debug + PartialEq + Codec> TestCodec for T {}

#[derive(Debug, PartialEq, Encode, Decode)]
enum EnumV1 {
	A,
}

#[derive(Debug, PartialEq, Encode, Decode)]
enum EnumV2 {
	A,
	B(i32),
}

#[derive(Debug, PartialEq, Encode, Decode)]
enum EnumV3<T: Codec> {
	A,
	B(i32),
	C(i32, T),
}

#[derive(Debug, PartialEq, Encode, Decode)]
enum EnumV4<T1: Codec, T2: Codec> {
	A,
	B(i32),
	C(i32, T1),
	D(T2),
}

fn assert_upgrade<T1: TestCodec, T2: TestCodec>(v1: T1, v2: T2) {
	assert_eq!(T2::decode(&mut &*v1.encode()).unwrap(), v2);
	assert_eq!(v1.encode(), v2.encode());
}

#[test]
fn should_support_add_new_variant_to_enum() {
	assert_upgrade(EnumV1::A, EnumV2::A);
	assert_upgrade(EnumV2::A, EnumV3::A as EnumV3<i32>);
	assert_upgrade(EnumV2::A, EnumV3::A as EnumV3<Vec<u8>>);
	assert_upgrade(EnumV3::A as EnumV3<i32>, EnumV4::A as EnumV4<i32, i32>);
	assert_upgrade(EnumV3::A as EnumV3<Vec<u8>>, EnumV4::A as EnumV4<Vec<u8>, i32>);

	assert_upgrade(EnumV2::B(123), EnumV3::B(123) as EnumV3<i32>);
	assert_upgrade(EnumV3::B(123) as EnumV3<i32>, EnumV4::B(123) as EnumV4<i32, i32>);
	assert_upgrade(
		EnumV3::B(123) as EnumV3<Vec<u8>>,
		EnumV4::B(123) as EnumV4<Vec<u8>, i32>,
	);

	assert_upgrade(
		EnumV3::C(123, 456) as EnumV3<i32>,
		EnumV4::C(123, 456) as EnumV4<i32, i32>,
	);
	assert_upgrade(
		EnumV3::C(123, vec![1u8, 2u8]) as EnumV3<Vec<u8>>,
		EnumV4::C(123, vec![1u8, 2u8]) as EnumV4<Vec<u8>, i32>,
	);
}

#[test]
fn should_support_dependent_type_upgrades() {
	assert_upgrade(
		EnumV3::C(1, EnumV1::A) as EnumV3<EnumV1>,
		EnumV3::C(1, EnumV2::A) as EnumV3<EnumV2>,
	);
	assert_upgrade(
		EnumV4::C(1, EnumV2::B(12)) as EnumV4<EnumV2, i32>,
		EnumV4::C(1, EnumV3::B(12)) as EnumV4<EnumV3<i32>, i32>,
	);
}
