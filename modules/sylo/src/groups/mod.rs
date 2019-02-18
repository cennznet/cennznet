// Needed for tests (`with_externalities`).
#[cfg(test)]
extern crate sr_io;

extern crate substrate_primitives;
// Needed for various traits. In our case, `OnFinalise`.
extern crate sr_primitives;

// `system` module provides us with all sorts of useful stuff and macros
// depend on it being around.
extern crate srml_system as system;

extern crate parity_codec;

mod tests;

use groups::sr_primitives::Ed25519Signature;
use groups::substrate_primitives::hash::{H256, H512};
use self::parity_codec::{Decode, Encode};
use srml_support::runtime_primitives::traits::Verify;
use srml_support::{dispatch::Result, dispatch::Vec, StorageMap};
use {balances, inbox, response, system::ensure_signed, vec};

pub trait Trait: balances::Trait + inbox::Trait + response::Trait {
    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

// Meta type stored on group, members and invites
pub type Meta = Vec<(Text, Text)>;

pub type PKB = (u32 /* device_id */, Text /* pkb */);

pub type Text = Vec<u8>;

#[derive(Encode, Decode, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum MemberRoles {
    Admin,
    Member,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Invite<Hash: Decode + Encode> {
    invite_key: Hash,
    meta: Meta,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Member<AccountId: Encode + Decode> {
    user_id: AccountId,
    roles: Vec<MemberRoles>,
    meta: Meta,
}

impl<A: Encode + Decode> Member<A> {
    fn is_admin(&self) -> bool {
        for role in &self.roles {
            if role == &MemberRoles::Admin {
                return true;
            }
        }
        false
    }
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, Default)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Group<AccountId, Hash>
where
    AccountId: Encode + Decode,
    Hash: Encode + Decode,
{
    group_id: Hash,
    members: Vec<Member<AccountId>>,
    invites: Vec<Invite<H256>>,
    meta: Meta,
}

decl_module! {
  pub struct Module<T: Trait> for enum Call where origin: T::Origin {
    fn deposit_event<T>() = default;

    fn create_group(origin, group_id: T::Hash, pkbs: Vec<PKB>, meta: Meta) -> Result {
      let sender = ensure_signed(origin)?;

      ensure!(!<Groups<T>>::exists(&group_id), "Group already exists");

      let admin: Member<T::AccountId> = Member {
        user_id: sender.clone(),
        roles: vec![MemberRoles::Admin],
        meta: Vec::new(),
      };
      // Build up group
      let group = Group {
        group_id: group_id.clone(),
        members: vec![admin],
        meta,
        invites: vec![]
      };
      Self::store_pkbs(group_id.clone(), sender, pkbs);
      // Store new group
      <Groups<T>>::insert(group_id.clone(), group);
      Ok(())
    }

    fn leave_group(origin, group_id: T::Hash) -> Result {
      let sender = ensure_signed(origin)?;

      ensure!(<Groups<T>>::exists(&group_id), "Group not found");
      ensure!(Self::is_group_member(&group_id, &sender), "Not a member of group");

      let mut group = <Groups<T>>::get(&group_id);
      // Remove the member from the group
      group.members = group.members
        .into_iter()
        .filter(|member| &member.user_id != &sender)
        .collect();

      if group.members.len() > 0 {
        // Store the updated group
        <Groups<T>>::insert(&group_id, group);
      } else {
        <Groups<T>>::remove(&group_id);
      }
      Self::remove_pkbs(group_id, sender);

      Ok(())
    }

    fn update_member(origin, group_id: T::Hash, meta: Meta) -> Result {
      let sender = ensure_signed(origin)?;
      ensure!(<Groups<T>>::exists(&group_id), "Group not found");
      ensure!(Self::is_group_member(&group_id, &sender), "Not a member of group");

      let mut group = <Groups<T>>::get(&group_id);

      // Map members and update member with matching accountId
      group.members = group.members
        .into_iter()
        .map(|member| {
          if &member.user_id == &sender {
            return Member {
              meta: meta.clone(),
              ..member
            };
          }
          return member;
        })
        .collect();

      // Store the updated group
      <Groups<T>>::insert(&group_id, group);

      Ok(())
    }

    fn upsert_group_meta(origin, group_id: T::Hash, meta: Meta) -> Result {
      let sender = ensure_signed(origin)?;

      ensure!(<Groups<T>>::exists(&group_id), "Group not found");
      ensure!(Self::is_group_member(&group_id, &sender), "Not a member of group");

      let mut group = <Groups<T>>::get(&group_id);

      /* Merge the existing meta with new meta. There are 3 scenarios:
       * 1. Remove: The key exists and the new data is empty
       * 2. Update: The key exists and the new data is not empty
       * 3. Add: The key doesn't exist and the data is not empty
      */
      for k_v in meta {
        let has_value = k_v.1.len() > 0;
        let meta_copy = group.meta.clone();
        if let Some((i,_)) = meta_copy.iter()
                                    .enumerate()
                                    .find(|(_,item)| item.0 == k_v.0) {
            if has_value  {
                group.meta[i].1 = k_v.1;
            } else {
                group.meta.remove(i);
            }
        } else {
            if has_value {
                group.meta.push(k_v);
            }
        }
      }

      // Store the updated group
      <Groups<T>>::insert(&group_id, group);

      Ok(())
    }

    fn create_invite(origin, group_id: T::Hash, peer_id: T::AccountId, invite_data: Vec<u8>, invite_key: H256, meta: Meta) -> Result {
      let sender = ensure_signed(origin)?;

      ensure!(<Groups<T>>::exists(&group_id), "Group not found");
      ensure!(Self::is_group_member(&group_id, &sender), "Not a member of group");
      ensure!(Self::is_group_admin(&group_id, &sender), "Insufficient permissions for group");

      let mut group = <Groups<T>>::get(&group_id);
      ensure!(!group.invites.iter().any(|i| i.invite_key == invite_key), "Invite already exists");

      group.invites.push(Invite {
        invite_key,
        meta,
      });

      <Groups<T>>::insert(&group_id, group);

      <inbox::Module<T>>::add(peer_id, invite_data)
    }

    fn accept_invite(origin, group_id: T::Hash, payload: (T::AccountId, Vec<PKB>), invite_key: H256, inbox_id: u32, signature: H512) -> Result {
      let sender = ensure_signed(origin)?;

      ensure!(<Groups<T>>::exists(&group_id), "Group not found");
      ensure!(!Self::is_group_member(&group_id, &sender), "Already a member of group");

      let mut group = <Groups<T>>::get(&group_id);
      let invite = group.clone().invites
        .into_iter()
        .find(|invite| invite.invite_key == invite_key)
        .ok_or("Invite not found")?;

      let sig = Ed25519Signature::from(signature);
      // TODO ensure payload is encoded properly
      ensure!(
        sig.verify(payload.encode().as_slice(), &invite.invite_key),
        "Failed to verify invite"
      );

      let new_member: Member<T::AccountId> = Member {
        user_id: payload.0.clone(),
        roles: vec![MemberRoles::Member],
        meta: Vec::new(),
      };

      // Add member and remove invite from group
      group.members.push(new_member);
      group.invites = group.invites
        .into_iter()
        .filter(|invite| invite.invite_key != invite_key)
        .collect();

      Self::store_pkbs(group_id.clone(), payload.0, payload.1);

      <Groups<T>>::insert(&group_id, group);

      <inbox::Module<T>>::delete(sender, vec![inbox_id])
    }

    fn revoke_invites(origin, group_id: T::Hash, invite_keys: Vec<H256>) -> Result {
      let sender = ensure_signed(origin)?;

      ensure!(<Groups<T>>::exists(&group_id), "Group not found");
      ensure!(Self::is_group_member(&group_id, &sender), "Not a member of group");
      ensure!(Self::is_group_admin(&group_id, &sender), "Insufficient permissions for group");

      let mut group = <Groups<T>>::get(&group_id);

      // Filter invites
      group.invites = group.invites
        .into_iter()
        .filter(|invite| !invite_keys.contains(&invite.invite_key))
        .collect();

      <Groups<T>>::insert(&group_id, group);

      Ok(())
    }

    fn replenish_pkbs(origin, group_id: T::Hash, pkbs: Vec<PKB>) -> Result {
      let sender = ensure_signed(origin)?;

      ensure!(<Groups<T>>::exists(&group_id), "Group not found");
      ensure!(Self::is_group_member(&group_id, &sender), "Not a member of group");

      Self::store_pkbs(group_id, sender, pkbs);

      Ok(())
    }

    fn withdraw_pkbs(origin, group_id: T::Hash, request_id: T::Hash, wanted_pkbs: Vec<(T::AccountId, u32)>) -> Result {
      let sender = ensure_signed(origin)?;

      ensure!(<Groups<T>>::exists(&group_id), "Group not found");
      ensure!(Self::is_group_member(&group_id, &sender), "Not a member of group");

      // Make sure we are withdrawing keys from members
      ensure!(
        !wanted_pkbs.iter().any(|wanted_pkb| !Self::is_group_member(&group_id, &wanted_pkb.0)),
        "Member not found"
      );

      let acquired_pkbs: Vec<(T::AccountId, u32, Text)> = wanted_pkbs
        .into_iter()
        .map(|wanted_pkb| {
          let mut device_pkbs = <Pkbs<T>>::get((group_id.clone(), wanted_pkb.0.clone(), wanted_pkb.1));

          let pkb = device_pkbs.pop();

          <Pkbs<T>>::insert((group_id.clone(), wanted_pkb.0.clone(), wanted_pkb.1.clone()), device_pkbs);

          (wanted_pkb.0, wanted_pkb.1, pkb)
        })
        .filter(|a_pkb| a_pkb.2.is_some())
        .map(|a_pkb| (a_pkb.0, a_pkb.1, a_pkb.2.unwrap()))
        .collect();

      Self::deposit_event(RawEvent::PKBsWithdrawn(sender.clone(), request_id.clone(), acquired_pkbs.clone()));
      <response::Module<T>>::set_response(sender, request_id, response::Response::Pkb(acquired_pkbs));
      Ok(())
    }
  }
}

decl_storage! {
  trait Store for Module<T: Trait> as SyloGroups {
    Groups get(group): map T::Hash => Group<T::AccountId, T::Hash>;
    /* PKBs */
    Pkbs get(pkbs): map (T::Hash /* group_id */, T::AccountId, u32 /* device_id */) => Vec<Text>;
    SignalAddresses get(signal_addresses): map T::Hash => Vec<(T::AccountId, Vec<u32>)>;
  }
}

decl_event!(
  pub enum Event<T> where <T as system::Trait>::AccountId, <T as system::Trait>::Hash {
    PKBsWithdrawn(AccountId, Hash /* request_id */, Vec<(AccountId, u32 /* device id */, Text)> /* pkbs */),
  }
);

impl<T: Trait> Module<T> {
    fn is_group_member(group_id: &T::Hash, account_id: &T::AccountId) -> bool {
        <Groups<T>>::get(group_id)
            .members
            .into_iter()
            .find(|member| &member.user_id == account_id)
            .is_some()
    }

    fn is_group_admin(group_id: &T::Hash, account_id: &T::AccountId) -> bool {
        <Groups<T>>::get(group_id)
            .members
            .into_iter()
            .find(|member| &member.user_id == account_id && member.is_admin())
            .is_some()
    }

    fn store_pkbs(group_id: T::Hash, account_id: T::AccountId, pkbs: Vec<PKB>) {
        // Get pkbs references
        let mut pbk_arr = <SignalAddresses<T>>::get(&group_id);

        // needs to drop mut ref to pbk_arr, drop fn doesn't work for this mut ref
        {
            #[allow(unused_assignments)]
            let mut pkbs_map: &mut Vec<u32> = &mut vec![];
            match pbk_arr.iter().position(|(acc_id, _)| *acc_id == account_id) {
                Some(i) => pkbs_map = &mut pbk_arr[i].1,
                None => {
                    let len = pbk_arr.len();
                    pbk_arr.push((account_id.clone(), vec![]));
                    pkbs_map = &mut pbk_arr[len].1;
                }
            }

            for pkb in pkbs {
                // Get pkbs for device
                let mut pkbs = <Pkbs<T>>::get((group_id.clone(), account_id.clone(), pkb.0));

                // Update pkbs
                pkbs.push(pkb.1);
                pkbs.sort();
                pkbs.dedup();

                // Add device id
                pkbs_map.push(pkb.0);
                pkbs_map.sort();
                pkbs_map.dedup();

                // Store pkbs
                <Pkbs<T>>::insert((group_id.clone(), account_id.clone(), pkb.0), pkbs);
            }
        }

        // Store updated pkbs references
        <SignalAddresses<T>>::insert(group_id.clone(), pbk_arr);
    }

    fn remove_pkbs(group_id: T::Hash, account_id: T::AccountId) {
        let mut pkbs = <SignalAddresses<T>>::get(&group_id);
        let mut devices: Vec<u32> = vec![];
        if let Some(i) = pkbs.iter().position(|(acc_id, _)| *acc_id == account_id) {
            devices = pkbs[i].1.clone();
            pkbs.remove(i);
            <SignalAddresses<T>>::insert(group_id.clone(), pkbs);
        };

        for device in devices {
            // Remove pkbs for device
            <Pkbs<T>>::remove((group_id.clone(), account_id.clone(), device));
        }
    }
}
