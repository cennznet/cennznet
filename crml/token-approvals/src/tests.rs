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
use crate::mock::{ExtBuilder, Test, TokenApprovals};
use cennznet_primitives::types::TokenId;
use frame_support::{assert_noop, assert_ok};
use hex_literal::hex;

#[test]
fn set_erc721_approval() {
	ExtBuilder::default().build().execute_with(|| {
		let caller = H160::from_slice(&hex!("a86e122EdbDcBA4bF24a2Abf89F5C230b37DF49d"));
		let operator = H160::from_slice(&hex!("1000000000000000000000000000000000000000"));
		let token_id: TokenId = (0, 0, 0);

		assert!(!ERC721Approvals::contains_key(token_id));
		assert_ok!(TokenApprovals::erc721_approval(None.into(), caller, operator, token_id));
		assert_eq!(TokenApprovals::erc721_approvals(token_id), operator);
	});
}

#[test]
fn set_erc721_approval_not_token_owner_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let caller = H160::default();
		let operator = H160::from_slice(&hex!("1000000000000000000000000000000000000000"));
		let token_id: TokenId = (0, 0, 0);

		assert_noop!(
			TokenApprovals::erc721_approval(None.into(), caller, operator, token_id),
			Error::<Test>::NotTokenOwner,
		);
		assert!(!ERC721Approvals::contains_key(token_id));
	});
}

#[test]
fn set_erc721_approval_caller_is_operator_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let caller = H160::from_slice(&hex!("a86e122EdbDcBA4bF24a2Abf89F5C230b37DF49d"));
		let operator = H160::from_slice(&hex!("a86e122EdbDcBA4bF24a2Abf89F5C230b37DF49d"));
		let token_id: TokenId = (0, 0, 0);

		assert_noop!(
			TokenApprovals::erc721_approval(None.into(), caller, operator, token_id),
			Error::<Test>::CallerNotOperator,
		);
		assert!(!ERC721Approvals::contains_key(token_id));
	});
}

#[test]
fn erc721_approval_removed_on_transfer() {
	ExtBuilder::default().build().execute_with(|| {
		let caller = H160::from_slice(&hex!("a86e122EdbDcBA4bF24a2Abf89F5C230b37DF49d"));
		let operator = H160::from_slice(&hex!("1000000000000000000000000000000000000000"));
		let token_id: TokenId = (0, 0, 0);

		assert_ok!(TokenApprovals::erc721_approval(None.into(), caller, operator, token_id));
		assert_eq!(TokenApprovals::erc721_approvals(token_id), operator);
		TokenApprovals::on_nft_transfer(&token_id);
		assert!(!ERC721Approvals::contains_key(token_id));
	});
}

#[test]
fn set_erc20_approval() {
	ExtBuilder::default().build().execute_with(|| {
		let caller = H160::default();
		let spender = H160::from_slice(&hex!("1000000000000000000000000000000000000000"));
		let asset_id: AssetId = 0;
		let amount: Balance = 10;

		assert!(!ERC20Approvals::contains_key((caller, asset_id), spender));
		assert_ok!(TokenApprovals::erc20_approval(
			None.into(),
			caller,
			spender,
			asset_id,
			amount
		));
		assert_eq!(TokenApprovals::erc20_approvals((caller, asset_id), spender), amount);
	});
}

#[test]
fn set_erc20_approval_caller_is_operator_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let caller = H160::default();
		let spender = H160::default();
		let asset_id: AssetId = 0;
		let amount: Balance = 10;

		assert_noop!(
			TokenApprovals::erc20_approval(None.into(), caller, spender, asset_id, amount),
			Error::<Test>::CallerNotOperator,
		);
		assert!(!ERC20Approvals::contains_key((caller, asset_id), spender));
	});
}

#[test]
fn remove_erc20_approval() {
	ExtBuilder::default().build().execute_with(|| {
		let caller = H160::default();
		let spender = H160::from_slice(&hex!("1000000000000000000000000000000000000000"));
		let asset_id: AssetId = 0;
		let amount: Balance = 10;

		assert_ok!(TokenApprovals::erc20_approval(
			None.into(),
			caller,
			spender,
			asset_id,
			amount
		));
		assert_eq!(TokenApprovals::erc20_approvals((caller, asset_id), spender), amount);

		// Remove approval
		assert_ok!(TokenApprovals::erc20_remove_approval(
			None.into(),
			caller,
			spender,
			asset_id
		));
		assert!(!ERC20Approvals::contains_key((caller, asset_id), spender));
	});
}
