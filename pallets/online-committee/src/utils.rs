use crate::{BalanceOf, Config, NextSlashId, Pallet};
use dbc_support::{custom_err::VerifyErr, traits::ManageCommittee};
use sp_std::vec::Vec;

impl<T: Config> Pallet<T> {
    pub fn get_new_slash_id() -> u64 {
        let slash_id = Self::next_slash_id();

        if slash_id == u64::MAX {
            NextSlashId::<T>::put(0);
        } else {
            NextSlashId::<T>::put(slash_id + 1);
        };

        slash_id
    }

    pub fn change_committee_used_stake(
        committee: T::AccountId,
        amount: BalanceOf<T>,
        is_add: bool,
    ) -> Result<(), VerifyErr> {
        <T as Config>::ManageCommittee::change_used_stake(committee, amount, is_add)
            .map_err(|_| VerifyErr::Overflow)
    }

    pub fn change_committee_total_stake(
        committee: T::AccountId,
        amount: BalanceOf<T>,
        is_add: bool,
        change_reserve: bool,
    ) -> Result<(), VerifyErr> {
        <T as Config>::ManageCommittee::change_total_stake(
            committee,
            amount,
            is_add,
            change_reserve,
        )
        .map_err(|_| VerifyErr::Overflow)
    }

    pub fn change_committee_stake(
        committee_list: Vec<T::AccountId>,
        amount: BalanceOf<T>,
        is_slash: bool,
    ) -> Result<(), ()> {
        for a_committee in committee_list {
            if is_slash {
                Self::change_committee_total_stake(a_committee.clone(), amount, false, false)
                    .map_err(|_| ())?;
            }

            Self::change_committee_used_stake(a_committee, amount, false).map_err(|_| ())?;
        }

        Ok(())
    }
}
