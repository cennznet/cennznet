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

//!
//! CENNZnet governance
//!
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
mod types;
pub use types::*;

use cennznet_primitives::types::Balance;
use codec::{Decode, Encode};
use crml_support::{StakingAmount, RegistrationInfo};
use frame_support::pallet_prelude::*;
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage,
	dispatch::{DispatchResult, Dispatchable},
	traits::{
		schedule::{DispatchTime, Named as ScheduleNamed},
		Currency, LockIdentifier, ReservableCurrency,
	},
};
use frame_system::pallet_prelude::*;
use log::warn;
use sp_runtime::traits::Zero;
use sp_runtime::Permill;
use sp_std::prelude::*;

/// Identifies governance scheduled calls
const GOVERNANCE_ID: LockIdentifier = *b"governan";
/// Minimum number of registered identities required to become a council member
const MINIMUM_REGISTERED_IDENTITIES: u32 = 2;
/// The length in blocks of a referendum voting cycle
const REFERENDUM_LENGTH: u32 = 21600;
/// The interval in which the referendum ending is checked
const REFERENDUM_CHECK_INTERVAL: u32 = 1000;

pub trait Config: frame_system::Config {
	/// Maximum size of the council
	type MaxCouncilSize: Get<u16>;
	/// The Scheduler.
	type Scheduler: ScheduleNamed<Self::BlockNumber, <Self as Config>::Call, Self::PalletsOrigin>;
	/// Overarching type of all pallets origins.
	type PalletsOrigin: From<frame_system::RawOrigin<Self::AccountId>>;
	/// Runtime currency system
	type Currency: Currency<Self::AccountId, Balance = Balance> + ReservableCurrency<Self::AccountId>;
	/// Runtime call type
	type Call: Parameter + Dispatchable<Origin = Self::Origin> + From<Call<Self>>;
	/// The system event type
	type Event: From<Event> + Into<<Self as frame_system::Config>::Event>;
	/// Weight information for extrinsics in this module.
	type WeightInfo: WeightInfo;
	/// Registrations for identities
	type Registration: RegistrationInfo<AccountId = Self::AccountId>;
	/// staking information of an account
	type StakingAmount: StakingAmount<AccountId = Self::AccountId, Balance = Balance>;
}

/// TODO: move to weights
pub trait WeightInfo {}
impl WeightInfo for () {}

decl_event! {
	pub enum Event {
		/// A proposal was submitted
		SubmitProposal(ProposalId),
		/// A proposal was enacted, success
		EnactReferendum(ProposalId, bool),
		/// A proposal was vetoed by the council
		ProposalVeto(ProposalId),
		/// A referendum was vetoed by vote
		ReferendumVeto(ProposalId),
		/// A proposal was approved by council and a referendum has been created
		ReferendumCreated(ProposalId),
		/// A referendum has been approved and is awaiting enactment
		ReferendumApproved(ProposalId),
	}
}

decl_error! {
	pub enum Error for Module<T: Config> {
		/// Operation can only be performed by an active council member
		NotCouncilor,
		/// Operation can only be performed by the proposal's sponsor
		NotSponsor,
		/// Reached the max. number of elected councilors
		MaxCouncilReached,
		/// Reached the min. number of elected councilors
		MinCouncilReached,
		/// Proposal was not found
		ProposalMissing,
		/// Cannot vote twice
		DoubleVote,
		/// Referendum Threshold hasn't been set
		NoReferendumThreshold,
		/// This account does not meet the minimum required stake amount
		NotEnoughStaked,
		/// The referendum hasn't started yet
		ReferendumNotStarted,
		/// The referendum isn't currently accepting votes
		ReferendumNotDeliberating,
		/// This account does not meet the required amount of registered identities
		NotEnoughRegistrations,
	}
}

decl_storage! {
	trait Store for Module<T: Config> as Governance {
		/// Map from proposal Id to proposal info
		Proposals get(fn proposals): map hasher(twox_64_concat) ProposalId => Option<Proposal<T>>;
		/// Map from proposal Id to call if any
		ProposalCalls get(fn proposal_calls): map hasher(twox_64_concat) ProposalId => Option<Vec<u8>>;
		/// Map from proposal Id to votes
		ProposalVotes get(fn proposal_votes): map hasher(twox_64_concat) ProposalId => ProposalVoteInfo;
		/// Map from proposal Id to status
		ProposalStatus get(fn proposal_status): map hasher(twox_64_concat) ProposalId => Option<ProposalStatusInfo>;
		/// Map from proposal Id to referendum votes
		ReferendumVotes get(fn referendum_votes): double_map hasher(twox_64_concat) ProposalId, hasher(twox_64_concat) T::AccountId => ReferendumVoteCount;
		/// Running tally of referendum votes
		ReferendumVetoSum get(fn referendum_veto_sum): map hasher(twox_64_concat) ProposalId => u32;
		/// Map from proposal id to referendum start time
		ReferendumStartTime get(fn referendum_start_time): map hasher(twox_64_concat) ProposalId => Option<T::BlockNumber>;
		/// Ordered set of active council members
		Council get(fn council): Vec<T::AccountId>;
		/// Next available ID for proposal
		NextProposalId get(fn next_proposal_id): ProposalId;
		/// Proposal bond amount in 'wei'
		ProposalBond get(fn proposal_bond): Balance;
		/// Minimum amount of staked CENNZ required to vote
		MinVoterStakedAmount get(fn min_voter_staked_amount): Balance = 10_000;
		/// Permill of vetos needed for a referendum to fail
		ReferendumThreshold get(fn referendum_threshold): Permill = Permill::from_percent(33);
		/// Minimum stake required to create a new council member
		MinimumCouncilStake get(fn minimum_council_stake): Balance = 10_000_000;
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {

		fn deposit_event() = default;

		/// Check whether any referendums have finished
		fn on_initialize(block_number: T::BlockNumber) -> Weight {
			if (block_number % T::BlockNumber::from(REFERENDUM_CHECK_INTERVAL)).is_zero() {
				// Check referendums
				let mut weight_count = 0;
				let proposal_ids = <ReferendumStartTime<T>>::iter();
				proposal_ids.for_each(|(proposal_id, block)| {
					if block_number >= block + T::BlockNumber::from(REFERENDUM_LENGTH) {
						if Self::proposal_status(proposal_id) == Some(ProposalStatusInfo::ReferendumDeliberation) {
							Self::end_referendum(proposal_id);
							weight_count += 1;
						}
					}
				});
				weight_count * 1_000_000u64
			} else  {
				0
			}
		}

		#[weight = 1_000_000]
		/// Submit a proposal for consideration by the council
		/// Caller must be a council member
		fn submit_proposal(
			origin,
			call: Vec<u8>,
			justification_uri: Vec<u8>,
			enactment_delay: T::BlockNumber,
		) {
			let origin = ensure_signed(origin)?;
			// Validate council members identity and staking assets
			Self::check_council_account_validity(&origin)?;
			let sponsor_idx = Self::council().binary_search(&origin);
			ensure!(sponsor_idx.is_ok(), Error::<T>::NotCouncilor);
			let proposal_id = Self::next_proposal_id();
			let _ = T::Currency::reserve(&origin, Self::proposal_bond())?;
			let enactment_delay = sp_std::cmp::max(T::BlockNumber::from(1u32), enactment_delay);

			<Proposals<T>>::insert(proposal_id, Proposal {
				sponsor: origin,
				justification_uri,
				enactment_delay,
			});
			ProposalCalls::insert(proposal_id, call);

			// sponsor should vote yes
			let mut votes = ProposalVoteInfo::default();
			votes.record_vote(sponsor_idx.unwrap() as u8, true);
			ProposalVotes::insert(proposal_id, votes);
			ProposalStatus::insert(proposal_id, ProposalStatusInfo::Deliberation);

			NextProposalId::put(proposal_id.saturating_add(1));
		}

		#[weight = 1_000_000]
		/// Vote on an active proposal
		/// Caller must be a council member
		fn vote_on_proposal(
			origin,
			proposal_id: ProposalId,
			vote: bool,
		) {
			let origin = ensure_signed(origin)?;
			let voter_idx = Self::council().binary_search(&origin);
			ensure!(voter_idx.is_ok(), Error::<T>::NotCouncilor);

			let proposal = Self::proposals(proposal_id).ok_or(Error::<T>::ProposalMissing)?;
			let mut votes = Self::proposal_votes(proposal_id);

			let voter_idx = voter_idx.unwrap() as u8;
			ensure!(votes.get_vote(voter_idx).is_none(), Error::<T>::DoubleVote);

			votes.record_vote(voter_idx, vote);
			let tally = votes.count_votes();
			ProposalVotes::insert(proposal_id, votes);

			//TODO Check identity is still valid and stake is valid

			// if we have more than 50% approval
			let threshold = <Council<T>>::decode_len().unwrap_or(1) as u32 / 2;
			if tally.yes > threshold {
				if ProposalCalls::contains_key(proposal_id) {
					let start_time: T::BlockNumber = <frame_system::Module<T>>::block_number();

					ProposalStatus::insert(proposal_id, ProposalStatusInfo::ReferendumDeliberation);
					ProposalVotes::remove(proposal_id);
					ReferendumStartTime::<T>::insert(proposal_id, start_time);
					ReferendumVetoSum::insert(proposal_id, 0);
					Self::deposit_event(Event::ReferendumCreated(proposal_id));
				} else {
					// Proposal does not have a onchain call, it can be considered enacted
					ProposalStatus::insert(proposal_id, ProposalStatusInfo::ApprovedEnacted(true));
				}
			} else if tally.no > threshold {
				// failed, clean up...
				Self::deposit_event(Event::ProposalVeto(proposal_id));
				let _ = T::Currency::slash_reserved(&proposal.sponsor, Self::proposal_bond());
				<Proposals<T>>::remove(proposal_id);
				ProposalCalls::remove(proposal_id);
				ProposalVotes::remove(proposal_id);
				ProposalStatus::insert(proposal_id, ProposalStatusInfo::Disapproved);
			}
		}

		/// Add a member to the council
		/// This must be submitted like any other proposal
		#[weight = 100_000]
		fn add_council_member(
			origin,
			new_member: T::AccountId,
		) {
			ensure_root(origin)?;
			let mut council = Self::council();
			// TODO: add voter to all active proposals

			// Validate council members identity and staking assets
			Self::check_council_account_validity(&new_member)?;

			ensure!(council.len() < T::MaxCouncilSize::get() as usize, Error::<T>::MaxCouncilReached);
			if let Err(idx) = council.binary_search(&new_member) {
				council.insert(idx, new_member);
				Council::<T>::put(council);
			}
		}

		/// Remove a member from the council
		/// This must be submitted like any other proposal
		#[weight = 100_000]
		fn remove_council_member(
			origin,
			remove_member: T::AccountId,
		) {
			ensure_root(origin)?;
			let mut council = Self::council();
			ensure!(council.len() > 1, Error::<T>::MinCouncilReached);
			// TODO: remove voter from all active proposals
			if let Ok(idx) = council.binary_search(&remove_member) {
				council.remove(idx);
				Council::<T>::put(council);
			}
		}

		/// Cancel a proposal queued for enactment.
		#[weight = 1_000_000]
		fn cancel_enactment(origin, proposal_id: ProposalId) -> DispatchResult {
			ensure_root(origin)?;
			let proposal = Self::proposals(proposal_id).ok_or(Error::<T>::ProposalMissing)?;
			T::Scheduler::cancel_named((GOVERNANCE_ID, proposal_id).encode())
				.map_err(|_| Error::<T>::ProposalMissing)?;

			let _ = T::Currency::slash_reserved(&proposal.sponsor, Self::proposal_bond());
			ProposalStatus::insert(proposal_id, ProposalStatusInfo::ApprovedEnactmentCancelled);
			ProposalCalls::remove(proposal_id);
			ProposalVotes::remove(proposal_id);
			<ReferendumStartTime<T>>::remove(proposal_id);

			Ok(())
		}

		/// Submit a veto for a referendum
		#[weight = 1_000_000]
		fn vote_against_referendum(
			origin,
			proposal_id: ProposalId,
		) -> DispatchResult {
			let origin = ensure_signed(origin)?;
			ensure!(Self::proposal_status(proposal_id) == Some(ProposalStatusInfo::ReferendumDeliberation), Error::<T>::ReferendumNotDeliberating);
			ensure!(ReferendumVotes::<T>::contains_key(proposal_id, &origin), Error::<T>::DoubleVote);
			// Validate council members identity and staking assets
			Self::check_voter_account_validity(&origin)?;
			let block_number = <frame_system::Module<T>>::block_number();
			let start_time = Self::referendum_start_time(proposal_id).ok_or(Error::<T>::ProposalMissing)?;
			ensure!(block_number >= start_time, Error::<T>::ReferendumNotStarted);
			// Enter vote in storage
			ReferendumVotes::<T>::insert(
				proposal_id,
				origin,
				ReferendumVoteCount {
					vote: 1, // 1 for no vote
				}
			);
			ReferendumVetoSum::mutate(proposal_id, |n| *n += 1);
			Ok(())
		}

		/// Execute a proposal transaction
		#[weight = 1_000_000]
		fn enact_referendum(origin, proposal_id: ProposalId) -> DispatchResult {
			ensure_root(origin)?;
			let proposal_call = Self::proposal_calls(proposal_id).ok_or(Error::<T>::ProposalMissing)?;
			let proposal = Self::proposals(proposal_id).ok_or(Error::<T>::ProposalMissing)?;

			if let Ok(call) = <T as Config>::Call::decode(&mut &proposal_call[..]) {
				let ok = call.dispatch(frame_system::RawOrigin::Root.into()).is_ok();
				Self::deposit_event(Event::EnactReferendum(proposal_id, ok));

				let _ = T::Currency::unreserve(&proposal.sponsor, Self::proposal_bond());
				ProposalStatus::insert(proposal_id, ProposalStatusInfo::ApprovedEnacted(ok));
				<Proposals<T>>::remove(proposal_id);
				ProposalCalls::remove(proposal_id);
				ReferendumVetoSum::remove(proposal_id);
				<ReferendumStartTime<T>>::remove(proposal_id);
			}

			Ok(())
		}

		/// Adjust the proposal bond
		/// This must be submitted like any other proposal
		#[weight = 100_000]
		fn set_proposal_bond(
			origin,
			new_proposal_bond: Balance
		) {
			ensure_root(origin)?;
			ProposalBond::put(new_proposal_bond);
		}

		/// Adjust the minimum stake required for new council members
		#[weight = 100_000]
		fn set_minimum_council_stake(
			origin,
			new_minimum_council_stake: Balance
		) {
			ensure_root(origin)?;
			MinimumCouncilStake::put(new_minimum_council_stake);
		}


		/// Adjust the referendum veto threshold
		/// This must be submitted like any other proposal
		#[weight = 100_000]
		fn set_referendum_threshold(
			origin,
			new_referendum_threshold: Permill,
		) {
			ensure_root(origin)?;
			ReferendumThreshold::put(new_referendum_threshold);
		}

		/// Adjust the minimum staked amount
		/// This must be submitted like any other proposal
		#[weight = 100_000]
		fn set_minimum_voter_staked_amount(
			origin,
			new_minimum_staked_amount: Balance,
		) {
			ensure_root(origin)?;
			MinVoterStakedAmount::put(new_minimum_staked_amount);
		}
	}
}

impl<T: Config> Module<T> {
	/// Return current council members
	pub fn get_council() -> Vec<T::AccountId> {
		Self::council()
	}
	/// Return all vote information on active proposals
	pub fn get_proposal_votes() -> Vec<(ProposalId, ProposalVoteInfo)> {
		ProposalVotes::iter().collect()
	}
	/// Check an accounts staked amount and total number of registered identities
	pub fn check_council_account_validity(account: &T::AccountId) -> DispatchResult {
		// Check the amount they have staked
		let staked_amount: Balance = T::StakingAmount::active_balance(&account);
		ensure!(
			staked_amount >= Self::minimum_council_stake(),
			Error::<T>::NotEnoughStaked
		);

		// Check their verified identities
		let registration: u32 = T::Registration::registered_identity_count(account);
		ensure!(
			registration >= MINIMUM_REGISTERED_IDENTITIES,
			Error::<T>::NotEnoughRegistrations
		);
		Ok(())
	}
	/// Check whether an account is eligible to vote on a referendum
	pub fn check_voter_account_validity(account: &T::AccountId) -> DispatchResult {
		// Check the amount they have staked
		let staked_amount: Balance = T::StakingAmount::active_balance(&account);
		ensure!(
			staked_amount > Self::min_voter_staked_amount(),
			Error::<T>::NotEnoughStaked
		);

		// Check their verified identities
		let registration: u32 = T::Registration::registered_identity_count(account);
		ensure!(
			registration >= MINIMUM_REGISTERED_IDENTITIES,
			Error::<T>::NotEnoughRegistrations
		);
		Ok(())
	}
	/// Finish up a referendum and tally vetos
	pub fn end_referendum(proposal_id: ProposalId) {
		let proposal = match Self::proposals(proposal_id) {
			Some(proposal) => proposal,
			None => {
				warn!("clean up proposal: {:?} failed, not found", proposal_id);
				return;
			}
		};
		let max_stakers: u32 = T::StakingAmount::count_nominators();
		ReferendumVotes::<T>::remove_prefix(proposal_id, None);

		if Permill::from_rational(Self::referendum_veto_sum(proposal_id), max_stakers)
			>= Self::referendum_threshold()
		{
			// Too many veto votes, not going ahead
			Self::deposit_event(Event::ReferendumVeto(proposal_id));
			let _ = T::Currency::slash_reserved(&proposal.sponsor, Self::proposal_bond());
			<Proposals<T>>::remove(proposal_id);
			ProposalCalls::remove(proposal_id);
			<ReferendumStartTime<T>>::remove(proposal_id);
			ReferendumVetoSum::remove(proposal_id);
			ProposalStatus::insert(proposal_id, ProposalStatusInfo::ReferendumVetoed);
		} else {
			if ProposalCalls::contains_key(proposal_id) {
				if T::Scheduler::schedule_named(
					(GOVERNANCE_ID, proposal_id).encode(),
					DispatchTime::At(<frame_system::Module<T>>::block_number() + proposal.enactment_delay),
					None,
					63,
					frame_system::RawOrigin::Root.into(),
					Call::enact_referendum{proposal_id}.into(),
				)
				.is_err()
				{
					frame_support::print("LOGIC ERROR: governance/schedule_named failed");
				}
				Self::deposit_event(Event::ReferendumApproved(proposal_id));
				ProposalStatus::insert(proposal_id, ProposalStatusInfo::ApprovedWaitingEnactment);
			} else {
				// Proposal does not have a onchain call, it can be considered enacted
				ProposalStatus::insert(proposal_id, ProposalStatusInfo::ApprovedEnacted(true));
			}
		}
	}
}
