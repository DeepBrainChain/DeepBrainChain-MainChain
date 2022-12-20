use codec::{Decode, Encode};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

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
