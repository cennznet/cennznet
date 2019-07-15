//!
//! The DispatchVerifier impl for the doughnuts in the CENNZnet permission domain
//!
use crate::Runtime;
use cennznet_primitives::CennznetDoughnut;
use cennznut::CENNZnutV0;
use parity_codec::Decode;
use runtime_primitives::traits::DoughnutApi;
use support::additional_traits::DispatchVerifier;

impl DispatchVerifier<CennznetDoughnut> for Runtime {
	const DOMAIN: &'static str = "cennznet";

	fn verify(doughnut: &CennznetDoughnut, module: &str, method: &str) -> Result<(), &'static str> {
		let mut domain = doughnut
			.get_domain(Self::DOMAIN)
			.ok_or("Doughnut does not grant permission for cennznet domain")?;
		let cennznut: CENNZnutV0 = Decode::decode(&mut domain).ok_or("Bad CENNZnut encoding")?;
		// Strips [c|p|s]rml- prefix
		let module = cennznut
			.get_module(&module[5..])
			.ok_or("Doughnut does not grant permission for module")?;
		module
			.get_method(method)
			.map(|_| ())
			.ok_or("Doughnut does not grant permission for method")
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use cennznut::{self};
	use parity_codec::Encode;
	use runtime_primitives::doughnut::DoughnutV0;
	use support::{assert_err, assert_ok};

	// A helper to make test doughnuts
	fn make_doughnut(domain: &str, domain_payload: Vec<u8>) -> CennznetDoughnut {
		let doughnut = DoughnutV0 {
			holder: Default::default(),
			issuer: Default::default(),
			domains: vec![(domain.to_string(), domain_payload)],
			expiry: 0,
			not_before: 0,
			payload_version: 0,
			signature_version: 0,
			signature: Default::default(),
		};
		CennznetDoughnut::new(doughnut)
	}

	// A helper to make test CENNZnuts
	fn make_cennznut(module: &str, method: &str) -> CENNZnutV0 {
		let method_obj = cennznut::Method {
			name: method.to_string(),
			block_cooldown: None,
		};
		let module_obj = cennznut::Module {
			name: module.to_string(),
			block_cooldown: None,
			methods: vec![(method.to_string(), method_obj)],
		};
		CENNZnutV0 {
			modules: vec![(module.to_string(), module_obj)],
		}
	}

	#[test]
	fn it_works() {
		let cennznut = make_cennznut("attestation", "attest");
		let doughnut = make_doughnut("cennznet", cennznut.encode());
		assert_ok!(<Runtime as DispatchVerifier<CennznetDoughnut>>::verify(
			&doughnut,
			"trml-attestation",
			"attest"
		));
	}

	#[test]
	fn it_fails_when_not_using_the_cennznet_domain() {
		let doughnut = make_doughnut("test", Default::default());
		assert_err!(
			<Runtime as DispatchVerifier<CennznetDoughnut>>::verify(&doughnut, "trml-module", "method"),
			"Doughnut does not grant permission for cennznet domain"
		);
	}

	#[test]
	fn it_fails_with_bad_cennznut_encoding() {
		let doughnut = make_doughnut("cennznet", vec![1, 2, 3, 4, 5]);
		assert_err!(
			<Runtime as DispatchVerifier<CennznetDoughnut>>::verify(&doughnut, "trml-module", "method"),
			"Bad CENNZnut encoding"
		);
	}

	#[test]
	fn it_fails_when_module_is_not_authorized() {
		let cennznut = make_cennznut("attestation", "attest");
		let doughnut = make_doughnut("cennznet", cennznut.encode());
		assert_err!(
			<Runtime as DispatchVerifier<CennznetDoughnut>>::verify(&doughnut, "trml-generic-asset", "attest"),
			"Doughnut does not grant permission for module"
		);
	}

	#[test]
	fn it_fails_when_method_is_not_authorized() {
		let cennznut = make_cennznut("attestation", "attest");
		let doughnut = make_doughnut("cennznet", cennznut.encode());
		assert_err!(
			<Runtime as DispatchVerifier<CennznetDoughnut>>::verify(&doughnut, "trml-attestation", "remove"),
			"Doughnut does not grant permission for method"
		);
	}

}
