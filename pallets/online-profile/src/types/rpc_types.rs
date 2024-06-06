use parity_scale_codec::{Decode, Encode};
#[cfg(feature = "std")]
use dbc_support::rpc_types::{serde_text, RpcText};
// use generic_func::rpc_types::RpcText;
use crate::{LiveMachine, MachineId, StakerCustomizeInfo};
use dbc_support::{
    machine_info::MachineInfo,
    machine_type::{CommitteeUploadInfo, Latitude, Longitude, MachineInfoDetail, MachineStatus},
    verify_online::StashMachine,
    EraIndex,
};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_core::H256;
use sp_std::vec::Vec;
#[cfg(feature = "std")]
use std::convert::From;

#[cfg(feature = "std")]
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct RpcMachineInfo<AccountId: Ord, BlockNumber, Balance> {
    pub controller: AccountId,
    pub machine_stash: AccountId,
    pub renters: Vec<AccountId>,
    pub last_machine_restake: BlockNumber,
    pub bonding_height: BlockNumber,
    pub online_height: BlockNumber,
    pub last_online_height: BlockNumber,
    pub init_stake_per_gpu: Balance,
    pub stake_amount: Balance,
    pub machine_status: MachineStatus<BlockNumber, AccountId>,
    pub total_rented_duration: BlockNumber,
    pub total_rented_times: u64,
    pub total_rent_fee: Balance,
    pub total_burn_fee: Balance,
    pub machine_info_detail: RpcMachineInfoDetail,
    pub reward_committee: Vec<AccountId>,
    pub reward_deadline: EraIndex,
}

#[cfg(feature = "std")]
impl<AccountId: Ord, BlockNumber, Balance> From<MachineInfo<AccountId, BlockNumber, Balance>>
    for RpcMachineInfo<AccountId, BlockNumber, Balance>
{
    fn from(info: MachineInfo<AccountId, BlockNumber, Balance>) -> Self {
        Self {
            controller: info.controller,
            machine_stash: info.machine_stash,
            renters: info.renters,
            last_machine_restake: info.last_machine_restake,
            bonding_height: info.bonding_height,
            online_height: info.online_height,
            last_online_height: info.last_online_height,
            init_stake_per_gpu: info.init_stake_per_gpu,
            stake_amount: info.stake_amount,
            machine_status: info.machine_status,
            total_rented_duration: info.total_rented_duration,
            total_rented_times: info.total_rented_times,
            total_rent_fee: info.total_rent_fee,
            total_burn_fee: info.total_burn_fee,
            machine_info_detail: info.machine_info_detail.into(),
            reward_committee: info.reward_committee,
            reward_deadline: info.reward_deadline,
        }
    }
}

#[cfg(feature = "std")]
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct RpcMachineInfoDetail {
    pub committee_upload_info: CommitteeUploadInfo,
    pub staker_customize_info: RpcStakerCustomizeInfo,
}

#[cfg(feature = "std")]
impl From<MachineInfoDetail> for RpcMachineInfoDetail {
    fn from(info: MachineInfoDetail) -> Self {
        Self {
            committee_upload_info: info.committee_upload_info,
            staker_customize_info: info.staker_customize_info.into(),
        }
    }
}

#[cfg(feature = "std")]
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct RpcStakerCustomizeInfo {
    pub server_room: H256,
    pub upload_net: u64,
    pub download_net: u64,
    pub longitude: Longitude,
    pub latitude: Latitude,
    pub telecom_operators: Vec<RpcText>,
}

#[cfg(feature = "std")]
impl From<StakerCustomizeInfo> for RpcStakerCustomizeInfo {
    fn from(info: StakerCustomizeInfo) -> Self {
        Self {
            server_room: info.server_room,
            upload_net: info.upload_net,
            download_net: info.download_net,
            longitude: info.longitude,
            latitude: info.latitude,
            telecom_operators: info
                .telecom_operators
                .iter()
                .map(|telecom| telecom.into())
                .collect(),
        }
    }
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct StakerInfo<Balance, BlockNumber, AccountId> {
    pub stash_statistic: StashMachine<Balance>,
    pub bonded_machines: Vec<MachineBriefInfo<BlockNumber, AccountId>>,
}

#[cfg(feature = "std")]
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct RpcStakerInfo<Balance, BlockNumber, AccountId> {
    pub stash_statistic: RpcStashMachine<Balance>,
    pub bonded_machines: Vec<MachineBriefInfo<BlockNumber, AccountId>>,
}

#[cfg(feature = "std")]
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct RpcStashMachine<Balance> {
    pub total_machine: Vec<RpcText>,
    pub online_machine: Vec<RpcText>,
    pub total_calc_points: u64,
    pub total_gpu_num: u64,
    pub total_rented_gpu: u64,
    pub total_earned_reward: Balance,
    pub total_claimed_reward: Balance,
    pub can_claim_reward: Balance,
    pub total_rent_fee: Balance,
    pub total_burn_fee: Balance,
}

#[cfg(feature = "std")]
impl<Balance> From<StashMachine<Balance>> for RpcStashMachine<Balance> {
    fn from(stash_machine: StashMachine<Balance>) -> Self {
        Self {
            total_machine: stash_machine
                .total_machine
                .iter()
                .map(|machine_id| machine_id.into())
                .collect(),
            online_machine: stash_machine
                .online_machine
                .iter()
                .map(|machine_id| machine_id.into())
                .collect(),
            total_calc_points: stash_machine.total_calc_points,
            total_gpu_num: stash_machine.total_gpu_num,
            total_rented_gpu: stash_machine.total_rented_gpu,
            total_earned_reward: stash_machine.total_earned_reward,
            total_claimed_reward: stash_machine.total_claimed_reward,
            can_claim_reward: stash_machine.can_claim_reward,
            total_rent_fee: stash_machine.total_rent_fee,
            total_burn_fee: stash_machine.total_burn_fee,
        }
    }
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct MachineBriefInfo<BlockNumber, AccountId> {
    #[cfg_attr(feature = "std", serde(with = "serde_text"))]
    pub machine_id: MachineId,
    pub gpu_num: u32,
    pub calc_point: u64,
    pub machine_status: MachineStatus<BlockNumber, AccountId>,
}

#[cfg(feature = "std")]
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct RpcLiveMachine {
    /// After call bond_machine, machine is stored waitting for controller add info
    pub bonding_machine: Vec<RpcText>,
    /// Machines, have added info, waiting for distributing to committee
    pub confirmed_machine: Vec<RpcText>,
    /// Machines, have booked by committees
    pub booked_machine: Vec<RpcText>,
    /// Verified by committees, and is online to get rewrad
    pub online_machine: Vec<RpcText>,
    /// Verified by committees, but stake is not enough:
    /// One gpu is staked first time call bond_machine, after committee verification,
    /// actual stake is calced by actual gpu num
    pub fulfilling_machine: Vec<RpcText>,
    /// Machines, refused by committee
    pub refused_machine: Vec<RpcText>,
    /// Machines, is rented
    pub rented_machine: Vec<RpcText>,
    /// Machines, called offline by controller
    pub offline_machine: Vec<RpcText>,
    /// Machines, want to change hardware info, but refused by committee
    pub refused_mut_hardware_machine: Vec<RpcText>,
}

#[cfg(feature = "std")]
impl From<LiveMachine> for RpcLiveMachine {
    fn from(live_machine: LiveMachine) -> Self {
        Self {
            bonding_machine: live_machine
                .bonding_machine
                .iter()
                .map(|machine_id| machine_id.into())
                .collect(),
            confirmed_machine: live_machine
                .confirmed_machine
                .iter()
                .map(|machine_id| machine_id.into())
                .collect(),
            booked_machine: live_machine
                .booked_machine
                .iter()
                .map(|machine_id| machine_id.into())
                .collect(),
            online_machine: live_machine
                .online_machine
                .iter()
                .map(|machine_id| machine_id.into())
                .collect(),
            fulfilling_machine: live_machine
                .fulfilling_machine
                .iter()
                .map(|machine_id| machine_id.into())
                .collect(),
            refused_machine: live_machine
                .refused_machine
                .iter()
                .map(|machine_id| machine_id.into())
                .collect(),
            rented_machine: live_machine
                .rented_machine
                .iter()
                .map(|machine_id| machine_id.into())
                .collect(),
            offline_machine: live_machine
                .offline_machine
                .iter()
                .map(|machine_id| machine_id.into())
                .collect(),
            refused_mut_hardware_machine: live_machine
                .refused_mut_hardware_machine
                .iter()
                .map(|machine_id| machine_id.into())
                .collect(),
        }
    }
}
