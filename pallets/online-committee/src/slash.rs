use crate::{Config, Pallet, PendingSlash, PendingSlashReview, UnhandledSlash};
use dbc_support::{
    traits::{GNOps, OCOps},
    verify_committee_slash::OCSlashResult,
    verify_online::OCBookResultType,
    ItemList, SlashId,
};
use frame_support::IterableStorageMap;
use sp_runtime::traits::Zero;
use sp_std::{vec, vec::Vec};

impl<T: Config> Pallet<T> {
    pub fn check_and_exec_pending_review() {
        let all_pending_review = <PendingSlashReview<T> as IterableStorageMap<SlashId, _>>::iter()
            .map(|(slash_id, _)| slash_id)
            .collect::<Vec<_>>();

        for a_pending_review in all_pending_review {
            if Self::do_a_pending_review(a_pending_review).is_err() {
                continue;
            };
        }
    }

    fn do_a_pending_review(a_pending_review: SlashId) -> Result<(), ()> {
        let now = <frame_system::Pallet<T>>::block_number();

        let review_info = Self::pending_slash_review(a_pending_review).ok_or(())?;
        let slash_info = Self::pending_slash(a_pending_review).ok_or(())?;

        if review_info.expire_time < now {
            return Ok(());
        }

        if let Some(machine_stash) = slash_info.machine_stash {
            let is_slashed_stash =
                matches!(slash_info.book_result, OCBookResultType::OnlineRefused) &&
                    machine_stash == review_info.applicant;

            if is_slashed_stash {
                // Change stake amount
                // NOTE: should not change slash_info.slash_amount, because it will be done in
                // check_and_exec_pending_slash
                T::OCOps::exec_slash(machine_stash.clone(), review_info.staked_amount)?;

                <T as Config>::SlashAndReward::slash_and_reward(
                    vec![machine_stash],
                    slash_info.stash_slash_amount,
                    slash_info.reward_committee,
                )?;
            }
        } else {
            // applicant is slashed_committee
            Self::change_committee_stake(
                vec![review_info.applicant.clone()],
                review_info.staked_amount,
                true,
            )?;
        }

        // Slash applicant to treasury
        <T as Config>::SlashAndReward::slash_and_reward(
            vec![review_info.applicant],
            review_info.staked_amount,
            vec![],
        )?;

        // Keep PendingSlashReview after pending review is expired will result in performance
        // problem
        PendingSlashReview::<T>::remove(a_pending_review);
        Ok(())
    }

    pub fn check_and_exec_pending_slash() {
        let mut pending_unhandled_id = Self::unhandled_slash();

        for slash_id in pending_unhandled_id.clone() {
            if Self::do_a_slash(slash_id, &mut pending_unhandled_id).is_err() {
                continue;
            };
        }
        UnhandledSlash::<T>::put(pending_unhandled_id);
    }

    fn do_a_slash(slash_id: SlashId, pending_unhandled_slash: &mut Vec<SlashId>) -> Result<(), ()> {
        let now = <frame_system::Pallet<T>>::block_number();
        let mut slash_info = Self::pending_slash(slash_id).ok_or(())?;
        if now < slash_info.slash_exec_time {
            return Ok(());
        }

        // 将资金退还给已经完成了任务的委员会（降低已使用的质押）
        // 根据 slash_info 获得奖励，释放，惩罚的委员会列表！
        let mut slashed_committee = vec![];
        // 无论如何都会惩罚unruly_committee
        slashed_committee.extend_from_slice(&slash_info.unruly_committee);

        let mut release_committee = vec![];
        if !slash_info.stash_slash_amount.is_zero() {
            // When `stash` is slashed:
            // Slash `inconsistent` and `unruly` committee.
            // Relase `reward_committee`'s stake.
            slashed_committee.extend_from_slice(&slash_info.inconsistent_committee);
            release_committee.extend_from_slice(&slash_info.reward_committee);

            if let Some(machine_stash) = slash_info.machine_stash.clone() {
                T::OCOps::exec_slash(machine_stash.clone(), slash_info.stash_slash_amount)?;
                <T as Config>::SlashAndReward::slash_and_reward(
                    vec![machine_stash],
                    slash_info.stash_slash_amount,
                    slash_info.reward_committee.clone(),
                )?;
            }
        } else {
            if slash_info.reward_committee.is_empty() {
                // 机器无共识，只惩罚unruly；invalid_committee的质押被释放
                release_committee.extend_from_slice(&slash_info.inconsistent_committee);
            } else {
                // 机器上线，惩罚inconsistent 和 unruly，reward_committee的质押被释放
                slashed_committee.extend_from_slice(&slash_info.inconsistent_committee);
                release_committee.extend_from_slice(&slash_info.reward_committee);
            }
        }

        Self::change_committee_stake(slashed_committee.clone(), slash_info.committee_stake, true)?;
        Self::change_committee_stake(release_committee, slash_info.committee_stake, false)?;

        // NOTE: 这里没有奖励
        <T as Config>::SlashAndReward::slash_and_reward(
            slashed_committee,
            slash_info.committee_stake,
            vec![],
        )?;

        slash_info.slash_result = OCSlashResult::Executed;
        ItemList::rm_item(pending_unhandled_slash, &slash_id);
        PendingSlash::<T>::insert(slash_id, slash_info);

        Ok(())
    }
}
