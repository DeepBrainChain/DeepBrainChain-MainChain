#[cfg(feature = "std")]
use generic_func::rpc_types::serde_text;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use codec::{alloc::string::ToString, Decode, Encode};
use frame_support::ensure;
use generic_func::MachineId;
use sp_core::H256;
use sp_io::hashing::blake2_128;
use sp_runtime::{
    traits::{Saturating, Zero},
    RuntimeDebug,
};
use sp_std::{prelude::Box, vec, vec::Vec};

use crate::{CustomErr, OPSlashReason};

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
    ReporterReportOffline(OPSlashReason<BlockNumber>, Box<Self>, AccountId, Vec<AccountId>),

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
    pub committee_upload_info: IRCommitteeUploadInfo,
    pub staker_customize_info: IRStakerCustomizeInfo,
}

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct IRCommitteeUploadInfo {
    #[cfg_attr(feature = "std", serde(with = "serde_text"))]
    pub machine_id: MachineId,
    #[cfg_attr(feature = "std", serde(with = "serde_text"))]
    pub gpu_type: Vec<u8>, // GPU型号
    pub gpu_num: u32,    // GPU数量
    pub cuda_core: u32,  // CUDA core数量
    pub gpu_mem: u64,    // GPU显存
    pub calc_point: u64, // 算力值
    pub sys_disk: u64,   // 系统盘大小
    pub data_disk: u64,  // 数据盘大小
    #[cfg_attr(feature = "std", serde(with = "serde_text"))]
    pub cpu_type: Vec<u8>, // CPU型号
    pub cpu_core_num: u32, // CPU内核数
    pub cpu_rate: u64,   // CPU频率
    pub mem_num: u64,    // 内存数

    #[cfg_attr(feature = "std", serde(with = "serde_text"))]
    pub rand_str: Vec<u8>,
    pub is_support: bool, // 委员会是否支持该机器上线
}

impl IRCommitteeUploadInfo {
    fn join_str<A: ToString>(items: Vec<A>) -> Vec<u8> {
        let mut output = Vec::new();
        for item in items {
            let item: Vec<u8> = item.to_string().into();
            output.extend(item);
        }
        output
    }

    pub fn hash(&self) -> [u8; 16] {
        let is_support: Vec<u8> = if self.is_support { "1".into() } else { "0".into() };

        let mut raw_info = Vec::new();
        raw_info.extend(self.machine_id.clone());
        raw_info.extend(self.gpu_type.clone());
        raw_info.extend(Self::join_str(vec![
            self.gpu_num as u64,
            self.cuda_core as u64,
            self.gpu_mem,
            self.calc_point,
            self.sys_disk,
            self.data_disk,
        ]));
        raw_info.extend(self.cpu_type.clone());
        raw_info.extend(Self::join_str(vec![
            self.cpu_core_num as u64,
            self.cpu_rate,
            self.mem_num,
        ]));
        raw_info.extend(self.rand_str.clone());
        raw_info.extend(is_support);

        blake2_128(&raw_info)
    }
}

// 由机器管理者自定义的提交
#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct IRStakerCustomizeInfo {
    pub server_room: H256,
    /// 上行带宽
    pub upload_net: u64,
    /// 下行带宽
    pub download_net: u64,
    /// 经度(+东经; -西经)
    pub longitude: IRLongitude,
    /// 纬度(+北纬； -南纬)
    pub latitude: IRLatitude,
    /// 网络运营商
    pub telecom_operators: Vec<Vec<u8>>,
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
        add_machine_info: IRStakerCustomizeInfo,
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
    pub fn machine_online(
        &mut self,
        now: BlockNumber,
        committee_upload_info: IRCommitteeUploadInfo,
    ) {
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

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum IRLongitude {
    East(u64),
    West(u64),
}

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum IRLatitude {
    North(u64),
    South(u64),
}

impl Default for IRLongitude {
    fn default() -> Self {
        IRLongitude::East(0)
    }
}

impl Default for IRLatitude {
    fn default() -> Self {
        IRLatitude::North(0)
    }
}
