use codec::{Decode, Encode};
use sp_runtime::RuntimeDebug;

/// Standard GPU rent price Per Era
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct StandardGpuPointPrice {
    /// Standard GPU calc points
    pub gpu_point: u64,
    /// Standard GPU price
    pub gpu_price: u64,
}
