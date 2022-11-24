#[cfg(feature = "std")]
use generic_func::rpc_types::serde_text;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use crate::{EraIndex, OPSlashReason};
use codec::{alloc::string::ToString, Decode, Encode};
use generic_func::MachineId;
use sp_core::H256;
use sp_io::hashing::blake2_128;
use sp_runtime::{traits::Saturating, RuntimeDebug};
use sp_std::{prelude::Box, vec, vec::Vec};

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

/// All kind of status of a machine
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum MachineStatus<BlockNumber, AccountId> {
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
    StakerReportOffline(BlockNumber, Box<Self>),
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

impl<BlockNumber, AccountId> Default for MachineStatus<BlockNumber, AccountId> {
    fn default() -> Self {
        MachineStatus::AddingCustomizeInfo
    }
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

    pub fn can_add_customize_info(&self) -> bool {
        matches!(
            self.machine_status,
            MachineStatus::AddingCustomizeInfo |
                MachineStatus::CommitteeVerifying |
                MachineStatus::CommitteeRefused(..) |
                MachineStatus::WaitingFulfill |
                MachineStatus::StakerReportOffline(..)
        )
    }

    pub fn change_rent_fee(&mut self, amount: Balance, is_burn: bool) {
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

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct MachineInfoDetail {
    pub committee_upload_info: CommitteeUploadInfo,
    pub staker_customize_info: StakerCustomizeInfo,
}

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct CommitteeUploadInfo {
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

impl CommitteeUploadInfo {
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
pub struct StakerCustomizeInfo {
    pub server_room: H256,
    /// 上行带宽
    pub upload_net: u64,
    /// 下行带宽
    pub download_net: u64,
    /// 经度(+东经; -西经)
    pub longitude: Longitude,
    /// 纬度(+北纬； -南纬)
    pub latitude: Latitude,
    /// 网络运营商
    pub telecom_operators: Vec<Vec<u8>>,
}

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum Longitude {
    East(u64),
    West(u64),
}

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum Latitude {
    South(u64),
    North(u64),
}

impl Default for Longitude {
    fn default() -> Self {
        Longitude::East(0)
    }
}

impl Default for Latitude {
    fn default() -> Self {
        Latitude::North(0)
    }
}
