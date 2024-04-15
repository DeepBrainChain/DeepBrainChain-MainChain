use crate::{
    BalanceOf, Config, Error, Event, NextSlashId, Pallet, PendingSlash, PendingSlashReview,
    PendingSlashReviewChecking, StashStake, SysInfo,
};
use dbc_support::{
    machine_info,
    machine_type::MachineStatus,
    traits::GNOps,
    verify_slash::{OPPendingSlashInfo, OPSlashReason},
    MachineId, TWO_DAY,
};
use frame_support::traits::ReservableCurrency;
use sp_runtime::{
    traits::{Saturating, Zero},
    Perbill, SaturatedConversion,
};
use sp_std::{vec, vec::Vec};
use dbc_support::machine_info::MachineInfo;

impl<T: Config> Pallet<T> {
    pub fn get_new_slash_id() -> u64 {
        let slash_id = Self::next_slash_id();

        if slash_id == u64::MAX {
            NextSlashId::<T>::put(0);
        } else {
            NextSlashId::<T>::put(slash_id + 1);
        };

        slash_id
    }

    pub fn slash_and_reward(
        slash_who: T::AccountId,
        slash_amount: BalanceOf<T>,
        reward_to: Vec<T::AccountId>,
    ) -> Result<(), ()> {
        let _ =
            T::SlashAndReward::slash_and_reward(vec![slash_who.clone()], slash_amount, reward_to);

        StashStake::<T>::mutate(&slash_who, |stash_stake| {
            *stash_stake = stash_stake.saturating_sub(slash_amount);
        });
        SysInfo::<T>::mutate(|sys_info| {
            sys_info.total_stake = sys_info.total_stake.saturating_sub(slash_amount);
        });

        Ok(())
    }

    // NOTE: 确保 PendingSlash 添加时，添加该变量
    // PendingSlash 删除时，删除该变量
    pub fn exec_pending_slash() {
        let now = <frame_system::Pallet<T>>::block_number();
        let pending_exec_slash = Self::pending_exec_slash(now);

        for slash_id in pending_exec_slash {
            let slash_info = match Self::pending_slash(slash_id) {
                Some(slash_info) => slash_info,
                None => continue,
            };

            if matches!(
                slash_info.slash_reason,
                OPSlashReason::CommitteeRefusedOnline | OPSlashReason::CommitteeRefusedMutHardware
            ) {
                let _ = Self::slash_and_reward(
                    slash_info.slash_who.clone(),
                    slash_info.slash_amount,
                    slash_info.reward_to_committee.unwrap_or_default(),
                );
            } else {
                let _ = Self::do_slash_deposit(&slash_info);
            }

            Self::deposit_event(Event::<T>::SlashExecuted(
                slash_info.slash_who,
                slash_info.machine_id,
                slash_info.slash_amount,
            ));

            PendingSlash::<T>::remove(slash_id);
        }
    }

    pub fn check_pending_slash() -> Result<(), ()> {
        let now = <frame_system::Pallet<T>>::block_number();
        if !PendingSlashReviewChecking::<T>::contains_key(now) {
            return Ok(())
        }

        let pending_slash_checking = Self::pending_slash_review_checking(now);
        for slash_id in pending_slash_checking {
            let slash_apply_review_info = Self::pending_slash_review(slash_id).ok_or(())?;
            let stash = Self::controller_stash(slash_apply_review_info.applicant).ok_or(())?;

            Self::slash_and_reward(stash, slash_apply_review_info.staked_amount, vec![])?;
            PendingSlashReview::<T>::remove(slash_id);
        }
        PendingSlashReviewChecking::<T>::remove(now);
        Ok(())
    }

    // 当机器主动下线/被举报下线时，返回一个待执行的惩罚信息
    pub fn new_slash_when_offline(
        machine_id: MachineId,
        slash_reason: OPSlashReason<T::BlockNumber>,
        reporter: Option<T::AccountId>,
        // 机器当前租用人
        renters: Vec<T::AccountId>,
        committee: Option<Vec<T::AccountId>>,
        duration: T::BlockNumber,
    ) -> Result<OPPendingSlashInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>, ()> {
        let percent = crate::utils::slash_percent(&slash_reason, duration.saturated_into::<u64>());

        let (reporter, renters, committee) = match slash_reason {
            // 算工主动报告被租用的机器，主动下线
            OPSlashReason::RentedReportOffline(_) => {
                let reporter = match duration.saturated_into::<u64>() {
                    0..=5760 => None,
                    _ => reporter,
                };
                (reporter, renters, None)
            },
            // 算工主动报告在线的机器，主动下线
            OPSlashReason::OnlineReportOffline(_) => (None, vec![], None),
            // 机器处于租用状态，无法访问，这种情况下，reporter == renter
            OPSlashReason::RentedInaccessible(_) => {
                let reporter = match duration.saturated_into::<u64>() {
                    0..=5760 => None,
                    _ => reporter,
                };

                (reporter, renters, committee)
            },
            // 机器处于租用状态，机器出现故障. 10%给用户，20%给验证人，70%进入国库
            OPSlashReason::RentedHardwareMalfunction(_) => (reporter, renters, committee),
            // 机器处于租用状态，机器硬件造假. 10%给用户，20%给验证人，70%进入国库
            OPSlashReason::RentedHardwareCounterfeit(_) => (reporter, renters, committee),
            // 机器在线，被举报无法租用. 10%给用户，20%给验证人，70%进入国库
            OPSlashReason::OnlineRentFailed(_) => (reporter, renters, committee),
            _ => Default::default(),
        };

        Self::new_offline_slash(percent, machine_id, reporter, renters, committee, slash_reason)
    }

    pub fn new_offline_slash(
        slash_percent: u32,
        machine_id: MachineId,
        reporter: Option<T::AccountId>,
        renters: Vec<T::AccountId>,
        committee: Option<Vec<T::AccountId>>,
        slash_reason: OPSlashReason<T::BlockNumber>,
    ) -> Result<OPPendingSlashInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>, ()> {
        let now = <frame_system::Pallet<T>>::block_number();
        let machine_info = Self::machines_info(&machine_id).ok_or(())?;

        let slash_amount = Perbill::from_rational(slash_percent, 100) * machine_info.stake_amount;

        Ok(OPPendingSlashInfo {
            slash_who: machine_info.machine_stash,
            machine_id,
            slash_time: now,
            slash_amount,
            slash_exec_time: now + TWO_DAY.into(),
            reporter,
            renters,
            reward_to_committee: committee,
            slash_reason,
        })
    }

    // FIXME: 是否奖励其他租用人
    // 惩罚掉机器押金，如果执行惩罚后机器押金不够，则状态变为补充质押
    pub fn do_slash_deposit(
        slash_info: &OPPendingSlashInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>,
    ) -> Result<(), ()> {
        let mut machine_info = Self::machines_info(&slash_info.machine_id).ok_or(())?;
        if <T as Config>::Currency::reserved_balance(&machine_info.machine_stash) <
            slash_info.slash_amount
        {
            return Ok(())
        }

        let (mut reward_to_reporter, mut reward_to_committee) = (Zero::zero(), Zero::zero());
        if !slash_info.renters.is_empty() {
            reward_to_reporter = Perbill::from_rational(10u32, 100u32) * slash_info.slash_amount
        };
        if slash_info.reward_to_committee.is_some() {
            reward_to_committee = Perbill::from_rational(20u32, 100u32) * slash_info.slash_amount
        };
        let slash_to_treasury = slash_info
            .slash_amount
            .saturating_sub(reward_to_reporter)
            .saturating_sub(reward_to_committee);

        // reward to reporter:
        if reward_to_reporter > Zero::zero() && !slash_info.renters.is_empty() {
            let _ = Self::slash_and_reward(
                slash_info.slash_who.clone(),
                reward_to_reporter,
                slash_info.renters.clone(),
            );
        }
        // reward to committee
        if reward_to_committee > Zero::zero() && slash_info.reward_to_committee.is_some() {
            let _ = Self::slash_and_reward(
                slash_info.slash_who.clone(),
                reward_to_committee,
                slash_info.reward_to_committee.clone().unwrap(),
            );
        }

        // slash to treasury
        let _ = Self::slash_and_reward(slash_info.slash_who.clone(), slash_to_treasury, vec![]);

        Self::try_to_change_machine_status_to_fulfill(&slash_info.slash_who,machine_info)?;

        return Ok(())
    }

    // 检查已质押资金是否满足单GPU质押金额*gpu数量 若不满足则变更机器状态为fulfill
    pub fn try_to_change_machine_status_to_fulfill(slash_account:&T::AccountId,mut machine_info: MachineInfo<T::AccountId,T::BlockNumber,BalanceOf<T>>)->Result<(),()>{
        let staked_amount = Self::stash_stake(&slash_account);
        let online_stake_params = Self::online_stake_params().ok_or(())?;
        let gpu_num = machine_info.machine_info_detail.committee_upload_info.gpu_num;
        if staked_amount < online_stake_params.online_stake_per_gpu * gpu_num.into() {
            machine_info.machine_status = MachineStatus::WaitingFulfill;
        };
        Ok(())
    }
}
