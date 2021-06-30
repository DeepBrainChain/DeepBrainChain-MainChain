#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use crate::{CommitteeUploadInfo, LCMachineStatus};
use codec::{Decode, Encode};
use sp_std::vec::Vec;

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct RpcLCCommitteeOps<BlockNumber, Balance> {
    pub booked_time: BlockNumber,
    pub staked_dbc: Balance, // FIXME: return Balance type
    pub verify_time: Vec<BlockNumber>,
    pub confirm_hash: [u8; 16],
    pub hash_time: BlockNumber,
    pub confirm_time: BlockNumber, // 委员会提交raw信息的时间
    pub machine_status: LCMachineStatus,
    pub machine_info: CommitteeUploadInfo,
}
