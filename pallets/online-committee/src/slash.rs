use crate::{
    types::OCSlashResult, Config, OCBookResultType, Pallet, PendingSlash, PendingSlashReview,
    UnhandledSlash,
};
use dbc_support::traits::{GNOps, OCOps};
use frame_support::IterableStorageMap;
use generic_func::{ItemList, SlashId};
use sp_runtime::traits::Zero;
use sp_std::{vec, vec::Vec};

impl<T: Config> Pallet<T> {
    pub fn check_and_exec_pending_review() {
        let all_pending_review = <PendingSlashReview<T> as IterableStorageMap<SlashId, _>>::iter()
            .map(|(slash_id, _)| slash_id)
            .collect::<Vec<_>>();

        for a_pending_review in all_pending_review {
            if Self::do_a_pending_review(a_pending_review).is_err() {
                continue
            };
        }
    }

    fn do_a_pending_review(a_pending_review: SlashId) -> Result<(), ()> {
        let now = <frame_system::Module<T>>::block_number();

        let review_info = Self::pending_slash_review(a_pending_review);
        let slash_info = Self::pending_slash(a_pending_review);

        if review_info.expire_time < now {
            return Ok(())
        }

        let is_slashed_stash = matches!(slash_info.book_result, OCBookResultType::OnlineRefused) &&
            slash_info.machine_stash == review_info.applicant;

        if is_slashed_stash {
            // Change stake amount
            // NOTE: should not change slash_info.slash_amount, because it will be done in
            // check_and_exec_pending_slash
            T::OCOperations::oc_exec_slash(
                slash_info.machine_stash.clone(),
                review_info.staked_amount,
            )?;

            <T as Config>::SlashAndReward::slash_and_reward(
                vec![slash_info.machine_stash],
                slash_info.stash_slash_amount,
                slash_info.reward_committee,
            )?;
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
                continue
            };
        }
        UnhandledSlash::<T>::put(pending_unhandled_id);
    }

    fn do_a_slash(slash_id: SlashId, pending_unhandled_slash: &mut Vec<SlashId>) -> Result<(), ()> {
        let now = <frame_system::Module<T>>::block_number();
        let mut slash_info = Self::pending_slash(slash_id);
        if now < slash_info.slash_exec_time {
            return Ok(())
        }

        // stash is slashed
        if !slash_info.stash_slash_amount.is_zero() {
            T::OCOperations::oc_exec_slash(
                slash_info.machine_stash.clone(),
                slash_info.stash_slash_amount,
            )?;

            <T as Config>::SlashAndReward::slash_and_reward(
                vec![slash_info.machine_stash.clone()],
                slash_info.stash_slash_amount,
                slash_info.reward_committee.clone(),
            )?;
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
