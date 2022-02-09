/* Copyright 2019-2021 Centrality Investments Limited
*
* Licensed under the LGPL, Version 3.0 (the "License");
* you may not use this file except in compliance with the License.
* Unless required by applicable law or agreed to in writing, software
* distributed under the License is distributed on an "AS IS" BASIS,
* WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
* See the License for the specific language governing permissions and
* limitations under the License.
* You may obtain a copy of the License at the root of this project source code,
* or at:
*     https://centrality.ai/licenses/gplv3.txt
*     https://centrality.ai/licenses/lgplv3.txt
*/

use super::*;
use crate::mock::{AccountId, Event, ExtBuilder, GenericAsset, Nft, System, Test, TokenApprovals};
use cennznet_primitives::types::{CollectionId, SerialNumber, SeriesId, TokenId};
use crml_nft::MetadataScheme;
use frame_support::{assert_noop, assert_ok, traits::OnInitialize};
use sp_runtime::Permill;

#[test]
fn set_erc721_approval() {
	ExtBuilder::default().build().execute_with(|| {
		// setup token collection + one token
		let caller = 0u64;
		let operator = 1u64;
		let token_id: TokenId = (0, 0, 0);

		assert!(!ERC721Approvals::<Test>::contains_key(token_id));
		assert_ok!(TokenApprovals::erc721_approval(None.into(), caller, operator, token_id));
		assert_eq!(TokenApprovals::erc721_approvals(token_id), operator);
	});
}

#[test]
fn set_erc721_approval_not_token_owner_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		// setup token collection + one token
		let caller = 1u64;
		let operator = 2u64;
		let token_id: TokenId = (0, 0, 0);

		assert_noop!(
			TokenApprovals::erc721_approval(None.into(), caller, operator, token_id),
			Error::<Test>::NotTokenOwner,
		);
		assert!(!ERC721Approvals::<Test>::contains_key(token_id));
	});
}

#[test]
fn set_erc721_approval_caller_is_operator_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		// setup token collection + one token
		let caller = 0u64;
		let operator = 0u64;
		let token_id: TokenId = (0, 0, 0);

		assert_noop!(
			TokenApprovals::erc721_approval(None.into(), caller, operator, token_id),
			Error::<Test>::CallerNotOperator,
		);
		assert!(!ERC721Approvals::<Test>::contains_key(token_id));
	});
}

#[test]
fn erc721_approval_removed_on_transfer() {
	ExtBuilder::default().build().execute_with(|| {
		// setup token collection + one token
		let caller = 0u64;
		let operator = 1u64;
		let token_id: TokenId = (0, 0, 0);

		assert_ok!(TokenApprovals::erc721_approval(None.into(), caller, operator, token_id));
		assert_eq!(TokenApprovals::erc721_approvals(token_id), operator);
		TokenApprovals::on_nft_transfer(&token_id);
		assert!(!ERC721Approvals::<Test>::contains_key(token_id));
	});
}
