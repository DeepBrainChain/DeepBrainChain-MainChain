#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use codec::{Decode, Encode};
use generic_func::{ItemList, MachineId};
use sp_runtime::{traits::Saturating, RuntimeDebug};
use sp_std::vec::Vec;

/// stash account overview self-status
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct StashMachine<Balance> {
    /// All machines bonded to stash account, if machine is offline,
    /// rm from this field after 150 Eras for linear release
    pub total_machine: Vec<MachineId>,
    /// Machines, that is in passed committee verification
    pub online_machine: Vec<MachineId>,
    /// Total grades of all online machine, inflation(for multiple GPU of one stash / reward by rent) is counted
    pub total_calc_points: u64,
    /// Total online gpu num, will be added after online, reduced after offline
    pub total_gpu_num: u64,
    /// Total rented gpu
    pub total_rented_gpu: u64,
    /// All reward stash account got, locked reward included
    pub total_earned_reward: Balance,
    /// Sum of all claimed reward
    pub total_claimed_reward: Balance,
    /// Reward can be claimed now
    pub can_claim_reward: Balance,
    /// How much has been earned by rent before Galaxy is on
    pub total_rent_fee: Balance,
    /// How much has been burned after Galaxy is on
    pub total_burn_fee: Balance,
}

impl<B: Saturating + Copy> StashMachine<B> {
    // 新加入的机器，放到total_machine中
    pub fn new_bonding(&mut self, machine_id: MachineId) {
        ItemList::add_item(&mut self.total_machine, machine_id);
    }

    pub fn change_rent_fee(&mut self, amount: B, is_burn: bool) {
        if is_burn {
            self.total_burn_fee = self.total_burn_fee.saturating_add(amount);
        } else {
            self.total_rent_fee = self.total_rent_fee.saturating_add(amount);
        }
    }
}
