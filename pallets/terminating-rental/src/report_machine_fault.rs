use crate::*;
use dbc_support::{
    machine_type::MachineStatus,
    traits::{GNOps, ManageCommittee},
    ItemList, ReportId, ONE_DAY, ONE_HOUR, THREE_HOUR,
};
use frame_support::{dispatch::DispatchResultWithPostInfo, ensure, traits::ReservableCurrency};
use sp_runtime::traits::{Saturating, Zero};
use sp_std::{vec, vec::Vec};

pub const HALF_HOUR: u32 = 60;

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

            report_result_info.slash_result = IRReportSlashResult::Executed;
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

    pub fn summary_fault_report_hook() {
        let mut live_report = Self::live_report();

        // 需要检查的report可能是正在被委员会验证/仍然可以预订的状态
        let mut verifying_report = live_report.verifying_report.clone();
        verifying_report.extend(live_report.bookable_report.clone());
        let submitting_raw_report = live_report.waiting_raw_report.clone();

        // 委员会正在验证的报告
        verifying_report.iter().for_each(|&report_id| {
            let _ = Self::summary_verifying_report(report_id, &mut live_report);
        });

        // 委员会正在提交原始值的报告
        submitting_raw_report.iter().for_each(|&report_id| {
            let _ = Self::summary_submitting_raw(report_id, &mut live_report);
        });

        LiveReport::<T>::put(live_report);
    }

    fn summary_verifying_report(
        report_id: ReportId,
        live_report: &mut IRLiveReportList,
    ) -> Result<(), ()> {
        let now = <frame_system::Module<T>>::block_number();
        let committee_order_stake = Self::get_stake_per_order().unwrap_or_default();

        let report_info = Self::report_info(&report_id);
        report_info.can_summary_fault()?;

        let mut reporter_report = Self::reporter_report(&report_info.reporter);
        let mut report_result = Self::report_result(report_id);

        // 初始化report_result
        report_result = IRReportResultInfo {
            report_id,
            reporter: report_info.reporter.clone(),
            reporter_stake: report_info.reporter_stake,
            committee_stake: committee_order_stake,
            slash_time: now,
            slash_exec_time: now + TWO_DAY.into(),
            slash_result: IRReportSlashResult::Pending,

            ..report_result
        };

        if now - report_info.first_book_time < THREE_HOUR.into() {
            // 处理三小时之前的问题，报告人/委员会不按时提交信息的情况
            Self::summary_before_submit_raw(
                report_id,
                now,
                live_report,
                &mut reporter_report,
                &mut report_result,
            )?;
        } else {
            // 处理超过3小时，仍然处于验证中|等待预订情况
            Self::summary_after_submit_raw(report_id, now, live_report)?;
        }

        Ok(())
    }

    // 在第一个预订后，3个小时前进行检查
    fn summary_before_submit_raw(
        report_id: ReportId,
        now: T::BlockNumber,

        live_report: &mut IRLiveReportList,
        reporter_report: &mut IRReporterReportList,
        report_result: &mut IRReportResultInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>,
    ) -> Result<(), ()> {
        let mut report_info = Self::report_info(&report_id);

        // Reported, WaitingBook, CommitteeConfirmed, SubmittingRaw
        if !matches!(report_info.report_status, IRReportStatus::Verifying) {
            return Ok(())
        }

        let verifying_committee = report_info.verifying_committee.clone().ok_or(())?;
        let committee_ops = Self::committee_report_ops(&verifying_committee, &report_id);

        // 报告人没有在规定时间内提交给加密信息，则惩罚报告人到国库，不进行奖励
        if now - committee_ops.booked_time >= HALF_HOUR.into() &&
            committee_ops.encrypted_err_info.is_none()
        {
            reporter_report.clean_not_submit_encrypted_report(report_id);
            ReporterReport::<T>::insert(&report_info.reporter, reporter_report);

            // 清理存储: CommitteeOps, LiveReport, CommitteeOrder, ReporterRecord
            report_info.booked_committee.iter().for_each(|a_committee| {
                let committee_ops = Self::committee_report_ops(a_committee, &report_id);
                let _ = <T as Config>::ManageCommittee::change_used_stake(
                    a_committee.clone(),
                    committee_ops.staked_balance,
                    false,
                );
                CommitteeReportOps::<T>::remove(a_committee, report_id);

                CommitteeReportOrder::<T>::mutate(a_committee, |committee_order| {
                    committee_order.clean_unfinished_order(&report_id);
                });
            });

            ItemList::rm_item(&mut live_report.verifying_report, &report_id);
            report_result.report_result = IRReportResultType::ReporterNotSubmitEncryptedInfo;
            Self::update_unhandled_report(report_id, true, report_result.slash_exec_time);
            ReportResult::<T>::insert(report_id, report_result);

            return Ok(())
        }

        // 委员会没有提交Hash，删除该委员会，并惩罚
        if now - committee_ops.booked_time >= ONE_HOUR.into() {
            report_info.clean_not_submit_hash_committee(&verifying_committee);
            live_report.clean_not_submit_hash_report(report_id);

            CommitteeReportOrder::<T>::mutate(&verifying_committee, |committee_order| {
                ItemList::rm_item(&mut committee_order.booked_report, &report_id);
            });

            ReportInfo::<T>::insert(report_id, report_info.clone());
            CommitteeReportOps::<T>::remove(&verifying_committee, &report_id);

            // NOTE: should not insert directly when summary result, but should alert exist data
            ItemList::add_item(&mut report_result.unruly_committee, verifying_committee.clone());
            Self::update_unhandled_report(report_id, true, report_result.slash_exec_time);
            ReportResult::<T>::insert(report_id, report_result);
        }
        Ok(())
    }

    // 统计委员会正在提交原始值的机器
    fn summary_submitting_raw(report_id: ReportId, live_report: &mut IRLiveReportList) {
        let now = <frame_system::Module<T>>::block_number();
        let committee_order_stake = Self::get_stake_per_order().unwrap_or_default();

        let mut report_info = Self::report_info(&report_id);
        let mut report_result = Self::report_result(report_id);

        if !report_info.can_summary(now) {
            return
        }

        let fault_report_result = report_info.summary();
        live_report.get_verify_result(report_id, fault_report_result.clone());
        report_result.get_verify_result(now, report_id, committee_order_stake, &report_info);

        match fault_report_result.clone() {
            // 报告成功
            ReportConfirmStatus::Confirmed(sp_committee, ag_committee, _) => {
                // 改变committee_order
                let mut committee = sp_committee.clone();
                committee.extend(ag_committee);
                sp_committee.iter().for_each(|a_committee| {
                    CommitteeReportOrder::<T>::mutate(&a_committee, |committee_order| {
                        ItemList::rm_item(&mut committee_order.confirmed_report, &report_id);
                        ItemList::add_item(&mut committee_order.finished_report, report_id);
                    });
                });

                // 根据错误类型，下线机器并记录
                let mut machine_info = Self::machines_info(&report_info.machine_id);
                let mut live_machine = Self::live_machines();

                live_machine.machine_offline(report_info.machine_id.clone());

                // After re-online, machine status is same as former
                machine_info.machine_status = MachineStatus::ReporterReportOffline(
                    report_info.machine_fault_type.into_op_err(report_info.report_time),
                    // fault_type,
                    Box::new(machine_info.machine_status),
                    report_info.reporter.clone(),
                    committee,
                );

                LiveMachines::<T>::put(live_machine);
                MachinesInfo::<T>::insert(&report_info.machine_id, machine_info.clone());

                // When Reported offline, after 5 days, reach max slash amount;
                let now = <frame_system::Module<T>>::block_number();
                PendingOfflineSlash::<T>::insert(
                    now + (5 * ONE_DAY).saturated_into::<T::BlockNumber>(),
                    &report_info.machine_id,
                    (Some(report_info.reporter.clone()), machine_info.renters),
                );
            },
            // 报告失败
            ReportConfirmStatus::Refuse(mut sp_committees, ag_committee) => {
                sp_committees.extend(ag_committee);
                sp_committees.iter().for_each(|a_committee| {
                    CommitteeReportOrder::<T>::mutate(&a_committee, |committee_order| {
                        ItemList::rm_item(&mut committee_order.confirmed_report, &report_id);
                        ItemList::add_item(&mut committee_order.finished_report, report_id);
                    });
                });
            },
            // 如果没有人提交，会出现NoConsensus的情况，并重新派单
            ReportConfirmStatus::NoConsensus => {
                // 所有booked_committee都应该被惩罚
                report_info.booked_committee.clone().iter().for_each(|a_committee| {
                    CommitteeReportOps::<T>::remove(&a_committee, report_id);

                    CommitteeReportOrder::<T>::mutate(&a_committee, |committee_order| {
                        ItemList::rm_item(&mut committee_order.booked_report, &report_id);
                        ItemList::rm_item(&mut committee_order.hashed_report, &report_id);
                    })
                });

                let mut reporter_report = Self::reporter_report(&report_info.reporter);
                // 重新举报
                let _ = Self::do_report_machine_fault(
                    report_info.reporter.clone(),
                    report_info.machine_fault_type.clone(),
                    Some(report_info.report_time),
                    live_report,
                    &mut reporter_report,
                );
                ReporterReport::<T>::insert(&report_info.reporter, reporter_report);
            },
        }

        Self::update_unhandled_report(report_id, true, now + TWO_DAY.into());

        if report_info.report_status != IRReportStatus::Reported {
            report_info.report_status = IRReportStatus::CommitteeConfirmed;
        }
        ReportResult::<T>::insert(report_id, report_result);
        ReportInfo::<T>::insert(report_id, report_info);
    }

    // 在到提交raw的时间点后，修改report_info的状态；
    // 并在提交raw开始前，如果有正在验证的委员会(还未完成工作)，则移除其信息，退还质押，不作处理。
    fn summary_after_submit_raw(
        report_id: ReportId,
        now: T::BlockNumber,
        live_report: &mut IRLiveReportList,
    ) -> Result<(), ()> {
        live_report.clean_unfinished_report(&report_id);
        ItemList::add_item(&mut live_report.waiting_raw_report, report_id);

        let mut report_info = Self::report_info(&report_id);

        if let IRReportStatus::WaitingBook = report_info.report_status {
            report_info.report_status = IRReportStatus::SubmittingRaw;
            ReportInfo::<T>::insert(report_id, report_info);
            return Ok(())
        }

        // 但是最后一个委员会订阅时间小于1个小时
        let verifying_committee = report_info.verifying_committee.ok_or(())?;
        let committee_ops = Self::committee_report_ops(&verifying_committee, &report_id);

        if now - committee_ops.booked_time < ONE_HOUR.into() {
            // 从最后一个委员会的存储中删除,并退还质押
            CommitteeReportOrder::<T>::mutate(&verifying_committee, |committee_order| {
                committee_order.clean_unfinished_order(&report_id);
            });

            let _ = <T as Config>::ManageCommittee::change_used_stake(
                verifying_committee.clone(),
                committee_ops.staked_balance,
                false,
            );

            CommitteeReportOps::<T>::remove(&verifying_committee, report_id);

            ReportInfo::<T>::mutate(report_id, |report_info| {
                // 将最后一个委员会移除，不惩罚
                report_info.clean_not_submit_raw_committee(&verifying_committee);
            });
        }
        Ok(())
    }

    pub fn update_unhandled_report(
        report_id: ReportId,
        is_add: bool,
        slash_exec_time: T::BlockNumber,
    ) {
        UnhandledReportResult::<T>::mutate(slash_exec_time, |unhandled_report_result| {
            if is_add {
                ItemList::add_item(unhandled_report_result, report_id);
            } else {
                ItemList::rm_item(unhandled_report_result, &report_id);
            }
        });
    }
}
