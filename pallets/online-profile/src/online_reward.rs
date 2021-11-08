use crate::{
    types::{EraIndex, EraStashPoints, MachineGradeStatus, MachineRecentRewardInfo, BLOCK_PER_ERA},
    AllMachineIdSnap, BackupMachineGradeSnap, BackupStashGradeSnap, BalanceOf, Config, CurrentEra, EraReward,
    ErasMachinePoints, ErasMachineReleasedReward, ErasMachineReward, ErasStashPoints, ErasStashReleasedReward,
    ErasStashReward, MachineRecentReward, Pallet, StashMachines,
};
use codec::Decode;
use generic_func::MachineId;
use online_profile_machine::{DbcPrice, ManageCommittee, OPRPCQuery};
use sp_runtime::{
    traits::{CheckedMul, Zero},
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
        let current_era = Self::current_era();
        CurrentEra::<T>::put(current_era + 1);

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
            2819 => {
                // back up all machine_id; current era machine grade snap; current era stash grade snap
                let mut all_machine = Vec::new();
                let all_stash = Self::get_all_stash();
                for a_stash in &all_stash {
                    let stash_machine = Self::stash_machines(a_stash);
                    all_machine.extend(stash_machine.total_machine);
                }

                let current_era = Self::current_era();
                let current_era_stash_snapshot = Self::eras_stash_points(current_era);
                let current_era_machine_snapshot = Self::eras_machine_points(current_era);

                let machine_num = all_machine.len() as u64;

                AllMachineIdSnap::<T>::put((all_machine, machine_num));
                BackupMachineGradeSnap::<T>::put(current_era_machine_snapshot);
                BackupStashGradeSnap::<T>::put(current_era_stash_snapshot);
            },
            2820..=2879 => {
                // distribute reward
                let mut all_machine = Self::all_machine_id_snap();
                let release_num = all_machine.1 / 60;

                let current_era = Self::current_era();
                let era_total_reward = Self::era_reward(current_era);
                let era_machine_points = Self::backup_machine_grade_snap();
                let era_stash_points = Self::backup_stash_grade_snap();

                for _ in 0..=release_num {
                    if let Some(machine_id) = all_machine.0.pop_front() {
                        Self::distribute_reward_to_machine(
                            machine_id,
                            current_era,
                            era_total_reward,
                            &era_machine_points,
                            &era_stash_points,
                        );
                    } else {
                        return
                    }
                }

                AllMachineIdSnap::<T>::put(all_machine);
            },
            _ => return,
        }
    }

    pub fn distribute_reward_to_machine(
        machine_id: MachineId,
        current_era: EraIndex,
        era_total_reward: BalanceOf<T>,
        era_machine_points: &BTreeMap<MachineId, MachineGradeStatus>,
        era_stash_points: &EraStashPoints<T::AccountId>,
    ) -> Result<(), ()> {
        let mut machine_recent_reward_info = Self::machine_recent_reward(&machine_id);

        let machine_info = Self::machines_info(&machine_id);

        // 计算当时机器实际获得的奖励
        let machine_points = era_machine_points.get(&machine_id).ok_or(())?;
        let stash_points = era_stash_points.staker_statistic.get(&machine_info.machine_stash).ok_or(())?;
        let machine_actual_grade = machine_points.machine_actual_grade(stash_points.inflation);
        let mut stash_machine = Self::stash_machines(&machine_info.machine_stash);

        // 该Era机器获得的总奖励
        let machine_total_reward =
            Perbill::from_rational_approximation(machine_actual_grade, era_stash_points.total) * era_total_reward;

        MachineRecentRewardInfo::add_new_reward(&mut machine_recent_reward_info, machine_total_reward);
        MachineRecentReward::<T>::insert(&machine_id, machine_recent_reward_info);

        let machine_recent_reward = Self::machine_recent_reward(&machine_id);

        if machine_recent_reward.recent_reward_sum == Zero::zero() {
            return Ok(())
        }

        let latest_reward = if machine_recent_reward.recent_machine_reward.len() > 0 {
            machine_recent_reward.recent_machine_reward[machine_recent_reward.recent_machine_reward.len() - 1]
        } else {
            Zero::zero()
        };

        let released_reward = Perbill::from_rational_approximation(24u32, 100u32) * latest_reward +
            Perbill::from_rational_approximation(1u32, 100u32) * machine_recent_reward.recent_reward_sum;

        // if should reward to committee
        let (reward_to_stash, reward_to_committee) = if current_era > machine_recent_reward.reward_committee_deadline {
            // only reward stash
            (released_reward, Zero::zero())
        } else {
            // 1% of released_reward to committee, 99% of released reward to stash
            let release_to_stash = Perbill::from_rational_approximation(99u32, 100u32) * released_reward;
            let release_to_committee = released_reward - release_to_stash;
            (release_to_stash, release_to_committee)
        };

        stash_machine.can_claim_reward += reward_to_stash;
        let committee_each_get =
            Perbill::from_rational_approximation(1u32, machine_recent_reward.reward_committee.len() as u32) *
                reward_to_committee;
        for a_committee in machine_recent_reward.reward_committee.clone() {
            T::ManageCommittee::add_reward(a_committee, committee_each_get);
        }

        // record this
        ErasMachineReward::<T>::insert(current_era, &machine_id, reward_to_stash);
        ErasStashReward::<T>::mutate(&current_era, &machine_info.machine_stash, |old_value| {
            *old_value += reward_to_stash;
        });

        ErasMachineReleasedReward::<T>::mutate(&current_era, &machine_id, |old_value| *old_value += reward_to_stash);
        ErasStashReleasedReward::<T>::mutate(&current_era, &machine_info.machine_stash, |old_value| {
            *old_value += reward_to_stash
        });

        StashMachines::<T>::insert(&machine_info.machine_stash, stash_machine);
        return Ok(())
    }
}
