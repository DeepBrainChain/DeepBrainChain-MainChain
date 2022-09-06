use crate::{
    types::{MachineStatus, OPPendingSlashInfo, OPSlashReason, TWO_DAY},
    BalanceOf, Config, Event, NextSlashId, Pallet, PendingExecMaxOfflineSlash, PendingExecSlash, PendingSlash,
    PendingSlashReview, PendingSlashReviewChecking, StashStake, SysInfo,
};
use dbc_support::traits::GNOps;
use frame_support::traits::ReservableCurrency;
use generic_func::{ItemList, MachineId};
use sp_runtime::{
    traits::{CheckedSub, Zero},
    Perbill, SaturatedConversion,
};
use sp_std::{vec, vec::Vec};

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
        let mut stash_stake = Self::stash_stake(&slash_who);
        let mut sys_info = Self::sys_info();

        sys_info.total_stake = sys_info.total_stake.checked_sub(&slash_amount).ok_or(())?;
        stash_stake = stash_stake.checked_sub(&slash_amount).ok_or(())?;

        let _ = T::SlashAndReward::slash_and_reward(vec![slash_who.clone()], slash_amount, reward_to);

        StashStake::<T>::insert(&slash_who, stash_stake);
        SysInfo::<T>::put(sys_info);
        Ok(())
    }

    // NOTE: 确保 PendingSlash 添加时，添加该变量
    // PendingSlash 删除时，删除该变量
    pub fn exec_pending_slash() {
        let now = <frame_system::Module<T>>::block_number();
        let pending_exec_slash = Self::pending_exec_slash(now);

        for slash_id in pending_exec_slash {
            let slash_info = Self::pending_slash(slash_id);

            match slash_info.slash_reason {
                OPSlashReason::CommitteeRefusedOnline | OPSlashReason::CommitteeRefusedMutHardware => {
                    let _ = Self::slash_and_reward(
                        slash_info.slash_who.clone(),
                        slash_info.slash_amount,
                        slash_info.reward_to_committee.unwrap_or_default(),
                    );
                },
                _ => {
                    Self::do_slash_deposit(&slash_info);
                },
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
        let now = <frame_system::Module<T>>::block_number();
        if !PendingSlashReviewChecking::<T>::contains_key(now) {
            return Ok(());
        }

        let pending_slash_checking = Self::pending_slash_review_checking(now);
        for slash_id in pending_slash_checking {
            let slash_apply_review_info = Self::pending_slash_review(slash_id);
            let stash = Self::controller_stash(slash_apply_review_info.applicant).ok_or(())?;

            Self::slash_and_reward(stash, slash_apply_review_info.staked_amount, vec![])?;
            PendingSlashReview::<T>::remove(slash_id);
        }
        PendingSlashReviewChecking::<T>::remove(now);
        Ok(())
    }

    pub fn check_offline_machine_duration() {
        let now = <frame_system::Module<T>>::block_number();
        let pending_exec_slash = Self::get_pending_max_slash(now);

        // 主动下线的机器
        for (machine_id, reward_to) in pending_exec_slash {
            let machine_info = Self::machines_info(&machine_id);
            let slash_info: OPPendingSlashInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>;

            match machine_info.machine_status {
                // 主动报告下线
                MachineStatus::StakerReportOffline(offline_time, status) => match *status {
                    // 如果下线前是Online状态，且空闲（为Online状态）超过10天，则不进行惩罚
                    MachineStatus::Online => {
                        // 下线达到10天，达到最大惩罚，则添加惩罚
                        slash_info = Self::new_offline_slash(
                            80,
                            machine_id.clone(),
                            reward_to.0,
                            reward_to.1,
                            None,
                            OPSlashReason::OnlineReportOffline(offline_time),
                        );
                    },
                    MachineStatus::Rented => {
                        // 租用时主动下线，最多5天达到惩罚最大
                        slash_info = Self::new_offline_slash(
                            50,
                            machine_id.clone(),
                            reward_to.0,
                            reward_to.1,
                            None,
                            OPSlashReason::RentedReportOffline(offline_time),
                        );
                    },
                    _ => continue,
                },
                MachineStatus::ReporterReportOffline(offline_reason, _status, _reporter, committee) => {
                    // 被举报时，超过5天达到惩罚最大
                    match offline_reason {
                        OPSlashReason::RentedInaccessible(_)
                        | OPSlashReason::RentedHardwareCounterfeit(_)
                        | OPSlashReason::RentedHardwareMalfunction(_)
                        | OPSlashReason::OnlineRentFailed(_) => {
                            slash_info = Self::new_offline_slash(
                                100,
                                machine_id.clone(),
                                reward_to.0,
                                reward_to.1,
                                Some(committee),
                                offline_reason,
                            );
                        },
                        _ => continue,
                    }
                },
                _ => continue,
            }
            // 插入一个新的SlashId
            if slash_info.slash_amount != Zero::zero() {
                let slash_id = Self::get_new_slash_id();

                let mut pending_exec_slash = Self::pending_exec_slash(slash_info.slash_exec_time);
                ItemList::add_item(&mut pending_exec_slash, slash_id);
                PendingExecSlash::<T>::insert(slash_info.slash_exec_time, pending_exec_slash);

                PendingSlash::<T>::insert(slash_id, slash_info);

                Self::deposit_event(Event::NewSlash(slash_id));
            }

            PendingExecMaxOfflineSlash::<T>::remove(now, machine_id);
        }
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
    ) -> OPPendingSlashInfo<T::AccountId, T::BlockNumber, BalanceOf<T>> {
        match slash_reason {
            // 算工主动报告被租用的机器，主动下线
            OPSlashReason::RentedReportOffline(_) => {
                Self::new_slash_rented_report_offline(machine_id, reporter, renters, duration, slash_reason)
            },
            // 算工主动报告在线的机器，主动下线
            OPSlashReason::OnlineReportOffline(_) => {
                Self::new_slash_online_report_offline(machine_id, duration, slash_reason)
            },
            // 机器处于租用状态，无法访问，这种情况下，reporter == renter
            OPSlashReason::RentedInaccessible(_) => {
                Self::new_slash_rented_inaccessible(machine_id, duration, slash_reason, reporter, renters, committee)
            },
            // 机器处于租用状态，机器出现故障
            OPSlashReason::RentedHardwareMalfunction(_) => Self::new_slash_rented_hardware_mulfunction(
                machine_id,
                duration,
                slash_reason,
                reporter,
                renters,
                committee,
            ),
            // 机器处于租用状态，机器硬件造假
            OPSlashReason::RentedHardwareCounterfeit(_) => Self::new_slash_rented_hardware_counterfeit(
                machine_id,
                duration,
                slash_reason,
                reporter,
                renters,
                committee,
            ),
            // 机器在线，被举报无法租用
            OPSlashReason::OnlineRentFailed(_) => {
                Self::new_slash_online_rent_failed(machine_id, duration, slash_reason, reporter, renters, committee)
            },
            _ => OPPendingSlashInfo::default(),
        }
    }

    fn new_slash_rented_report_offline(
        machine_id: MachineId,
        reporter: Option<T::AccountId>,
        renters: Vec<T::AccountId>,
        duration: T::BlockNumber,
        slash_reason: OPSlashReason<T::BlockNumber>,
    ) -> OPPendingSlashInfo<T::AccountId, T::BlockNumber, BalanceOf<T>> {
        let duration = duration.saturated_into::<u64>();
        match duration {
            0 => OPPendingSlashInfo::default(),
            // 下线不超过7分钟, 扣除2%质押币。100%进入国库。
            1..=14 => Self::new_offline_slash(2, machine_id, None, renters, None, slash_reason),
            // 不超过48小时, 扣除4%质押币。100%进入国库
            15..=5760 => Self::new_offline_slash(4, machine_id, None, renters, None, slash_reason),
            // 不超过120小时, 扣除30%质押币，10%给到用户，90%进入国库
            5761..=14400 => Self::new_offline_slash(30, machine_id, reporter, renters, None, slash_reason),
            // 超过120小时, 扣除50%押金。10%给到用户，90%进入国库
            _ => Self::new_offline_slash(50, machine_id, reporter, renters, None, slash_reason),
        }
    }

    fn new_slash_online_report_offline(
        machine_id: MachineId,
        duration: T::BlockNumber,
        slash_reason: OPSlashReason<T::BlockNumber>,
    ) -> OPPendingSlashInfo<T::AccountId, T::BlockNumber, BalanceOf<T>> {
        let now = <frame_system::Module<T>>::block_number();
        let machine_info = Self::machines_info(&machine_id);

        // 判断是否已经下线十天，如果是，则不进行惩罚，仅仅下线处理
        // NOTE: 此时，machine_info.last_online_height还未改变
        if now > 28800u32.saturated_into::<T::BlockNumber>() + duration + machine_info.last_online_height {
            return OPPendingSlashInfo::default();
        }
        let duration = duration.saturated_into::<u64>();
        match duration {
            0 => OPPendingSlashInfo::default(),
            // 下线不超过7分钟, 扣除2%质押币，质押币全部进入国库。
            1..=14 => Self::new_offline_slash(2, machine_id, None, vec![], None, slash_reason),
            // 下线不超过48小时, 扣除4%质押币，质押币全部进入国库
            15..=5760 => Self::new_offline_slash(4, machine_id, None, vec![], None, slash_reason),
            // 不超过240小时, 扣除30%质押币，质押币全部进入国库
            5761..=28800 => Self::new_offline_slash(30, machine_id, None, vec![], None, slash_reason),
            // TODO: 如果机器从首次上线时间起超过365天，剩下20%押金可以申请退回。
            // 扣除80%质押币。质押币全部进入国库。
            _ => Self::new_offline_slash(80, machine_id, None, vec![], None, slash_reason),
        }
    }

    fn new_slash_rented_inaccessible(
        machine_id: MachineId,
        duration: T::BlockNumber,
        slash_reason: OPSlashReason<T::BlockNumber>,
        reporter: Option<T::AccountId>,
        renters: Vec<T::AccountId>,
        committee: Option<Vec<T::AccountId>>,
    ) -> OPPendingSlashInfo<T::AccountId, T::BlockNumber, BalanceOf<T>> {
        let duration = duration.saturated_into::<u64>();
        match duration {
            0 => OPPendingSlashInfo::default(),
            // 不超过7分钟, 扣除4%质押币。10%给验证人，90%进入国库
            1..=14 => Self::new_offline_slash(4, machine_id, None, renters, committee, slash_reason),
            // 不超过48小时, 扣除8%质押币。10%给验证人，90%进入国库
            15..=5760 => Self::new_offline_slash(8, machine_id, None, renters, committee, slash_reason),
            // 不超过120小时, 扣除60%质押币。10%给到用户，20%给到验证人，70%进入国库
            5761..=14400 => Self::new_offline_slash(60, machine_id, reporter, renters, committee, slash_reason),
            // 超过120小时, 扣除100%押金。10%给到用户，20%给到验证人，70%进入国库
            _ => Self::new_offline_slash(100, machine_id, reporter, renters, committee, slash_reason),
        }
    }

    fn new_slash_rented_hardware_mulfunction(
        machine_id: MachineId,
        duration: T::BlockNumber,
        slash_reason: OPSlashReason<T::BlockNumber>,
        reporter: Option<T::AccountId>,
        renters: Vec<T::AccountId>,
        committee: Option<Vec<T::AccountId>>,
    ) -> OPPendingSlashInfo<T::AccountId, T::BlockNumber, BalanceOf<T>> {
        let duration = duration.saturated_into::<u64>();
        match duration {
            0 => OPPendingSlashInfo::default(),
            //不超过4小时, 扣除6%质押币。10%给到用户，20%给到验证人，70%进入国库
            1..=480 => Self::new_offline_slash(6, machine_id, reporter, renters, committee, slash_reason),
            // 不超过24小时, 扣除12%质押币。10%给到用户，20%给到验证人，70%进入国库
            481..=2880 => Self::new_offline_slash(12, machine_id, reporter, renters, committee, slash_reason),
            // 不超过48小时, 扣除16%质押币。10%给到用户，20%给到验证人，70%进入国库
            2881..=5760 => Self::new_offline_slash(16, machine_id, reporter, renters, committee, slash_reason),
            // 不超过120小时, 扣除60%质押币。10%给到用户，20%给到验证人，70%进入国库
            5761..=14400 => Self::new_offline_slash(60, machine_id, reporter, renters, committee, slash_reason),
            // 扣除100%押金，10%给到用户，20%给到验证人，70%进入国库
            _ => Self::new_offline_slash(100, machine_id, reporter, renters, committee, slash_reason),
        }
    }

    fn new_slash_rented_hardware_counterfeit(
        machine_id: MachineId,
        duration: T::BlockNumber,
        slash_reason: OPSlashReason<T::BlockNumber>,
        reporter: Option<T::AccountId>,
        renters: Vec<T::AccountId>,
        committee: Option<Vec<T::AccountId>>,
    ) -> OPPendingSlashInfo<T::AccountId, T::BlockNumber, BalanceOf<T>> {
        let duration = duration.saturated_into::<u64>();
        match duration {
            0 => OPPendingSlashInfo::default(),
            // 下线不超过4小时, 扣除12%质押币。10%给到用户，20%给到验证人，70%进入国库
            1..=480 => Self::new_offline_slash(12, machine_id, reporter, renters, committee, slash_reason),
            // 不超过24小时, 扣除24%质押币。10%给到用户，20%给到验证人，70%进入国库
            481..=2880 => Self::new_offline_slash(24, machine_id, reporter, renters, committee, slash_reason),
            // 不超过48小时, 扣除32%质押币。10%给到用户，20%给到验证人，70%进入国库
            2881..=5760 => Self::new_offline_slash(32, machine_id, reporter, renters, committee, slash_reason),
            // 不超过120小时, 扣除60%质押币。10%给到用户，20%给到验证人，70%进入国库
            5761..=14400 => Self::new_offline_slash(60, machine_id, reporter, renters, committee, slash_reason),
            // 扣除100%押金，10%给到用户，20%给到验证人，70%进入国库
            _ => Self::new_offline_slash(100, machine_id, reporter, renters, committee, slash_reason),
        }
    }

    fn new_slash_online_rent_failed(
        machine_id: MachineId,
        duration: T::BlockNumber,
        slash_reason: OPSlashReason<T::BlockNumber>,
        reporter: Option<T::AccountId>,
        renters: Vec<T::AccountId>,
        committee: Option<Vec<T::AccountId>>,
    ) -> OPPendingSlashInfo<T::AccountId, T::BlockNumber, BalanceOf<T>> {
        let duration = duration.saturated_into::<u64>();
        match duration {
            0 => OPPendingSlashInfo::default(),
            // 扣除6%质押币。10%给到用户，20%给到验证人，70%进入国库
            1..=480 => Self::new_offline_slash(6, machine_id, reporter, renters, committee, slash_reason),
            // 扣除12%质押币。10%给到用户，20%给到验证人，70%进入国库
            481..=2880 => Self::new_offline_slash(12, machine_id, reporter, renters, committee, slash_reason),
            // 扣除16%质押币。10%给到用户，20%给到验证人，70%进入国库
            2881..=5760 => Self::new_offline_slash(16, machine_id, reporter, renters, committee, slash_reason),
            // 扣除60%质押币。10%给到用户，20%给到验证人，70%进入国库
            5761..=14400 => Self::new_offline_slash(60, machine_id, reporter, renters, committee, slash_reason),
            // 扣除100%押金，10%给到用户，20%给到验证人，70%进入国库
            _ => Self::new_offline_slash(100, machine_id, reporter, renters, committee, slash_reason),
        }
    }

    pub fn new_offline_slash(
        slash_percent: u32,
        machine_id: MachineId,
        reporter: Option<T::AccountId>,
        renters: Vec<T::AccountId>,
        committee: Option<Vec<T::AccountId>>,
        slash_reason: OPSlashReason<T::BlockNumber>,
    ) -> OPPendingSlashInfo<T::AccountId, T::BlockNumber, BalanceOf<T>> {
        let now = <frame_system::Module<T>>::block_number();
        let machine_info = Self::machines_info(&machine_id);
        let slash_amount = Perbill::from_rational_approximation(slash_percent, 100) * machine_info.stake_amount;

        OPPendingSlashInfo {
            slash_who: machine_info.machine_stash,
            machine_id,
            slash_time: now,
            slash_amount,
            slash_exec_time: now + TWO_DAY.into(),
            reporter,
            renters,
            reward_to_committee: committee,
            slash_reason,
        }
    }

    // 惩罚掉机器押金，如果执行惩罚后机器押金不够，则状态变为补充质押
    pub fn do_slash_deposit(slash_info: &OPPendingSlashInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>) {
        let machine_info = Self::machines_info(&slash_info.machine_id);

        let mut reward_to_reporter = Zero::zero();
        let mut reward_to_committee = Zero::zero();

        if !slash_info.renters.is_empty() {
            reward_to_reporter = Perbill::from_rational_approximation(10u32, 100u32) * slash_info.slash_amount;
        }
        if slash_info.reward_to_committee.is_some() {
            reward_to_committee = Perbill::from_rational_approximation(20u32, 100u32) * slash_info.slash_amount;
        }
        let slash_to_treasury = slash_info.slash_amount - reward_to_reporter - reward_to_committee;

        if <T as Config>::Currency::reserved_balance(&machine_info.machine_stash) < slash_info.slash_amount {
            return;
        }

        // FIXME: 奖励分配给slasl_info.committee
        // reward to reporter:
        if reward_to_reporter > Zero::zero() && !slash_info.renters.is_empty() {
            let _ =
                Self::slash_and_reward(slash_info.slash_who.clone(), reward_to_reporter, slash_info.renters.clone());
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
    }
}
