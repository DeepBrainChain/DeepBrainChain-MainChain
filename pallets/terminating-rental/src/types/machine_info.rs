#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use codec::{Decode, Encode};
use dbc_support::machine_type::{CommitteeUploadInfo, StakerCustomizeInfo};
use frame_support::ensure;
use sp_runtime::{
    traits::{Saturating, Zero},
    RuntimeDebug,
};
use sp_std::{prelude::Box, vec::Vec};

use crate::{CustomErr, IRSlashReason};

/// All details of a machine
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct IRMachineInfo<AccountId: Ord, BlockNumber, Balance> {
    // /// Who can control this machine
    // pub controller: AccountId,
    /// Who own this machine and will get machine's reward
    pub machine_stash: AccountId,
    /// Last machine renter
    pub renters: Vec<AccountId>,
    // /// Every 365 days machine can restake(For token price maybe changed)
    // pub last_machine_restake: BlockNumber,
    /// When controller bond this machine
    pub bonding_height: BlockNumber,
    /// When machine is passed verification and is online
    pub online_height: BlockNumber,
    /// Last time machine is online
    /// (When first online; Rented -> Online, Offline -> Online e.t.)
    pub last_online_height: BlockNumber,
    // /// When first bond_machine, record how much should stake per GPU
    // pub init_stake_per_gpu: Balance,
    /// How much machine staked
    pub stake_amount: Balance,
    /// Status of machine
    pub machine_status: IRMachineStatus<BlockNumber, AccountId>,
    /// How long machine has been rented(will be update after one rent is end)
    /// NOTE: 单位从天改为BlockNumber
    pub total_rented_duration: BlockNumber,
    /// How many times machine has been rented
    pub total_rented_times: u64,
    /// How much rent fee machine has earned for rented(before Galaxy is ON)
    pub total_rent_fee: Balance,
    // /// How much rent fee is burn after Galaxy is ON
    // pub total_burn_fee: Balance,
    /// Machine's hardware info
    pub machine_info_detail: IRMachineInfoDetail,
    /// Committees, verified machine and will be rewarded in the following days.
    /// (After machine is online, get 1% rent fee)
    pub reward_committee: Vec<AccountId>,
    // /// When reward will be over for committees
    // pub reward_deadline: EraIndex,
}

/// All kind of status of a machine
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum IRMachineStatus<BlockNumber, AccountId> {
    /// After controller bond machine; means waiting for submit machine info
    AddingCustomizeInfo,
    /// After submit machine info; will waiting to distribute order to committees
    DistributingOrder,
    /// After distribute to committees, should take time to verify hardware
    CommitteeVerifying,
    /// Machine is refused by committees, so cannot be online
    CommitteeRefused(BlockNumber),
    /// After committee agree machine online, stake should be paied depend on gpu num
    WaitingFulfill,
    /// Machine online successfully
    Online,
    /// Controller offline machine
    StakerReportOffline(BlockNumber),
    /// Reporter report machine is fault, so machine go offline (SlashReason, StatusBeforeOffline,
    /// Reporter, Committee)
    ReporterReportOffline(IRSlashReason<BlockNumber>, Box<Self>, AccountId, Vec<AccountId>),

    /// Machine is rented, and waiting for renter to confirm virtual machine is created
    /// successfully NOTE: 该状态被弃用。
    /// 机器上线后，正常情况下，只有Rented和Online两种状态
    /// 对DBC来说要查询某个用户是否能创建虚拟机，到rent_machine中查看machine对应的租用人即可
    Creating,
    /// Machine is rented now
    Rented,
    /// Machine is exit
    Exit,
}

impl<BlockNumber, AccountId> Default for IRMachineStatus<BlockNumber, AccountId> {
    fn default() -> Self {
        IRMachineStatus::AddingCustomizeInfo
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct IRMachineInfoDetail {
    pub committee_upload_info: CommitteeUploadInfo,
    pub staker_customize_info: StakerCustomizeInfo,
}

impl<AccountId, BlockNumber, Balance> IRMachineInfo<AccountId, BlockNumber, Balance>
where
    AccountId: Ord + Default,
    BlockNumber: Copy + Default,
    Balance: Copy + Default + Saturating + Zero,
{
    pub fn bond_machine(stash: AccountId, now: BlockNumber, stake_amount: Balance) -> Self {
        Self {
            machine_stash: stash,
            bonding_height: now,
            stake_amount,
            machine_status: IRMachineStatus::AddingCustomizeInfo,
            ..Default::default()
        }
    }

    fn can_add_customize_info(&self) -> bool {
        matches!(
            self.machine_status,
            IRMachineStatus::AddingCustomizeInfo |
                IRMachineStatus::CommitteeVerifying |
                IRMachineStatus::CommitteeRefused(_) |
                IRMachineStatus::WaitingFulfill |
                IRMachineStatus::StakerReportOffline(_)
        )
    }

    pub fn add_machine_info(
        &mut self,
        add_machine_info: StakerCustomizeInfo,
    ) -> Result<(), CustomErr> {
        // 必须提供网络运营商
        ensure!(!add_machine_info.telecom_operators.is_empty(), CustomErr::TelecomIsNull);

        // 检查当前机器状态是否允许
        ensure!(&self.can_add_customize_info(), CustomErr::NotAllowedChangeMachineInfo);
        self.machine_info_detail.staker_customize_info = add_machine_info;
        self.machine_status = IRMachineStatus::DistributingOrder;

        Ok(())
    }

    // 通过了委员会验证
    pub fn machine_online(&mut self, now: BlockNumber, committee_upload_info: CommitteeUploadInfo) {
        self.stake_amount = Zero::zero();
        self.machine_status = IRMachineStatus::Online;
        self.last_online_height = now;
        self.online_height = now;
        self.machine_info_detail.committee_upload_info = committee_upload_info;
    }

    // 机器被重新派单
    pub fn revert_book(&mut self) {
        self.machine_status = IRMachineStatus::DistributingOrder;
    }

    pub fn machine_offline(&mut self, time: BlockNumber) {
        self.machine_status = IRMachineStatus::StakerReportOffline(time);
    }

    /// Return machine total gpu_num
    pub fn gpu_num(&self) -> u32 {
        self.machine_info_detail.committee_upload_info.gpu_num
    }

    /// Return `calc point` of machine
    pub fn calc_point(&self) -> u64 {
        self.machine_info_detail.committee_upload_info.calc_point
    }

    pub fn can_rent(&self) -> bool {
        matches!(self.machine_status, IRMachineStatus::Online | IRMachineStatus::Rented)
    }
}
