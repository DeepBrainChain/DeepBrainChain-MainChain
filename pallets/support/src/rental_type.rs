#[cfg(feature = "std")]
use super::rpc_types::serde_text;
use super::MachineId;
use codec::{Decode, Encode};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::RuntimeDebug;
use sp_std::vec::Vec;

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct RentOrderDetail<AccountId, BlockNumber, Balance> {
    /// 租用的机器ID
    #[cfg_attr(feature = "std", serde(with = "serde_text"))]
    pub machine_id: MachineId,
    /// 租用者
    pub renter: AccountId,
    /// 租用开始时间
    pub rent_start: BlockNumber,
    /// 用户确认租成功的时间
    pub confirm_rent: BlockNumber,
    /// 租用结束时间
    pub rent_end: BlockNumber,
    /// 用户对该机器的质押
    pub stake_amount: Balance,
    /// 当前订单的状态
    pub rent_status: RentStatus,
    /// 租用的GPU数量
    pub gpu_num: u32,
    /// 租用的GPU index
    pub gpu_index: Vec<u32>,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum RentStatus {
    WaitingVerifying,
    Renting,
    RentExpired,
}

impl Default for RentStatus {
    fn default() -> Self {
        RentStatus::RentExpired
    }
}

// A: AccountId, B: BlockNumber, C: Balance
impl<A, B: Default, C: Default> RentOrderDetail<A, B, C> {
    pub fn new(
        machine_id: MachineId,
        renter: A,
        rent_start: B,
        rent_end: B,
        stake_amount: C,
        gpu_num: u32,
        gpu_index: Vec<u32>,
    ) -> Self {
        Self {
            machine_id,
            renter,
            rent_start,
            confirm_rent: B::default(),
            rent_end,
            stake_amount,
            rent_status: RentStatus::WaitingVerifying,
            gpu_num,
            // 增加gpu_index记录
            gpu_index,
        }
    }

    pub fn confirm_rent(&mut self, confirm_rent_time: B) {
        self.confirm_rent = confirm_rent_time;
        self.rent_status = RentStatus::Renting;
    }
}
