use crate::{
    pallet,
    types::{MCSlashResult, ReportId, ReportResultType},
    Config, Pallet, PendingSlashReview, ReportResult, UnhandledReportResult,
};
use dbc_support::traits::{GNOps, MTOps};
use frame_support::IterableStorageMap;
use generic_func::ItemList;
use sp_std::{vec, vec::Vec};

impl<T: Config> Pallet<T> {
    pub fn check_and_exec_slash() -> Result<(), ()> {
        let now = <frame_system::Module<T>>::block_number();

        for slashed_report_id in Self::unhandled_report_result(now) {
            let mut report_result_info = Self::report_result(&slashed_report_id);

            // TODO: refa
            match report_result_info.report_result {
                ReportResultType::ReportSucceed => {
                    let _ = Self::change_reporter_stake_on_report_close(
                        &report_result_info.reporter,
                        report_result_info.reporter_stake,
                        false,
                    );

                    // slash unruly & inconsistent, reward to reward_committee & reporter
                    let mut slash_who = report_result_info.unruly_committee.clone();
                    for a_inconsistent in report_result_info.inconsistent_committee.clone() {
                        ItemList::add_item(&mut slash_who, a_inconsistent);
                    }

                    let mut reward_who = report_result_info.reward_committee.clone();
                    ItemList::add_item(&mut reward_who, report_result_info.reporter.clone());
                    let _ = <T as pallet::Config>::SlashAndReward::slash_and_reward(
                        slash_who.clone(),
                        report_result_info.committee_stake,
                        reward_who,
                    );

                    let _ = Self::change_committee_stake_on_report_close(
                        report_result_info.reward_committee.clone(),
                        report_result_info.committee_stake,
                        false,
                    );

                    let _ = Self::change_committee_stake_on_report_close(
                        slash_who,
                        report_result_info.committee_stake,
                        true,
                    );
                },
                // NoConsensus means no committee confirm confirmation, should be slashed all
                ReportResultType::NoConsensus => {
                    let _ = Self::change_committee_stake_on_report_close(
                        report_result_info.unruly_committee.clone(),
                        report_result_info.committee_stake,
                        true,
                    );

                    // only slash unruly_committee, no reward
                    let _ = <T as pallet::Config>::SlashAndReward::slash_and_reward(
                        report_result_info.unruly_committee.clone(),
                        report_result_info.committee_stake,
                        vec![],
                    );
                },
                ReportResultType::ReportRefused => {
                    let _ = Self::change_reporter_stake_on_report_close(
                        &report_result_info.reporter,
                        report_result_info.reporter_stake,
                        true,
                    );

                    // slash reporter, slash committee
                    let _ = <T as pallet::Config>::SlashAndReward::slash_and_reward(
                        vec![report_result_info.reporter.clone()],
                        report_result_info.reporter_stake,
                        report_result_info.reward_committee.clone(),
                    );

                    let mut slash_who = report_result_info.unruly_committee.clone();
                    for a_inconsistent in report_result_info.inconsistent_committee.clone() {
                        ItemList::add_item(&mut slash_who, a_inconsistent);
                    }

                    let _ = <T as pallet::Config>::SlashAndReward::slash_and_reward(
                        slash_who.clone(),
                        report_result_info.committee_stake,
                        report_result_info.reward_committee.clone(),
                    );

                    let _ = Self::change_committee_stake_on_report_close(
                        slash_who,
                        report_result_info.committee_stake,
                        true,
                    );
                    let _ = Self::change_committee_stake_on_report_close(
                        report_result_info.reward_committee.clone(),
                        report_result_info.committee_stake,
                        false,
                    );
                },
                ReportResultType::ReporterNotSubmitEncryptedInfo => {
                    let _ = Self::change_reporter_stake_on_report_close(
                        &report_result_info.reporter,
                        report_result_info.reporter_stake,
                        true,
                    );

                    // slash reporter, slash committee
                    let _ = <T as pallet::Config>::SlashAndReward::slash_and_reward(
                        vec![report_result_info.reporter.clone()],
                        report_result_info.reporter_stake,
                        vec![],
                    );
                    let _ = <T as pallet::Config>::SlashAndReward::slash_and_reward(
                        report_result_info.unruly_committee.clone(),
                        report_result_info.committee_stake,
                        vec![],
                    );

                    let _ = Self::change_committee_stake_on_report_close(
                        report_result_info.unruly_committee.clone(),
                        report_result_info.committee_stake,
                        true,
                    );
                },
            }

            report_result_info.slash_result = MCSlashResult::Executed;
            ReportResult::<T>::insert(slashed_report_id, report_result_info);
        }

        // NOTE: 检查之后再处理，速度上要快非常多
        if UnhandledReportResult::<T>::contains_key(now) {
            UnhandledReportResult::<T>::remove(now);
        }

        Ok(())
    }

    pub fn check_and_exec_pending_review() -> Result<(), ()> {
        let now = <frame_system::Module<T>>::block_number();
        let all_pending_review = <PendingSlashReview<T> as IterableStorageMap<ReportId, _>>::iter()
            .map(|(renter, _)| renter)
            .collect::<Vec<_>>();

        for a_pending_review in all_pending_review {
            let review_info = Self::pending_slash_review(a_pending_review);
            let report_result_info = Self::report_result(&a_pending_review);

            if review_info.expire_time < now {
                continue
            }

            let is_slashed_reporter =
                report_result_info.is_slashed_reporter(&review_info.applicant);
            let is_slashed_committee =
                report_result_info.is_slashed_committee(&review_info.applicant);
            let is_slashed_stash = report_result_info.is_slashed_stash(&review_info.applicant);

            if is_slashed_reporter {
                let _ = Self::change_reporter_stake_on_report_close(
                    &review_info.applicant,
                    review_info.staked_amount,
                    true,
                );
            } else if is_slashed_committee {
                let _ = Self::change_committee_stake_on_report_close(
                    vec![review_info.applicant.clone()],
                    review_info.staked_amount,
                    true,
                );
            } else if is_slashed_stash {
                let _ = T::MTOps::mt_rm_stash_total_stake(
                    review_info.applicant.clone(),
                    review_info.staked_amount,
                );
            }

            let _ = <T as pallet::Config>::SlashAndReward::slash_and_reward(
                vec![review_info.applicant],
                review_info.staked_amount,
                vec![],
            );

            PendingSlashReview::<T>::remove(a_pending_review);
        }
        Ok(())
    }
}
