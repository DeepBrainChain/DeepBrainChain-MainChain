use sp_std::vec::Vec;

use crate::{BalanceOf, Config, MachineGPUOrder, Pallet, RentOrderId};
use dbc_support::{rental_type::RentOrderDetail, MachineId};

// RPC
impl<T: Config> Pallet<T> {
    pub fn get_rent_order(
        rent_id: RentOrderId,
    ) -> Option<RentOrderDetail<T::AccountId, T::BlockNumber, BalanceOf<T>>> {
        Self::rent_info(&rent_id)
    }

    pub fn get_rent_list(renter: T::AccountId) -> Vec<RentOrderId> {
        Self::user_order(&renter)
    }

    pub fn is_machine_renter(machine_id: MachineId, renter: T::AccountId) -> bool {
        let machine_order = Self::machine_rent_order(machine_id);

        for order_id in machine_order.rent_order {
            if let Some(rent_info) = Self::rent_info(order_id) {
                if rent_info.renter == renter {
                    return true;
                }
            }
        }

        false
    }

    pub fn get_machine_rent_id(machine_id: MachineId) -> MachineGPUOrder {
        Self::machine_rent_order(machine_id)
    }
}
