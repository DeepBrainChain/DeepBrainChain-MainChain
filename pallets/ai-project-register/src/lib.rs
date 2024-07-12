#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]
#![warn(unused_crate_dependencies)]
extern crate core;

#[cfg(test)]
mod mock;
#[allow(non_upper_case_globals)]
#[cfg(test)]
mod tests;

use frame_support::{
    ensure,
    pallet_prelude::*,

};

use frame_system::{ ensure_signed, pallet_prelude::*};
use sp_std::{prelude::*, vec, vec::Vec};

use dbc_support::{
     MachineId, RentOrderId,
     rental_type::{RentStatus},

};

pub use pallet::*;



#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + online_profile::Config  + rent_machine::Config{
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    }


    #[pallet::type_value]
    pub(super) fn MaxLimitPerMachineIdCanRegisterDefault<T: Config>() -> u32 {
        3
    }
    #[pallet::storage]
    #[pallet::getter(fn max_limit_per_machine_id_can_register)]
    pub(super) type MaxLimitPerMachineIdCanRegister<T: Config> = StorageValue<
        _, u32, ValueQuery, MaxLimitPerMachineIdCanRegisterDefault<T>,
    >;

    // machine_id=> node_id => registered_ai_project_name
    #[pallet::storage]
    #[pallet::getter(fn machine_id_to_ai_project_name)]
    pub(super) type MachineId2AIProjectName<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        MachineId,
        Vec<Vec<u8>>,
        ValueQuery,
    >;

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        AddMachineRegisteredProject(MachineId,  Vec<u8>),
        RemoveMachineRegisteredProject(MachineId, Vec<u8>),
    }

    #[pallet::error]
    pub enum Error<T> {
        RentInfoNotFound,
        NotRentOwner,
        NotRegistered,
        StatusNotRenting,
        NotRentMachine,
        OverMaxLimitPerMachineIdCanRegister,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(10000)]
        pub fn add_machine_registered_project(
            origin: OriginFor<T>,
            rent_id : RentOrderId,
            machine_id : MachineId,
            project_name : Vec<u8>,
        ) -> DispatchResultWithPostInfo {

            // check the machine_id and rent_id is valid
            let who = ensure_signed(origin)?;
            let rent_info = <rent_machine::Pallet<T>>::rent_info(rent_id).ok_or(Error::<T>::RentInfoNotFound)?;
            ensure!(who == rent_info.renter, Error::<T>::NotRentOwner);
            ensure!(rent_info.rent_status == RentStatus::Renting, Error::<T>::StatusNotRenting);
            ensure!(rent_info.machine_id == machine_id, Error::<T>::NotRentMachine);

            if !MachineId2AIProjectName::<T>::contains_key(&machine_id){
                MachineId2AIProjectName::<T>::insert(&machine_id, vec![project_name]);
                return Ok(().into());
            }

            let mut project_names = Self::machine_id_to_ai_project_name(&machine_id);
            let projects_num = project_names.len() as u32;
            ensure!(projects_num < Self::max_limit_per_machine_id_can_register(), Error::<T>::OverMaxLimitPerMachineIdCanRegister);
            if project_names.contains(&project_name){
                return Ok(().into());
            }
            project_names.push(project_name.clone());
            MachineId2AIProjectName::<T>::insert(&machine_id, project_names);

            Self::deposit_event(Event::AddMachineRegisteredProject(machine_id, project_name));
            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(10000)]
        pub fn remove_machine_registered_project(
            origin: OriginFor<T>,
            rent_id : RentOrderId,
            machine_id : MachineId,
            project_name : Vec<u8>,
        ) -> DispatchResultWithPostInfo {

            let who = ensure_signed(origin)?;
            let rent_info = <rent_machine::Pallet<T>>::rent_info(rent_id).ok_or(Error::<T>::RentInfoNotFound)?;
            ensure!(who == rent_info.renter, Error::<T>::NotRentOwner);
            ensure!(MachineId2AIProjectName::<T>::contains_key(&machine_id), Error::<T>::NotRegistered);
            let  project_names = Self::machine_id_to_ai_project_name(&machine_id);
            ensure!(project_names.contains(&project_name), Error::<T>::NotRegistered);

            MachineId2AIProjectName::<T>::mutate(&machine_id, |project_names|{
                    project_names.into_iter().position(|x| *x == project_name).map(|i| project_names.remove(i));
            });
            Ok(().into())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(10000)]
        pub fn set_max_limit_per_machine_id_can_register(origin: OriginFor<T>,value: u32)-> DispatchResultWithPostInfo{
            let _who = ensure_root(origin)?;
            MaxLimitPerMachineIdCanRegister::<T>::mutate(|v| *v = value);
            Ok(().into())
        }
    }
}

impl<T: Config> Pallet<T> {
    pub fn is_registered(machine_id: MachineId, project_name: Vec<u8>)-> bool{
        Self::machine_id_to_ai_project_name(&machine_id,).contains(&project_name)
    }

    // get calc_point of machine
    pub fn machine_calc_point(
        machine_id: MachineId,
    ) -> Result<u64, &'static str>  {
        let machine_info = online_profile::Pallet::<T>::machines_info(machine_id).ok_or("machine not found")?;
        Ok(machine_info.calc_point())
    }
}
