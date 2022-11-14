use frame_support::IterableStorageMap;
use sp_std::vec::Vec;

use crate::{
    rpc_types::{MachineBriefInfo, RpcIRCommitteeOps, StakerInfo},
    BalanceOf, Config, IRCommitteeMachineList, IRLiveMachine, IRMachineCommitteeList,
    IRMachineGPUOrder, IRMachineInfo, IRRentOrderDetail, Pallet, RentOrderId, StashMachines,
};
use codec::EncodeLike;
use generic_func::MachineId;

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
            let machine_info = Self::machines_info(machine_id);
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
    pub fn get_machine_list() -> IRLiveMachine {
        Self::live_machines()
    }

    /// 获取机器详情
    pub fn get_machine_info(
        machine_id: MachineId,
    ) -> IRMachineInfo<T::AccountId, T::BlockNumber, BalanceOf<T>> {
        Self::machines_info(&machine_id)
    }
}

// onlineCommittee RPC
impl<T: Config> Pallet<T> {
    pub fn get_machine_committee_list(
        machine_id: MachineId,
    ) -> IRMachineCommitteeList<T::AccountId, T::BlockNumber> {
        Self::machine_committee(machine_id)
    }

    pub fn get_committee_machine_list(committee: T::AccountId) -> IRCommitteeMachineList {
        Self::committee_machine(committee)
    }

    pub fn get_committee_ops(
        committee: T::AccountId,
        machine_id: MachineId,
    ) -> RpcIRCommitteeOps<T::BlockNumber, BalanceOf<T>> {
        let oc_committee_ops = Self::committee_ops(&committee, &machine_id);
        let committee_info = Self::machine_committee(&machine_id);

        RpcIRCommitteeOps {
            booked_time: committee_info.book_time,
            staked_dbc: oc_committee_ops.staked_dbc,
            verify_time: oc_committee_ops.verify_time,
            confirm_hash: oc_committee_ops.confirm_hash,
            hash_time: oc_committee_ops.hash_time,
            confirm_time: oc_committee_ops.confirm_time,
            machine_status: oc_committee_ops.machine_status,
            machine_info: oc_committee_ops.machine_info,
        }
    }
}

impl<T: Config> Pallet<T> {
    pub fn get_rent_order(
        rent_id: RentOrderId,
    ) -> IRRentOrderDetail<T::AccountId, T::BlockNumber, BalanceOf<T>> {
        Self::rent_order(&rent_id)
    }

    pub fn get_rent_list(renter: T::AccountId) -> Vec<RentOrderId> {
        Self::user_rented(&renter)
    }

    pub fn is_machine_renter(machine_id: MachineId, renter: T::AccountId) -> bool {
        let machine_order = Self::machine_rent_order(machine_id);

        for order_id in machine_order.rent_order {
            let rent_order = Self::rent_order(order_id);

            if rent_order.renter == renter {
                return true
            }
        }
        false
    }

    pub fn get_machine_rent_id(machine_id: MachineId) -> IRMachineGPUOrder {
        Self::machine_rent_order(machine_id)
    }
}
