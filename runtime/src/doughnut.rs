//!
//! The DispatchVerifier impl for the doughnuts in the CENNZnet permission domain
//!
use crate::Runtime;
use cennznet_primitives::Doughnut;
use cennznut::CENNZnutV0;
use parity_codec::Decode;
use runtime_primitives::traits::DoughnutApi;
use support::additional_traits::DispatchVerifier;

impl DispatchVerifier<Doughnut> for Runtime {
	const DOMAIN: &'static str = "cennznet";

	fn verify(doughnut: &Doughnut, module: &str, method: &str) -> Result<(), &'static str> {
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
