use crate::{
    BalanceOf, CommitteeReportOps, CommitteeReportOrder, Config, Error, Event, IRLiveReportList,
    IRMachineFaultType, IRReportInfoDetail, IRReporterReportList, LiveReport, NextReportId, Pallet,
    ReportId, ReportInfo, ReporterStake,
};
use frame_support::{dispatch::DispatchResultWithPostInfo, ensure, traits::ReservableCurrency};
use generic_func::ItemList;
use sp_runtime::traits::{Saturating, Zero};

impl<T: Config> Pallet<T> {
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
}
