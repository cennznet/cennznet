#[cfg(test)]
mod tests {
    use groups::sr_io::with_externalities;
    use groups::substrate_primitives::ed25519::Pair;
    use groups::substrate_primitives::{Blake2Hasher, H256};
    // The testing primitives are very useful for avoiding having to work with signatures
    // or public keys. `u64` is used as the `AccountId` and no `Signature`s are requried.
    use groups::sr_primitives::{
        testing::{Digest, DigestItem, Header},
        traits::BlakeTwo256,
        BuildStorage,
    };
    // use groups::tests::{Call, Origin, Event as OuterEvent};
    use groups::{
        balances, response, system, Encode, Group, Member, MemberRoles, Module, Trait, PKB,
    };
    // use system::{EventRecord, Phase};

    impl_outer_origin! {
      pub enum Origin for Test {}
    }

    impl_outer_event! {
        pub enum Event for Test {}
    }

    // For testing the module, we construct most of a mock runtime. This means
    // first constructing a configuration type (`Test`) which `impl`s each of the
    // configuration traits of modules we want to use.
    #[derive(Clone, Eq, PartialEq)]
    pub struct Test;
    impl system::Trait for Test {
        type Origin = Origin;
        type Index = u64;
        type BlockNumber = u64;
        type Hash = H256;
        type Hashing = BlakeTwo256;
        type Digest = Digest;
        type AccountId = u64;
        type Header = Header;
        type Event = ();
        type Log = DigestItem;
    }
    impl balances::Trait for Test {
        type Balance = u64;
        type AccountIndex = u64;
        type OnFreeBalanceZero = ();
        type EnsureAccountLiquid = ();
        type Event = ();
    }
    impl Trait for Test {
        type Event = ();
    }
    impl response::Trait for Test {}
    type Groups = Module<Test>;
    type Responses = response::Module<response::tests::Test>;
    // type System = system::Module<Test>;

    // This function basically just builds a genesis storage key/value store according to
    // our desired mockup.
    fn new_test_ext() -> sr_io::TestExternalities<Blake2Hasher> {
        system::GenesisConfig::<Test>::default()
            .build_storage()
            .unwrap()
            .0
            .into()
    }

    #[test]
    fn it_works_creating_a_group() {
        with_externalities(&mut new_test_ext(), || {
            let meta_1 = vec![(b"key".to_vec(), b"value".to_vec())];
            let group_id = H256::from([1;32]);
            //Create a group
            assert_ok!(Groups::create_group(
                Origin::signed(1),
                group_id.clone(),
                vec![(1, b"dummy_pkb".to_vec())],
                meta_1.clone()
            ));

            assert_eq!(
                Groups::group(group_id.clone()),
                Group {
                    group_id: group_id.clone(),
                    members: vec![Member {
                        user_id: 1,
                        roles: vec![MemberRoles::Admin],
                        meta: vec![],
                    }],
                    invites: vec![],
                    meta: meta_1.clone(),
                }
            );

            assert_eq!(
                Groups::create_group(
                    Origin::signed(1),
                    group_id.clone(),
                    vec![(1, b"dummy_pkb".to_vec())],
                    meta_1.clone()
                ),
                Err("Group already exists")
            );
        });
    }

    #[test]
    fn it_works_modifying_meta() {
        with_externalities(&mut new_test_ext(), || {
            let group_id = H256::from([1;32]);
            let mut meta_1 = vec![(b"key".to_vec(), b"value".to_vec())];
            let mut meta_2 = vec![(b"key2".to_vec(), b"value2".to_vec())];

            //Create a group
            assert_ok!(Groups::create_group(
                Origin::signed(1),
                group_id.clone(),
                vec![(1, b"dummy_pkb".to_vec())],
                meta_1.clone()
            ));

            // Check initial meta
            assert_eq!(Groups::group(group_id.clone()).meta, meta_1.clone());

            // Add another key
            assert_ok!(Groups::upsert_group_meta(
                Origin::signed(1),
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
                Origin::signed(1),
                group_id.clone(),
                meta_1.clone()
            ));

            meta_res[0].1 = b"foo".to_vec();
            assert_eq!(Groups::group(group_id.clone()).meta, meta_res.clone());
        });
    }

    #[test]
    fn it_works_replenishing_and_withdrawing_pkbs() {
        with_externalities(&mut new_test_ext(), || {
            let group_id = H256::from([1;32]);
            let meta_1 = vec![(b"key".to_vec(), b"value".to_vec())];
            let mock_pkb = vec![
                (1, b"10".to_vec()),
                (1, b"11".to_vec()),
                (2, b"20".to_vec()),
            ];
            let req_id = H256::from([3;32]);

            //Create a group
            assert_ok!(Groups::create_group(
                Origin::signed(1),
                group_id.clone(),
                mock_pkb.clone(),
                meta_1.clone()
            ));

            assert_eq!(
                Groups::pkbs((group_id.clone(), 1, 1)),
                vec![b"10".to_vec(), b"11".to_vec()]
            );

            // check signall addresses
            assert_eq!(Groups::signal_addresses(group_id.clone())[0].1, vec![1, 2]);

            // Withdraw pkbs
            assert_ok!(Groups::withdraw_pkbs(
                Origin::signed(1),
                group_id.clone(),
                req_id.clone(),
                vec![(1, 1), (1, 2)]
            ));

            // check saved response
            assert_eq!(
                Responses::response((1, req_id.clone())),
                response::Response::Pkb(vec![(1, 1, b"11".to_vec()), (1, 2, b"20".to_vec())])
            );

            // TODO test event

            assert_eq!(Groups::pkbs((group_id.clone(), 1, 1)), vec![b"10".to_vec()]);

            assert_eq!(
                Groups::pkbs((group_id.clone(), 1, 2)),
                vec![] as Vec<Vec<u8>>
            );

            // Replenish pkbs

            assert_ok!(Groups::replenish_pkbs(
                Origin::signed(1),
                group_id.clone(),
                vec![
                    (1, b"12".to_vec()),
                    (1, b"13".to_vec()),
                    (2, b"21".to_vec())
                ]
            ));

            assert_eq!(
                Groups::pkbs((group_id.clone(), 1, 1)),
                vec![b"10".to_vec(), b"12".to_vec(), b"13".to_vec()]
            );

            assert_eq!(Groups::pkbs((group_id.clone(), 1, 2)), vec![b"21".to_vec()]);
        });
    }

    #[test]
    fn should_leave_group() {
        with_externalities(&mut new_test_ext(), || {
            let group_id = H256::from([1;32]);
            let meta_1 = vec![(b"key".to_vec(), b"value".to_vec())];

            //Create a group
            assert_ok!(Groups::create_group(
                Origin::signed(1),
                group_id.clone(),
                vec![(1, b"dummy_pkb".to_vec())],
                meta_1.clone()
            ));

            // leave wrong group
            assert_eq!(
                Groups::leave_group(Origin::signed(1), H256::from([3;32])),
                Err("Group not found")
            );

            // trying to live group user who is not a member
            assert_eq!(
                Groups::leave_group(Origin::signed(2), group_id.clone()),
                Err("Not a member of group")
            );

            assert_ok!(Groups::leave_group(Origin::signed(1), group_id.clone()));

            // todo: check empty group
        });
    }

    #[test]
    fn should_accept_invite() {
        with_externalities(&mut new_test_ext(), || {
            let group_id = H256::from([2;32]);
            let meta_1 = vec![(b"key".to_vec(), b"value".to_vec())];

            //Create a group
            assert_ok!(Groups::create_group(
                Origin::signed(1),
                group_id.clone(),
                vec![(1, b"dummy_pkb".to_vec())],
                meta_1.clone()
            ));

            let pkbs: Vec<PKB> = vec![];
            let payload = (2, pkbs);
            let encoded = payload.encode();
            let message = encoded.as_slice();
            let (invite_key, signature) = {
                let pair = Pair::generate();
                (H256::from(pair.public().0), pair.sign(&message[..]))
            };

            // sending invite key
            assert_ok!(Groups::add_pending_invite(
                Origin::signed(1),
                group_id.clone(),
                invite_key.clone(),
                vec![]
            ));

            // invite should be added
            let invites = Groups::group(group_id.clone()).invites;
            assert_eq!(invites.len(), 1);
            assert_eq!(invites[0].invite_key, invite_key.clone());

            // sending same invite should fail
            assert_eq!(
                Groups::add_pending_invite(
                    Origin::signed(1),
                    group_id.clone(),
                    invite_key.clone(),
                    vec![]
                ),
                Err("Invite already exists")
            );

            let wrong_sig = Pair::generate().sign(&message[..]);
            // Check generating diff signature
            assert_ne!(signature, wrong_sig);

            // accept wrong invite
            assert_eq!(
                Groups::accept_invite(
                    Origin::signed(2),
                    group_id.clone(),
                    payload.clone(),
                    invite_key.clone(),
                    wrong_sig
                ),
                Err("Failed to verify invite")
            );

            // accept right sig
            assert_ok!(Groups::accept_invite(
                Origin::signed(2),
                group_id.clone(),
                payload.clone(),
                invite_key.clone(),
                signature
            ));

            let group = Groups::group(group_id.clone());
            // user should be added to group
            assert_eq!(group.members.len(), 2);
            assert_eq!(
                group.members[1],
                Member {
                    user_id: 2,
                    roles: vec![MemberRoles::Member],
                    meta: vec![],
                }
            );
            // invite should be deleted
            assert_eq!(group.invites.len(), 0);
        });
    }

    #[test]
    fn should_revoke_invites() {
        with_externalities(&mut new_test_ext(), || {
            let group_id = H256::from([1;32]);
            let meta_1 = vec![(b"key".to_vec(), b"value".to_vec())];

            //Create a group
            assert_ok!(Groups::create_group(
                Origin::signed(1),
                group_id.clone(),
                vec![(1, b"dummy_pkb".to_vec())],
                meta_1.clone()
            ));
            let invite_keys = vec![
                H256::from([1; 32]),
                H256::from([2; 32]),
                H256::from([3; 32]),
            ];
            let send_invite = |invite_key: &H256| {
                // sending invite key
                assert_ok!(Groups::add_pending_invite(
                    Origin::signed(1),
                    group_id.clone(),
                    invite_key.clone(),
                    vec![]
                ));
            };
            for i_key in &invite_keys {
                send_invite(i_key);
            }

            // invite should be added
            let invites = Groups::group(group_id.clone()).invites;
            assert_eq!(invites.len(), 3);
            assert_eq!(invites[0].invite_key, invite_keys[0]);

            // revoke 2 invites
            assert_ok!(Groups::revoke_invites(
                Origin::signed(1),
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
            let group_id = H256::from([1;32]);
            let meta_1 = vec![(b"key".to_vec(), b"value".to_vec())];

            //Create a group
            assert_ok!(Groups::create_group(
                Origin::signed(1),
                group_id.clone(),
                vec![(1, b"dummy_pkb".to_vec())],
                meta_1.clone()
            ));

            // update member's meta
            assert_ok!(Groups::update_member(
                Origin::signed(1),
                group_id.clone(),
                meta_1.clone()
            ));

            assert_eq!(
                Groups::group(group_id.clone()).members[0].meta,
                meta_1.clone()
            )
        });
    }
}
