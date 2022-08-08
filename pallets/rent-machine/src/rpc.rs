use crate::{BalanceOf, Config, Pallet, RentOrderDetail, RentOrderId};
use sp_std::vec::Vec;

// RPC
impl<T: Config> Pallet<T> {
    pub fn get_rent_order(rent_id: RentOrderId) -> RentOrderDetail<T::AccountId, T::BlockNumber, BalanceOf<T>> {
        Self::rent_order(&rent_id)
    }

    pub fn get_rent_list(renter: T::AccountId) -> Vec<RentOrderId> {
        Self::user_rented(&renter)
    }
}
