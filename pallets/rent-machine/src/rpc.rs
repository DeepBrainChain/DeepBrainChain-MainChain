use sp_std::vec::Vec;

use crate::{BalanceOf, Config, MachineGPUOrder, Pallet, RentOrderDetail, RentOrderId};
use generic_func::MachineId;

// RPC
impl<T: Config> Pallet<T> {
    pub fn get_rent_order(rent_id: RentOrderId) -> RentOrderDetail<T::AccountId, T::BlockNumber, BalanceOf<T>> {
        Self::rent_order(&rent_id)
    }

    pub fn get_rent_list(renter: T::AccountId) -> Vec<RentOrderId> {
        Self::user_rented(&renter)
    }

    // TODO: 新增API，补充RPC文档
    pub fn is_machine_renter(machine_id: MachineId, renter: T::AccountId) -> bool {
        let machine_order = Self::machine_rent_order(machine_id);

        for order_id in machine_order.rent_order {
            let rent_order = Self::rent_order(order_id);

            if rent_order.renter == renter {
                return true;
            }
        }

        false
    }

    // TODO: 新增API，补充RPC文档
    pub fn get_machine_rent_id(machine_id: MachineId) -> MachineGPUOrder {
        Self::machine_rent_order(machine_id)
    }
}
