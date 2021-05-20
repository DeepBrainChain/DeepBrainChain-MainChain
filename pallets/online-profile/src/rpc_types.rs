#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use codec::{Codec, Decode, Encode};

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

// #[rustfmt::skip]
// #[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
// #[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
// #[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
// #[cfg_attr(feature = "std", serde(bound(serialize = "Balance: std::fmt::Display")))]
// #[cfg_attr(feature = "std", serde(bound(deserialize = "Balance: std::str::FromStr")))]
// pub struct StakerListInfo<Balance, AccountId: Codec> {
//     pub staker_name: Vec<u8>,
//     pub staker_account: AccountId,
//     pub calc_points: u64,
//     pub gpu_num: u64,
//     pub gpu_rent_rate: u64,

//     #[cfg_attr(feature = "std", serde(with = "serde_balance"))]
//     pub total_reward: Balance,
// }
