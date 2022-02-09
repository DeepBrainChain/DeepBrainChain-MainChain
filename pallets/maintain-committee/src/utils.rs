use crate::{
    types::{ReportId, ReporterStakeParamsInfo},
    BalanceOf, Config, Error, NextReportId, Pallet, ReporterStake, UnhandledReportResult,
};
use frame_support::{dispatch::DispatchResultWithPostInfo, ensure, traits::ReservableCurrency};
use generic_func::ItemList;
use online_profile_machine::ManageCommittee;
use sp_io::hashing::blake2_128;
use sp_runtime::traits::{CheckedSub, Zero};
use sp_std::vec::Vec;

impl<T: Config> Pallet<T> {
    pub fn get_hash(raw_str: &Vec<u8>) -> [u8; 16] {
        blake2_128(raw_str)
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

    pub fn update_unhandled_report(report_id: ReportId, is_add: bool) {
        let mut unhandled_report_result = Self::unhandled_report_result();
        if is_add {
            ItemList::add_item(&mut unhandled_report_result, report_id);
        } else {
            ItemList::rm_item(&mut unhandled_report_result, &report_id);
        }
        UnhandledReportResult::<T>::put(unhandled_report_result);
    }

    pub fn pay_stake_when_report(
        reporter: T::AccountId,
        stake_params: &ReporterStakeParamsInfo<BalanceOf<T>>,
    ) -> DispatchResultWithPostInfo {
        let mut reporter_stake = Self::reporter_stake(&reporter);

        // 各种报告类型，都需要质押 1000 DBC
        // 如果是第一次绑定，则需要质押2w DBC，其他情况:
        if reporter_stake.staked_amount == Zero::zero() {
            ensure!(
                <T as Config>::Currency::can_reserve(&reporter, stake_params.stake_baseline),
                Error::<T>::BalanceNotEnough
            );

            <T as Config>::Currency::reserve(&reporter, stake_params.stake_baseline)
                .map_err(|_| Error::<T>::BalanceNotEnough)?;
            reporter_stake.staked_amount = stake_params.stake_baseline;
            reporter_stake.used_stake = stake_params.stake_per_report;
        } else {
            reporter_stake.used_stake += stake_params.stake_per_report;
            ensure!(
                reporter_stake.staked_amount - reporter_stake.used_stake
                    >= stake_params.min_free_stake_percent * reporter_stake.staked_amount,
                Error::<T>::StakeNotEnough
            );
        }

        ReporterStake::<T>::insert(&reporter, reporter_stake);
        Ok(().into())
    }

    // - Writes:
    // if is_slash:
    //      used_stake, total_stake
    // else:
    //      used_stake
    pub fn change_reporter_stake_on_report_close(
        reporter: &T::AccountId,
        amount: BalanceOf<T>,
        is_slash: bool,
    ) -> Result<(), ()> {
        let mut reporter_stake = Self::reporter_stake(reporter);
        reporter_stake.used_stake = reporter_stake.used_stake.checked_sub(&amount).ok_or(())?;

        if is_slash {
            reporter_stake.staked_amount = reporter_stake.staked_amount.checked_sub(&amount).ok_or(())?;
        }

        ReporterStake::<T>::insert(reporter, reporter_stake);
        Ok(())
    }

    // - Writes:
    // if is_slash:
    //      used_stake, total_stake
    // else:
    //      used_stake
    pub fn change_committee_stake_on_report_close(
        committee_list: Vec<T::AccountId>,
        amount: BalanceOf<T>,
        is_slash: bool,
    ) -> Result<(), ()> {
        for a_committee in committee_list {
            if is_slash {
                <T as Config>::ManageCommittee::change_total_stake(a_committee.clone(), amount, false)?;
            }

            <T as Config>::ManageCommittee::change_used_stake(a_committee, amount, false)?;
        }

        Ok(())
    }
}
