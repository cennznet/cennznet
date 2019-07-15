// Copyright (C) 2019 Centrality Investments Limited
// This file is part of CENNZnet.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.
#[cfg(test)]
mod tests {
	use groups::sr_io::with_externalities;
	use groups::{vault, AcceptPayload, Encode, Group, Invite, Member, MemberRoles, Module};
	use mock::{new_test_ext, Origin, Test};
	use primitives::{ed25519, Pair, H256};

	type Groups = Module<Test>;
	type Vault = vault::Module<Test>;

	#[test]
	fn it_works_creating_a_group() {
		with_externalities(&mut new_test_ext(), || {
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

			assert_eq!(
				Groups::create_group(
					Origin::signed(H256::from_low_u64_be(1)),
					group_id.clone(),
					meta_1.clone(),
					vec![],
					(vec![], vec![])
				),
				Err("Group already exists")
			);
		});
	}

	#[test]
	fn it_works_modifying_meta() {
		with_externalities(&mut new_test_ext(), || {
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
		with_externalities(&mut new_test_ext(), || {
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
			assert_eq!(
				Groups::leave_group(Origin::signed(H256::from_low_u64_be(1)), H256::from([3; 32]), None),
				Err("Group not found")
			);

			// trying to live group user who is not a member
			assert_eq!(
				Groups::leave_group(Origin::signed(H256::from_low_u64_be(2)), group_id.clone(), None),
				Err("Not a member of group")
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
		with_externalities(&mut new_test_ext(), || {
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
				let pair = ed25519::Pair::generate();
				(H256::from(pair.public().0), pair.sign(&message[..]))
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

			let wrong_sig = ed25519::Pair::generate().sign(&message[..]);
			// Check generating diff signature
			assert_ne!(signature, wrong_sig);

			// accept wrong invite
			assert_eq!(
				Groups::accept_invite(
					Origin::signed(H256::from_low_u64_be(2)),
					group_id.clone(),
					payload.clone(),
					invite_key.clone(),
					0,
					wrong_sig,
					(vec![], vec![])
				),
				Err("Failed to verify invite")
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
		with_externalities(&mut new_test_ext(), || {
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
		with_externalities(&mut new_test_ext(), || {
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
}
