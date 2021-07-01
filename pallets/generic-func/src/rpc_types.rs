use codec::{Decode, Encode};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

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
