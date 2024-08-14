use crate::{
    types::*, BalanceOf, Config, ControllerMachines, LiveMachines, MachineRecentReward,
    MachineRentedGPU, MachinesInfo, Pallet, RentedFinished, StashMachines, StashStake, SysInfo,
    UserMutHardwareStake,
};
use dbc_support::{
    machine_type::{CommitteeUploadInfo, MachineStatus},
    traits::{MTOps, OCOps, OPRPCQuery, RTOps},
    verify_online::StashMachine,
    verify_slash::OPSlashReason,
    ItemList, MachineId,
};
use frame_support::IterableStorageMap;
use sp_runtime::{
    traits::{CheckedSub, Saturating, Zero},
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
    fn booked_machine(id: MachineId) -> Result<(), ()> {
        MachinesInfo::<T>::try_mutate(&id, |machine_info| {
            let machine_info = machine_info.as_mut().ok_or(())?;
            machine_info.machine_status = MachineStatus::CommitteeVerifying;
            Ok::<(), ()>(())
        })?;
        LiveMachines::<T>::mutate(|live_machines| {
            ItemList::rm_item(&mut live_machines.confirmed_machine, &id);
            ItemList::add_item(&mut live_machines.booked_machine, id.clone());
        });
        Ok::<(), ()>(())
    }

    // 由于委员会没有达成一致，需要重新返回到bonding_machine
    fn revert_booked_machine(id: MachineId) -> Result<(), ()> {
        MachinesInfo::<T>::mutate(&id, |machine_info| {
            let machine_info = machine_info.as_mut().ok_or(())?;
            machine_info.machine_status = MachineStatus::DistributingOrder;
            Ok::<(), ()>(())
        })?;
        LiveMachines::<T>::mutate(|live_machines| {
            ItemList::rm_item(&mut live_machines.booked_machine, &id);
            ItemList::add_item(&mut live_machines.confirmed_machine, id.clone());
        });
        Ok::<(), ()>(())
    }

    // 当多个委员会都对机器进行了确认之后，添加机器信息，并更新机器得分
    // 机器被成功添加, 则添加上可以获取收益的委员会
    fn confirm_machine(
        verify_committee: Vec<T::AccountId>,
        hardware_info: CommitteeUploadInfo,
    ) -> Result<(), ()> {
        let now = <frame_system::Pallet<T>>::block_number();
        let current_era = Self::current_era();
        let machine_id = hardware_info.machine_id.clone();

        let mut machine_info = Self::machines_info(&machine_id).ok_or(())?;
        let mut live_machines = Self::live_machines();

        let machine_stash = machine_info.machine_stash.clone();

        let is_reonline = UserMutHardwareStake::<T>::contains_key(&machine_stash, &machine_id);
        let mut reonline_stake = Self::user_mut_hardware_stake(&machine_stash, &machine_id);

        if is_reonline {
            // 奖励委员会
            let _ = Self::slash_and_reward(
                machine_stash.clone(),
                reonline_stake.verify_fee,
                verify_committee.clone(),
            );
            // 将质押惩罚到国库
            let _ =
                Self::slash_and_reward(machine_stash.clone(), reonline_stake.offline_slash, vec![]);
            // 当补交失败时，记录下已经变更的质押
            reonline_stake.verify_fee = Zero::zero();
            reonline_stake.offline_slash = Zero::zero();
        } else {
            machine_info.reward_committee = verify_committee.clone();
        }

        machine_info.machine_info_detail.committee_upload_info = hardware_info.clone();
        ItemList::rm_item(&mut live_machines.booked_machine, &machine_id);

        // 改变用户的绑定数量。如果用户余额足够，则直接质押。否则将机器状态改为补充质押
        let stake_need = machine_info
            .init_stake_per_gpu
            .saturating_mul(hardware_info.gpu_num.saturated_into::<BalanceOf<T>>());

        // 需要补交质押，并且补交质押失败
        if stake_need > machine_info.stake_amount {
            let extra_stake = stake_need.saturating_sub(machine_info.stake_amount);
            if Self::change_stake(&machine_stash, extra_stake, true).is_err() {
                // 补交质押失败
                reonline_stake.need_fulfilling = true;
                UserMutHardwareStake::<T>::insert(&machine_stash, &machine_id, reonline_stake);

                ItemList::add_item(&mut live_machines.fulfilling_machine, machine_id.clone());
                machine_info.machine_status = MachineStatus::WaitingFulfill;
                MachinesInfo::<T>::insert(&machine_id, machine_info.clone());
                LiveMachines::<T>::put(live_machines);
                return Ok(())
            }
        }
        // NOTE: 下线更改机器配置的时候，如果余额超过所需（比如从多卡变成单卡）则**不需要**退还质押
        // 因为实际上机器更改硬件时不允许减少GPU

        ItemList::add_item(&mut live_machines.online_machine, machine_id.clone());
        machine_info.stake_amount = stake_need;
        machine_info.machine_status = MachineStatus::Online;
        machine_info.last_online_height = now;
        machine_info.last_machine_restake = now;

        if is_reonline {
            // 机器正常上线(非fulfilling)，移除质押信息
            UserMutHardwareStake::<T>::remove(&machine_stash, &machine_id);
        } else {
            // 如果是reonline, 则不需要更改下面信息
            machine_info.online_height = now;
            machine_info.reward_deadline = current_era + REWARD_DURATION;

            MachineRecentReward::<T>::insert(
                &machine_id,
                MachineRecentRewardInfo {
                    machine_stash,
                    reward_committee_deadline: machine_info.reward_deadline,
                    reward_committee: machine_info.reward_committee.clone(),
                    recent_machine_reward: Default::default(),
                    recent_reward_sum: Default::default(),
                },
            );
        }

        MachinesInfo::<T>::insert(&machine_id, machine_info.clone());
        LiveMachines::<T>::put(live_machines);

        // NOTE: Must be after MachinesInfo change, which depend on machine_info
        // if matches!(machine_info.machine_status, MachineStatus::Online) {
        Self::update_region_on_online_changed(&machine_info, true);
        let _ = Self::update_snap_on_online_changed(machine_id.clone(), true);
        return Ok(())
    }

    // When committees reach an agreement to refuse machine, change machine status and record refuse
    // time
    fn refuse_machine(
        verify_committee: Vec<T::AccountId>,
        machine_id: MachineId,
    ) -> Option<(T::AccountId, BalanceOf<T>)> {
        // Refuse controller bond machine, and clean storage
        let machine_info = Self::machines_info(&machine_id)?;

        // In case this offline is for change hardware info, when reonline is refused, reward to
        // committee and machine info should not be deleted
        let is_mut_hardware =
            UserMutHardwareStake::<T>::contains_key(&machine_info.machine_stash, &machine_id);
        if is_mut_hardware {
            let mut reonline_stake =
                Self::user_mut_hardware_stake(&machine_info.machine_stash, &machine_id);

            LiveMachines::<T>::mutate(|live_machines| {
                ItemList::rm_item(&mut live_machines.booked_machine, &machine_id);
                ItemList::add_item(&mut live_machines.bonding_machine, machine_id.clone());
            });

            // 拒绝时直接将惩罚分发给验证人即可
            let _ = Self::slash_and_reward(
                machine_info.machine_stash.clone(),
                reonline_stake.verify_fee,
                verify_committee.clone(),
            );
            reonline_stake.verify_fee = Zero::zero();
            UserMutHardwareStake::<T>::insert(
                &machine_info.machine_stash,
                &machine_id,
                reonline_stake,
            );

            return None
        }

        // let mut sys_info = Self::sys_info();

        // Slash 5% of init stake(5% of one gpu stake)
        let slash = Perbill::from_rational(5u64, 100u64) * machine_info.stake_amount;

        let left_stake = machine_info.stake_amount.checked_sub(&slash)?;
        // Remain 5% of init stake(5% of one gpu stake)
        // Return 95% left stake(95% of one gpu stake)
        let _ = Self::change_stake(&machine_info.machine_stash, left_stake, false);

        // Clean storage

        StashMachines::<T>::mutate(&machine_info.machine_stash, |stash_machines| {
            ItemList::rm_item(&mut stash_machines.total_machine, &machine_id);
        });
        ControllerMachines::<T>::mutate(&machine_info.controller, |controller_machines| {
            ItemList::rm_item(controller_machines, &machine_id);
        });
        LiveMachines::<T>::mutate(|live_machines| {
            ItemList::rm_item(&mut live_machines.booked_machine, &machine_id);
            ItemList::add_item(&mut live_machines.refused_machine, machine_id.clone());
        });

        MachinesInfo::<T>::remove(&machine_id);

        Some((machine_info.machine_stash, slash))
    }

    // stake some balance when apply for slash review
    // Should stake some balance when apply for slash review
    fn change_staked_balance(
        stash: T::AccountId,
        amount: BalanceOf<T>,
        is_add: bool,
    ) -> Result<(), ()> {
        Self::change_stake(&stash, amount, is_add)
    }

    // just change stash_stake & sys_info, slash and reward should be execed in oc module
    fn exec_slash(stash: T::AccountId, amount: BalanceOf<T>) -> Result<(), ()> {
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
    fn change_machine_status_on_rent_start(machine_id: &MachineId, gpu_num: u32) -> Result<(), ()> {
        MachinesInfo::<T>::try_mutate(machine_id, |machine_info| {
            let machine_info = machine_info.as_mut().ok_or(())?;
            machine_info.machine_status = MachineStatus::Rented;
            Ok::<(), ()>(())
        })?;
        MachineRentedGPU::<T>::mutate(machine_id, |machine_rented_gpu| {
            *machine_rented_gpu = machine_rented_gpu.saturating_add(gpu_num);
        });
        Ok(())
    }

    // 在confirm_rent中使用
    fn change_machine_status_on_confirmed(
        machine_id: &MachineId,
        renter: Self::AccountId,
    ) -> Result<(), ()> {
        let mut machine_info = Self::machines_info(machine_id).ok_or(())?;
        let mut live_machines = Self::live_machines();

        ItemList::add_item(&mut machine_info.renters, renter);

        // 机器创建成功
        machine_info.total_rented_times += 1;

        // NOTE: 该检查确保得分快照不被改变多次
        if live_machines.rented_machine.binary_search(machine_id).is_err() {
            Self::update_snap_on_rent_changed(machine_id.to_vec(), true)?;

            ItemList::rm_item(&mut live_machines.online_machine, machine_id);
            ItemList::add_item(&mut live_machines.rented_machine, machine_id.clone());
            LiveMachines::<T>::put(live_machines);

            Self::update_region_on_rent_changed(&machine_info, true);
        }

        MachinesInfo::<T>::insert(&machine_id, machine_info);
        Ok(())
    }

    fn change_machine_status_on_rent_end(
        machine_id: &MachineId,
        rented_gpu_num: u32,
        rent_duration: Self::BlockNumber,
        is_machine_last_rent: bool,
        is_renter_last_rent: bool,
        renter: Self::AccountId,
    ) -> Result<(), ()> {
        let mut machine_info = Self::machines_info(machine_id).ok_or(())?;
        let mut live_machines = Self::live_machines();
        let mut machine_rented_gpu = Self::machine_rented_gpu(machine_id);
        machine_rented_gpu = machine_rented_gpu.saturating_sub(rented_gpu_num);

        // 租用结束
        let gpu_num = machine_info.gpu_num();
        if gpu_num == 0 {
            return Ok(())
        }
        machine_info.total_rented_duration +=
            Perbill::from_rational(rented_gpu_num, gpu_num) * rent_duration;

        if is_renter_last_rent {
            // NOTE: 只有在是最后一个renter时，才移除
            ItemList::rm_item(&mut machine_info.renters, &renter);
        }

        match machine_info.machine_status {
            MachineStatus::ReporterReportOffline(..) | MachineStatus::StakerReportOffline(..) => {
                RentedFinished::<T>::insert(machine_id, renter);
            },
            MachineStatus::Rented => {
                // machine_info.machine_status = new_status;

                // NOTE: 考虑是不是last_rent
                if is_machine_last_rent {
                    ItemList::rm_item(&mut live_machines.rented_machine, machine_id);
                    ItemList::add_item(&mut live_machines.online_machine, machine_id.clone());

                    machine_info.last_online_height = <frame_system::Pallet<T>>::block_number();
                    machine_info.machine_status = MachineStatus::Online;

                    // 租用结束
                    Self::update_snap_on_rent_changed(machine_id.to_vec(), false)?;
                    Self::update_region_on_rent_changed(&machine_info, false);
                }
            },
            _ => {},
        }

        MachineRentedGPU::<T>::insert(&machine_id, machine_rented_gpu);
        LiveMachines::<T>::put(live_machines);
        MachinesInfo::<T>::insert(&machine_id, machine_info);
        Ok(())
    }

    fn change_machine_status_on_confirm_expired(
        machine_id: &MachineId,
        gpu_num: u32,
    ) -> Result<(), ()> {
        let mut machine_rented_gpu = Self::machine_rented_gpu(&machine_id);

        machine_rented_gpu = machine_rented_gpu.saturating_sub(gpu_num);

        if machine_rented_gpu == 0 {
            // 已经没有正在租用的机器时，改变机器的状态
            MachinesInfo::<T>::try_mutate(machine_id, |machine_info| {
                let machine_info = machine_info.as_mut().ok_or(())?;
                machine_info.machine_status = MachineStatus::Online;
                Ok::<(), ()>(())
            })?;
        }

        MachineRentedGPU::<T>::insert(&machine_id, machine_rented_gpu);
        Ok(())
    }

    // NOTE: 银河竞赛开启前，租金付给stash账户；开启后租金转到销毁账户
    // NOTE: 租金付给stash账户时，检查是否满足单卡10w/$300的质押条件，不满足，先质押.
    fn change_machine_rent_fee(
        machine_stash: T::AccountId,
        machine_id: MachineId,
        fee_to_destroy: BalanceOf<T>,
        fee_to_stash: BalanceOf<T>,
    ) -> Result<(), ()> {
        Self::fulfill_machine_stake(machine_stash, fee_to_stash).map_err(|_| ())?;

        let mut machine_info = Self::machines_info(&machine_id).ok_or(())?;
        let mut staker_machine = Self::stash_machines(&machine_info.machine_stash);
        let mut sys_info = Self::sys_info();

        sys_info.change_rent_fee(fee_to_destroy, fee_to_stash);
        staker_machine.change_rent_fee(fee_to_destroy, fee_to_stash);
        machine_info.change_rent_fee(fee_to_destroy, fee_to_stash);

        SysInfo::<T>::put(sys_info);
        StashMachines::<T>::insert(&machine_info.machine_stash, staker_machine);
        MachinesInfo::<T>::insert(&machine_id, machine_info);
        Ok::<(), ()>(())
    }

    fn reset_machine_renters(machine_id: MachineId, renters: Vec<T::AccountId>) -> Result<(), ()> {
        MachinesInfo::<T>::mutate(machine_id, |machine_info| {
            let machine_info = machine_info.as_mut().ok_or(())?;
            machine_info.renters = renters;
            Ok(())
        })
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
    ) -> Result<(), ()> {
        Self::machine_offline(
            machine_id.clone(),
            MachineStatus::ReporterReportOffline(
                fault_type,
                Box::new(Self::machines_info(&machine_id).ok_or(())?.machine_status),
                reporter.clone(),
                committee,
            ),
        )
    }

    // stake some balance when apply for slash review
    // Should stake some balance when apply for slash review
    fn mt_change_staked_balance(
        stash: T::AccountId,
        amount: BalanceOf<T>,
        is_add: bool,
    ) -> Result<(), ()> {
        Self::change_stake(&stash, amount, is_add)
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
