#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use crate::{ImageName, MachineId, MachineStatus};
use codec::{Decode, Encode};
use sp_std::prelude::Vec;

// 系统统计信息，提供给RPC
#[rustfmt::skip]
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "std", serde(bound(serialize = "Balance: std::fmt::Display")))]
#[cfg_attr(feature = "std", serde(bound(deserialize = "Balance: std::str::FromStr")))]
pub struct SysInfo<Balance> {
    pub total_gpu_num: u64,     // 当前系统中工作的GPU数量
    pub total_staker: u64,      // 矿工总数
    pub total_calc_points: u64, // 系统算力点数

    #[cfg_attr(feature = "std", serde(with = "serde_balance"))]
    pub total_stake: Balance,   // 添加机器质押的DBC总数量 + staking模块的总质押数量
}

#[rustfmt::skip]
#[cfg(feature = "std")]
mod serde_balance {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer, T: std::fmt::Display>(t: &T, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&t.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>, T: std::str::FromStr>(deserializer: D) -> Result<T, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse::<T>().map_err(|_| serde::de::Error::custom("Parse from string failed"))
    }
}

#[rustfmt::skip]
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "std", serde(bound(serialize = "Balance: std::fmt::Display")))]
#[cfg_attr(feature = "std", serde(bound(deserialize = "Balance: std::str::FromStr")))]
pub struct StakerInfo<Balance> {
    pub calc_points: u64,
    pub gpu_num: u64,

    #[cfg_attr(feature = "std", serde(with = "serde_balance"))]
    pub total_reward: Balance,
}

#[rustfmt::skip]
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "std", serde(bound(serialize = "Balance: std::fmt::Display, AccountId: std::fmt::Display")))]
#[cfg_attr(feature = "std", serde(bound(deserialize = "Balance: std::str::FromStr, AccountId: std::str::FromStr")))]
pub struct StakerListInfo<Balance, AccountId> {
    pub staker_name: Vec<u8>,
    #[cfg_attr(feature = "std", serde(with = "serde_account"))]
    pub staker_account: AccountId,
    pub calc_points: u64,
    pub gpu_num: u64,
    pub gpu_rent_rate: u64,

    #[cfg_attr(feature = "std", serde(with = "serde_balance"))]
    pub total_reward: Balance,
}

#[rustfmt::skip]
#[cfg(feature = "std")]
mod serde_account {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer, T: std::fmt::Display>(t: &T, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&t.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>, T: std::str::FromStr>(deserializer: D) -> Result<T, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse::<T>().map_err(|_| serde::de::Error::custom("Parse from string failed"))
    }
}

#[rustfmt::skip]
#[cfg(feature = "std")]
mod serde_block_number {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer, T: std::fmt::Display>(t: &T, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&t.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>, T: std::str::FromStr>(deserializer: D) -> Result<T, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse::<T>().map_err(|_| serde::de::Error::custom("Parse from string failed"))
    }
}

// 构造machine_info rpc结构，将crate::MachineInfo flatten

#[rustfmt::skip]
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "std", serde(bound(serialize = "Balance: std::fmt::Display, AccountId: std::fmt::Display, BlockNumber: std::fmt::Display")))]
#[cfg_attr(feature = "std", serde(bound(deserialize = "Balance: std::str::FromStr, AccountId: std::str::FromStr, BlockNumber: std::str::FromStr")))]
pub struct RPCMachineInfo<AccountId, BlockNumber, Balance> {
    #[cfg_attr(feature = "std", serde(with = "serde_account"))]
    pub machine_owner: AccountId,
    #[cfg_attr(feature = "std", serde(with = "serde_block_number"))]
    pub bonding_height: BlockNumber,
    #[cfg_attr(feature = "std", serde(with = "serde_balance"))]
    pub stake_amount: Balance,
    pub machine_status: MachineStatus,
    pub machine_info_detail: RPCMachineInfoDetail,
    pub machine_price: u64,
    // #[cfg_attr(feature = "std", serde(with = "serde_account"))]
    // pub reward_committee: Vec<AccountId>,
    #[cfg_attr(feature = "std", serde(with = "serde_block_number"))]
    pub reward_deadline: BlockNumber,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
pub struct RPCMachineInfoDetail {
    pub committee_upload_info: RPCCommitteeUploadInfo,
    pub staker_customize_info: RPCStakerCustomizeInfo,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
pub struct RPCCommitteeUploadInfo {
    pub machine_id: MachineId,
    pub gpu_type: Vec<u8>,
    pub gpu_num: u32,
    pub cuda_core: u32,
    pub gpu_mem: u64,
    pub calc_point: u64,
    pub hard_disk: u64,
    pub cpu_type: Vec<u8>,
    pub cpu_core_num: u32,
    pub cpu_rate: u64,
    pub mem_num: u64,

    pub rand_str: Vec<u8>,
    pub is_support: bool,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
pub struct RPCStakerCustomizeInfo {
    pub left_change_time: u64,

    pub upload_net: u64,
    pub download_net: u64,
    pub longitude: u64,
    pub latitude: u64,

    pub images: Vec<ImageName>,
}
