use crate::{
    rpc_types::{MachineBriefInfo, RpcIRCommitteeOps, StakerInfo},
    BalanceOf, Config, Pallet, RentOrderDetail, RentOrderId, StashMachines,
};
use codec::EncodeLike;
use dbc_support::{
    live_machine::LiveMachine,
    machine_info::MachineInfo,
    rental_type::MachineGPUOrder,
    verify_online::{OCCommitteeMachineList, OCMachineCommitteeList},
    MachineId,
};
use frame_support::IterableStorageMap;
use sp_std::vec::Vec;

impl<T: Config> Pallet<T> {
    pub fn get_total_staker_num() -> u64 {
        <StashMachines<T> as IterableStorageMap<T::AccountId, _>>::iter().count() as u64
    }

    pub fn get_staker_info(
        account: impl EncodeLike<T::AccountId>,
    ) -> StakerInfo<BalanceOf<T>, T::BlockNumber, T::AccountId> {
        let staker_info = Self::stash_machines(account);

        let mut staker_machines = Vec::new();

        for machine_id in &staker_info.total_machine {
            let machine_info = match Self::machines_info(machine_id) {
                Some(machine_info) => machine_info,
                None => continue,
            };
            staker_machines.push(MachineBriefInfo {
                machine_id: machine_id.to_vec(),
                gpu_num: machine_info.gpu_num(),
                calc_point: machine_info.calc_point(),
                machine_status: machine_info.machine_status,
            })
        }

        StakerInfo { stash_statistic: staker_info, bonded_machines: staker_machines }
    }

    /// 获取机器列表
    pub fn get_machine_list() -> LiveMachine {
        Self::live_machines()
    }

    /// 获取机器详情
    pub fn get_machine_info(
        machine_id: MachineId,
    ) -> Option<MachineInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>> {
        Self::machines_info(&machine_id)
    }
}

// onlineCommittee RPC
impl<T: Config> Pallet<T> {
    pub fn get_machine_committee_list(
        machine_id: MachineId,
    ) -> Option<OCMachineCommitteeList<T::AccountId, T::BlockNumber>> {
        Self::machine_committee(machine_id)
    }

    pub fn get_committee_machine_list(committee: T::AccountId) -> OCCommitteeMachineList {
        Self::committee_machine(committee)
    }

    pub fn get_committee_ops(
        committee: T::AccountId,
        machine_id: MachineId,
    ) -> Option<RpcIRCommitteeOps<T::BlockNumber, BalanceOf<T>>> {
        let oc_committee_ops = Self::committee_online_ops(&committee, &machine_id);

        let committee_info = match Self::machine_committee(&machine_id) {
            Some(committee_info) => committee_info,
            None => return None,
        };
        Some(RpcIRCommitteeOps {
            booked_time: committee_info.book_time,
            staked_dbc: oc_committee_ops.staked_dbc,
            verify_time: oc_committee_ops.verify_time,
            confirm_hash: oc_committee_ops.confirm_hash,
            hash_time: oc_committee_ops.hash_time,
            confirm_time: oc_committee_ops.confirm_time,
            machine_status: oc_committee_ops.machine_status,
            machine_info: oc_committee_ops.machine_info,
        })
    }
}

impl<T: Config> Pallet<T> {
    pub fn get_rent_order(
        rent_id: RentOrderId,
    ) -> Option<RentOrderDetail<T::AccountId, T::BlockNumber, BalanceOf<T>>> {
        Self::rent_order(&rent_id)
    }

    pub fn get_rent_list(renter: T::AccountId) -> Vec<RentOrderId> {
        Self::user_rented(&renter)
    }

    pub fn is_machine_renter(machine_id: MachineId, renter: T::AccountId) -> bool {
        let machine_order = Self::machine_rent_order(machine_id);

        for order_id in machine_order.rent_order {
            if let Some(rent_info) = Self::rent_order(order_id) {
                if rent_info.renter == renter {
                    return true
                }
            }
        }
        false
    }

    pub fn get_machine_rent_id(machine_id: MachineId) -> MachineGPUOrder {
        Self::machine_rent_order(machine_id)
    }
}
