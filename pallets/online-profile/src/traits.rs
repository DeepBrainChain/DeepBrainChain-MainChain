use crate::{
    types::*, BalanceOf, Config, ControllerMachines, LiveMachines, MachineRecentReward,
    MachineRentedGPU, MachinesInfo, Pallet, PendingExecSlash, PendingOfflineSlash, PendingSlash,
    RentedFinished, StashMachines, StashStake, SysInfo, UserMutHardwareStake,
};
use dbc_support::traits::{MTOps, OCOps, OPRPCQuery, RTOps};
use frame_support::IterableStorageMap;
use generic_func::{ItemList, MachineId};
use sp_runtime::{
    traits::{CheckedMul, CheckedSub},
    Perbill, SaturatedConversion,
};
use sp_std::{prelude::Box, vec, vec::Vec};

/// 审查委员会可以执行的操作
impl<T: Config> OCOps for Pallet<T> {
    type MachineId = MachineId;
    type AccountId = T::AccountId;
    type CommitteeUploadInfo = CommitteeUploadInfo;
    type Balance = BalanceOf<T>;

    // 委员会订阅了一个机器ID
    // 将机器状态从ocw_confirmed_machine改为booked_machine，同时将机器状态改为booked
    // - Writes: LiveMachine, MachinesInfo
    fn oc_booked_machine(id: MachineId) {
        let mut live_machines = Self::live_machines();

        ItemList::rm_item(&mut live_machines.confirmed_machine, &id);
        ItemList::add_item(&mut live_machines.booked_machine, id.clone());

        let mut machine_info = Self::machines_info(&id);
        machine_info.machine_status = MachineStatus::CommitteeVerifying;

        LiveMachines::<T>::put(live_machines);
        MachinesInfo::<T>::insert(&id, machine_info);
    }

    // 由于委员会没有达成一致，需要重新返回到bonding_machine
    fn oc_revert_booked_machine(id: MachineId) {
        let mut live_machines = Self::live_machines();

        ItemList::rm_item(&mut live_machines.booked_machine, &id);
        ItemList::add_item(&mut live_machines.confirmed_machine, id.clone());

        let mut machine_info = Self::machines_info(&id);
        machine_info.machine_status = MachineStatus::DistributingOrder;

        LiveMachines::<T>::put(live_machines);
        MachinesInfo::<T>::insert(&id, machine_info);
    }

    // 当多个委员会都对机器进行了确认之后，添加机器信息，并更新机器得分
    // 机器被成功添加, 则添加上可以获取收益的委员会
    fn oc_confirm_machine(
        reported_committee: Vec<T::AccountId>,
        committee_upload_info: CommitteeUploadInfo,
    ) -> Result<(), ()> {
        let now = <frame_system::Module<T>>::block_number();
        let current_era = Self::current_era();
        let machine_id = committee_upload_info.machine_id.clone();

        let mut machine_info = Self::machines_info(&machine_id);
        let mut live_machines = Self::live_machines();

        let is_reonline =
            UserMutHardwareStake::<T>::contains_key(&machine_info.machine_stash, &machine_id);

        ItemList::rm_item(&mut live_machines.booked_machine, &machine_id);

        machine_info.machine_info_detail.committee_upload_info = committee_upload_info.clone();
        if !is_reonline {
            machine_info.reward_committee = reported_committee.clone();
        }

        // 改变用户的绑定数量。如果用户余额足够，则直接质押。否则将机器状态改为补充质押
        let stake_need = machine_info
            .init_stake_per_gpu
            .checked_mul(&committee_upload_info.gpu_num.saturated_into::<BalanceOf<T>>())
            .ok_or(())?;
        if let Some(extra_stake) = stake_need.checked_sub(&machine_info.stake_amount) {
            if Self::change_user_total_stake(machine_info.machine_stash.clone(), extra_stake, true)
                .is_ok()
            {
                ItemList::add_item(&mut live_machines.online_machine, machine_id.clone());
                machine_info.stake_amount = stake_need;
                machine_info.machine_status = MachineStatus::Online;
                machine_info.last_online_height = now;
                machine_info.last_machine_restake = now;

                if !is_reonline {
                    machine_info.online_height = now;
                    machine_info.reward_deadline = current_era + REWARD_DURATION;
                }
            } else {
                ItemList::add_item(&mut live_machines.fulfilling_machine, machine_id.clone());
                machine_info.machine_status = MachineStatus::WaitingFulfill;
            }
        } else {
            ItemList::add_item(&mut live_machines.online_machine, machine_id.clone());
            machine_info.machine_status = MachineStatus::Online;
            if !is_reonline {
                machine_info.reward_deadline = current_era + REWARD_DURATION;
            }
        }

        MachinesInfo::<T>::insert(&machine_id, machine_info.clone());
        LiveMachines::<T>::put(live_machines);

        if is_reonline {
            // 根据质押，奖励给这些委员会
            let reonline_stake =
                Self::user_mut_hardware_stake(&machine_info.machine_stash, &machine_id);

            let _ = Self::slash_and_reward(
                machine_info.machine_stash.clone(),
                reonline_stake.stake_amount,
                reported_committee,
            );
        }

        // NOTE: Must be after MachinesInfo change, which depend on machine_info
        if let MachineStatus::Online = machine_info.machine_status {
            Self::change_pos_info_by_online(&machine_info, true);
            Self::update_snap_by_online_status(machine_id.clone(), true);

            if is_reonline {
                // 仅在Oline成功时删掉reonline_stake记录，以便补充质押时惩罚时检查状态
                let reonline_stake = Self::user_mut_hardware_stake(
                    &machine_info.machine_stash,
                    &committee_upload_info.machine_id,
                );

                UserMutHardwareStake::<T>::remove(
                    &machine_info.machine_stash,
                    &committee_upload_info.machine_id,
                );

                // 惩罚该机器，如果机器是Fulfill，则等待Fulfill之后，再进行惩罚
                let offline_duration = now - reonline_stake.offline_time;
                // 记录该惩罚数据
                let slash_info = Self::new_slash_when_offline(
                    committee_upload_info.machine_id,
                    OPSlashReason::OnlineReportOffline(reonline_stake.offline_time),
                    None,
                    vec![],
                    None,
                    offline_duration,
                );
                let slash_id = Self::get_new_slash_id();

                let mut pending_exec_slash = Self::pending_exec_slash(slash_info.slash_exec_time);
                ItemList::add_item(&mut pending_exec_slash, slash_id);
                PendingExecSlash::<T>::insert(slash_info.slash_exec_time, pending_exec_slash);

                PendingSlash::<T>::insert(slash_id, slash_info);
            } else {
                MachineRecentReward::<T>::insert(
                    &machine_id,
                    MachineRecentRewardInfo {
                        machine_stash: machine_info.machine_stash.clone(),
                        reward_committee_deadline: machine_info.reward_deadline,
                        reward_committee: machine_info.reward_committee,
                        ..Default::default()
                    },
                );
            }
        }

        Ok(())
    }

    // When committees reach an agreement to refuse machine, change machine status and record refuse
    // time
    fn oc_refuse_machine(machine_id: MachineId) -> Option<(T::AccountId, BalanceOf<T>)> {
        // Refuse controller bond machine, and clean storage
        let machine_info = Self::machines_info(&machine_id);
        let mut live_machines = Self::live_machines();

        // In case this offline is for change hardware info, when reonline is refused, reward to
        // committee and machine info should not be deleted
        let is_mut_hardware =
            UserMutHardwareStake::<T>::contains_key(&machine_info.machine_stash, &machine_id);
        if is_mut_hardware {
            let reonline_stake =
                Self::user_mut_hardware_stake(&machine_info.machine_stash, &machine_id);

            ItemList::rm_item(&mut live_machines.booked_machine, &machine_id);
            ItemList::add_item(&mut live_machines.bonding_machine, machine_id.clone());

            LiveMachines::<T>::put(live_machines);
            return Some((machine_info.machine_stash, reonline_stake.stake_amount))
        }

        // let mut sys_info = Self::sys_info();
        let mut stash_machines = Self::stash_machines(&machine_info.machine_stash);
        let mut controller_machines = Self::controller_machines(&machine_info.controller);

        // Slash 5% of init stake(5% of one gpu stake)
        let slash = Perbill::from_rational_approximation(5u64, 100u64) * machine_info.stake_amount;

        let left_stake = machine_info.stake_amount.checked_sub(&slash)?;
        // Remain 5% of init stake(5% of one gpu stake)
        // Return 95% left stake(95% of one gpu stake)
        let _ =
            Self::change_user_total_stake(machine_info.machine_stash.clone(), left_stake, false);

        // Clean storage
        ItemList::rm_item(&mut controller_machines, &machine_id);
        ItemList::rm_item(&mut stash_machines.total_machine, &machine_id);

        let mut live_machines = Self::live_machines();
        ItemList::rm_item(&mut live_machines.booked_machine, &machine_id);
        ItemList::add_item(&mut live_machines.refused_machine, machine_id.clone());

        LiveMachines::<T>::put(live_machines);
        MachinesInfo::<T>::remove(&machine_id);
        ControllerMachines::<T>::insert(&machine_info.controller, controller_machines);
        StashMachines::<T>::insert(&machine_info.machine_stash, stash_machines);

        Some((machine_info.machine_stash, slash))
    }

    // stake some balance when apply for slash review
    // Should stake some balance when apply for slash review
    fn oc_change_staked_balance(
        stash: T::AccountId,
        amount: BalanceOf<T>,
        is_add: bool,
    ) -> Result<(), ()> {
        Self::change_user_total_stake(stash, amount, is_add)
    }

    // just change stash_stake & sys_info, slash and reward should be execed in oc module
    fn oc_exec_slash(stash: T::AccountId, amount: BalanceOf<T>) -> Result<(), ()> {
        let mut stash_stake = Self::stash_stake(&stash);
        let mut sys_info = Self::sys_info();

        sys_info.total_stake = sys_info.total_stake.checked_sub(&amount).ok_or(())?;
        stash_stake = stash_stake.checked_sub(&amount).ok_or(())?;

        StashStake::<T>::insert(&stash, stash_stake);
        SysInfo::<T>::put(sys_info);
        Ok(())
    }
}

impl<T: Config> RTOps for Pallet<T> {
    type MachineId = MachineId;
    type MachineStatus = MachineStatus<T::BlockNumber, T::AccountId>;
    type AccountId = T::AccountId;
    type Balance = BalanceOf<T>;
    type BlockNumber = T::BlockNumber;

    /// 根据GPU数量和该机器算力点数，计算该机器相比标准配置的租用价格
    // standard_point / machine_point ==  standard_price / machine_price
    // =>
    // machine_price = standard_price * machine_point / standard_point
    fn get_machine_price(machine_point: u64, need_gpu: u32, total_gpu: u32) -> Option<u64> {
        if total_gpu == 0 {
            return None
        }
        let standard_gpu_point_price = Self::standard_gpu_point_price()?;
        standard_gpu_point_price
            .gpu_price
            .checked_mul(machine_point)?
            .checked_mul(10_000)?
            .checked_div(standard_gpu_point_price.gpu_point)?
            .checked_mul(need_gpu as u64)?
            .checked_div(total_gpu as u64)?
            .checked_div(10_000)
    }

    // 在rent_machine; rent_machine_by_minutes中使用, confirm_rent之前
    fn change_machine_status_on_rent_start(machine_id: &MachineId, gpu_num: u32) {
        let mut machine_info = Self::machines_info(machine_id);
        let mut machine_rented_gpu = Self::machine_rented_gpu(&machine_id);

        machine_info.machine_status = MachineStatus::Rented;
        machine_rented_gpu = machine_rented_gpu.saturating_add(gpu_num);

        MachinesInfo::<T>::insert(&machine_id, machine_info);
        MachineRentedGPU::<T>::insert(&machine_id, machine_rented_gpu);
    }

    // 在confirm_rent中使用
    fn change_machine_status_on_confirmed(machine_id: &MachineId, renter: Self::AccountId) {
        let mut machine_info = Self::machines_info(machine_id);
        let mut live_machines = Self::live_machines();

        ItemList::add_item(&mut machine_info.renters, renter);

        // 机器创建成功
        machine_info.total_rented_times += 1;

        // NOTE: 该检查确保得分快照不被改变多次
        if live_machines.rented_machine.binary_search(machine_id).is_err() {
            Self::update_snap_by_rent_status(machine_id.to_vec(), true);

            ItemList::rm_item(&mut live_machines.online_machine, machine_id);
            ItemList::add_item(&mut live_machines.rented_machine, machine_id.clone());
            LiveMachines::<T>::put(live_machines);

            Self::change_pos_info_by_rent(&machine_info, true);
        }

        MachinesInfo::<T>::insert(&machine_id, machine_info);
    }

    fn change_machine_status_on_rent_end(
        machine_id: &MachineId,
        rented_gpu_num: u32,
        rent_duration: Self::BlockNumber,
        is_last_rent: bool,
        renter: Self::AccountId,
    ) {
        let mut machine_info = Self::machines_info(machine_id);
        let mut live_machines = Self::live_machines();
        let mut machine_rented_gpu = Self::machine_rented_gpu(machine_id);
        machine_rented_gpu = machine_rented_gpu.saturating_sub(rented_gpu_num);

        // 租用结束
        let gpu_num = machine_info.gpu_num();
        if gpu_num == 0 {
            return
        }
        machine_info.total_rented_duration +=
            Perbill::from_rational_approximation(rented_gpu_num, gpu_num) * rent_duration;
        ItemList::rm_item(&mut machine_info.renters, &renter);

        match machine_info.machine_status {
            MachineStatus::ReporterReportOffline(..) | MachineStatus::StakerReportOffline(..) => {
                RentedFinished::<T>::insert(machine_id, renter);
            },
            MachineStatus::Rented => {
                // machine_info.machine_status = new_status;

                // NOTE: 考虑是不是last_rent
                if is_last_rent {
                    ItemList::rm_item(&mut live_machines.rented_machine, machine_id);
                    ItemList::add_item(&mut live_machines.online_machine, machine_id.clone());

                    machine_info.last_online_height = <frame_system::Module<T>>::block_number();
                    machine_info.machine_status = MachineStatus::Online;

                    // 租用结束
                    Self::update_snap_by_rent_status(machine_id.to_vec(), false);
                    Self::change_pos_info_by_rent(&machine_info, false);
                }
            },
            _ => {},
        }

        MachineRentedGPU::<T>::insert(&machine_id, machine_rented_gpu);
        LiveMachines::<T>::put(live_machines);
        MachinesInfo::<T>::insert(&machine_id, machine_info);
    }

    fn change_machine_status_on_confirm_expired(machine_id: &MachineId, gpu_num: u32) {
        let mut machine_rented_gpu = Self::machine_rented_gpu(&machine_id);

        machine_rented_gpu = machine_rented_gpu.saturating_sub(gpu_num);

        if machine_rented_gpu == 0 {
            // 已经没有正在租用的机器时，改变机器的状态
            let mut machine_info = Self::machines_info(machine_id);
            machine_info.machine_status = MachineStatus::Online;
            MachinesInfo::<T>::insert(&machine_id, machine_info);
        }

        MachineRentedGPU::<T>::insert(&machine_id, machine_rented_gpu);
    }

    fn change_machine_rent_fee(amount: BalanceOf<T>, machine_id: MachineId, is_burn: bool) {
        let mut machine_info = Self::machines_info(&machine_id);
        let mut staker_machine = Self::stash_machines(&machine_info.machine_stash);
        let mut sys_info = Self::sys_info();

        sys_info.change_rent_fee(amount, is_burn);
        staker_machine.change_rent_fee(amount, is_burn);
        machine_info.change_rent_fee(amount, is_burn);

        SysInfo::<T>::put(sys_info);
        StashMachines::<T>::insert(&machine_info.machine_stash, staker_machine);
        MachinesInfo::<T>::insert(&machine_id, machine_info);
    }
}

impl<T: Config> OPRPCQuery for Pallet<T> {
    type AccountId = T::AccountId;
    type StashMachine = StashMachine<BalanceOf<T>>;

    fn get_all_stash() -> Vec<T::AccountId> {
        <StashMachines<T> as IterableStorageMap<T::AccountId, _>>::iter()
            .map(|(staker, _)| staker)
            .collect::<Vec<_>>()
    }

    fn get_stash_machine(stash: T::AccountId) -> StashMachine<BalanceOf<T>> {
        Self::stash_machines(stash)
    }
}

impl<T: Config> MTOps for Pallet<T> {
    type MachineId = MachineId;
    type AccountId = T::AccountId;
    type FaultType = OPSlashReason<T::BlockNumber>;
    type Balance = BalanceOf<T>;

    fn mt_machine_offline(
        reporter: T::AccountId,
        committee: Vec<T::AccountId>,
        machine_id: MachineId,
        fault_type: OPSlashReason<T::BlockNumber>,
    ) {
        let machine_info = Self::machines_info(&machine_id);

        Self::machine_offline(
            machine_id.clone(),
            MachineStatus::ReporterReportOffline(
                fault_type,
                Box::new(machine_info.machine_status),
                reporter.clone(),
                committee,
            ),
        );

        // When Reported offline, after 5 days, reach max slash amount;
        let now = <frame_system::Module<T>>::block_number();
        PendingOfflineSlash::<T>::insert(
            now + (5 * BLOCK_PER_ERA).saturated_into::<T::BlockNumber>(),
            machine_id,
            (Some(reporter), machine_info.renters),
        );
    }

    // stake some balance when apply for slash review
    // Should stake some balance when apply for slash review
    fn mt_change_staked_balance(
        stash: T::AccountId,
        amount: BalanceOf<T>,
        is_add: bool,
    ) -> Result<(), ()> {
        Self::change_user_total_stake(stash, amount, is_add)
    }

    fn mt_rm_stash_total_stake(stash: T::AccountId, amount: BalanceOf<T>) -> Result<(), ()> {
        let mut stash_stake = Self::stash_stake(&stash);
        let mut sys_info = Self::sys_info();

        sys_info.total_stake = sys_info.total_stake.checked_sub(&amount).ok_or(())?;
        stash_stake = stash_stake.checked_sub(&amount).ok_or(())?;

        StashStake::<T>::insert(&stash, stash_stake);
        SysInfo::<T>::put(sys_info);
        Ok(())
    }
}
