use codec::{Decode, Encode};
#[cfg(feature = "std")]
use generic_func::rpc_types::serde_text;
use generic_func::{ItemList, MachineId};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::RuntimeDebug;
use sp_std::{vec, vec::Vec};

pub type RentOrderId = u64;

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct IRRentOrderDetail<AccountId, BlockNumber, Balance> {
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
    pub rent_status: IRRentStatus,
    /// 租用的GPU数量
    pub gpu_num: u32,
    /// 租用的GPU index
    pub gpu_index: Vec<u32>,
}

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct IRMachineGPUOrder {
    // 机器所有GPU对应的RentOrder
    pub rent_order: Vec<RentOrderId>,
    // 机器订单已经使用的gpu:
    pub used_gpu: Vec<u32>,
}

impl IRMachineGPUOrder {
    // 获取可以被租用的GPU index
    pub fn gen_rentable_gpu(&mut self, need_gpu: u32, total_gpu: u32) -> Vec<u32> {
        let mut out = vec![];

        for i in 0..total_gpu {
            if out.len() == need_gpu as usize {
                return out
            }

            if self.used_gpu.binary_search(&i).is_err() {
                out.push(i);
                ItemList::add_item(&mut self.used_gpu, i);
            }
        }

        out
    }

    // 根据gpu_index清理使用的GPU index
    pub fn clean_expired_order(&mut self, order_id: RentOrderId, gpu_index: Vec<u32>) {
        ItemList::rm_item(&mut self.rent_order, &order_id);
        for index in gpu_index {
            ItemList::rm_item(&mut self.used_gpu, &index);
        }
    }
}

// A: AccountId, B: BlockNumber, C: Balance
impl<A, B: Default, C: Default> IRRentOrderDetail<A, B, C> {
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
            rent_status: IRRentStatus::WaitingVerifying,
            gpu_num,
            // 增加gpu_index记录
            gpu_index,
        }
    }

    pub fn confirm_rent(&mut self, confirm_rent_time: B) {
        self.confirm_rent = confirm_rent_time;
        self.rent_status = IRRentStatus::Renting;
    }
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum IRRentStatus {
    WaitingVerifying,
    Renting,
    RentExpired,
}

impl Default for IRRentStatus {
    fn default() -> Self {
        IRRentStatus::RentExpired
    }
}

/// Standard GPU rent price Per Era
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct StandardGpuPointPrice {
    /// Standard GPU calc points
    pub gpu_point: u64,
    /// Standard GPU price
    pub gpu_price: u64,
}
