extern crate parity_codec;
#[macro_use]
extern crate parity_codec_derive;

use parity_codec::Codec;

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
fn shoul_support_depdent_type_upgrades() {
	assert_upgrade(
		EnumV3::C(1, EnumV1::A) as EnumV3<EnumV1>,
		EnumV3::C(1, EnumV2::A) as EnumV3<EnumV2>,
	);
	assert_upgrade(
		EnumV4::C(1, EnumV2::B(12)) as EnumV4<EnumV2, i32>,
		EnumV4::C(1, EnumV3::B(12)) as EnumV4<EnumV3<i32>, i32>,
	);
}
