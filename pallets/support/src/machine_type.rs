#[cfg(feature = "std")]
use super::rpc_types::serde_text;
use super::{verify_slash::OPSlashReason, MachineId};
use codec::{alloc::string::ToString, Decode, Encode};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_core::H256;
use sp_io::hashing::blake2_128;
use sp_runtime::RuntimeDebug;
use sp_std::{prelude::Box, vec, vec::Vec};
use scale_info::TypeInfo;

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum Longitude {
    East(u64),
    West(u64),
}

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode, TypeInfo)]
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

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode, Default, PartialOrd, Ord, TypeInfo)]
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
#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode, Default, TypeInfo)]
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

/// Standard GPU rent price Per Era
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug, TypeInfo)]
pub struct StandardGpuPointPrice {
    /// Standard GPU calc points
    pub gpu_point: u64,
    /// Standard GPU price
    pub gpu_price: u64,
}

/// All kind of status of a machine
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
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

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode, Default, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct MachineInfoDetail {
    pub committee_upload_info: CommitteeUploadInfo,
    pub staker_customize_info: StakerCustomizeInfo,
}
