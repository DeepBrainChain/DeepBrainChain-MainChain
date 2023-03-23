use crate::{types::CommitteeList, Config, Pallet};

impl<T: Config> Pallet<T> {
    pub fn get_committee_list() -> CommitteeList<T::AccountId> {
        Self::committee()
    }
}
