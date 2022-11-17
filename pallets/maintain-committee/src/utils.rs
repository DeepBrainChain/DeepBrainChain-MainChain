use crate::{
    types::{ReportId, ReportResultType, ReporterStakeParamsInfo},
    BalanceOf, Config, Error, NextReportId, Pallet, ReporterStake, UnhandledReportResult,
};
use dbc_support::traits::{GNOps, ManageCommittee};
use frame_support::{dispatch::DispatchResultWithPostInfo, ensure, traits::ReservableCurrency};
use generic_func::ItemList;
use sp_io::hashing::blake2_128;
use sp_runtime::traits::Zero;
use sp_std::vec::Vec;

impl<T: Config> Pallet<T> {
    // Warp for SlashAndReward::slash_and_reward
    pub fn slash_and_reward(
        slash_who: Vec<T::AccountId>,
        slash_amount: BalanceOf<T>,
        reward_who: Vec<T::AccountId>,
    ) -> Result<(), ()> {
        <T as Config>::SlashAndReward::slash_and_reward(slash_who, slash_amount, reward_who)
    }

    pub fn get_hash(raw_str: Vec<Vec<u8>>) -> [u8; 16] {
        let mut full_str = Vec::new();
        for a_str in raw_str {
            full_str.extend(a_str);
        }
        blake2_128(&full_str)
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
                reporter_stake.staked_amount - reporter_stake.used_stake >=
                    stake_params.min_free_stake_percent * reporter_stake.staked_amount,
                Error::<T>::StakeNotEnough
            );
        }

        ReporterStake::<T>::insert(&reporter, reporter_stake);
        Ok(().into())
    }

    // - Writes:
    // if is_slash: used_stake, total_stake
    // else:        used_stake
    pub fn change_reporter_stake_on_report_close(
        reporter: &T::AccountId,
        amount: BalanceOf<T>,
        report_result: ReportResultType,
    ) {
        // 未达成共识，则退还报告人质押
        if let ReportResultType::NoConsensus = report_result {
            return
        }

        ReporterStake::<T>::mutate(reporter, |reporter_stake| {
            // 报告被拒绝或报告人没完成工作，将被惩罚，否则不惩罚并退还
            let is_slashed = matches!(
                report_result,
                ReportResultType::ReportRefused | ReportResultType::ReporterNotSubmitEncryptedInfo
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
}
