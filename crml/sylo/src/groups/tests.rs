/* Copyright 2019-2020 Centrality Investments Limited
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

#[cfg(test)]
mod tests {
	use crate::groups::{AcceptPayload, Encode, Error, Group, Invite, Member, MemberRoles, Module, PendingInvite};
	use crate::mock::{ExtBuilder, Origin, Test};
	use crate::vault;
	use frame_support::{assert_err, assert_ok};
	use sp_core::{ed25519, Pair, H256};

	type Groups = Module<Test>;
	type Vault = vault::Module<Test>;

	#[test]
	fn it_works_creating_a_group() {
		ExtBuilder::default().build().execute_with(|| {
			let meta_1 = vec![(b"key".to_vec(), b"value".to_vec())];
			let group_id = H256::from([1; 32]);
			//Create a group
			assert_ok!(Groups::create_group(
				Origin::signed(H256::from_low_u64_be(1)),
				group_id.clone(),
				meta_1.clone(),
				vec![],
				(b"group".to_vec(), b"data".to_vec())
			));

			assert_eq!(
				Groups::group(group_id.clone()),
				Group {
					group_id: group_id.clone(),
					members: vec![Member {
						user_id: H256::from_low_u64_be(1),
						roles: vec![MemberRoles::Admin],
						meta: vec![],
					}],
					invites: vec![],
					meta: meta_1.clone(),
				}
			);

			assert_eq!(
				Vault::values(H256::from_low_u64_be(1)),
				vec![(b"group".to_vec(), b"data".to_vec())]
			);

			assert_err!(
				Groups::create_group(
					Origin::signed(H256::from_low_u64_be(1)),
					group_id.clone(),
					meta_1.clone(),
					vec![],
					(vec![], vec![])
				),
				Error::<Test>::GroupExists,
			);
		});
	}

	#[test]
	fn it_works_modifying_meta() {
		ExtBuilder::default().build().execute_with(|| {
			let group_id = H256::from([1; 32]);
			let mut meta_1 = vec![(b"key".to_vec(), b"value".to_vec())];
			let mut meta_2 = vec![(b"key2".to_vec(), b"value2".to_vec())];

			//Create a group
			assert_ok!(Groups::create_group(
				Origin::signed(H256::from_low_u64_be(1)),
				group_id.clone(),
				meta_1.clone(),
				vec![],
				(vec![], vec![])
			));

			// Check initial meta
			assert_eq!(Groups::group(group_id.clone()).meta, meta_1.clone());

			// Add another key
			assert_ok!(Groups::upsert_group_meta(
				Origin::signed(H256::from_low_u64_be(1)),
				group_id.clone(),
				meta_2.clone()
			));

			let mut meta_res = meta_1.clone();
			meta_res.append(&mut meta_2);

			// Check key added
			assert_eq!(Groups::group(group_id.clone()).meta, meta_res.clone());

			meta_1[0].1 = b"foo".to_vec();
			// Update value
			assert_ok!(Groups::upsert_group_meta(
				Origin::signed(H256::from_low_u64_be(1)),
				group_id.clone(),
				meta_1.clone()
			));

			meta_res[0].1 = b"foo".to_vec();
			assert_eq!(Groups::group(group_id.clone()).meta, meta_res.clone());
		});
	}

	#[test]
	fn should_leave_group() {
		ExtBuilder::default().build().execute_with(|| {
			let group_id = H256::from([1; 32]);

			//Create a group
			assert_ok!(Groups::create_group(
				Origin::signed(H256::from_low_u64_be(1)),
				group_id.clone(),
				vec![],
				vec![],
				(b"key".to_vec(), b"value".to_vec())
			));

			// leave wrong group
			assert_err!(
				Groups::leave_group(Origin::signed(H256::from_low_u64_be(1)), H256::from([3; 32]), None),
				Error::<Test>::GroupNotFound,
			);

			// trying to live group user who is not a member
			assert_err!(
				Groups::leave_group(Origin::signed(H256::from_low_u64_be(2)), group_id.clone(), None),
				Error::<Test>::MemberNotFound,
			);

			assert_ok!(Groups::leave_group(
				Origin::signed(H256::from_low_u64_be(1)),
				group_id.clone(),
				Some(b"key".to_vec())
			));

			// check member has left group
			assert_eq!(Groups::group(group_id.clone()).members, vec![]);

			// check group data has been removed from user's vault
			assert_eq!(Vault::values(H256::from_low_u64_be(1)), vec![]);
		});
	}

	#[test]
	fn should_accept_invite() {
		ExtBuilder::default().build().execute_with(|| {
			let group_id = H256::from([2; 32]);

			//Create a group
			assert_ok!(Groups::create_group(
				Origin::signed(H256::from_low_u64_be(1)),
				group_id.clone(),
				vec![],
				vec![],
				(b"group".to_vec(), b"data".to_vec())
			));

			let payload = AcceptPayload {
				account_id: H256::from_low_u64_be(2),
			};
			let encoded = payload.encode();
			let message = encoded.as_slice();
			let (invite_key, signature) = {
				let key = ed25519::Pair::generate().0;
				(H256::from(key.public()), key.sign(&message[..]))
			};

			let invite = Invite {
				peer_id: H256::from_low_u64_be(2),
				invite_data: vec![],
				invite_key: invite_key.clone(),
				meta: vec![],
				roles: vec![],
			};

			// create invite
			assert_ok!(Groups::create_invites(
				Origin::signed(H256::from_low_u64_be(1)),
				group_id.clone(),
				vec![invite],
			));

			// invite should be added
			let invites = Groups::group(group_id.clone()).invites;
			assert_eq!(invites.len(), 1);
			assert_eq!(invites[0].invite_key, invite_key.clone());

			let key = ed25519::Pair::generate().0;
			let wrong_sig = key.sign(&message[..]);
			// Check generating diff signature
			assert_ne!(signature, wrong_sig);

			// accept wrong invite
			assert_err!(
				Groups::accept_invite(
					Origin::signed(H256::from_low_u64_be(2)),
					group_id.clone(),
					payload.clone(),
					invite_key.clone(),
					0,
					wrong_sig,
					(vec![], vec![])
				),
				Error::<Test>::InvitationSignatureRejected,
			);

			// accept right sig
			assert_ok!(Groups::accept_invite(
				Origin::signed(H256::from_low_u64_be(2)),
				group_id.clone(),
				payload.clone(),
				invite_key.clone(),
				0,
				signature,
				(vec![], vec![])
			));

			let group = Groups::group(group_id.clone());
			// user should be added to group
			assert_eq!(group.members.len(), 2);
			assert_eq!(
				group.members[1],
				Member {
					user_id: H256::from_low_u64_be(2),
					roles: vec![MemberRoles::Member],
					meta: vec![],
				}
			);
			// invite should be deleted
			assert_eq!(group.invites.len(), 0);

			assert_eq!(
				Vault::values(H256::from_low_u64_be(1)),
				vec![(b"group".to_vec(), b"data".to_vec())]
			);
		});
	}

	#[test]
	fn should_revoke_invites() {
		ExtBuilder::default().build().execute_with(|| {
			let group_id = H256::from([1; 32]);

			//Create a group
			assert_ok!(Groups::create_group(
				Origin::signed(H256::from_low_u64_be(1)),
				group_id.clone(),
				vec![],
				vec![],
				(vec![], vec![])
			));
			let invite_keys = vec![H256::from([1; 32]), H256::from([2; 32]), H256::from([3; 32])];
			let invites = invite_keys
				.clone()
				.into_iter()
				.map(|invite_key| Invite {
					peer_id: H256::from_low_u64_be(2),
					invite_data: vec![],
					invite_key: invite_key,
					meta: vec![],
					roles: vec![],
				})
				.collect();

			assert_ok!(Groups::create_invites(
				Origin::signed(H256::from_low_u64_be(1)),
				group_id.clone(),
				invites
			));

			// invite should be added
			let invites = Groups::group(group_id.clone()).invites;
			assert_eq!(invites.len(), 3);
			assert_eq!(invites[0].invite_key, invite_keys[0]);

			// revoke 2 invites
			assert_ok!(Groups::revoke_invites(
				Origin::signed(H256::from_low_u64_be(1)),
				group_id.clone(),
				invite_keys[1..].to_vec()
			));

			// invites should be revoked
			let invites = Groups::group(group_id.clone()).invites;
			assert_eq!(invites.len(), 1);
			assert_eq!(invites[0].invite_key, invite_keys[0]);
		});
	}

	#[test]
	fn should_update_member() {
		ExtBuilder::default().build().execute_with(|| {
			let group_id = H256::from([1; 32]);
			let meta_1 = vec![(b"key".to_vec(), b"value".to_vec())];

			//Create a group
			assert_ok!(Groups::create_group(
				Origin::signed(H256::from_low_u64_be(1)),
				group_id.clone(),
				vec![],
				vec![],
				(vec![], vec![])
			));

			// update member's meta
			assert_ok!(Groups::update_member(
				Origin::signed(H256::from_low_u64_be(1)),
				group_id.clone(),
				meta_1.clone()
			));

			assert_eq!(Groups::group(group_id.clone()).members[0].meta, meta_1.clone())
		});
	}

	use crate::migration;

	type Migration = migration::Module<Test>;
	type TestGroup = Group<H256, H256>;
	type GroupVec = Vec<TestGroup>;

	/// Set up a vector of groups by taking their number as the input argument and migrate them to the chain.
	/// The user id 0 does the migration. The groups will have ids ranging from 0..num_of_groups. Each group
	/// will have two members. The user id 1 is a fixed member of all of them. The second member of the group
	/// will have the id of i + 2 when the group id is i. For example group 0 will have user ids 1 and 2 as
	/// its members and group 1 will have 1 and 3.
	fn setup_migrate_groups(num_of_groups: u32) -> GroupVec {
		let mut groups: GroupVec = Vec::new();

		// Allow user id 0 to do the groups migration
		let migrator = H256::from_low_u64_be(0);
		assert_ok!(Migration::set_migrator_account(Origin::ROOT, migrator));

		for i in 0..num_of_groups {
			let members_vec = vec![
				Member {
					user_id: H256::from_low_u64_be((i + 2) as u64),
					roles: vec![MemberRoles::Admin],
					meta: vec![(
						format!("key_{}", i + 2).into_bytes(),
						format!("value_{}", i + 2).into_bytes(),
					)],
				},
				Member {
					user_id: H256::from_low_u64_be(1), // leave 1 to be a member in all groups
					roles: vec![MemberRoles::Member],
					meta: vec![(b"key_1".to_vec(), b"value_1".to_vec())],
				},
			];

			let invites_vec = vec![
				PendingInvite::<H256> {
					invite_key: H256::from(ed25519::Pair::generate().0.public()),
					meta: vec![(b"key_i1".to_vec(), b"value_i1".to_vec())],
					roles: vec![MemberRoles::Member],
				},
				PendingInvite::<H256> {
					invite_key: H256::from(ed25519::Pair::generate().0.public()),
					meta: vec![(b"key_i2".to_vec(), b"value_i2".to_vec())],
					roles: vec![MemberRoles::Admin, MemberRoles::Member],
				},
			];

			let group = Group {
				group_id: H256::from_low_u64_be(i as u64),
				members: members_vec,
				invites: invites_vec,
				meta: vec![(format!("key_g{}", i).into_bytes(), format!("value_g{}", i).into_bytes())],
			};

			assert_ok!(Groups::migrate_groups(Origin::signed(migrator), group.clone()));

			groups.push(group);
		}
		groups
	}

	#[test]
	fn migrate_groups_works() {
		ExtBuilder::default().build().execute_with(|| {
			// Set up 1 group
			let group = &setup_migrate_groups(1)[0];

			// Retrieve the group from the storage by using its id and check everything has stored properly
			assert_eq!(Groups::group(group.group_id.clone()), *group);
		});
	}

	#[test]
	fn migrate_groups_updates_memberships() {
		ExtBuilder::default().build().execute_with(|| {
			// Set up 2 groups
			setup_migrate_groups(2);

			// Test user id 1 is a member of both groups
			assert_eq!(
				Groups::memberships(H256::from_low_u64_be(1)),
				vec![H256::from_low_u64_be(0), H256::from_low_u64_be(1)]
			);

			// Test user id 2 is just a member of group 0
			assert_eq!(
				Groups::memberships(H256::from_low_u64_be(2)),
				vec![H256::from_low_u64_be(0)]
			);
			assert_ne!(
				Groups::memberships(H256::from_low_u64_be(2)),
				vec![H256::from_low_u64_be(1)]
			);

			// Test user id 3 is not a member of any groups
			assert_ne!(Groups::memberships(H256::from_low_u64_be(3)), vec![]);
		});
	}
}
