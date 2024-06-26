#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]
#![warn(unused_crate_dependencies)]

use frame_support::{
    ensure,
    pallet_prelude::*,
    traits::{Currency},

};

use frame_system::{ ensure_signed, pallet_prelude::*};
use sp_std::{prelude::*, vec, vec::Vec};

use dbc_support::{
     traits::{DbcPrice},
     MachineId, RentOrderId,
     rental_type::{RentStatus},

};

pub use pallet::*;


type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + online_profile::Config  + rent_machine::Config{
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type Currency: Currency<Self::AccountId>;

        type DbcPrice: DbcPrice<Balance=BalanceOf<Self>>;

    }


    #[pallet::type_value]
    pub(super) fn MaxLimitPerNodeIdCanRegisterDefault<T: Config>() -> u32 {
        3
    }
    #[pallet::storage]
    #[pallet::getter(fn max_limit_per_node_id_can_register)]
    pub(super) type MaxLimitPerNodeIdCanRegister<T: Config> = StorageValue<
        _, u32, ValueQuery, MaxLimitPerNodeIdCanRegisterDefault<T>,
    >;

    // machine_id=> node_id => registered_ai_project_name
    #[pallet::storage]
    #[pallet::getter(fn machine_node_id_to_ai_project_name)]
    pub(super) type MachineNodeId2AIProjectName<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        MachineId,
        Blake2_128Concat,
        Vec<u8>,
        Vec<Vec<u8>>,
        ValueQuery,
    >;

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::event]
    // #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        AddMachineGPURegisteredProject(MachineId, Vec<u8>, Vec<u8>),
        RemoveMachineGPURegisteredProject(MachineId,Vec<u8>, Vec<u8>),
    }

    #[pallet::error]
    pub enum Error<T> {
        RentInfoNotFound,
        NotRentOwner,
        StatusNotRenting,
        NodeIdNotFound,
        NotRentMachine,

        OverMaxLimitPerNodeIdCanRegister,
        NodeIdAlreadyRegisteredThisProject,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(10000)]
        pub fn add_machine_gpu_registered_project(
            origin: OriginFor<T>,
            rent_id : RentOrderId,
            machine_id : MachineId,
            node_id: Vec<u8>,
            project_name : Vec<u8>,
        ) -> DispatchResultWithPostInfo {

            // check the machine_id and rent_id is valid
            let who = ensure_signed(origin)?;
            let rent_info = <rent_machine::Pallet<T>>::rent_info(rent_id).ok_or(Error::<T>::RentInfoNotFound)?;
            ensure!(who == rent_info.renter, Error::<T>::NotRentOwner);
            ensure!(rent_info.rent_status == RentStatus::Renting, Error::<T>::StatusNotRenting);
            ensure!(rent_info.machine_id == machine_id, Error::<T>::NotRentMachine);

            if !MachineNodeId2AIProjectName::<T>::contains_key(&machine_id, &node_id){
                MachineNodeId2AIProjectName::<T>::insert(&machine_id, node_id, vec![project_name]);
                return Ok(().into());
            }

            let mut project_names = Self::machine_node_id_to_ai_project_name(&machine_id, &node_id);
            let projects_num = project_names.len() as u32;
            ensure!(projects_num < Self::max_limit_per_node_id_can_register(), Error::<T>::OverMaxLimitPerNodeIdCanRegister);
            ensure!(!project_names.contains(&project_name), Error::<T>::NodeIdAlreadyRegisteredThisProject);

            project_names.push(project_name.clone());
            Self::deposit_event(Event::AddMachineGPURegisteredProject(machine_id, node_id, project_name));
            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(10000)]
        pub fn remove_machine_gpu_registered_project(
            origin: OriginFor<T>,
            rent_id : RentOrderId,
            machine_id : MachineId,
            node_id : Vec<u8>,
            project_name : Vec<u8>,
        ) -> DispatchResultWithPostInfo {

            let who = ensure_signed(origin)?;
            let rent_info = <rent_machine::Pallet<T>>::rent_info(rent_id).ok_or(Error::<T>::RentInfoNotFound)?;
            ensure!(who == rent_info.renter, Error::<T>::NotRentOwner);
            ensure!(MachineNodeId2AIProjectName::<T>::contains_key(&machine_id, &node_id), Error::<T>::NotRentOwner);

            MachineNodeId2AIProjectName::<T>::mutate(&machine_id, &node_id, |project_names|{
                    project_names.into_iter().position(|x| *x == project_name).map(|i| project_names.remove(i));
            });
            Ok(().into())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(10000)]
        pub fn set_max_limit_per_node_id_can_register(origin: OriginFor<T>,value: u32)-> DispatchResultWithPostInfo{
            let _who = ensure_root(origin)?;
            MaxLimitPerNodeIdCanRegister::<T>::mutate(|v| *v = value);
            Ok(().into())
        }


        // get calc_point of machine
        // pub fn machine_calc_point(
        //     machine_id: MachineId,
        //     rented_gpu_num : u32,
        // ) -> Result<u64, &'static str>  {
        //     let machine_info = online_profile::Pallet::<T>::machines_info(machine_id).ok_or("machine not found")?;
        //     let rented_calc_point =  machine_info.calc_point().saturating_mul(rented_gpu_num as u64);
        //     Ok(rented_calc_point)
        // }
    }
}

impl<T: Config> Pallet<T> {
    pub fn is_registered(machine_id: MachineId, node_id: Vec<u8>, project_name: Vec<u8>)-> bool{
        let mut is_registered = false;
        MachineNodeId2AIProjectName::<T>::mutate(&machine_id, &node_id, |project_names|{
            is_registered = project_names.contains(&project_name)
        });
        is_registered
    }
}
