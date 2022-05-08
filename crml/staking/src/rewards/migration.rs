use super::*;

/// V0 storage entires to migrate
pub(crate) mod storage_v0 {
	use super::*;
	pub struct Module<T>(sp_std::marker::PhantomData<T>);
	decl_storage! {
		trait Store for Module<T: Config> as RewardsV0 {
			pub ScheduledPayoutEra: EraIndex;
			pub ScheduledPayouts: map hasher(twox_64_concat) T::BlockNumber => Option<(T::AccountId, BalanceOf<T>)>;
		}
	}
}

/// Migrate reward storage from v0 to v1
/// Reward payout scheduling is updated
pub fn do_v0_to_v1<T: Config>() -> Weight {
	let mut weight = DbWeight::get().writes(2);
	StorageVersion::put(Releases::V1 as u32);
	let payout_era = storage_v0::ScheduledPayoutEra::take();

	// if there's any payouts in progress migrate them
	let mut payout_count = 0;
	for (_block, (stash, amount)) in storage_v0::ScheduledPayouts::<T>::drain() {
		<ScheduledPayoutAmounts<T>>::insert(payout_era, stash, amount);
		payout_count += 1;
	}
	weight += DbWeight::get().writes(2) * payout_count as Weight;
	if payout_count > 0 {
		weight += DbWeight::get().writes(1);
		ScheduledPayoutErasAndCounts::put(vec![(payout_era, payout_count)]);
	}

	weight
}
