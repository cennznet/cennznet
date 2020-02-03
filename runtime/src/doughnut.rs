#[cfg(test)]
mod test {
	use crate::{CennznetDoughnut, Runtime};
	use cennznut::{self};
	use cennznut::{CENNZnut, CENNZnutV0};
	use codec::Encode;
	use sp_runtime::DoughnutV0;
	use frame_support::additional_traits::DelegatedDispatchVerifier;
	use frame_support::{assert_err, assert_ok};

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

	fn verify_dispatch(doughnut: &CennznetDoughnut, module: &str, method: &str) -> Result<(), &'static str> {
		<Runtime as DelegatedDispatchVerifier<CennznetDoughnut>>::verify_dispatch(doughnut, module, method)
	}

	// A helper to make test CENNZnuts
	fn make_cennznut(module: &str, method: &str) -> CENNZnut {
		let method_obj = cennznut::Method {
			name: method.to_string(),
			block_cooldown: None,
			constraints: None,
		};
		let module_obj = cennznut::Module {
			name: module.to_string(),
			block_cooldown: None,
			methods: vec![(method.to_string(), method_obj)],
		};
		CENNZnut::V0(CENNZnutV0 {
			modules: vec![(module.to_string(), module_obj)],
		})
	}

	#[test]
	fn it_works() {
		let cennznut = make_cennznut("attestation", "attest");
		let doughnut = make_doughnut("cennznet", cennznut.encode());
		assert_ok!(verify_dispatch(&doughnut, "trml-attestation", "attest"));
	}

	#[test]
	fn it_works_with_arbitrary_prefix_short() {
		let cennznut = make_cennznut("attestation", "attest");
		let doughnut = make_doughnut("cennznet", cennznut.encode());
		assert_ok!(verify_dispatch(&doughnut, "t-attestation", "attest"));
	}

	#[test]
	fn it_works_with_arbitrary_prefix_long() {
		let cennznut = make_cennznut("attestation", "attest");
		let doughnut = make_doughnut("cennznet", cennznut.encode());
		assert_ok!(verify_dispatch(&doughnut, "trmlcrml-attestation", "attest"));
	}

	#[test]
	fn it_fails_when_not_using_the_cennznet_domain() {
		let doughnut = make_doughnut("test", Default::default());
		assert_err!(
			verify_dispatch(&doughnut, "trml-module", "method"),
			"CENNZnut does not grant permission for cennznet domain"
		);
	}

	#[test]
	fn it_fails_with_bad_cennznut_encoding() {
		let doughnut = make_doughnut("cennznet", vec![1, 2, 3, 4, 5]);
		assert_err!(
			verify_dispatch(&doughnut, "trml-module", "method"),
			"Bad CENNZnut encoding"
		);
	}

	#[test]
	fn it_fails_when_module_is_not_authorized() {
		let cennznut = make_cennznut("attestation", "attest");
		let doughnut = make_doughnut("cennznet", cennznut.encode());
		assert_err!(
			verify_dispatch(&doughnut, "trml-generic-asset", "attest"),
			"CENNZnut does not grant permission for module"
		);
	}

	#[test]
	fn it_fails_when_method_is_not_authorized() {
		let cennznut = make_cennznut("attestation", "attest");
		let doughnut = make_doughnut("cennznet", cennznut.encode());
		assert_err!(
			verify_dispatch(&doughnut, "trml-attestation", "remove"),
			"CENNZnut does not grant permission for method"
		);
	}

	#[test]
	fn it_fails_when_module_name_is_invalid() {
		let cennznut = make_cennznut("attestation", "attest");
		let doughnut = make_doughnut("cennznet", cennznut.encode());
		assert_err!(
			verify_dispatch(&doughnut, "trmlattestation", "attest"),
			"error during module name segmentation"
		);
	}

	#[test]
	fn it_fails_when_prefix_is_empty() {
		let cennznut = make_cennznut("attestation", "attest");
		let doughnut = make_doughnut("cennznet", cennznut.encode());
		assert_err!(
			verify_dispatch(&doughnut, "-attestation", "attest"),
			"error during module name segmentation"
		);
	}

	#[test]
	fn it_fails_when_module_name_is_empty() {
		let cennznut = make_cennznut("attestation", "attest");
		let doughnut = make_doughnut("cennznet", cennznut.encode());
		assert_err!(
			verify_dispatch(&doughnut, "trml-", "attest"),
			"error during module name segmentation"
		);
	}

	#[test]
	fn it_fails_when_module_name_and_prefix_are_empty() {
		let cennznut = make_cennznut("attestation", "attest");
		let doughnut = make_doughnut("cennznet", cennznut.encode());
		assert_err!(
			verify_dispatch(&doughnut, "-", "attest"),
			"error during module name segmentation"
		);
	}
}
