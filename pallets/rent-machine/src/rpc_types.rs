#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use codec::{Decode, Encode};
use sp_std::prelude::Vec;

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "std", serde(bound(serialize = "Balance: std::fmt::Display, AccountId: std::fmt::Display, BlockNumber: std::fmt::Display")))]
#[cfg_attr(feature = "std", serde(bound(deserialize = "Balance: std::str::FromStr, AccountId: std::str::FromStr, BlockNumber: std::str::FromStr")))]
pub struct RpcRentOrderDetail<AccountId, BlockNumber, Balance> {
    #[cfg_attr(feature = "std", serde(with = "serde_account"))]
    pub renter: AccountId, // 租用者
    #[cfg_attr(feature = "std", serde(with = "serde_block_number"))]
    pub rent_start: BlockNumber, // 租用开始时间
    #[cfg_attr(feature = "std", serde(with = "serde_block_number"))]
    pub confirm_rent: BlockNumber, // 用户确认租成功的时间
    #[cfg_attr(feature = "std", serde(with = "serde_block_number"))]
    pub rent_end: BlockNumber, // 租用结束时间
    #[cfg_attr(feature = "std", serde(with = "serde_balance"))]
    pub stake_amount: Balance, // 用户对该机器的质押
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
