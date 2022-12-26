use crate::{BalanceOf, Config, Error, NextReportId, Pallet, ReporterStake, UnhandledReportResult};
use dbc_support::{
    report::{MTReportInfoDetail, ReportResultType},
    traits::{GNOps, ManageCommittee},
    ItemList, ReportHash, ReportId,
};
use frame_support::{dispatch::DispatchResultWithPostInfo, ensure, traits::ReservableCurrency};
use sp_io::hashing::blake2_128;
use sp_runtime::traits::{Saturating, Zero};
use sp_std::vec::Vec;

impl<T: Config> Pallet<T> {
    pub fn get_stake_per_order() -> Result<BalanceOf<T>, Error<T>> {
        <T as Config>::ManageCommittee::stake_per_order().ok_or(Error::<T>::GetStakeAmountFailed)
    }

    pub fn is_valid_committee(who: &T::AccountId) -> DispatchResultWithPostInfo {
        ensure!(<T as Config>::ManageCommittee::is_valid_committee(who), Error::<T>::NotCommittee);
        Ok(().into())
    }

    pub fn pay_fixed_tx_fee(who: T::AccountId) -> DispatchResultWithPostInfo {
        <generic_func::Module<T>>::pay_fixed_tx_fee(who).map_err(|_| Error::<T>::PayTxFeeFailed)?;
        Ok(().into())
    }

    // 判断Hash是否被提交过
    pub fn is_uniq_hash(
        report_id: ReportId,
        report_info: &MTReportInfoDetail<T::AccountId, T::BlockNumber, BalanceOf<T>>,
        hash: ReportHash,
    ) -> DispatchResultWithPostInfo {
        for a_committee in &report_info.hashed_committee {
            let committee_ops = Self::committee_ops(a_committee, report_id);
            if committee_ops.confirm_hash == hash {
                return Err(Error::<T>::DuplicateHash.into())
            }
        }
        Ok(().into())
    }

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

    // 各种报告类型，都需要质押 1000 DBC
    // 如果是第一次绑定，则需要质押2w DBC，其他情况:
    pub fn pay_stake_when_report(reporter: T::AccountId) -> DispatchResultWithPostInfo {
        let stake_params = Self::reporter_stake_params().ok_or(Error::<T>::GetStakeAmountFailed)?;

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
