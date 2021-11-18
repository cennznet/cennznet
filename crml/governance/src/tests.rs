use super::*;
use crate::mock::{AccountId, Event, ExtBuilder, GenericAsset, Governance, System, Test};
use frame_support::{assert_noop, assert_ok, traits::OnInitialize};
use sp_runtime::Permill;
/// The asset Id used for payment in these tests
// const PAYMENT_ASSET: AssetId = 16_001;

// Check the test system contains an event record `event`
// fn has_event(event: RawEvent<SubmitProposal, EnactProposal, ProposalVeto>) -> bool {
// 	System::events()
// 		.iter()
// 		.find(|e| e.event == Event::crml_governance(event.clone()))
// 		.is_some()
// }

#[test]
fn add_council_member() {
	ExtBuilder::default().build().execute_with(|| {
		// setup token collection + one token
		let account = 1_u64;

		assert_ok!(Governance::add_council_member(Some(account).into()));
	});
}
