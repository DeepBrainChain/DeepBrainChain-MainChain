use codec::{Decode, Encode};
#[cfg(feature = "std")]
use generic_func::{rpc_types::serde_text, RpcText};
// use generic_func::rpc_types::RpcText;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_std::vec::Vec;
#[cfg(feature = "std")]
use std::convert::From;

use crate::{LiveMachine, MachineId, MachineStatus, StashMachine};

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
            total_machine: stash_machine.total_machine.iter().map(|machine_id| machine_id.into()).collect(),
            online_machine: stash_machine.online_machine.iter().map(|machine_id| machine_id.into()).collect(),
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
            bonding_machine: live_machine.bonding_machine.iter().map(|machine_id| machine_id.into()).collect(),
            confirmed_machine: live_machine.confirmed_machine.iter().map(|machine_id| machine_id.into()).collect(),
            booked_machine: live_machine.booked_machine.iter().map(|machine_id| machine_id.into()).collect(),
            online_machine: live_machine.online_machine.iter().map(|machine_id| machine_id.into()).collect(),
            fulfilling_machine: live_machine.fulfilling_machine.iter().map(|machine_id| machine_id.into()).collect(),
            refused_machine: live_machine.refused_machine.iter().map(|machine_id| machine_id.into()).collect(),
            rented_machine: live_machine.rented_machine.iter().map(|machine_id| machine_id.into()).collect(),
            offline_machine: live_machine.offline_machine.iter().map(|machine_id| machine_id.into()).collect(),
            refused_mut_hardware_machine: live_machine
                .refused_mut_hardware_machine
                .iter()
                .map(|machine_id| machine_id.into())
                .collect(),
        }
    }
}
