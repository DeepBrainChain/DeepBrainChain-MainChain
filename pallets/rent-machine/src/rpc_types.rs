use codec::{Decode, Encode};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct RpcRentOrderDetail<AccountId, BlockNumber, Balance> {
    pub renter: AccountId,         // 租用者
    pub rent_start: BlockNumber,   // 租用开始时间
    pub confirm_rent: BlockNumber, // 用户确认租成功的时间
    pub rent_end: BlockNumber,     // 租用结束时间
    pub stake_amount: Balance,     // 用户对该机器的质押
}
