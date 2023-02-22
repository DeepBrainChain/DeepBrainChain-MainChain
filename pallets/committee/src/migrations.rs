use crate::{CommitteeStake, Config, StorageVersion};
use frame_support::{debug::info, weights::Weight, IterableStorageMap};
use sp_runtime::traits::Zero;
use sp_std::vec::Vec;

pub fn apply<T: Config>() -> Weight {
    frame_support::debug::RuntimeLogger::init();

    info!(
        target: "runtime::committee",
        "Running migration for committee pallet"
    );

    let storage_version = StorageVersion::<T>::get();

    if storage_version <= 1 {
        StorageVersion::<T>::put(3);
        fix_committee_used_stake::<T>()
    } else {
        frame_support::debug::info!(" >>> Unused migration!");
        0
    }
}

fn fix_committee_used_stake<T: Config>() -> Weight {
    let all_committee = <CommitteeStake<T> as IterableStorageMap<T::AccountId, _>>::iter()
        .map(|(committee, _)| committee)
        .collect::<Vec<_>>();

    for a_committee in all_committee {
        CommitteeStake::<T>::mutate(a_committee, |committee_stake| {
            committee_stake.used_stake = Zero::zero()
        });
    }
    0
}
