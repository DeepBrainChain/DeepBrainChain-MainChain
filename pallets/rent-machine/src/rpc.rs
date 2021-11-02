use crate::{BalanceOf, Config, Pallet, RentOrderDetail};
use generic_func::MachineId;
use sp_std::vec::Vec;

// RPC
impl<T: Config> Pallet<T> {
    pub fn get_rent_order(machine_id: MachineId) -> RentOrderDetail<T::AccountId, T::BlockNumber, BalanceOf<T>> {
        Self::rent_order(&machine_id)
    }

    pub fn get_rent_list(renter: T::AccountId) -> Vec<MachineId> {
        Self::user_rented(&renter)
    }
}
