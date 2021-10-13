use crate::{BalanceOf, Config, Pallet};
use codec::{Decode, Encode};
use generic_func::MachineId;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_std::{prelude::*, str, vec::Vec};

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

// RPC
impl<T: Config> Pallet<T> {
    pub fn get_rent_order(
        renter: T::AccountId,
        machine_id: MachineId,
    ) -> RpcRentOrderDetail<T::AccountId, T::BlockNumber, BalanceOf<T>> {
        let order_info = Self::rent_order(&renter, &machine_id);
        if let None = order_info {
            return RpcRentOrderDetail { ..Default::default() }
        }
        let order_info = order_info.unwrap();
        RpcRentOrderDetail {
            renter: order_info.renter,
            rent_start: order_info.rent_start,
            confirm_rent: order_info.confirm_rent,
            rent_end: order_info.rent_end,
            stake_amount: order_info.stake_amount,
        }
    }

    pub fn get_rent_list(renter: T::AccountId) -> Vec<MachineId> {
        Self::user_rented(&renter)
    }

    pub fn get_machine_renter(machine_id: MachineId) -> Option<T::AccountId> {
        Self::machine_renter(machine_id)
    }
}
