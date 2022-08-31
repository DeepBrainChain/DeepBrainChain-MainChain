use codec::{Decode, Encode};
#[cfg(feature = "std")]
use generic_func::RpcText;
// use generic_func::rpc_types::RpcText;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_std::vec::Vec;
#[cfg(feature = "std")]
use std::convert::From;

use crate::LiveMachine;

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
