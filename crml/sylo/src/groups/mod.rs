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

use self::parity_codec::{Decode, Encode};
use groups::sr_primitives::Ed25519Signature;
use groups::substrate_primitives::hash::{H256, H512};
use srml_support::runtime_primitives::traits::Verify;
use srml_support::{dispatch::Result, dispatch::Vec, StorageMap};
use {device, inbox, system::ensure_signed, vault, vec};
use vault::{VaultKey, VaultValue};

pub trait Trait: system::Trait + inbox::Trait + device::Trait + vault::Trait {}

const INVITES_MAX: usize = 15;

// Meta type stored on group, members and invites
pub type Meta = Vec<(Text, Text)>;

pub type Text = Vec<u8>;

#[derive(Encode, Decode, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum MemberRoles {
	Admin,
	Member,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Invite<AccountId> {
	peer_id: AccountId,
	invite_data: Vec<u8>,
	invite_key: H256,
	meta: Meta,
	roles: Vec<MemberRoles>,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct PendingInvite<Hash> {
	invite_key: Hash,
	meta: Meta,
	roles: Vec<MemberRoles>,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct AcceptPayload<AccountId: Encode + Decode> {
	account_id: AccountId,
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
	invites: Vec<PendingInvite<H256>>,
	meta: Meta,
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn create_group(origin, group_id: T::Hash, meta: Meta, invites: Vec<Invite<T::AccountId>>, group_data: (VaultKey, VaultValue)) -> Result {
			let sender = ensure_signed(origin)?;

			ensure!(!<Groups<T>>::exists(&group_id), "Group already exists");
			ensure!(invites.len() < INVITES_MAX, "Can not invite more than maximum amount");
			ensure!(<vault::Vault<T>>::get(&sender).len() < vault::KEYS_MAX, "Can not store more than maximum amount of keys for user's vault");

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

			// Store new group
			<Groups<T>>::insert(group_id.clone(), group);

			// Record new membership
			Self::store_membership(&sender, group_id.clone());

			// Record user's devices
			let member_devices: Vec<(T::AccountId, u32)> =
				<device::Devices<T>>::get(&sender)
					.into_iter()
					.map(|device| (sender.clone(), device))
					.collect();

			<MemberDevices<T>>::insert(group_id.clone(), member_devices);

			<vault::Module<T>>::upsert(sender.clone(), group_data.0, group_data.1);

			// Create invites
			for invite in invites {
				let _ = Self::create_invite(&group_id, invite);
			}

			Ok(())
		}

		fn leave_group(origin, group_id: T::Hash, group_key: Option<VaultKey>) -> Result {
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

			if let Some(key) = group_key {
				<vault::Module<T>>::delete(sender.clone(), vec![key])
			}

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
				if let Some((i,_)) = meta_copy.iter().enumerate().find(|(_,item)| item.0 == k_v.0) {
					if has_value {
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

		fn create_invites(origin, group_id: T::Hash, invites: Vec<Invite<T::AccountId>>) -> Result {
			let sender = ensure_signed(origin)?;

			ensure!(<Groups<T>>::exists(&group_id), "Group not found");
			ensure!(Self::is_group_member(&group_id, &sender), "Not a member of group");
			ensure!(Self::is_group_admin(&group_id, &sender), "Insufficient permissions for group");
			ensure!(invites.len() < INVITES_MAX, "Can not invite more than maximum amount");

			for invite in invites {
				let _ = Self::create_invite(&group_id, invite);
			}

			Ok(())
		}

		fn accept_invite(origin, group_id: T::Hash, payload: AcceptPayload<T::AccountId>, invite_key: H256, inbox_id: u32, signature: H512, group_data: (VaultKey, VaultValue)) -> Result {
			let sender = ensure_signed(origin)?;

			ensure!(<Groups<T>>::exists(&group_id), "Group not found");
			ensure!(!Self::is_group_member(&group_id, &payload.account_id), "Already a member of group");
			ensure!(<vault::Vault<T>>::get(&sender).len() < vault::KEYS_MAX, "Can not store more than maximum amount of keys for user's vault");

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

			let mut roles = vec![MemberRoles::Member];
			roles.extend(invite.roles);

			let new_member: Member<T::AccountId> = Member {
				user_id: payload.account_id.clone(),
				meta: Vec::new(),
				roles,
			};

			// Add member and remove invite from group
			group.members.push(new_member);
			group.invites = group.invites
				.into_iter()
				.filter(|invite| invite.invite_key != invite_key)
				.collect();

			<Groups<T>>::insert(&group_id, group);

			// Record new membership
			Self::store_membership(&sender, group_id.clone());

			// Record user's devices
			let member_devices: Vec<(T::AccountId, u32)> =
				<device::Devices<T>>::get(&sender)
					.into_iter()
					.map(|device| (sender.clone(), device))
					.collect();

			<vault::Module<T>>::upsert(sender.clone(), group_data.0, group_data.1);

			let mut all_devices = <MemberDevices<T>>::get(&group_id);
			all_devices.extend(member_devices);

			<MemberDevices<T>>::insert(group_id.clone(), all_devices);

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
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as SyloGroups {
		Groups get(group): map T::Hash => Group<T::AccountId, T::Hash>;

		/// Stores the group ids that a user is a member of
		pub Memberships get(memberships): map T::AccountId => Vec<T::Hash>;

		/// Stores the known member/deviceId tuples for a particular group
		MemberDevices get(member_devices): map T::Hash => Vec<(T::AccountId, u32)>;
	}
}

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

	fn store_membership(account_id: &T::AccountId, group_id: T::Hash) {
		if <Memberships<T>>::exists(account_id) {
			let mut memberships = <Memberships<T>>::get(account_id);
			memberships.push(group_id);
			<Memberships<T>>::insert(account_id, memberships)
		} else {
			<Memberships<T>>::insert(account_id, vec![group_id])
		}
	}

	fn create_invite(group_id: &T::Hash, invite: Invite<T::AccountId>) -> Result {
		let peer_id = invite.peer_id;
		let invite_data = invite.invite_data;
		let invite_key = invite.invite_key;
		let meta = invite.meta;
		let roles = invite.roles;

		let mut group = <Groups<T>>::get(group_id);
		ensure!(
			!group.invites.iter().any(|i| i.invite_key == invite_key),
			"Invite already exists"
		);

		group.invites.push(PendingInvite {
			invite_key,
			meta,
			roles,
		});

		<Groups<T>>::insert(group_id, group);

		<inbox::Module<T>>::add(peer_id, invite_data)
	}

	pub fn append_member_device(group_id: &T::Hash, account_id: T::AccountId, device_id: u32) {
		let mut devices = <MemberDevices<T>>::get(group_id);

		let exists = devices
			.iter()
			.find(|device| &device.0 == &account_id && &device.1 == &device_id)
			.is_some();

		if !exists {
			devices.push((account_id, device_id));
			<MemberDevices<T>>::insert(group_id, devices);
		}
	}
}
