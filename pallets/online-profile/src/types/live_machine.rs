use codec::{Decode, Encode};
use generic_func::{ItemList, MachineId};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::RuntimeDebug;
use sp_std::vec::Vec;

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
    pub fn is_bonding(&self, machine_id: &MachineId) -> bool {
        self.bonding_machine.binary_search(machine_id).is_ok()
    }
    pub fn is_confirmed(&self, machine_id: &MachineId) -> bool {
        self.confirmed_machine.binary_search(machine_id).is_ok()
    }
    pub fn is_booked(&self, machine_id: &MachineId) -> bool {
        self.booked_machine.binary_search(machine_id).is_ok()
    }
    pub fn is_online(&self, machine_id: &MachineId) -> bool {
        self.online_machine.binary_search(machine_id).is_ok()
    }
    pub fn is_fulfilling(&self, machine_id: &MachineId) -> bool {
        self.fulfilling_machine.binary_search(machine_id).is_ok()
    }
    pub fn is_refused(&self, machine_id: &MachineId) -> bool {
        self.refused_machine.binary_search(machine_id).is_ok()
    }
    pub fn is_rented(&self, machine_id: &MachineId) -> bool {
        self.rented_machine.binary_search(machine_id).is_ok()
    }
    pub fn is_offline(&self, machine_id: &MachineId) -> bool {
        self.offline_machine.binary_search(machine_id).is_ok()
    }
    pub fn is_refused_mut_hardware(&self, machine_id: &MachineId) -> bool {
        self.refused_machine.binary_search(machine_id).is_ok()
    }

    /// Check if machine_id exist
    pub fn machine_id_exist(&self, machine_id: &MachineId) -> bool {
        self.is_bonding(machine_id)
            || self.is_confirmed(machine_id)
            || self.is_booked(machine_id)
            || self.is_online(machine_id)
            || self.is_fulfilling(machine_id)
            || self.is_refused(machine_id)
            || self.is_rented(machine_id)
            || self.is_offline(machine_id)
            || self.is_refused_mut_hardware(machine_id)
    }

    // 添加到LiveMachine的bonding_machine字段
    pub fn new_bonding(&mut self, machine_id: MachineId) {
        ItemList::add_item(&mut self.bonding_machine, machine_id);
    }
}
