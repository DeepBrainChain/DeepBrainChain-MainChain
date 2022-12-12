use codec::{Decode, Encode};
use dbc_support::MachineId;
use generic_func::ItemList;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::RuntimeDebug;
use sp_std::vec::Vec;

use crate::MachineStatus;

/// MachineList in online module
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct LiveMachine {
    /// After call bond_machine, machine is stored waitting for controller add info
    pub bonding_machine: Vec<MachineId>,
    /// Machines, have added info, waiting for distributing to committee
    pub confirmed_machine: Vec<MachineId>,
    /// Machines, have booked by committees
    pub booked_machine: Vec<MachineId>,
    /// Verified by committees, and is online to get rewrad
    pub online_machine: Vec<MachineId>,
    /// Verified by committees, but stake is not enough:
    /// One gpu is staked first time call bond_machine, after committee verification,
    /// actual stake is calced by actual gpu num
    pub fulfilling_machine: Vec<MachineId>,
    /// Machines, refused by committee
    pub refused_machine: Vec<MachineId>,
    /// Machines, is rented
    pub rented_machine: Vec<MachineId>,
    /// Machines, called offline by controller
    pub offline_machine: Vec<MachineId>,
    /// Machines, want to change hardware info, but refused by committee
    pub refused_mut_hardware_machine: Vec<MachineId>,
}

impl LiveMachine {
    // 添加到LiveMachine的bonding_machine字段
    pub fn new_bonding(&mut self, machine_id: MachineId) {
        ItemList::add_item(&mut self.bonding_machine, machine_id);
    }

    pub fn add_server_room_info<BlockNumber, AccountId>(
        &mut self,
        machine_id: MachineId,
        machine_status: MachineStatus<BlockNumber, AccountId>,
    ) {
        // 当是第一次上线时添加机房信息
        if matches!(
            machine_status,
            MachineStatus::AddingCustomizeInfo | MachineStatus::StakerReportOffline(..)
        ) {
            ItemList::rm_item(&mut self.bonding_machine, &machine_id);
            ItemList::add_item(&mut self.confirmed_machine, machine_id);
        }
    }

    pub fn offline_to_change_hardware(&mut self, machine_id: MachineId) {
        ItemList::rm_item(&mut self.online_machine, &machine_id);
        ItemList::add_item(&mut self.bonding_machine, machine_id);
    }

    // 机器从online/rented状态，暂时下线
    pub fn machine_offline(&mut self, machine_id: MachineId) {
        ItemList::rm_item(&mut self.online_machine, &machine_id);
        ItemList::rm_item(&mut self.rented_machine, &machine_id);
        ItemList::add_item(&mut self.offline_machine, machine_id);
    }

    pub fn machine_exit(&mut self, machine_id: &MachineId) {
        ItemList::rm_item(&mut self.online_machine, machine_id);
    }
}
