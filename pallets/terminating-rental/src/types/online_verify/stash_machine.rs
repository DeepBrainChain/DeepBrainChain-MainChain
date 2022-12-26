#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use codec::{Decode, Encode};
use dbc_support::{ItemList, MachineId};
use sp_runtime::RuntimeDebug;
use sp_std::vec::Vec;

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct IRStashMachine<Balance> {
    /// All machines bonded to stash account, if machine is offline,
    /// rm from this field after 150 Eras for linear release
    pub total_machine: Vec<MachineId>,
    /// Machines, that is in passed committee verification
    pub online_machine: Vec<MachineId>,
    /// Total grades of all online machine, inflation(for multiple GPU of one stash / reward by
    /// rent) is counted
    pub total_calc_points: u64,
    /// Total online gpu num, will be added after online, reduced after offline
    pub total_gpu_num: u64,
    /// Total rented gpu
    pub total_rented_gpu: u64,
    // /// All reward stash account got, locked reward included
    // pub total_earned_reward: Balance,
    // /// Sum of all claimed reward
    // pub total_claimed_reward: Balance,
    // /// Reward can be claimed now
    // pub can_claim_reward: Balance,
    /// How much has been earned by rent before Galaxy is on
    pub total_rent_fee: Balance,
    // /// How much has been burned after Galaxy is on
    // pub total_burn_fee: Balance,
}

impl<Balance> IRStashMachine<Balance> {
    // 新加入的机器，放到total_machine中
    pub fn bond_machine(&mut self, machine_id: MachineId) {
        ItemList::add_item(&mut self.total_machine, machine_id);
    }

    // 拒绝machine上线
    pub fn refuse_machine(&mut self, machine_id: &MachineId) {
        ItemList::rm_item(&mut self.total_machine, machine_id);
    }

    // machine通过了委员会验证
    pub fn machine_online(&mut self, machine_id: MachineId, gpu_num: u32, calc_point: u64) {
        ItemList::add_item(&mut self.online_machine, machine_id.clone());
        self.total_gpu_num = self.total_gpu_num.saturating_add(gpu_num as u64);
        self.total_calc_points = self.total_calc_points.saturating_add(calc_point);
    }

    pub fn machine_exit(
        &mut self,
        machine_id: MachineId,
        calc_point: u64,
        gpu_count: u64,
        rented_gpu_count: u64,
    ) {
        ItemList::rm_item(&mut self.total_machine, &machine_id);
        ItemList::rm_item(&mut self.online_machine, &machine_id);
        self.total_calc_points = self.total_calc_points.saturating_sub(calc_point);
        self.total_gpu_num = self.total_gpu_num.saturating_sub(gpu_count);
        self.total_rented_gpu = self.total_rented_gpu.saturating_sub(rented_gpu_count);
    }
}