use crate::{ItemList, MachineId};
use codec::{Decode, Encode};
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::RuntimeDebug;
use sp_std::vec::Vec;

/// MachineList in online module
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug, TypeInfo)]
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
    // NOTE: NOT used in terminating-rental pallet
    pub fulfilling_machine: Vec<MachineId>,
    /// Machines, refused by committee
    pub refused_machine: Vec<MachineId>,
    /// Machines, is rented
    pub rented_machine: Vec<MachineId>,
    /// Machines, called offline by controller
    pub offline_machine: Vec<MachineId>,
    /// Machines, want to change hardware info, but refused by committee
    // NOTE: NOT used in terminating-rental pallet
    pub refused_mut_hardware_machine: Vec<MachineId>,
}

// Used in terminating-rental pallet.
impl LiveMachine {
    // 添加到LiveMachine的bonding_machine字段
    pub fn on_bonding(&mut self, machine_id: MachineId) {
        ItemList::add_item(&mut self.bonding_machine, machine_id);
    }

    pub fn on_add_server_room(&mut self, machine_id: MachineId) {
        // 当是第一次上线时添加机房信息，或者主动下线更改硬件配置时
        // 机器ID都会在live_machine.bonding_machine中
        if self.bonding_machine.binary_search(&machine_id).is_ok() {
            ItemList::rm_item(&mut self.bonding_machine, &machine_id);
            ItemList::add_item(&mut self.confirmed_machine, machine_id);
        }
    }

    pub fn on_offline_change_hardware(&mut self, machine_id: MachineId) {
        ItemList::rm_item(&mut self.online_machine, &machine_id);
        ItemList::add_item(&mut self.bonding_machine, machine_id);
    }

    // 机器从online/rented状态，暂时下线
    pub fn on_offline(&mut self, machine_id: MachineId) {
        ItemList::rm_item(&mut self.online_machine, &machine_id);
        ItemList::rm_item(&mut self.rented_machine, &machine_id);
        ItemList::add_item(&mut self.offline_machine, machine_id);
    }

    pub fn on_exit(&mut self, machine_id: &MachineId) {
        ItemList::rm_item(&mut self.online_machine, machine_id);
    }
}

// TODO: Used in terminating-rental pallet.
// TODO: Should be merged into onlne-profile pallet
impl LiveMachine {
    // 添加到LiveMachine的bonding_machine字段
    pub fn bond_machine(&mut self, machine_id: MachineId) {
        ItemList::add_item(&mut self.bonding_machine, machine_id);
    }

    pub fn add_machine_info(&mut self, machine_id: MachineId) {
        ItemList::rm_item(&mut self.bonding_machine, &machine_id);
        ItemList::add_item(&mut self.confirmed_machine, machine_id);
    }

    // 拒绝机器上线请求
    pub fn refuse_machine(&mut self, machine_id: MachineId) {
        ItemList::rm_item(&mut self.booked_machine, &machine_id);
        ItemList::add_item(&mut self.refused_machine, machine_id);
    }

    // 机器被重新派单
    pub fn revert_book(&mut self, machine_id: MachineId) {
        ItemList::rm_item(&mut self.booked_machine, &machine_id);
        ItemList::add_item(&mut self.confirmed_machine, machine_id);
    }

    pub fn machine_exit(&mut self, machine_id: &MachineId) {
        ItemList::rm_item(&mut self.online_machine, machine_id);
        ItemList::rm_item(&mut self.rented_machine, machine_id)
    }

    // 机器从online/rented状态，暂时下线
    pub fn machine_offline(&mut self, machine_id: MachineId) {
        ItemList::rm_item(&mut self.online_machine, &machine_id);
        ItemList::rm_item(&mut self.rented_machine, &machine_id);
        ItemList::add_item(&mut self.offline_machine, machine_id);
    }

    // 从记录中清除该机器ID
    pub fn clean(&mut self, machine_id: &MachineId) {
        ItemList::rm_item(&mut self.bonding_machine, machine_id);
        ItemList::rm_item(&mut self.confirmed_machine, machine_id);
        ItemList::rm_item(&mut self.booked_machine, machine_id);
        ItemList::rm_item(&mut self.online_machine, machine_id);
        ItemList::rm_item(&mut self.fulfilling_machine, machine_id);
        ItemList::rm_item(&mut self.refused_machine, machine_id);
        ItemList::rm_item(&mut self.rented_machine, machine_id);
        ItemList::rm_item(&mut self.offline_machine, machine_id);
        ItemList::rm_item(&mut self.refused_mut_hardware_machine, machine_id);
    }
}
