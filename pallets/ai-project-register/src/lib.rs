#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]
#![warn(unused_crate_dependencies)]
extern crate core;

#[cfg(test)]
mod mock;
#[allow(non_upper_case_globals)]
#[cfg(test)]
mod tests;

use frame_support::{ensure, pallet_prelude::*};

use frame_system::{ensure_signed, pallet_prelude::*};
use sp_std::{prelude::*, vec, vec::Vec};

use dbc_support::{rental_type::RentStatus, traits::AiProjectRegister, MachineId, RentOrderId};
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + online_profile::Config + rent_machine::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    }

    #[pallet::type_value]
    pub(super) fn MaxLimitPerMachineIdCanRegisterDefault<T: Config>() -> u32 {
        3
    }
    #[pallet::storage]
    #[pallet::getter(fn max_limit_per_machine_id_can_register)]
    pub(super) type MaxLimitPerMachineIdCanRegister<T: Config> =
        StorageValue<_, u32, ValueQuery, MaxLimitPerMachineIdCanRegisterDefault<T>>;

    // machine_id=> registered_ai_project_name
    #[pallet::storage]
    #[pallet::getter(fn machine_id_to_ai_project_name)]
    pub(super) type MachineId2AIProjectName<T: Config> =
        StorageMap<_, Blake2_128Concat, MachineId, Vec<Vec<u8>>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn projec_machine_to_unregistered_times)]
    pub(super) type ProjectMachine2UnregisteredTimes<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        Vec<u8>,
        Blake2_128Concat,
        MachineId,
        <T as frame_system::Config>::BlockNumber,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn registered_info_to_owner)]
    pub(super) type RegisteredInfo2Owner<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, MachineId, Blake2_128Concat, Vec<u8>, T::AccountId>;

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        AddMachineRegisteredProject(MachineId, Vec<u8>, T::AccountId),
        RemoveMachineRegisteredProject(
            MachineId,
            Vec<u8>,
            <T as frame_system::Config>::BlockNumber,
        ),
    }

    #[pallet::error]
    pub enum Error<T> {
        RentInfoNotFound,
        NotRentOwner,
        NotRegisteredInfoOwner,
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
            rent_id: RentOrderId,
            machine_id: MachineId,
            project_name: Vec<u8>,
        ) -> DispatchResultWithPostInfo {
            // check the machine_id and rent_id is valid
            let who = ensure_signed(origin)?;

            let rent_info = <rent_machine::Pallet<T>>::rent_info(rent_id)
                .ok_or(Error::<T>::RentInfoNotFound)?;
            ensure!(who == rent_info.renter, Error::<T>::NotRentOwner);
            ensure!(rent_info.rent_status == RentStatus::Renting, Error::<T>::StatusNotRenting);
            ensure!(rent_info.machine_id == machine_id, Error::<T>::NotRentMachine);

            if !MachineId2AIProjectName::<T>::contains_key(&machine_id) {
                MachineId2AIProjectName::<T>::insert(&machine_id, vec![&project_name]);
            } else {
                let mut project_names = Self::machine_id_to_ai_project_name(&machine_id);
                let projects_num = project_names.len() as u32;
                ensure!(
                    projects_num < Self::max_limit_per_machine_id_can_register(),
                    Error::<T>::OverMaxLimitPerMachineIdCanRegister
                );
                if project_names.contains(&project_name) {
                    return Ok(().into())
                }
                project_names.push(project_name.clone());
                MachineId2AIProjectName::<T>::insert(&machine_id, project_names);
            }

            RegisteredInfo2Owner::<T>::insert(&machine_id, &project_name, &who);
            Self::deposit_event(Event::AddMachineRegisteredProject(machine_id, project_name, who));
            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(10000)]
        pub fn remove_machine_registered_project(
            origin: OriginFor<T>,
            machine_id: MachineId,
            project_name: Vec<u8>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let owner = Self::registered_info_to_owner(&machine_id, &project_name)
                .ok_or(Error::<T>::NotRegistered)?;
            ensure!(who == owner, Error::<T>::NotRegisteredInfoOwner);
            ensure!(
                MachineId2AIProjectName::<T>::contains_key(&machine_id),
                Error::<T>::NotRegistered
            );
            let project_names = Self::machine_id_to_ai_project_name(&machine_id);
            ensure!(project_names.contains(&project_name), Error::<T>::NotRegistered);

            MachineId2AIProjectName::<T>::mutate(&machine_id, |project_names| {
                project_names
                    .into_iter()
                    .position(|x| *x == project_name)
                    .map(|i| project_names.remove(i));
            });

            let now = <frame_system::Pallet<T>>::block_number();
            ProjectMachine2UnregisteredTimes::<T>::insert(&project_name, &machine_id, now);
            RegisteredInfo2Owner::<T>::remove(&machine_id, &project_name);
            Self::deposit_event(Event::RemoveMachineRegisteredProject(
                machine_id,
                project_name,
                now,
            ));
            Ok(().into())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(10000)]
        pub fn set_max_limit_per_machine_id_can_register(
            origin: OriginFor<T>,
            value: u32,
        ) -> DispatchResultWithPostInfo {
            let _who = ensure_root(origin)?;
            MaxLimitPerMachineIdCanRegister::<T>::mutate(|v| *v = value);
            Ok(().into())
        }
    }
}

impl<T: Config> AiProjectRegister for Pallet<T> {
    type AccountId = T::AccountId;
    type BlockNumber = T::BlockNumber;
    fn is_registered(machine_id: MachineId, project_name: Vec<u8>) -> bool {
        Self::machine_id_to_ai_project_name(&machine_id).contains(&project_name)
    }

    // get calc_point of machine
    fn get_machine_calc_point(machine_id: MachineId) -> u64 {
        let machine_info_result = online_profile::Pallet::<T>::machines_info(machine_id);
        if let Some(machine_info) = machine_info_result {
            return machine_info.calc_point()
        }
        0
    }

    fn get_machine_valid_stake_duration(
        renter: T::AccountId,
        stake_start_at: T::BlockNumber,
        machine_id: MachineId,
        rent_ids: Vec<RentOrderId>,
    ) -> T::BlockNumber {
        let now = <frame_system::Pallet<T>>::block_number();
        let mut rent_duration: T::BlockNumber = T::BlockNumber::default();
        rent_ids.iter().for_each(|rent_id| {
            let rent_info_result =
                <rent_machine::Pallet<T>>::rent_info(rent_id).ok_or(Error::<T>::RentInfoNotFound);
            if let Ok(rent_info) = rent_info_result {
                if renter == rent_info.renter &&
                    rent_info.machine_id == machine_id &&
                    rent_info.rent_end >= stake_start_at
                {
                    if rent_info.rent_end >= now {
                        rent_duration += now - stake_start_at
                    } else {
                        rent_duration += rent_info.rent_end - stake_start_at
                    }
                }
            }
        });
        rent_duration
    }
}
