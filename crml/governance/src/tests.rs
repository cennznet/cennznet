use super::*;
use crate::mock::{ExtBuilder, Governance};
use frame_support::assert_ok;

#[test]
fn add_council_member() {
	ExtBuilder::default().build().execute_with(|| {
		// setup token collection + one token
		let account = 1_u64;
		assert_ok!(Governance::add_council_member(
			frame_system::RawOrigin::Root.into(),
			account.into()
		));
	});
}
