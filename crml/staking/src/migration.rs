use super::*;
use frame_support::Twox64Concat;

/// Update storages to current version
///
/// In old version the staking module has several issue about handling session delay, the
/// current era was always considered the active one.
///
/// After the migration the current era will still be considered the active one for the era of
/// the upgrade. And the delay issue will be fixed when planning the next era.
// * create:
//   * ActiveEra
//   * ErasStakers
//   * ErasStakersClipped
//   * ErasValidatorPrefs
//   * ErasTotalStake
//   * ErasStartSessionIndex
// * removal of:
//   * Stakers
//   * SlotStake
//   * CurrentElected
//   * CurrentEraStart
//   * CurrentEraStartSessionIndex
pub fn upgrade_v1_to_v2<T: Config>() {
	/// Deprecated storages used for migration only.
	mod deprecated {
		use crate::{BalanceOf, Config, Exposure, SessionIndex};
		use frame_support::{decl_module, decl_storage};
		use sp_std::prelude::*;

		decl_module! { pub struct Module<T: Config> for enum Call where origin: T::Origin {} }

		decl_storage! {
			pub trait Store for Module<T: Config> as Staking {
				pub SlotStake: BalanceOf<T>;

				/// The currently elected validator set keyed by stash account ID.
				pub CurrentElected: Vec<T::AccountId>;

				/// The start of the current era.
				pub CurrentEraStart: u64; // 'MomentOf<T>' in v38

				/// The session index at which the current era started.
				pub CurrentEraStartSessionIndex: SessionIndex;

				/// Nominators for a particular account that is in action right now. You can't iterate
				/// through validators here, but you can find them in the Session module.
				///
				/// This is keyed by the stash account.
				pub Stakers: map hasher(twox_64_concat) T::AccountId => Exposure<T::AccountId, BalanceOf<T>>;
			}
		}
	}

	for (_stash, controller) in Bonded::<T>::iter() {
		// Ledger is twox_64_concat in v38, transition to blake256
		Ledger::<T>::migrate_key::<Twox64Concat, T::AccountId>(controller);
	}

	let current_era_start_index = deprecated::CurrentEraStartSessionIndex::get();
	let current_era = <Module<T> as Store>::CurrentEra::get().unwrap_or(0);
	let current_era_start = deprecated::CurrentEraStart::get();
	<Module<T> as Store>::ErasStartSessionIndex::insert(current_era, current_era_start_index);
	<Module<T> as Store>::ActiveEra::put(ActiveEraInfo {
		index: current_era,
		start: Some(current_era_start),
	});

	let current_elected = deprecated::CurrentElected::<T>::get();
	let mut current_total_stake = <BalanceOf<T>>::zero();
	for validator in &current_elected {
		let exposure = deprecated::Stakers::<T>::get(validator);
		current_total_stake += exposure.total;
		<Module<T> as Store>::ErasStakers::insert(current_era, validator, &exposure);

		let mut exposure_clipped = exposure;
		let clipped_max_len = T::MaxNominatorRewardedPerValidator::get() as usize;
		if exposure_clipped.others.len() > clipped_max_len {
			exposure_clipped
				.others
				.sort_unstable_by(|a, b| a.value.cmp(&b.value).reverse());
			exposure_clipped.others.truncate(clipped_max_len);
		}
		<Module<T> as Store>::ErasStakersClipped::insert(current_era, validator, exposure_clipped);

		let pref = <Module<T> as Store>::Validators::get(validator);
		<Module<T> as Store>::ErasValidatorPrefs::insert(current_era, validator, pref);
	}
	<Module<T> as Store>::ErasTotalStake::insert(current_era, current_total_stake);

	// Kill old storages
	deprecated::Stakers::<T>::remove_all();
	deprecated::SlotStake::<T>::kill();
	deprecated::CurrentElected::<T>::kill();
	deprecated::CurrentEraStart::kill();
	deprecated::CurrentEraStartSessionIndex::kill();

	// Release
	StorageVersion::put(Releases::V2 as u32);
}
