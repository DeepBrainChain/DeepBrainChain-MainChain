#[cfg(feature = "std")]
use codec::{Decode, Encode};
#[cfg(feature = "std")]
use serde::{de, ser, Deserialize, Serialize};
#[cfg(feature = "std")]
use std::result::Result as StdResult;

#[cfg(feature = "std")]
#[derive(Eq, PartialEq, Encode, Decode, Default, Clone, Copy, Serialize, Deserialize)]
pub struct RpcBalance<T: std::fmt::Display + std::str::FromStr>(
    #[serde(with = "self::serde_balance")] T,
);

#[cfg(feature = "std")]
impl<T: std::fmt::Display + std::str::FromStr> From<T> for RpcBalance<T> {
    fn from(value: T) -> Self {
        RpcBalance(value)
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

#[cfg(feature = "std")]
#[derive(Eq, PartialEq, Encode, Decode, Default, Clone, Serialize, Deserialize)]
pub struct RpcText(#[serde(with = "self::serde_text")] Vec<u8>);

#[cfg(feature = "std")]
impl From<Vec<u8>> for RpcText {
    fn from(value: Vec<u8>) -> Self {
        RpcText(value)
    }
}

#[cfg(feature = "std")]
impl From<&Vec<u8>> for RpcText {
    fn from(value: &Vec<u8>) -> Self {
        RpcText(value.to_vec())
    }
}

/// Text serialization/deserialization
#[cfg(feature = "std")]
pub mod serde_text {
    use super::*;

    /// A serializer that encodes the bytes as a string
    pub fn serialize<T, S>(value: &T, serializer: S) -> StdResult<S::Ok, S::Error>
    where
        S: ser::Serializer,
        T: AsRef<[u8]>,
    {
        let output = String::from_utf8_lossy(value.as_ref());
        serializer.serialize_str(&output)
    }

    /// A deserializer that decodes the string to the bytes (Vec<u8>)
    pub fn deserialize<'de, D>(deserializer: D) -> StdResult<Vec<u8>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let data = String::deserialize(deserializer)?;
        Ok(data.into_bytes())
    }
}

/// Text serialization/deserialization
#[cfg(feature = "std")]
pub mod serde_hash {
    use super::*;
    use std::convert::TryInto;

    /// A serializer that encodes the [u8; 16] as a string
    pub fn serialize<T, S>(value: &T, serializer: S) -> StdResult<S::Ok, S::Error>
    where
        S: ser::Serializer,
        T: AsRef<[u8]>,
    {
        let output = format!("0x{}", hex::encode(value));
        serializer.serialize_str(&output)
    }

    /// A deserializer that decodes the string to the [u8; 16]
    pub fn deserialize<'de, D>(deserializer: D) -> StdResult<[u8; 16], D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let data: String = String::deserialize(deserializer)?
            .strip_prefix("0x")
            .ok_or(serde::de::Error::custom("Parse from string failed"))?
            .to_string();

        let hash: [u8; 16] = hex::decode(data)
            .map_err(|_| serde::de::Error::custom("Parse from string failed"))?
            .try_into()
            .map_err(|_| serde::de::Error::custom("Parse from string failed"))?;

        Ok(hash)
    }
}
