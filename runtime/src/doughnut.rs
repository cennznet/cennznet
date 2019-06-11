//!
//! The DoughnutVerifier impl for the CENNZnet permission domain
//!
use crate::Runtime;
use cennznet_primitives::Doughnut;
use cennznut::CENNZnutV0;
use parity_codec::Decode;
use support::additional_traits::DoughnutVerifier;

impl DoughnutVerifier<Doughnut> for Runtime {
	const DOMAIN: &'static str = "cennznet";

	fn verify_doughnut(doughnut: &Doughnut, module: &str, method: &str) -> Result<(), &'static str> {
		if !doughnut.domains.contains_key(Self::DOMAIN) {
			return Err("Doughnut does not grant permission for domain");
		}
		let cennznut: CENNZnutV0 = Decode::decode(&mut &doughnut.domains[Self::DOMAIN][..]).ok_or("Bad CENNZnut encoding")?;
		// TODO: Strip `[p|s|c]rml-` quick fix. Need research into better options
		if !cennznut.modules.contains_key(&module[5..]) {
			return Err("Doughnut does not grant permission for module");
		}
		if !cennznut.modules[&module[5..]].methods.contains_key(method) {
			return Err("Doughnut does not grant permission for method");
		}

		Ok(())
	}
}

// TODO: Do we have a race condition / threading issue where doughnut is being shared??