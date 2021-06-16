#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use crate::MachineInfoDetail;
use codec::{Decode, Encode};
use sp_std::prelude::Vec;

// 系统统计信息，提供给RPC
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
    pub total_stake: Balance, // 添加机器质押的DBC总数量 + staking模块的总质押数量
}

#[cfg(feature = "std")]
mod serde_balance {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer, T: std::fmt::Display>(
        t: &T,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&t.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>, T: std::str::FromStr>(
        deserializer: D,
    ) -> Result<T, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse::<T>()
            .map_err(|_| serde::de::Error::custom("Parse from string failed"))
    }
}

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

#[cfg(feature = "std")]
mod serde_account {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer, T: std::fmt::Display>(
        t: &T,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&t.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>, T: std::str::FromStr>(
        deserializer: D,
    ) -> Result<T, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse::<T>()
            .map_err(|_| serde::de::Error::custom("Parse from string failed"))
    }
}

#[cfg(feature = "std")]
mod serde_block_number {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer, T: std::fmt::Display>(
        t: &T,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&t.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>, T: std::str::FromStr>(
        deserializer: D,
    ) -> Result<T, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse::<T>()
            .map_err(|_| serde::de::Error::custom("Parse from string failed"))
    }
}

// https://stackoverflow.com/questions/48288988/how-do-i-write-a-serde-visitor-to-convert-an-array-of-arrays-of-strings-to-a-vec
// #[cfg(feature = "std")]
// mod serde_seq_account {
//     use serde::de::{Deserialize, Deserializer, SerializeSeq, Serializer};

//     pub fn serialize<S: Serializer, T>(t: &T, serializer: S) -> Result<S::Ok, S::Error> {
//         let mut seq = serializer.serialize_seq(Some(t.len()))?;
//         for e in t {
//             seq.serialize_element(e)?;
//         }
//         seq.end()
//     }

//     pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<T, D::Error> {
//         struct InnerVisitor;

//         impl<'de> Visitor<'de> for InnerVisitor {
//             type Value = Inner;

//             fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
//                 formatter.write_str("a nonempty sequence of numbers")
//             }

//             #[inline]
//             fn visit_seq<V>(self, mut visitor: V) -> Result<Inner, V::Error>
//             where
//                 V: SeqAccess<'de>,
//             {

//                 let mut vec = Vec::new();

//                 while let Some(Value(elem)) = try!(visitor.next_element()) {
//                     vec.push(elem);
//                 }

//                 Ok(Inner(vec))
//             }
//         }
//     }
// }

// // 构造machine_info rpc结构，将crate::MachineInfo flatten

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
    // pub machine_status: MachineStatus<BlockNumber>,
    pub machine_info_detail: MachineInfoDetail,
    pub machine_price: u64,
    // #[cfg_attr(feature = "std", serde(with = "serde_seq_account"))]
    // #[serde(inner(AccountId, deserialize_with = "serde_account"))]
    // pub reward_committee: Vec<AccountId>,
    #[cfg_attr(feature = "std", serde(with = "serde_block_number"))]
    pub reward_deadline: BlockNumber,
}
