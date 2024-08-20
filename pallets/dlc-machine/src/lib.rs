#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]
#![warn(unused_crate_dependencies)]
extern crate core;

#[cfg(test)]
mod mock;
// #[allow(non_upper_case_globals)]
// #[cfg(test)]
// mod tests;

pub use dbc_support::machine_type::MachineStatus;
use dbc_support::{
    traits::{DLCMachineReportStakingTrait, MachineInfoTrait},
    utils::account_id,
    MachineId,
};
use frame_support::pallet_prelude::*;
pub use pallet::*;
use sp_std::{prelude::*, str, vec::Vec};

#[frame_support::pallet]
pub mod pallet {

    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + rent_machine::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    }

    #[pallet::storage]
    #[pallet::getter(fn dlc_machine_ids_in_staking)]
    pub type DLCMachineIdsInStaking<T: Config> = StorageValue<_, Vec<MachineId>, ValueQuery>;

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        ReportDLCStaking(T::AccountId, MachineId),
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {}
}

impl<T: Config> DLCMachineReportStakingTrait for Pallet<T> {
    fn report_dlc_staking(
        data: Vec<u8>,
        sig: sp_core::sr25519::Signature,
        from: sp_core::sr25519::Public,
        machine_id: MachineId,
    ) -> Result<(), &'static str> {
        let result =
            <rent_machine::Pallet<T> as MachineInfoTrait>::is_both_machine_renter_and_owner(
                data,
                sig,
                from.clone(),
                machine_id.clone(),
            )?;

        if !result {
            return Err("renter not owner")
        }

        DLCMachineIdsInStaking::<T>::mutate(|ids| ids.push(machine_id.clone()));
        let stakeholder = account_id::<T>(from)?;
        Self::deposit_event(Event::ReportDLCStaking(stakeholder, machine_id));
        Ok(())
    }

    fn report_dlc_end_staking(
        data: Vec<u8>,
        sig: sp_core::sr25519::Signature,
        from: sp_core::sr25519::Public,
        machine_id: MachineId,
    ) -> Result<(), &'static str> {
        let result = <rent_machine::Pallet<T> as MachineInfoTrait>::is_machine_owner(
            data,
            sig,
            from,
            machine_id.clone(),
        )?;
        if !result {
            return Err("not owner")
        }
        DLCMachineIdsInStaking::<T>::mutate(|ids| ids.retain(|id| id != &machine_id));
        Ok(())
    }
}

impl<T: Config> Pallet<T> {
    pub fn dlc_machine_in_staking(machine_id: MachineId) -> bool {
        DLCMachineIdsInStaking::<T>::get().contains(&machine_id)
    }

    pub fn report_dlc_machine_slashed(machine_id: MachineId) {
        DLCMachineIdsInStaking::<T>::mutate(|ids| ids.retain(|id| id != &machine_id));
    }
}
