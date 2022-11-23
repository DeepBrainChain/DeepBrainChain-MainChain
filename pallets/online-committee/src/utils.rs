use crate::{BalanceOf, Config, CustomErr, Pallet};
use dbc_support::traits::ManageCommittee;

impl<T: Config> Pallet<T> {
    pub fn change_committee_used_stake(
        committee: T::AccountId,
        amount: BalanceOf<T>,
        is_add: bool,
    ) -> Result<(), CustomErr> {
        <T as Config>::ManageCommittee::change_used_stake(committee, amount, is_add)
            .map_err(|_| CustomErr::Overflow)
    }

    pub fn change_committee_total_stake(
        committee: T::AccountId,
        amount: BalanceOf<T>,
        is_add: bool,
        change_reserve: bool,
    ) -> Result<(), CustomErr> {
        <T as Config>::ManageCommittee::change_total_stake(
            committee,
            amount,
            is_add,
            change_reserve,
        )
        .map_err(|_| CustomErr::Overflow)
    }
}
