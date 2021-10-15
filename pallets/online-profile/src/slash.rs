use crate::{
    types::{MachineStatus, OPPendingSlashInfo, OPSlashReason, MAX_SLASH_THRESHOLD, TWO_DAY},
    BalanceOf, Config, NextSlashId, Pallet, PendingSlash, StashStake, SysInfo,
};
use frame_support::{
    traits::{Currency, ExistenceRequirement::KeepAlive, OnUnbalanced, ReservableCurrency},
    IterableStorageMap,
};
use generic_func::MachineId;
use online_profile_machine::GNOps;
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
        _slash_reason: OPSlashReason<T::BlockNumber>,
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

    pub fn do_pending_slash() {
        let now = <frame_system::Module<T>>::block_number();
        let all_slash_id = <PendingSlash<T> as IterableStorageMap<u64, _>>::iter()
            .map(|(slash_id, _)| slash_id)
            .collect::<Vec<_>>();

        for slash_id in all_slash_id {
            let slash_info = Self::pending_slash(slash_id);
            if now < slash_info.slash_exec_time {
                continue
            }

            match slash_info.slash_reason {
                OPSlashReason::CommitteeRefusedOnline | OPSlashReason::CommitteeRefusedMutHardware => {
                    let _ = Self::slash_and_reward(
                        slash_info.slash_who,
                        slash_info.slash_amount,
                        slash_info.slash_reason,
                        slash_info.reward_to_committee.unwrap_or_default(),
                    );
                },
                _ => {
                    Self::do_slash_deposit(&slash_info);
                },
            }

            PendingSlash::<T>::remove(slash_id);
        }
    }

    // 主动惩罚超过下线阈值的机器
    pub fn check_offline_machine_duration() {
        let live_machine = Self::live_machines();
        let now = <frame_system::Module<T>>::block_number();

        for a_machine in live_machine.offline_machine {
            let machine_info = Self::machines_info(&a_machine);
            match machine_info.machine_status {
                MachineStatus::StakerReportOffline(offline_time, status) => {
                    if now - offline_time < MAX_SLASH_THRESHOLD.into() {
                        continue
                    }

                    match *status {
                        MachineStatus::Online => {
                            Self::add_offline_slash(
                                50,
                                a_machine,
                                machine_info.last_machine_renter,
                                None,
                                OPSlashReason::OnlineReportOffline(offline_time),
                            );
                        },

                        MachineStatus::Rented => {
                            if now - offline_time < (2 * MAX_SLASH_THRESHOLD).into() {
                                continue
                            }
                            Self::add_offline_slash(
                                80,
                                a_machine,
                                None,
                                None,
                                OPSlashReason::RentedReportOffline(offline_time),
                            );
                        },
                        _ => continue,
                    }
                },
                MachineStatus::ReporterReportOffline(offline_reason, _status, _reporter, committee) => {
                    match offline_reason {
                        // 被举报时
                        OPSlashReason::RentedInaccessible(report_time) |
                        OPSlashReason::RentedHardwareCounterfeit(report_time) |
                        OPSlashReason::RentedHardwareMalfunction(report_time) |
                        OPSlashReason::OnlineRentFailed(report_time) => {
                            if now - report_time < MAX_SLASH_THRESHOLD.into() {
                                continue
                            }
                            Self::add_offline_slash(
                                100,
                                a_machine,
                                machine_info.last_machine_renter,
                                Some(committee),
                                offline_reason,
                            );
                        },

                        _ => continue,
                    }
                },
                _ => continue,
            }
        }
    }

    // Return slashed amount when slash is executed
    pub fn slash_when_report_offline(
        machine_id: MachineId,
        slash_reason: OPSlashReason<T::BlockNumber>,
        reporter: Option<T::AccountId>,
        committee: Option<Vec<T::AccountId>>,
    ) -> BalanceOf<T> {
        let machine_info = Self::machines_info(&machine_id);
        let now = <frame_system::Module<T>>::block_number();
        match slash_reason {
            // 算工主动报告被租用的机器，主动下线
            OPSlashReason::RentedReportOffline(duration) => {
                let duration = duration.saturated_into::<u64>();
                match duration {
                    0 => return Zero::zero(),
                    // 下线不超过7分钟
                    1..=14 => {
                        // 扣除2%质押币。100%进入国库。
                        return Self::add_offline_slash(2, machine_id, None, None, slash_reason)
                    },
                    // 不超过48小时
                    15..=5760 => {
                        // 扣除4%质押币。100%进入国库
                        return Self::add_offline_slash(4, machine_id, None, None, slash_reason)
                    },
                    // 不超过120小时
                    5761..=14400 => {
                        // 扣除30%质押币，10%给到用户，90%进入国库
                        return Self::add_offline_slash(
                            30,
                            machine_id,
                            machine_info.last_machine_renter,
                            None,
                            slash_reason,
                        )
                    },
                    // 超过120小时
                    _ => {
                        // 扣除50%押金。10%给到用户，90%进入国库
                        return Self::add_offline_slash(
                            50,
                            machine_id,
                            machine_info.last_machine_renter,
                            None,
                            slash_reason,
                        )
                    },
                }
            },
            // 算工主动报告在线的机器，主动下线
            OPSlashReason::OnlineReportOffline(duration) => {
                // 判断是否已经下线十天，如果是，则不进行惩罚，仅仅下线处理
                // NOTE: 此时，machine_info.last_online_height还未改变
                if now > 28800u32.saturated_into::<T::BlockNumber>() + duration + machine_info.last_online_height {
                    return Zero::zero()
                }
                let duration = duration.saturated_into::<u64>();
                match duration {
                    0 => return Zero::zero(),
                    // 下线不超过7分钟
                    1..=14 => {
                        // 扣除2%质押币，质押币全部进入国库。
                        return Self::add_offline_slash(2, machine_id, None, None, slash_reason)
                    },
                    // 下线不超过48小时
                    15..=5760 => {
                        // 扣除4%质押币，质押币全部进入国库
                        return Self::add_offline_slash(4, machine_id, None, None, slash_reason)
                    },
                    // 不超过240小时
                    5761..=28800 => {
                        // 扣除30%质押币，质押币全部进入国库
                        return Self::add_offline_slash(30, machine_id, None, None, slash_reason)
                    },
                    _ => {
                        // TODO: 如果机器从首次上线时间起超过365天，剩下20%押金可以申请退回。

                        // 扣除80%质押币。质押币全部进入国库。
                        return Self::add_offline_slash(80, machine_id, None, None, slash_reason)
                    },
                }
            },
            // 机器处于租用状态，无法访问，这种情况下，reporter == renter
            OPSlashReason::RentedInaccessible(duration) => {
                let duration = duration.saturated_into::<u64>();
                match duration {
                    0 => return Zero::zero(),
                    // 不超过7分钟
                    1..=14 => {
                        // 扣除4%质押币。10%给验证人，90%进入国库
                        return Self::add_offline_slash(4, machine_id, None, committee, slash_reason)
                    },
                    // 不超过48小时
                    15..=5760 => {
                        // 扣除8%质押币。10%给验证人，90%进入国库
                        return Self::add_offline_slash(8, machine_id, None, committee, slash_reason)
                    },
                    // 不超过120小时
                    5761..=14400 => {
                        // 扣除60%质押币。10%给到用户，20%给到验证人，70%进入国库
                        return Self::add_offline_slash(60, machine_id, reporter, committee, slash_reason)
                    },
                    // 超过120小时
                    _ => {
                        // 扣除100%押金。10%给到用户，20%给到验证人，70%进入国库
                        return Self::add_offline_slash(100, machine_id, reporter, committee, slash_reason)
                    },
                }
            },
            // 机器处于租用状态，机器出现故障
            OPSlashReason::RentedHardwareMalfunction(duration) => {
                let duration = duration.saturated_into::<u64>();
                match duration {
                    0 => return Zero::zero(),
                    //不超过4小时
                    1..=480 => {
                        // 扣除6%质押币。10%给到用户，20%给到验证人，70%进入国库
                        return Self::add_offline_slash(6, machine_id, reporter, committee, slash_reason)
                    },
                    // 不超过24小时
                    481..=2880 => {
                        // 扣除12%质押币。10%给到用户，20%给到验证人，70%进入国库
                        return Self::add_offline_slash(12, machine_id, reporter, committee, slash_reason)
                    },
                    // 不超过48小时
                    2881..=5760 => {
                        // 扣除16%质押币。10%给到用户，20%给到验证人，70%进入国库
                        return Self::add_offline_slash(16, machine_id, reporter, committee, slash_reason)
                    },
                    // 不超过120小时
                    5761..=14400 => {
                        // 扣除60%质押币。10%给到用户，20%给到验证人，70%进入国库
                        return Self::add_offline_slash(60, machine_id, reporter, committee, slash_reason)
                    },
                    _ => {
                        // 扣除100%押金，10%给到用户，20%给到验证人，70%进入国库
                        return Self::add_offline_slash(100, machine_id, reporter, committee, slash_reason)
                    },
                }
            },
            // 机器处于租用状态，机器硬件造假
            OPSlashReason::RentedHardwareCounterfeit(duration) => {
                let duration = duration.saturated_into::<u64>();
                match duration {
                    0 => return Zero::zero(),
                    // 下线不超过4小时
                    1..=480 => {
                        // 扣除12%质押币。10%给到用户，20%给到验证人，70%进入国库
                        return Self::add_offline_slash(12, machine_id, reporter, committee, slash_reason)
                    },
                    // 不超过24小时
                    481..=2880 => {
                        // 扣除24%质押币。10%给到用户，20%给到验证人，70%进入国库
                        return Self::add_offline_slash(24, machine_id, reporter, committee, slash_reason)
                    },
                    // 不超过48小时
                    2881..=5760 => {
                        // 扣除32%质押币。10%给到用户，20%给到验证人，70%进入国库
                        return Self::add_offline_slash(32, machine_id, reporter, committee, slash_reason)
                    },
                    // 不超过120小时
                    5761..=14400 => {
                        // 扣除60%质押币。10%给到用户，20%给到验证人，70%进入国库
                        return Self::add_offline_slash(60, machine_id, reporter, committee, slash_reason)
                    },
                    _ => {
                        // 扣除100%押金，10%给到用户，20%给到验证人，70%进入国库
                        return Self::add_offline_slash(100, machine_id, reporter, committee, slash_reason)
                    },
                }
            },
            // 机器在线，被举报无法租用
            OPSlashReason::OnlineRentFailed(duration) => {
                let duration = duration.saturated_into::<u64>();
                match duration {
                    0 => return Zero::zero(),
                    1..=480 => {
                        // 扣除6%质押币。10%给到用户，20%给到验证人，50%进入国库
                        return Self::add_offline_slash(6, machine_id, reporter, committee, slash_reason)
                    },
                    481..=2880 => {
                        // 扣除12%质押币。10%给到用户，20%给到验证人，70%进入国库
                        return Self::add_offline_slash(12, machine_id, reporter, committee, slash_reason)
                    },
                    2881..=5760 => {
                        // 扣除16%质押币。10%给到用户，20%给到验证人，70%进入国库
                        return Self::add_offline_slash(16, machine_id, reporter, committee, slash_reason)
                    },
                    5761..=14400 => {
                        // 扣除60%质押币。10%给到用户，20%给到验证人，70%进入国库
                        return Self::add_offline_slash(60, machine_id, reporter, committee, slash_reason)
                    },
                    _ => {
                        // 扣除100%押金，10%给到用户，20%给到验证人，70%进入国库
                        return Self::add_offline_slash(100, machine_id, reporter, committee, slash_reason)
                    },
                }
            },
            _ => return Zero::zero(),
        }
    }

    pub fn add_offline_slash(
        slash_percent: u32,
        machine_id: MachineId,
        reporter: Option<T::AccountId>,
        committee: Option<Vec<T::AccountId>>,
        slash_reason: OPSlashReason<T::BlockNumber>,
    ) -> BalanceOf<T> {
        let now = <frame_system::Module<T>>::block_number();
        let machine_info = Self::machines_info(&machine_id);
        let slash_amount = Perbill::from_rational_approximation(slash_percent, 100) * machine_info.stake_amount;

        let slash_id = Self::get_new_slash_id();
        let slash_info = OPPendingSlashInfo {
            slash_who: machine_info.machine_stash,
            machine_id,
            slash_time: now,
            slash_amount,
            slash_exec_time: now + TWO_DAY.into(),
            reward_to_reporter: reporter,
            reward_to_committee: committee,
            slash_reason,
        };

        PendingSlash::<T>::insert(slash_id, slash_info);
        slash_amount
    }

    // FIXME: refa, and should not do transfer but unreserve
    // 惩罚掉机器押金，如果执行惩罚后机器押金不够，则状态变为补充质押
    pub fn do_slash_deposit(slash_info: &OPPendingSlashInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>) {
        let machine_info = Self::machines_info(&slash_info.machine_id);

        // FIXME: 检查分配比例;执行惩罚
        // 根据比例，分配slash_amoun
        let (slash_to_treasury, reward_to_reporter, reward_to_committee) = {
            let percent_10 = Perbill::from_rational_approximation(10u32, 100u32);
            let percent_20 = Perbill::from_rational_approximation(20u32, 100u32);

            if slash_info.reward_to_reporter.is_some() && slash_info.reward_to_committee.is_none() {
                let reward_to_reporter = percent_10 * slash_info.slash_amount;
                let slash_to_treasury = slash_info.slash_amount - reward_to_reporter;
                (slash_to_treasury, reward_to_reporter, Zero::zero())
            } else if slash_info.reward_to_reporter.is_some() && slash_info.reward_to_committee.is_some() {
                let reward_to_reporter = percent_10 * slash_info.slash_amount;
                let reward_to_committee = percent_20 * slash_info.slash_amount;
                let slash_to_treasury = slash_info.slash_amount - reward_to_reporter - reward_to_committee;
                (slash_to_treasury, reward_to_reporter, reward_to_committee)
            } else {
                (slash_info.slash_amount, Zero::zero(), Zero::zero())
            }
        };

        // 奖励给委员会的立即给委员会
        if !reward_to_reporter.is_zero() && slash_info.reward_to_reporter.is_some() {
            let _ = <T as Config>::Currency::transfer(
                &machine_info.machine_stash,
                &slash_info.reward_to_reporter.as_ref().unwrap(),
                reward_to_reporter,
                KeepAlive,
            );
        }

        // 奖励给报告人的立即给报告人
        if !reward_to_committee.is_zero() && slash_info.reward_to_committee.is_some() {
            let committees = slash_info.reward_to_committee.as_ref().unwrap();
            let reward_each_committee_get =
                Perbill::from_rational_approximation(1u32, committees.len() as u32) * reward_to_committee;
            for a_committee in committees {
                let _ = <T as Config>::Currency::transfer(
                    &machine_info.machine_stash,
                    &a_committee,
                    reward_each_committee_get,
                    KeepAlive,
                );
            }
        }

        // 执行惩罚
        if <T as Config>::Currency::reserved_balance(&machine_info.machine_stash) >= slash_to_treasury {
            let (imbalance, _missing) =
                <T as Config>::Currency::slash_reserved(&machine_info.machine_stash, slash_to_treasury);
            // Self::deposit_event(Event::Slash(machine_info.machine_stash.clone(), slash_amount, SlashReason::));
            <T as Config>::Slash::on_unbalanced(imbalance);
        }
    }
}
