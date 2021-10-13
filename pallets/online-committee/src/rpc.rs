use crate::{
    types::OCMachineCommitteeList, BalanceOf, CommitteeUploadInfo, Config, OCCommitteeMachineList, OCMachineStatus,
    Pallet,
};
use codec::{Decode, Encode};
use generic_func::MachineId;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_std::{str, vec::Vec};

// for RPC
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct RpcOCCommitteeOps<BlockNumber, Balance> {
    pub booked_time: BlockNumber,
    pub staked_dbc: Balance,
    pub verify_time: Vec<BlockNumber>,
    pub confirm_hash: [u8; 16],
    pub hash_time: BlockNumber,
    pub confirm_time: BlockNumber, // 委员会提交raw信息的时间
    pub machine_status: OCMachineStatus,
    pub machine_info: CommitteeUploadInfo,
}

// RPC
impl<T: Config> Pallet<T> {
    pub fn get_machine_committee_list(machine_id: MachineId) -> OCMachineCommitteeList<T::AccountId, T::BlockNumber> {
        Self::machine_committee(machine_id)
    }

    pub fn get_committee_machine_list(committee: T::AccountId) -> OCCommitteeMachineList {
        Self::committee_machine(committee)
    }

    pub fn get_committee_ops(
        committee: T::AccountId,
        machine_id: MachineId,
    ) -> RpcOCCommitteeOps<T::BlockNumber, BalanceOf<T>> {
        let oc_committee_ops = Self::committee_ops(&committee, &machine_id);
        let committee_info = Self::machine_committee(&machine_id);

        RpcOCCommitteeOps {
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
