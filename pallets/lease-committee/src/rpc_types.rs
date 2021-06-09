#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use crate::{CommitteeUploadInfo, MachineStatus};
use codec::{Decode, Encode};

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "std", serde(bound(serialize = "Balance: std::fmt::Display, BlockNumber: std::fmt::Display")))]
#[cfg_attr(feature = "std", serde(bound(deserialize = "Balance: std::str::FromStr, BlockNumber: std::str::FromStr")))]
pub struct RpcLCCommitteeOps<BlockNumber, Balance> {
    #[cfg_attr(feature = "std", serde(with = "serde_block_number"))]
    pub booked_time: BlockNumber,
    #[cfg_attr(feature = "std", serde(with = "serde_balance"))]
    pub staked_dbc: Balance,
    // pub verify_time: Vec<BlockNumber>, // FIXME: return Vec<BlockNumber> type
    pub confirm_hash: [u8; 16],
    #[cfg_attr(feature = "std", serde(with = "serde_block_number"))]
    pub hash_time: BlockNumber,
    #[cfg_attr(feature = "std", serde(with = "serde_block_number"))]
    pub confirm_time: BlockNumber, // 委员会提交raw信息的时间
    pub machine_status: MachineStatus,
    pub machine_info: CommitteeUploadInfo,
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
