use codec::{Decode, Encode};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_std::vec::Vec;

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
#[cfg_attr(
    feature = "std",
    serde(bound(serialize = "Balance: std::fmt::Display, AccountId: std::fmt::Display"))
)]
#[cfg_attr(
    feature = "std",
    serde(bound(deserialize = "Balance: std::str::FromStr, AccountId: std::str::FromStr"))
)]
pub struct StakerListInfo<Balance, AccountId> {
    pub index: u64,
    pub staker_name: Vec<u8>,
    #[cfg_attr(feature = "std", serde(with = "serde_account"))]
    pub staker_account: AccountId,
    pub calc_points: u64,
    pub total_gpu_num: u64,
    pub total_rented_gpu: u64,
    #[cfg_attr(feature = "std", serde(with = "serde_balance"))]
    pub total_rent_fee: Balance, // 总租金收益(银河竞赛前获得)
    #[cfg_attr(feature = "std", serde(with = "serde_balance"))]
    pub total_burn_fee: Balance, // 总销毁数量
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
        s.parse::<T>().map_err(|_| serde::de::Error::custom("Parse from string failed"))
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
        s.parse::<T>().map_err(|_| serde::de::Error::custom("Parse from string failed"))
    }
}
