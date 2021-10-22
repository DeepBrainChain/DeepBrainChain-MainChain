use crate::{types::OCSlashResult, Config, OCBookResultType, Pallet, PendingSlash, PendingSlashReview, UnhandledSlash};
use frame_support::IterableStorageMap;
use generic_func::{ItemList, SlashId};
use online_profile_machine::{GNOps, OCOps};
use sp_runtime::traits::Zero;
use sp_std::{vec, vec::Vec};

impl<T: Config> Pallet<T> {
    pub fn check_and_exec_pending_review() -> Result<(), ()> {
        let all_pending_review = <PendingSlashReview<T> as IterableStorageMap<SlashId, _>>::iter()
            .map(|(slash_id, _)| slash_id)
            .collect::<Vec<_>>();

        let now = <frame_system::Module<T>>::block_number();

        for a_pending_review in all_pending_review {
            let review_info = Self::pending_slash_review(a_pending_review);
            let slash_info = Self::pending_slash(a_pending_review);

            if review_info.expire_time < now {
                continue
            }

            let is_slashed_stash = match slash_info.book_result {
                OCBookResultType::OnlineRefused => &slash_info.machine_stash == &review_info.applicant,
                _ => false,
            };

            if is_slashed_stash {
                // slash stash

                // Change stake amount
                // NOTE: should not change slash_info.slash_amount, because it will be done in check_and_exec_pending_slash
                let _ = T::OCOperations::oc_exec_slash(slash_info.machine_stash.clone(), review_info.staked_amount);

                let _ = <T as Config>::SlashAndReward::slash_and_reward(
                    vec![slash_info.machine_stash],
                    slash_info.stash_slash_amount,
                    slash_info.reward_committee,
                );
            } else {
                let _ =
                    Self::change_committee_stake(vec![review_info.applicant.clone()], review_info.staked_amount, true);
            }

            // Slash applicant to treasury
            let _ = <T as Config>::SlashAndReward::slash_and_reward(
                vec![review_info.applicant],
                review_info.staked_amount,
                vec![],
            );

            PendingSlashReview::<T>::remove(a_pending_review);
        }

        Ok(())
    }

    pub fn check_and_exec_pending_slash() -> Result<(), ()> {
        let mut pending_unhandled_id = Self::unhandled_slash();

        for slash_id in pending_unhandled_id.clone() {
            if let Err(_) = Self::do_a_slash(slash_id, &mut pending_unhandled_id) {
                continue
            };
        }

        UnhandledSlash::<T>::put(pending_unhandled_id);
        Ok(())
    }

    fn do_a_slash(slash_id: SlashId, pending_unhandled_slash: &mut Vec<SlashId>) -> Result<(), ()> {
        let now = <frame_system::Module<T>>::block_number();
        let mut slash_info = Self::pending_slash(slash_id);
        if now < slash_info.slash_exec_time {
            return Ok(())
        }

        if !slash_info.stash_slash_amount.is_zero() {
            // stash is slashed
            T::OCOperations::oc_exec_slash(slash_info.machine_stash.clone(), slash_info.stash_slash_amount)?;

            <T as Config>::SlashAndReward::slash_and_reward(
                vec![slash_info.machine_stash.clone()],
                slash_info.stash_slash_amount,
                slash_info.reward_committee.clone(),
            )?;
        }

        // Change committee stake amount
        Self::change_committee_stake(slash_info.inconsistent_committee.clone(), slash_info.committee_stake, true)?;
        Self::change_committee_stake(slash_info.unruly_committee.clone(), slash_info.committee_stake, true)?;
        Self::change_committee_stake(slash_info.reward_committee.clone(), slash_info.committee_stake, false)?;

        <T as Config>::SlashAndReward::slash_and_reward(
            slash_info.unruly_committee.clone(),
            slash_info.committee_stake,
            vec![],
        )?;

        <T as Config>::SlashAndReward::slash_and_reward(
            slash_info.inconsistent_committee.clone(),
            slash_info.committee_stake,
            vec![],
        )?;

        slash_info.slash_result = OCSlashResult::Executed;
        ItemList::rm_item(pending_unhandled_slash, &slash_id);
        PendingSlash::<T>::insert(slash_id, slash_info);

        Ok(())
    }
}
