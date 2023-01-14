#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use crate::CustomErr;
use codec::{Decode, Encode};
use dbc_support::{
    machine_type::{Latitude, Longitude, MachineInfoDetail, MachineStatus, StakerCustomizeInfo},
    EraIndex, MachineId,
};
use sp_runtime::{traits::Saturating, RuntimeDebug};
use sp_std::vec::Vec;

/// All details of a machine
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct MachineInfo<AccountId: Ord, BlockNumber, Balance> {
    /// Who can control this machine
    pub controller: AccountId,
    /// Who own this machine and will get machine's reward
    pub machine_stash: AccountId,
    /// Last machine renter
    pub renters: Vec<AccountId>,
    /// Every 365 days machine can restake(For token price maybe changed)
    pub last_machine_restake: BlockNumber,
    /// When controller bond this machine
    pub bonding_height: BlockNumber,
    /// When machine is passed verification and is online
    pub online_height: BlockNumber,
    /// Last time machine is online
    /// (When first online; Rented -> Online, Offline -> Online e.t.)
    pub last_online_height: BlockNumber,
    /// When first bond_machine, record how much should stake per GPU
    pub init_stake_per_gpu: Balance,
    /// How much machine staked
    pub stake_amount: Balance,
    /// Status of machine
    pub machine_status: MachineStatus<BlockNumber, AccountId>,
    /// How long machine has been rented(will be update after one rent is end)
    /// NOTE: 单位从天改为BlockNumber
    pub total_rented_duration: BlockNumber,
    /// How many times machine has been rented
    pub total_rented_times: u64,
    /// How much rent fee machine has earned for rented(before Galaxy is ON)
    pub total_rent_fee: Balance,
    /// How much rent fee is burn after Galaxy is ON
    pub total_burn_fee: Balance,
    /// Machine's hardware info
    pub machine_info_detail: MachineInfoDetail,
    /// Committees, verified machine and will be rewarded in the following days.
    /// (In next 2 years after machine is online, get 1% unlocked reward)
    pub reward_committee: Vec<AccountId>,
    /// When reward will be over for committees
    pub reward_deadline: EraIndex,
}

impl<AccountId, BlockNumber, Balance> MachineInfo<AccountId, BlockNumber, Balance>
where
    AccountId: Ord + Default,
    BlockNumber: Default,
    Balance: Copy + Default + Saturating,
{
    pub fn new_bonding(
        controller: AccountId,
        stash: AccountId,
        now: BlockNumber,
        init_stake_per_gpu: Balance,
    ) -> Self {
        Self {
            controller,
            machine_stash: stash,
            bonding_height: now,
            init_stake_per_gpu,
            stake_amount: init_stake_per_gpu,
            machine_status: MachineStatus::AddingCustomizeInfo,
            ..Default::default()
        }
    }

    pub fn can_add_server_room(&self, who: &AccountId) -> Result<(), CustomErr> {
        // 检查当前机器状态是否允许
        if !matches!(
            self.machine_status,
            MachineStatus::AddingCustomizeInfo |
                MachineStatus::DistributingOrder |
                MachineStatus::CommitteeVerifying |
                MachineStatus::CommitteeRefused(..) |
                MachineStatus::WaitingFulfill |
                MachineStatus::StakerReportOffline(..)
        ) {
            return Err(CustomErr::NotAllowedChangeMachineInfo)
        }

        if &self.controller != who {
            return Err(CustomErr::NotMachineController)
        }
        Ok(())
    }

    pub fn add_server_room_info(&mut self, server_room_info: StakerCustomizeInfo) {
        self.machine_info_detail.staker_customize_info = server_room_info;
        if matches!(self.machine_status, MachineStatus::AddingCustomizeInfo) {
            self.machine_status = MachineStatus::DistributingOrder;
        }
    }

    pub fn update_rent_fee(&mut self, amount: Balance, is_burn: bool) {
        if is_burn {
            self.total_burn_fee = self.total_burn_fee.saturating_add(amount);
        } else {
            self.total_rent_fee = self.total_rent_fee.saturating_add(amount);
        }
    }

    /// Return longitude of machine
    pub fn longitude(&self) -> &Longitude {
        &self.machine_info_detail.staker_customize_info.longitude
    }

    /// Return latitude of machine
    pub fn latitude(&self) -> &Latitude {
        &self.machine_info_detail.staker_customize_info.latitude
    }

    /// Return machine total gpu_num
    pub fn gpu_num(&self) -> u32 {
        self.machine_info_detail.committee_upload_info.gpu_num
    }

    /// Return `calc point` of machine
    pub fn calc_point(&self) -> u64 {
        self.machine_info_detail.committee_upload_info.calc_point
    }

    pub fn machine_id(&self) -> MachineId {
        self.machine_info_detail.committee_upload_info.machine_id.clone()
    }

    pub fn is_controller(&self, who: AccountId) -> bool {
        self.controller == who
    }

    pub fn is_online(&self) -> bool {
        matches!(self.machine_status, MachineStatus::Online)
    }
}