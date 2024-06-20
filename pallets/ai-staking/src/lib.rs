#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]
#![warn(unused_crate_dependencies)]

use frame_support::{
    dispatch::DispatchResult,
    ensure,
    pallet_prelude::*,
    traits::{Currency},

};

use frame_system::{ensure_root, ensure_signed, pallet_prelude::*};
use sp_std::{prelude::*, vec, vec::Vec};

use dbc_support::{
     traits::{DbcPrice, RTOps},
     MachineId, RentOrderId,
     rental_type::{RentOrderDetail, RentStatus},

};
use sp_runtime::{Perbill};


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


    // machine_id=> gpu_index => registered_ai_project_name
    #[pallet::storage]
    #[pallet::getter(fn machine_gpu2ai_project_name)]
    pub(super) type MachineGPU2AIProjectName<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        MachineId,
        Blake2_128Concat,
        u8,
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
        RentEndAt(T::BlockNumber),
        AddMachineGPURegisteredProject(MachineId, u8, Vec<u8>),
        RemoveMachineGPURegisteredProject(MachineId, u8, Vec<u8>),
        MachineGPURegisteredProjectNotFound(MachineId, u8)
    }

    #[pallet::error]
    pub enum Error<T> {
        RentInfoNotFound,
        NotRentOwner,
        StatusNotRenting,
        InvalidGPUIndex,
    }


    impl<T: Config> Pallet<T> {

        // TODO
        // get account id by erc20 address
        pub fn get_account_id_by_erc20_address(
            erc20_address: T::AccountId
        ) -> Result<T::AccountId, &'static str > {
            Err("todo")
        }

        // get end time of rent_id if all check passed
        pub fn rent_end_at(
            erc20_address: T::AccountId,
            rent_id: RentOrderId,
            gpu_index :u32,
        ) -> Result<T::BlockNumber, &'static str> {
            let who = Self::get_account_id_by_erc20_address(erc20_address)?;
            let rent_info = <rent_machine::Pallet<T>>::rent_info(rent_id).ok_or(Error::<T>::RentInfoNotFound)?;
            ensure!(who == rent_info.renter, Error::<T>::RentInfoNotFound);
            ensure!(rent_info.rent_status == RentStatus::Renting, Error::<T>::StatusNotRenting);
            ensure!(rent_info.gpu_index.contains(&gpu_index), Error::<T>::InvalidGPUIndex);

            Ok(rent_info.rent_end)
        }

        // get calc_point of machine
        pub fn machine_gpu_calc_point(
            machine_id: MachineId,
            rented_gpu_num : u32,
        ) -> Result<u64, &'static str>  {
            let machine_info = online_profile::Pallet::<T>::machines_info(machine_id).ok_or("machine not found")?;
            let rented_calc_point =  machine_info.calc_point().saturating_mul(rented_gpu_num as u64);
            Ok(rented_calc_point)
        }

        pub fn get_machine_gpu_registered_project_num(machine_id : MachineId, gpu_index : u8) -> u32 {
            let usize_num = MachineGPU2AIProjectName::<T>::decode_len::<MachineId, u8>(machine_id.into(), gpu_index).unwrap_or(0);
            usize_num.try_into().unwrap_or(0)
        }

        pub fn add_machine_gpu_registered_project(machine_id : MachineId, gpu_num : u8, project_name : Vec<u8>) -> Result<Vec<u8>, &'static str>  {
            let machine_info = online_profile::Pallet::<T>::machines_info(&machine_id).ok_or("machine not found")?;
            if !MachineGPU2AIProjectName::<T>::contains_key(&machine_id, gpu_index){
                MachineGPU2AIProjectName::<T>::insert(&machine_id, gpu_index, vec![project_name]);
                return Ok(());
            }
            MachineGPU2AIProjectName::<T>::mutate(machine_id.clone(), gpu_index, |project_names|{
                if machine_info.gpu_num()>= project_names.len() as u32 -1{
                    return Err("gpu_index out of range");
                }

                if project_names.contains(&project_name)  {
                    return Err("gpu_index already registered");
                };

                project_names.push(project_name.clone());
                Self::deposit_event(Event::AddMachineGPURegisteredProject(machine_id, gpu_index, project_name));
                Ok(())
            })?;

            Ok(())
        }

        pub fn remove_machine_gpu_registered_project(machine_id : MachineId, gpu_index : u8, project_name : Vec<u8>)  {
            if !MachineGPU2AIProjectName::<T>::contains_key(&machine_id, gpu_index){
                Self::deposit_event(Event::MachineGPURegisteredProjectNotFound(machine_id, gpu_index));
                return;
            }
            MachineGPU2AIProjectName::<T>::mutate(&machine_id, &gpu_index, |project_names|{
                    project_names.into_iter().position(|x| *x == project_name).map(|i| project_names.remove(i));
            });

        }

        pub fn get_dbc_price() ->u64{
            <T as Config>::DbcPrice::get_dbc_avg_price().unwrap_or(0)
        }
    }
}

