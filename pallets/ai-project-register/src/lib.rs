#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]
#![warn(unused_crate_dependencies)]
extern crate core;

#[cfg(test)]
mod mock;
#[allow(non_upper_case_globals)]
#[cfg(test)]
mod tests;

use frame_support::pallet_prelude::*;

use frame_system::pallet_prelude::*;
use sp_std::{prelude::*, vec, vec::Vec};

use dbc_support::{
    traits::ProjectRegister,
    utils::{account_id, verify_signature},
    MachineId,
};
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + rent_machine::Config {
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
        SignatureVerifyResult(bool),
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(10000, 0))]
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

impl<T: Config> ProjectRegister for Pallet<T> {
    fn is_registered(machine_id: MachineId, project_name: Vec<u8>) -> bool {
        Self::machine_id_to_ai_project_name(&machine_id).contains(&project_name)
    }

    fn add_machine_registered_project(
        data: Vec<u8>,
        sig: sp_core::sr25519::Signature,
        from: sp_core::sr25519::Public,
        machine_id: MachineId,
        project_name: Vec<u8>,
    ) -> Result<(), &'static str> {
        let ok = verify_signature(data, sig, from.clone());
        if !ok {
            return Err("signature verify failed");
        };

        let who = account_id::<T>(from.clone())?;

        let rent_ids = <rent_machine::Pallet<T>>::get_rent_ids(machine_id.clone(), &who);
        if rent_ids.len() == 0 {
            return Err("machine not rented");
        }

        if !MachineId2AIProjectName::<T>::contains_key(&machine_id) {
            MachineId2AIProjectName::<T>::insert(&machine_id, vec![&project_name]);
        } else {
            let mut project_names = Self::machine_id_to_ai_project_name(&machine_id);
            let projects_num = project_names.len() as u32;
            if projects_num >= Self::max_limit_per_machine_id_can_register() {
                return Err("over max limit per machine id can register");
            }

            if project_names.contains(&project_name) {
                return Ok(().into());
            }
            project_names.push(project_name.clone());
            MachineId2AIProjectName::<T>::insert(&machine_id, project_names);
        }
        RegisteredInfo2Owner::<T>::insert(&machine_id, &project_name, &who);
        Self::deposit_event(Event::AddMachineRegisteredProject(machine_id, project_name, who));
        Ok(().into())
    }

    fn remove_machine_registered_project(
        data: Vec<u8>,
        sig: sp_core::sr25519::Signature,
        from: sp_core::sr25519::Public,
        machine_id: MachineId,
        project_name: Vec<u8>,
    ) -> Result<(), &'static str> {
        let ok = verify_signature(data, sig, from.clone());
        if !ok {
            return Err("signature verify failed");
        };

        let who = account_id::<T>(from.clone())?;
        let owner =
            Self::registered_info_to_owner(&machine_id, &project_name).ok_or("not registered")?;
        if who != owner {
            return Err("not registered info owner");
        }

        if !MachineId2AIProjectName::<T>::contains_key(&machine_id) {
            return Err("machine not registered");
        }
        let project_names = Self::machine_id_to_ai_project_name(&machine_id);
        if !project_names.contains(&project_name) {
            return Err("project not registered");
        }

        MachineId2AIProjectName::<T>::mutate(&machine_id, |project_names| {
            project_names
                .into_iter()
                .position(|x| *x == project_name)
                .map(|i| project_names.remove(i));
        });

        let now = <frame_system::Pallet<T>>::block_number();
        ProjectMachine2UnregisteredTimes::<T>::insert(&project_name, &machine_id, now);
        RegisteredInfo2Owner::<T>::remove(&machine_id, &project_name);
        Self::deposit_event(Event::RemoveMachineRegisteredProject(machine_id, project_name, now));
        Ok(().into())
    }

    fn is_registered_machine_owner(
        data: Vec<u8>,
        sig: sp_core::sr25519::Signature,
        from: sp_core::sr25519::Public,
        machine_id: MachineId,
        project_name: Vec<u8>,
    ) -> Result<bool, &'static str> {
        let ok = verify_signature(data, sig, from.clone());
        if !ok {
            return Err("signature verify failed");
        };
        let owner =
            Self::registered_info_to_owner(machine_id, project_name).ok_or("not registered")?;
        let who = account_id::<T>(from.clone())?;
        Ok(owner == who)
    }
}
