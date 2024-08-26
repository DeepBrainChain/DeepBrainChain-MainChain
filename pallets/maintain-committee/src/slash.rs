use crate::{
    pallet::AccountId2ReserveDLC, Config, DLCBalanceOf, DLCMachine2ReportInfo, Pallet,
    PendingSlashReview, ReportResult, ReporterStake, UnhandledReportResult, ONE_DLC,
};
use dbc_support::{
    report::{MCSlashResult, MTReportResultInfo, ReportResultType},
    traits::MTOps,
    ReportId,
};
use frame_support::{
    traits::{fungibles::Mutate, tokens::Preservation},
    IterableStorageMap,
};
use sp_runtime::{SaturatedConversion, Saturating};
use sp_std::{vec, vec::Vec};

impl<T: Config> Pallet<T> {
    pub fn exec_slash() -> Result<(), ()> {
        let now = <frame_system::Pallet<T>>::block_number();

        for slashed_report_id in Self::unhandled_report_result(now) {
            let mut report_result_info = Self::report_result(&slashed_report_id).ok_or(())?;

            let MTReportResultInfo {
                reporter,
                reporter_stake,
                inconsistent_committee,
                unruly_committee,
                reward_committee,
                committee_stake,
                report_result,
                ..
            } = report_result_info.clone();

            if !Self::is_dlc_machine_fault_report(report_result_info.report_id) {
                Self::change_reporter_stake_on_report_close(
                    &reporter,
                    reporter_stake,
                    report_result.clone(),
                );
            }

            let mut slashed_committee = unruly_committee;
            // 无论哪种情况，被惩罚的委员会都是 未完成工作 + 与多数不一致的委员会
            slashed_committee.extend_from_slice(&inconsistent_committee);

            let mut reward_who = vec![];

            match report_result {
                ReportResultType::ReportSucceed => {
                    reward_who.extend_from_slice(&reward_committee);
                    reward_who.push(reporter.clone());
                    if Self::is_dlc_machine_fault_report(report_result_info.report_id) {
                        let account_id_for_reserve_dlc = Self::pallet_account_id().ok_or(())?;
                        let asset_id = Self::get_dlc_asset_id();
                        let reserve_amount = Self::get_dlc_reserve_amount();

                        <pallet_assets::Pallet<T> as Mutate<T::AccountId>>::transfer(
                            asset_id,
                            &account_id_for_reserve_dlc,
                            &reporter,
                            reserve_amount,
                            Preservation::Expendable,
                        )
                        .map_err(|_| ())?;

                        AccountId2ReserveDLC::<T>::mutate(&reporter, |reserve_dlc| {
                            *reserve_dlc = reserve_dlc.saturating_sub(reserve_amount)
                        });

                        dlc_machine::Pallet::<T>::report_dlc_machine_slashed(
                            report_result_info.machine_id.clone(),
                        );
                    }
                },
                // NoConsensus means no committee confirm confirmation, should be slashed all
                ReportResultType::NoConsensus => {},
                ReportResultType::ReportRefused |
                ReportResultType::ReporterNotSubmitEncryptedInfo => {
                    if !Self::is_dlc_machine_fault_report(report_result_info.report_id) {
                        // 惩罚报告人
                        let _ = Self::slash_and_reward(
                            vec![reporter.clone()],
                            reporter_stake,
                            reward_committee.clone(),
                        );
                    } else {
                        let _ = Self::slash_reporter_dlc(
                            reporter,
                            reward_committee.clone(),
                            DLCBalanceOf::<T>::saturated_from(10000 * ONE_DLC as u64),
                        );
                    }
                },
            }

            let _ = Self::change_committee_stake_on_report_close(
                reward_committee.clone(),
                committee_stake,
                false,
            );
            let _ = Self::change_committee_stake_on_report_close(
                slashed_committee.clone(),
                committee_stake,
                true,
            );
            let _ = Self::slash_and_reward(slashed_committee, committee_stake, reward_who);

            report_result_info.slash_result = MCSlashResult::Executed;
            ReportResult::<T>::insert(slashed_report_id, report_result_info.clone());

            if Self::is_dlc_machine_fault_report(report_result_info.report_id) {
                DLCMachine2ReportInfo::<T>::mutate(report_result_info.machine_id, |report_info| {
                    if let Some((report_id, reporter_evm_address, _)) = report_info {
                        return Some((
                            report_id.clone(),
                            reporter_evm_address.clone(),
                            report_result_info.slash_exec_time.saturated_into::<u64>(),
                        ))
                    };
                    None
                });
            }
        }

        // NOTE: 检查之后再删除，速度上要快非常多
        if UnhandledReportResult::<T>::contains_key(now) {
            UnhandledReportResult::<T>::remove(now);
        }

        Ok(())
    }

    pub fn exec_review() -> Result<(), ()> {
        let now = <frame_system::Pallet<T>>::block_number();
        let all_pending_review = <PendingSlashReview<T> as IterableStorageMap<ReportId, _>>::iter()
            .map(|(renter, _)| renter)
            .collect::<Vec<_>>();

        for a_pending_review in all_pending_review {
            let review_info = Self::pending_slash_review(a_pending_review).ok_or(())?;
            let report_result_info = Self::report_result(&a_pending_review).ok_or(())?;

            if review_info.expire_time < now {
                continue
            }

            let is_slashed_reporter =
                report_result_info.is_slashed_reporter(&review_info.applicant);
            let is_slashed_committee =
                report_result_info.is_slashed_committee(&review_info.applicant);
            let is_slashed_stash =
                report_result_info.is_slashed_stash(review_info.applicant.clone());

            if is_slashed_reporter {
                ReporterStake::<T>::mutate(&review_info.applicant, |reporter_stake| {
                    reporter_stake.change_stake_on_report_close(review_info.staked_amount, true);
                })
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

            let _ = Self::slash_and_reward(
                vec![review_info.applicant],
                review_info.staked_amount,
                vec![],
            );

            PendingSlashReview::<T>::remove(a_pending_review);
        }
        Ok(())
    }

    pub fn is_dlc_machine_fault_report(report_id: ReportId) -> bool {
        if let Some(report_info) = Self::report_info(report_id) {
            return report_info.reporter_stake == 0u32.into()
        }
        return false
    }
}
