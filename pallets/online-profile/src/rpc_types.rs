use super::MachineId;
use crate::{MachineInfoDetail, MachineStatus};
use codec::{Decode, Encode};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_std::vec::Vec;

// 系统统计信息，提供给RPC
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct RpcSysInfo<Balance> {
    pub total_gpu_num: u64,
    pub total_rented_gpu: u64,
    pub total_staker: u64,
    pub total_calc_points: u64,
    pub total_stake: Balance,
    pub total_rent_fee: Balance,
    pub total_burn_fee: Balance,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct StakerInfo<Balance, BlockNumber> {
    pub calc_points: u64,
    pub gpu_num: u64,
    pub total_reward: Balance,
    pub bonded_machines: Vec<MachineBriefInfo<BlockNumber>>,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct MachineBriefInfo<BlockNumber> {
    pub machine_id: MachineId,
    pub gpu_num: u32,
    pub calc_point: u64,
    pub machine_status: MachineStatus<BlockNumber>,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct RPCMachineInfo<AccountId, BlockNumber, Balance> {
    pub machine_owner: AccountId,
    pub bonding_height: BlockNumber,
    pub stake_amount: Balance,
    pub machine_status: MachineStatus<BlockNumber>,
    pub total_rented_duration: u64,
    pub total_rented_times: u64,
    pub total_rent_fee: Balance,
    pub total_burn_fee: Balance,
    pub machine_info_detail: MachineInfoDetail,
    pub reward_committee: Vec<AccountId>,
    pub reward_deadline: BlockNumber,
}
