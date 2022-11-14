use crate::{IRCommitteeMachineList, IRCommitteeUploadInfo, IRVerifyMachineStatus};

use codec::{Decode, Encode};
#[cfg(feature = "std")]
use generic_func::RpcText;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_std::vec::Vec;

// for RPC
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct RpcIRCommitteeOps<BlockNumber, Balance> {
    pub booked_time: BlockNumber,
    pub staked_dbc: Balance,
    pub verify_time: Vec<BlockNumber>,
    #[cfg_attr(feature = "std", serde(with = "generic_func::rpc_types::serde_hash"))]
    pub confirm_hash: [u8; 16],
    pub hash_time: BlockNumber,
    pub confirm_time: BlockNumber, // 委员会提交raw信息的时间
    pub machine_status: IRVerifyMachineStatus,
    pub machine_info: IRCommitteeUploadInfo,
}

#[cfg(feature = "std")]
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct RpcIRCommitteeMachineList {
    pub booked_machine: Vec<RpcText>,
    pub hashed_machine: Vec<RpcText>,
    pub confirmed_machine: Vec<RpcText>,
    pub online_machine: Vec<RpcText>,
}

#[cfg(feature = "std")]
impl From<IRCommitteeMachineList> for RpcIRCommitteeMachineList {
    fn from(info: IRCommitteeMachineList) -> Self {
        Self {
            booked_machine: info.booked_machine.iter().map(|id| id.into()).collect(),
            hashed_machine: info.hashed_machine.iter().map(|id| id.into()).collect(),
            confirmed_machine: info.confirmed_machine.iter().map(|id| id.into()).collect(),
            online_machine: info.online_machine.iter().map(|id| id.into()).collect(),
        }
    }
}
