use crate::types::*;
use frame_support::{IterableStorageDoubleMap, IterableStorageMap};
use generic_func::MachineId;
use sp_std::vec::Vec;

use codec::EncodeLike;

use crate::{BalanceOf, Config, Pallet, PosGPUInfo, StashMachines};

type EraIndex = u32;

impl<T: Config> Pallet<T> {
    pub fn get_total_staker_num() -> u64 {
        let all_stash = <StashMachines<T> as IterableStorageMap<T::AccountId, _>>::iter()
            .map(|(staker, _)| staker)
            .collect::<Vec<_>>();

        all_stash.len() as u64
    }

    pub fn get_op_info() -> SysInfoDetail<BalanceOf<T>> {
        Self::sys_info()
    }

    pub fn get_staker_info(
        account: impl EncodeLike<T::AccountId>,
    ) -> RpcStakerInfo<BalanceOf<T>, T::BlockNumber, T::AccountId> {
        let staker_info = Self::stash_machines(account);

        let mut staker_machines = Vec::new();

        for a_machine in &staker_info.total_machine {
            let machine_info = Self::machines_info(a_machine);
            staker_machines.push(MachineBriefInfo {
                machine_id: a_machine.to_vec(),
                gpu_num: machine_info.gpu_num(),
                calc_point: machine_info.calc_point(),
                machine_status: machine_info.machine_status,
            })
        }

        RpcStakerInfo { stash_statistic: staker_info, bonded_machines: staker_machines }
    }

    /// 获取机器列表
    pub fn get_machine_list() -> LiveMachine {
        Self::live_machines()
    }

    /// 获取机器详情
    pub fn get_machine_info(machine_id: MachineId) -> MachineInfo<T::AccountId, T::BlockNumber, BalanceOf<T>> {
        Self::machines_info(&machine_id)
    }

    /// 获得系统中所有位置列表
    pub fn get_pos_gpu_info() -> Vec<(Longitude, Latitude, PosInfo)> {
        <PosGPUInfo<T> as IterableStorageDoubleMap<Longitude, Latitude, PosInfo>>::iter()
            .map(|(k1, k2, v)| (k1, k2, v))
            .collect()
    }

    /// 获得某个机器某个Era奖励数量
    pub fn get_machine_era_reward(machine_id: MachineId, era_index: EraIndex) -> BalanceOf<T> {
        Self::eras_machine_reward(era_index, machine_id)
    }

    /// 获得某个机器某个Era实际奖励数量
    pub fn get_machine_era_released_reward(machine_id: MachineId, era_index: EraIndex) -> BalanceOf<T> {
        Self::eras_machine_released_reward(era_index, machine_id)
    }

    /// 获得某个Stash账户某个Era获得的奖励数量
    pub fn get_stash_era_reward(stash: T::AccountId, era_index: EraIndex) -> BalanceOf<T> {
        Self::eras_stash_reward(era_index, stash)
    }

    /// 获得某个Stash账户某个Era实际解锁的奖励数量
    pub fn get_stash_era_released_reward(stash: T::AccountId, era_index: EraIndex) -> BalanceOf<T> {
        Self::eras_stash_released_reward(era_index, stash)
    }
}
