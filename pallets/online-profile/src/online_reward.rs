use crate::{
    types::{EraIndex, EraStashPoints, MachineGradeStatus, StashMachine, BLOCK_PER_ERA},
    BalanceOf, Config, CurrentEra, EraReward, ErasMachinePoints, ErasMachineReleasedReward, ErasMachineReward,
    ErasStashPoints, ErasStashReleasedReward, ErasStashReward, Pallet, StashMachines,
};
use generic_func::MachineId;
use online_profile_machine::{DbcPrice, ManageCommittee, OPRPCQuery};
use sp_runtime::{
    traits::{CheckedAdd, CheckedMul, CheckedSub},
    Perbill, SaturatedConversion,
};
use sp_std::collections::btree_map::BTreeMap;

impl<T: Config> Pallet<T> {
    pub fn update_snap_for_new_era(block_number: T::BlockNumber) {
        let current_era: u32 = (block_number.saturated_into::<u64>() / BLOCK_PER_ERA) as u32;
        CurrentEra::<T>::put(current_era);

        let era_reward = Self::current_era_reward().unwrap_or_default();
        EraReward::<T>::insert(current_era, era_reward);

        if current_era == 0 {
            ErasStashPoints::<T>::insert(0, EraStashPoints { ..Default::default() });
            ErasStashPoints::<T>::insert(1, EraStashPoints { ..Default::default() });
            let init_value: BTreeMap<MachineId, MachineGradeStatus> = BTreeMap::new();
            ErasMachinePoints::<T>::insert(0, init_value.clone());
            ErasMachinePoints::<T>::insert(1, init_value);
        } else {
            // 用当前的Era快照初始化下一个Era的信息
            let current_era_stash_snapshot = Self::eras_stash_points(current_era).unwrap_or_default();
            ErasStashPoints::<T>::insert(current_era + 1, current_era_stash_snapshot);
            let current_era_machine_snapshot = Self::eras_machine_points(current_era).unwrap_or_default();
            ErasMachinePoints::<T>::insert(current_era + 1, current_era_machine_snapshot);
        }
    }

    // 质押DBC机制：[0, 10000] GPU: 100000 DBC per GPU
    // (10000, +) -> min( 100000 * 10000 / (10000 + n), 5w RMB DBC
    pub fn stake_per_gpu() -> Option<BalanceOf<T>> {
        let sys_info = Self::sys_info();
        let online_stake_params = Self::online_stake_params()?;

        let dbc_stake_per_gpu = if sys_info.total_gpu_num > 10_000 {
            Perbill::from_rational_approximation(10_000u64, sys_info.total_gpu_num) *
                online_stake_params.online_stake_per_gpu
        } else {
            online_stake_params.online_stake_per_gpu
        };

        let stake_limit = T::DbcPrice::get_dbc_amount_by_value(online_stake_params.online_stake_usd_limit)?;
        Some(dbc_stake_per_gpu.min(stake_limit)) // .checked_mul(&gpu_num.saturated_into::<BalanceOf<T>>())
    }

    /// 计算当前Era在线奖励数量
    pub fn current_era_reward() -> Option<BalanceOf<T>> {
        let current_era = Self::current_era() as u64;
        let phase_reward_info = Self::phase_reward_info()?;

        let reward_start_era = phase_reward_info.online_reward_start_era as u64;
        let era_duration = (current_era >= reward_start_era).then(|| current_era - reward_start_era)?;

        let era_reward = if era_duration < phase_reward_info.first_phase_duration as u64 {
            phase_reward_info.phase_0_reward_per_era
        } else if era_duration < phase_reward_info.first_phase_duration as u64 + 1825 {
            // 365 * 5
            phase_reward_info.phase_1_reward_per_era
        } else {
            phase_reward_info.phase_2_reward_per_era
        };

        if Self::galaxy_is_on() && current_era < phase_reward_info.galaxy_on_era as u64 + 60 {
            Some(era_reward.checked_mul(&2u32.saturated_into::<BalanceOf<T>>())?)
        } else {
            Some(era_reward)
        }
    }

    // 根据机器得分快照，和委员会膨胀分数，计算应该奖励
    // end_era分发奖励
    pub fn distribute_reward() {
        let current_era = Self::current_era();
        let start_era = if current_era > 150 { current_era - 150 } else { 0u32 };
        let all_stash = Self::get_all_stash();

        for era_index in start_era..=current_era {
            let era_reward = Self::era_reward(era_index);
            let era_machine_points = Self::eras_machine_points(era_index).unwrap_or_default();
            let era_stash_points = Self::eras_stash_points(era_index).unwrap_or_default();

            for a_stash in &all_stash {
                let mut stash_machine = Self::stash_machines(a_stash);

                for machine_id in stash_machine.total_machine.clone() {
                    let _ = Self::distribute_a_machine(
                        machine_id,
                        a_stash,
                        era_reward,
                        &era_stash_points,
                        &era_machine_points,
                        current_era,
                        era_index,
                        &mut stash_machine,
                    );
                }
                StashMachines::<T>::insert(a_stash, stash_machine);
            }
        }
    }

    pub fn distribute_a_machine(
        machine_id: MachineId,
        a_stash: &T::AccountId,
        era_reward: BalanceOf<T>,
        era_stash_points: &EraStashPoints<T::AccountId>,
        era_machine_points: &BTreeMap<MachineId, MachineGradeStatus>,
        current_era: EraIndex,
        era_index: EraIndex,
        stash_machine: &mut StashMachine<BalanceOf<T>>,
    ) -> Result<(), ()> {
        let machine_info = Self::machines_info(&machine_id);
        let will_reward_committee = machine_info.reward_deadline >= current_era;
        let is_head_mining = era_index == current_era;

        // 计算当时机器实际获得的奖励
        let machine_points = era_machine_points.get(&machine_id).ok_or(())?;
        let stash_points = era_stash_points.staker_statistic.get(&a_stash).ok_or(())?;
        let machine_actual_grade = machine_points.machine_actual_grade(stash_points.inflation);

        // 该Era机器获得的总奖励
        let machine_total_reward =
            Perbill::from_rational_approximation(machine_actual_grade, era_stash_points.total) * era_reward;
        let linear_reward_part = Perbill::from_rational_approximation(75u32, 100u32) * machine_total_reward;

        // 根据是否是第一次释放奖励，计算era_index时奖励数量: 第一次释放25%, 否则释放剩余75%的1/150
        let release_now = if is_head_mining {
            machine_total_reward.checked_sub(&linear_reward_part).ok_or(())?
        } else {
            Perbill::from_rational_approximation(1u32, 150u32) * linear_reward_part
        };

        // 如果奖励不发给委员会
        if !will_reward_committee {
            if is_head_mining {
                // 更新一些数据
                stash_machine.total_earned_reward =
                    stash_machine.total_earned_reward.checked_add(&machine_total_reward).ok_or(())?;
                ErasMachineReward::<T>::insert(current_era, &machine_id, machine_total_reward);
                ErasStashReward::<T>::mutate(&current_era, &a_stash, |old_value| *old_value += machine_total_reward);
            }
            // 发放奖励
            // 没有委员会来分，则全部奖励给stash账户
            stash_machine.can_claim_reward = stash_machine.can_claim_reward.checked_add(&release_now).ok_or(())?;
            ErasMachineReleasedReward::<T>::mutate(&current_era, &machine_id, |old_value| *old_value += release_now);
            ErasStashReleasedReward::<T>::mutate(&current_era, &machine_info.machine_stash, |old_value| {
                *old_value += release_now
            });
        } else {
            // 头矿时更新记录
            if is_head_mining {
                // 如果委员的奖励时间会很快就要结束了
                // 则奖励的前一部分给委员会一部分，后一部分，不给委员会
                if machine_info.reward_deadline >= current_era + 150 {
                    let reward_to_stash = Perbill::from_rational_approximation(99u64, 100u64) * machine_total_reward;
                    stash_machine.total_earned_reward =
                        stash_machine.total_earned_reward.checked_add(&reward_to_stash).ok_or(())?;
                    ErasMachineReward::<T>::insert(current_era, &machine_id, reward_to_stash);
                    ErasStashReward::<T>::mutate(&current_era, &machine_info.machine_stash, |old_value| {
                        *old_value += reward_to_stash;
                    });
                } else {
                    // 计算委员会奖励结束后，机器拥有者单独获得的奖励
                    let only_reward_to_stash_duration = current_era + 150 - machine_info.reward_deadline;
                    let reward_to_stash2 =
                        Perbill::from_rational_approximation(only_reward_to_stash_duration, 150) * machine_total_reward;

                    // 计算委员会奖励结束前，机器拥有者单独获得的奖励
                    let reward_to_both = machine_total_reward - reward_to_stash2;
                    let reward_to_stash1 = Perbill::from_rational_approximation(99u32, 100u32) * reward_to_both;

                    let stash_all_get = reward_to_stash1 + reward_to_stash2;
                    stash_machine.total_earned_reward =
                        stash_machine.total_earned_reward.checked_add(&stash_all_get).ok_or(())?;

                    ErasMachineReward::<T>::insert(current_era, &machine_id, stash_all_get);
                    ErasStashReward::<T>::mutate(&current_era, &machine_info.machine_stash, |old_value| {
                        *old_value += stash_all_get
                    });
                }
            }

            // 99% 分给stash账户
            let release_to_stash = Perbill::from_rational_approximation(99u64, 100u64) * release_now;
            stash_machine.can_claim_reward += release_to_stash;

            ErasMachineReleasedReward::<T>::mutate(&current_era, &machine_id, |old_value| {
                *old_value += release_to_stash
            });
            ErasStashReleasedReward::<T>::mutate(&current_era, &machine_info.machine_stash, |old_value| {
                *old_value += release_to_stash
            });

            // 剩下分给committee
            let release_to_committee = release_now - release_to_stash;
            let committee_each_get =
                Perbill::from_rational_approximation(1u64, machine_info.reward_committee.len() as u64) *
                    release_to_committee;
            for a_committee in machine_info.reward_committee.clone() {
                T::ManageCommittee::add_reward(a_committee, committee_each_get);
            }
        }

        Ok(())
    }
}
