use crate::{
    BalanceOf, Config, Error, Pallet, PendingOnlineSlash, StashStake, UnhandledOnlineSlash,
};
use dbc_support::{
    traits::{GNOps, ManageCommittee},
    verify_committee_slash::OCSlashResult,
    ItemList, SlashId,
};
use frame_support::{dispatch::DispatchResultWithPostInfo, ensure};
use sp_runtime::traits::{CheckedSub, Zero};
use sp_std::{vec, vec::Vec};

impl<T: Config> Pallet<T> {
    pub fn get_stake_per_order() -> Result<BalanceOf<T>, Error<T>> {
        <T as Config>::ManageCommittee::stake_per_order().ok_or(Error::<T>::GetStakeAmountFailed)
    }

    pub fn is_valid_committee(who: &T::AccountId) -> DispatchResultWithPostInfo {
        ensure!(<T as Config>::ManageCommittee::is_valid_committee(who), Error::<T>::NotCommittee);
        Ok(().into())
    }

    fn change_committee_stake(
        committee_list: Vec<T::AccountId>,
        amount: BalanceOf<T>,
        is_slash: bool,
    ) -> Result<(), ()> {
        for a_committee in committee_list {
            if is_slash {
                <T as Config>::ManageCommittee::change_total_stake(
                    a_committee.clone(),
                    amount,
                    false,
                    false,
                )?;
            }

            <T as Config>::ManageCommittee::change_used_stake(a_committee, amount, false)?;
        }

        Ok(())
    }

    // just change stash_stake & sys_info, slash and reward should be execed in oc module
    fn exec_slash(stash: T::AccountId, amount: BalanceOf<T>) -> Result<(), ()> {
        let mut stash_stake = Self::stash_stake(&stash);

        stash_stake = stash_stake.checked_sub(&amount).ok_or(())?;

        StashStake::<T>::insert(&stash, stash_stake);
        Ok(())
    }

    pub fn check_and_exec_pending_slash() {
        let mut pending_unhandled_id = Self::unhandled_online_slash();

        for slash_id in pending_unhandled_id.clone() {
            if Self::do_a_slash(slash_id, &mut pending_unhandled_id).is_err() {
                continue
            };
        }
        UnhandledOnlineSlash::<T>::put(pending_unhandled_id);
    }

    fn do_a_slash(slash_id: SlashId, pending_unhandled_slash: &mut Vec<SlashId>) -> Result<(), ()> {
        let now = <frame_system::Pallet<T>>::block_number();
        let mut slash_info = Self::pending_online_slash(slash_id).ok_or(())?;
        if now < slash_info.slash_exec_time {
            return Ok(())
        }

        if !slash_info.stash_slash_amount.is_zero() {
            if let Some(stash) = slash_info.machine_stash.clone() {
                // stash is slashed
                Self::exec_slash(stash.clone(), slash_info.stash_slash_amount)?;
                <T as Config>::SlashAndReward::slash_and_reward(
                    vec![stash],
                    slash_info.stash_slash_amount,
                    // 拒绝上线，将stash质押的金额惩罚到国库，而不用来奖励委员会
                    vec![],
                )?;
            }
        }

        // 将资金退还给已经完成了任务的委员会（降低已使用的质押）
        let mut slashed_committee = vec![];
        slashed_committee.extend_from_slice(&slash_info.inconsistent_committee);
        slashed_committee.extend_from_slice(&slash_info.unruly_committee);

        Self::change_committee_stake(slashed_committee.clone(), slash_info.committee_stake, true)?;

        Self::change_committee_stake(
            slash_info.reward_committee.clone(),
            slash_info.committee_stake,
            false,
        )?;

        // 惩罚到国库
        <T as Config>::SlashAndReward::slash_and_reward(
            slashed_committee,
            slash_info.committee_stake,
            vec![],
        )?;

        slash_info.slash_result = OCSlashResult::Executed;
        ItemList::rm_item(pending_unhandled_slash, &slash_id);
        PendingOnlineSlash::<T>::insert(slash_id, slash_info);

        Ok(())
    }
}
