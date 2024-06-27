#[cfg(feature = "std")]
use dbc_support::rpc_types::RpcText;
use dbc_support::{
    machine_type::CommitteeUploadInfo,
    verify_online::{OCCommitteeMachineList, OCMachineStatus as VerifyMachineStatus},
};
use parity_scale_codec::{Decode, Encode};
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
    #[cfg_attr(feature = "std", serde(with = "dbc_support::rpc_types::serde_hash"))]
    pub confirm_hash: [u8; 16],
    pub hash_time: BlockNumber,
    pub confirm_time: BlockNumber, // 委员会提交raw信息的时间
    pub machine_status: VerifyMachineStatus,
    pub machine_info: CommitteeUploadInfo,
}

#[cfg(feature = "std")]
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct RpcOCCommitteeMachineList {
    pub booked_machine: Vec<RpcText>,
    pub hashed_machine: Vec<RpcText>,
    pub confirmed_machine: Vec<RpcText>,
    pub online_machine: Vec<RpcText>,
}

#[cfg(feature = "std")]
impl From<OCCommitteeMachineList> for RpcOCCommitteeMachineList {
    fn from(info: OCCommitteeMachineList) -> Self {
        Self {
            booked_machine: info.booked_machine.iter().map(|id| id.into()).collect(),
            hashed_machine: info.hashed_machine.iter().map(|id| id.into()).collect(),
            confirmed_machine: info.confirmed_machine.iter().map(|id| id.into()).collect(),
            online_machine: info.online_machine.iter().map(|id| id.into()).collect(),
        }
    }
}
