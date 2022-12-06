use crate::{
    BalanceOf, CommitteeReportOps, CommitteeReportOrder, Config, Error, Event, IRLiveReportList,
    IRMachineFaultType, IRReportInfoDetail, IRReportResultInfo, IRReportResultType,
    IRReporterReportList, IRSlashResult, LiveReport, NextReportId, Pallet, ReportId, ReportInfo,
    ReportResult, ReporterStake, UnhandledReportResult,
};
use dbc_support::traits::{GNOps, ManageCommittee};
use frame_support::{dispatch::DispatchResultWithPostInfo, ensure, traits::ReservableCurrency};
use generic_func::ItemList;
use sp_runtime::traits::{Saturating, Zero};
use sp_std::{vec, vec::Vec};

impl<T: Config> Pallet<T> {
    // Warp for SlashAndReward::slash_and_reward
    pub fn slash_and_reward(
        slash_who: Vec<T::AccountId>,
        slash_amount: BalanceOf<T>,
        reward_who: Vec<T::AccountId>,
    ) -> Result<(), ()> {
        <T as Config>::SlashAndReward::slash_and_reward(slash_who, slash_amount, reward_who)
    }

    // 各种报告类型，都需要质押 1000 DBC
    // 如果是第一次绑定，则需要质押2w DBC，其他情况:
    pub fn pay_stake_when_report(reporter: T::AccountId) -> DispatchResultWithPostInfo {
        let stake_params = Self::reporter_stake_params();
        if stake_params.stake_per_report == Zero::zero() {
            return Ok(().into())
        }

        ReporterStake::<T>::mutate(&reporter, |reporter_stake| {
            if reporter_stake.staked_amount == Zero::zero() {
                <T as Config>::Currency::reserve(&reporter, stake_params.stake_baseline)
                    .map_err(|_| Error::<T>::BalanceNotEnough)?;
                reporter_stake.staked_amount = stake_params.stake_baseline;
                reporter_stake.used_stake = stake_params.stake_per_report;
            } else {
                reporter_stake.used_stake =
                    reporter_stake.used_stake.saturating_add(stake_params.stake_per_report);
                ensure!(
                    reporter_stake.staked_amount - reporter_stake.used_stake >=
                        stake_params.min_free_stake_percent * reporter_stake.staked_amount,
                    Error::<T>::StakeNotEnough
                );
            }

            Ok(().into())
        })
    }

    // is_add: ReporterStake改变，并reserve 一定金额
    // !is_add: ReporterStake改变，并unreserve一定金额
    pub fn change_reporter_stake(
        reporter: T::AccountId,
        amount: BalanceOf<T>,
        is_add: bool,
    ) -> DispatchResultWithPostInfo {
        let stake_params = Self::reporter_stake_params();
        let mut reporter_stake = Self::reporter_stake(&reporter);

        if is_add {
            reporter_stake.staked_amount = reporter_stake.staked_amount.saturating_add(amount);
        } else {
            reporter_stake.staked_amount = reporter_stake.staked_amount.saturating_sub(amount);
        }

        if is_add || reporter_stake.used_stake > Zero::zero() {
            ensure!(
                reporter_stake.staked_amount >= reporter_stake.used_stake,
                Error::<T>::StakeNotEnough
            );

            ensure!(
                reporter_stake.staked_amount.saturating_sub(reporter_stake.used_stake) >=
                    stake_params.min_free_stake_percent * reporter_stake.staked_amount,
                Error::<T>::StakeNotEnough
            );
        }

        if is_add {
            <T as Config>::Currency::reserve(&reporter, amount)
                .map_err(|_| Error::<T>::BalanceNotEnough)?;
            ReporterStake::<T>::insert(&reporter, reporter_stake);
            Self::deposit_event(Event::ReporterAddStake(reporter, amount));
        } else {
            <T as Config>::Currency::unreserve(&reporter, amount);
            ReporterStake::<T>::insert(&reporter, reporter_stake);
            Self::deposit_event(Event::ReporterReduceStake(reporter, amount));
        }

        Ok(().into())
    }

    // 处理用户报告逻辑
    // 记录：ReportInfo, LiveReport, ReporterReport 并支付处理所需的金额
    pub fn do_report_machine_fault(
        reporter: T::AccountId,
        machine_fault_type: IRMachineFaultType,
        report_time: Option<T::BlockNumber>,
        live_report: &mut IRLiveReportList,
        reporter_report: &mut IRReporterReportList,
    ) -> DispatchResultWithPostInfo {
        // 获取处理报告需要的信息
        let stake_params = Self::reporter_stake_params();
        let report_id = Self::get_new_report_id();
        let report_time = report_time.unwrap_or_else(|| <frame_system::Module<T>>::block_number());

        // 记录到 live_report & reporter_report
        live_report.new_report(report_id);
        reporter_report.new_report(report_id);

        ReportInfo::<T>::insert(
            &report_id,
            IRReportInfoDetail::new(
                reporter.clone(),
                report_time,
                machine_fault_type.clone(),
                stake_params.stake_per_report,
            ),
        );

        Self::deposit_event(Event::ReportMachineFault(reporter, machine_fault_type));
        Ok(().into())
    }

    pub fn get_new_report_id() -> ReportId {
        let report_id = Self::next_report_id();
        if report_id == u64::MAX {
            NextReportId::<T>::put(0);
        } else {
            NextReportId::<T>::put(report_id + 1);
        };
        report_id
    }

    pub fn book_report(
        committee: T::AccountId,
        report_id: ReportId,
        report_info: &mut IRReportInfoDetail<T::AccountId, T::BlockNumber, BalanceOf<T>>,
        order_stake: BalanceOf<T>,
    ) {
        let now = <frame_system::Module<T>>::block_number();
        let mft = report_info.machine_fault_type.clone();

        report_info.book_report(committee.clone(), now);
        CommitteeReportOrder::<T>::mutate(&committee, |committee_order| {
            ItemList::add_item(&mut committee_order.booked_report, report_id);
        });
        CommitteeReportOps::<T>::mutate(&committee, &report_id, |committee_ops| {
            committee_ops.book_report(mft.clone(), now, order_stake);
        });
        LiveReport::<T>::mutate(|live_report| {
            live_report.book_report(report_id, mft, report_info.booked_committee.len());
        });

        ReportInfo::<T>::insert(&report_id, report_info);
    }

    pub fn exec_report_slash() -> Result<(), ()> {
        let now = <frame_system::Module<T>>::block_number();

        for slashed_report_id in Self::unhandled_report_result(now) {
            let mut report_result_info = Self::report_result(&slashed_report_id);

            let IRReportResultInfo {
                reporter,
                reporter_stake,
                inconsistent_committee,
                unruly_committee,
                reward_committee,
                committee_stake,
                report_result,
                ..
            } = report_result_info.clone();

            Self::change_reporter_stake_on_report_close(
                &reporter,
                reporter_stake,
                report_result.clone(),
            );

            let mut slashed_committee = unruly_committee;
            // 无论哪种情况，被惩罚的委员会都是 未完成工作 + 与多数不一致的委员会
            slashed_committee.extend_from_slice(&inconsistent_committee);

            let mut reward_who = vec![];

            match report_result {
                IRReportResultType::ReportSucceed => {
                    reward_who.extend_from_slice(&reward_committee);
                    reward_who.push(reporter);
                },
                // NoConsensus means no committee confirm confirmation, should be slashed all
                IRReportResultType::NoConsensus => {},
                IRReportResultType::ReportRefused |
                IRReportResultType::ReporterNotSubmitEncryptedInfo => {
                    // 惩罚报告人
                    let _ = Self::slash_and_reward(
                        vec![reporter.clone()],
                        reporter_stake,
                        reward_committee.clone(),
                    );
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

            report_result_info.slash_result = IRSlashResult::Executed;
            ReportResult::<T>::insert(slashed_report_id, report_result_info);
        }

        // NOTE: 检查之后再删除，速度上要快非常多
        if UnhandledReportResult::<T>::contains_key(now) {
            UnhandledReportResult::<T>::remove(now);
        }

        Ok(())
    }

    // - Writes:
    // if is_slash: used_stake, total_stake
    // else:        used_stake
    pub fn change_reporter_stake_on_report_close(
        reporter: &T::AccountId,
        amount: BalanceOf<T>,
        report_result: IRReportResultType,
    ) {
        // 未达成共识，则退还报告人质押
        if matches!(report_result, IRReportResultType::NoConsensus) {
            return
        }

        ReporterStake::<T>::mutate(reporter, |reporter_stake| {
            // 报告被拒绝或报告人没完成工作，将被惩罚，否则不惩罚并退还
            let is_slashed = matches!(
                report_result,
                IRReportResultType::ReportRefused |
                    IRReportResultType::ReporterNotSubmitEncryptedInfo
            );

            reporter_stake.change_stake_on_report_close(amount, is_slashed);
        });
    }

    // - Writes:
    // if is_slash: used_stake, total_stake
    // else:        used_stake
    pub fn change_committee_stake_on_report_close(
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

    pub fn summary_fault_report_hook() {}
}
