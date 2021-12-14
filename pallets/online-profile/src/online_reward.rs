use crate::{
    types::{EraIndex, EraStashPoints, MachineGradeStatus, MachineRecentRewardInfo, BLOCK_PER_ERA},
    AllMachineIdSnap, AllMachineIdSnapDetail, BalanceOf, Config, CurrentEra, EraReward, ErasMachinePoints,
    ErasMachineReleasedReward, ErasMachineReward, ErasStashPoints, ErasStashReleasedReward, ErasStashReward,
    MachineRecentReward, Pallet, StashMachines,
};
use codec::Decode;
use generic_func::MachineId;
use online_profile_machine::{DbcPrice, ManageCommittee, OPRPCQuery};
use sp_runtime::{
    traits::{CheckedMul, Saturating, Zero},
    Perbill, SaturatedConversion,
};
use sp_std::{collections::btree_map::BTreeMap, prelude::Vec};

impl<T: Config> Pallet<T> {
    pub fn get_account_from_str(addr: &Vec<u8>) -> Option<T::AccountId> {
        let account_id32: [u8; 32] = crate::utils::get_accountid32(addr)?;
        T::AccountId::decode(&mut &account_id32[..]).ok()
    }

    pub fn update_snap_for_new_era() {
        // current era cannot be calced from block_number, for chain upgrade
        let current_era = Self::current_era() + 1;
        CurrentEra::<T>::put(current_era);

        let era_reward = Self::current_era_reward().unwrap_or_default();
        EraReward::<T>::insert(current_era, era_reward);

        if current_era == 1 {
            ErasStashPoints::<T>::insert(0, EraStashPoints { ..Default::default() });
            ErasStashPoints::<T>::insert(1, EraStashPoints { ..Default::default() });
            ErasStashPoints::<T>::insert(2, EraStashPoints { ..Default::default() });
            let init_value: BTreeMap<MachineId, MachineGradeStatus> = BTreeMap::new();
            ErasMachinePoints::<T>::insert(0, init_value.clone());
            ErasMachinePoints::<T>::insert(1, init_value.clone());
            ErasMachinePoints::<T>::insert(2, init_value);
        } else {
            // 用当前的Era快照初始化下一个Era的信息
            let current_era_stash_snapshot = Self::eras_stash_points(current_era);
            ErasStashPoints::<T>::insert(current_era + 1, current_era_stash_snapshot);
            let current_era_machine_snapshot = Self::eras_machine_points(current_era);
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

    pub fn backup_and_reward(now: T::BlockNumber) {
        let block_offset = now.saturated_into::<u64>() % BLOCK_PER_ERA;

        match block_offset {
            2 => {
                // back up all machine_id; current era machine grade snap; current era stash grade snap
                let mut all_machine = Vec::new();
                let all_stash = Self::get_all_stash();
                for a_stash in &all_stash {
                    let stash_machine = Self::stash_machines(a_stash);
                    all_machine.extend(stash_machine.total_machine);
                }

                let machine_num = all_machine.len() as u64;

                AllMachineIdSnap::<T>::put(AllMachineIdSnapDetail {
                    all_machine_id: all_machine.into(),
                    snap_len: machine_num,
                });
            },
            3..=62 => {
                // distribute reward
                let mut all_machine = Self::all_machine_id_snap();
                let release_num = all_machine.snap_len / 60;

                let release_era = Self::current_era() - 1;
                let era_total_reward = Self::era_reward(release_era);
                let era_machine_points = Self::eras_machine_points(release_era);
                let era_stash_points = Self::eras_stash_points(release_era);

                for _ in 0..=release_num {
                    if let Some(machine_id) = all_machine.all_machine_id.pop_front() {
                        Self::distribute_reward_to_machine(
                            machine_id,
                            release_era,
                            era_total_reward,
                            &era_machine_points,
                            &era_stash_points,
                        );
                    } else {
                        AllMachineIdSnap::<T>::put(all_machine);
                        return
                    }
                }

                AllMachineIdSnap::<T>::put(all_machine);
            },
            _ => return,
        }
    }

    // 计算当时机器实际获得的总奖励 (to_stash + to_committee)
    fn calc_machine_total_reward(
        machine_id: &MachineId,
        machine_stash: &T::AccountId,
        era_total_reward: BalanceOf<T>,
        era_machine_points: &BTreeMap<MachineId, MachineGradeStatus>,
        era_stash_points: &EraStashPoints<T::AccountId>,
    ) -> BalanceOf<T> {
        let machine_points = era_machine_points.get(machine_id);
        let stash_points = era_stash_points.staker_statistic.get(machine_stash);
        let machine_actual_grade = if machine_points.is_none() || stash_points.is_none() {
            Zero::zero()
        } else {
            machine_points.unwrap().machine_actual_grade(stash_points.unwrap().inflation)
        };

        // 该Era机器获得的总奖励 (reward_to_stash + reward_to_committee)
        if era_stash_points.total == 0 {
            Zero::zero()
        } else {
            Perbill::from_rational_approximation(machine_actual_grade, era_stash_points.total) * era_total_reward
        }
    }

    pub fn distribute_reward_to_machine(
        machine_id: MachineId,
        release_era: EraIndex,
        era_total_reward: BalanceOf<T>,
        era_machine_points: &BTreeMap<MachineId, MachineGradeStatus>,
        era_stash_points: &EraStashPoints<T::AccountId>,
    ) {
        let mut machine_reward_info = Self::machine_recent_reward(&machine_id);
        let mut stash_machine = Self::stash_machines(&machine_reward_info.machine_stash);

        let machine_total_reward = Self::calc_machine_total_reward(
            &machine_id,
            &machine_reward_info.machine_stash,
            era_total_reward,
            era_machine_points,
            era_stash_points,
        );

        MachineRecentRewardInfo::add_new_reward(&mut machine_reward_info, machine_total_reward);

        if machine_reward_info.recent_reward_sum == Zero::zero() {
            MachineRecentReward::<T>::insert(&machine_id, machine_reward_info);
            return
        }

        let latest_reward = if machine_reward_info.recent_machine_reward.len() > 0 {
            machine_reward_info.recent_machine_reward[machine_reward_info.recent_machine_reward.len() - 1]
        } else {
            Zero::zero()
        };

        // total released reward = sum(1..n-1) * (1/200) + n * (50/200) = 49/200*n + 1/200 * sum(1..n)
        let released_reward = Perbill::from_rational_approximation(49u32, 200u32) * latest_reward +
            Perbill::from_rational_approximation(1u32, 200u32) * machine_reward_info.recent_reward_sum;

        // if should reward to committee
        let (reward_to_stash, reward_to_committee) = if release_era > machine_reward_info.reward_committee_deadline {
            // only reward stash
            (released_reward, Zero::zero())
        } else {
            // 1% of released_reward to committee, 99% of released reward to stash
            let release_to_stash = Perbill::from_rational_approximation(99u32, 100u32) * released_reward;
            let release_to_committee = released_reward - release_to_stash;
            (release_to_stash, release_to_committee)
        };

        let committee_each_get =
            Perbill::from_rational_approximation(1u32, machine_reward_info.reward_committee.len() as u32) *
                reward_to_committee;
        for a_committee in machine_reward_info.reward_committee.clone() {
            T::ManageCommittee::add_reward(a_committee, committee_each_get);
        }

        // NOTE: reward of actual get will change depend on how much days left
        let machine_actual_total_reward = if release_era > machine_reward_info.reward_committee_deadline {
            machine_total_reward
        } else if release_era > machine_reward_info.reward_committee_deadline - 150 {
            // 减去委员会释放的部分

            // 每天机器奖励释放总奖励的1/200 (150天释放75%)
            let total_daily_release = Perbill::from_rational_approximation(1u32, 200u32) * machine_total_reward;
            // 委员会每天分得释放奖励的1%
            let total_committee_release = Perbill::from_rational_approximation(1u32, 100u32) * total_daily_release;
            // 委员会还能获得奖励的天数
            let release_day = machine_reward_info.reward_committee_deadline - release_era;

            machine_total_reward - total_committee_release * release_day.saturated_into::<BalanceOf<T>>()
        } else {
            Perbill::from_rational_approximation(99u32, 100u32) * machine_total_reward
        };

        // record reward
        stash_machine.can_claim_reward = stash_machine.can_claim_reward.saturating_add(reward_to_stash);
        stash_machine.total_earned_reward =
            stash_machine.total_earned_reward.saturating_add(machine_actual_total_reward);

        ErasMachineReward::<T>::insert(release_era, &machine_id, machine_actual_total_reward);
        ErasStashReward::<T>::mutate(&release_era, &machine_reward_info.machine_stash, |old_value| {
            *old_value += machine_actual_total_reward;
        });

        ErasMachineReleasedReward::<T>::mutate(&release_era, &machine_id, |old_value| *old_value += reward_to_stash);
        ErasStashReleasedReward::<T>::mutate(&release_era, &machine_reward_info.machine_stash, |old_value| {
            *old_value += reward_to_stash
        });

        StashMachines::<T>::insert(&machine_reward_info.machine_stash, stash_machine);
        MachineRecentReward::<T>::insert(&machine_id, machine_reward_info);
    }
}
