#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]
#![warn(unused_crate_dependencies)]
extern crate core;

#[cfg(test)]
mod mock;
#[allow(non_upper_case_globals)]
#[cfg(test)]
mod tests;

pub use dbc_support::machine_type::MachineStatus;
use dbc_support::{
    traits::{DLCMachineReportStakingTrait, MachineInfoTrait},
    utils::account_id,
    MachineId,
};
use frame_support::{
    pallet_prelude::*,
    sp_runtime::{SaturatedConversion, Saturating},
};
pub use pallet::*;
use sp_std::{prelude::*, str, vec::Vec};

type RewardPausedAt<T> = <T as frame_system::Config>::BlockNumber;
type RewardRecoveredAt<T> = <T as frame_system::Config>::BlockNumber;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use dbc_support::ONE_HOUR;
    use frame_system::{
        ensure_root,
        pallet_prelude::{BlockNumberFor, OriginFor},
    };

    #[pallet::config]
    pub trait Config: frame_system::Config + rent_machine::Config + online_profile::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    }

    #[pallet::storage]
    #[pallet::getter(fn dlc_machine_ids_in_staking)]
    pub type DLCMachineIdsInStaking<T: Config> = StorageValue<_, Vec<MachineId>, ValueQuery>;

    #[pallet::type_value]
    pub fn PhaseOneStartThresholdDefault<T: Config>() -> u64 {
        500
    }
    #[pallet::storage]
    #[pallet::getter(fn pahse_one_start_threshold)]
    pub type PhaseOneStartThreshold<T: Config> =
        StorageValue<_, u64, ValueQuery, PhaseOneStartThresholdDefault<T>>;

    #[pallet::storage]
    #[pallet::getter(fn phase_one_dlc_machine_ids_in_staking)]
    pub type PhaseOneDLCMachineIdsInStaking<T: Config> =
        StorageValue<_, Vec<MachineId>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn pahse_one_reward_start_at)]
    pub type PhaseOneRewardStartAt<T: Config> = StorageValue<_, T::BlockNumber, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn pahse_one_reward_paused_details)]
    pub type PhaseOneRewardPausedDetails<T: Config> =
        StorageValue<_, Vec<(RewardPausedAt<T>, RewardRecoveredAt<T>)>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn pahse_one_gpu_type_2_number_in_staking)]
    pub type PhaseOneGPUType2NumberInStaking<T: Config> =
        StorageMap<_, Blake2_128Concat, Vec<u8>, u64, ValueQuery>;

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        ReportDLCStaking(T::AccountId, MachineId),
    }

    #[pallet::error]
    pub enum Error<T> {
        InvalidValue,
        RenterNotOwner,
        AlreadyInStaking,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(block_number: T::BlockNumber) -> Weight {
            if block_number.saturated_into::<u64>() % ONE_HOUR as u64 == 0 {
                DLCMachineIdsInStaking::<T>::mutate(|machine_ids| {
                    machine_ids.retain(|machine_id| {
                        let mut should_retain = true;
                        let machine_info_result =
                            online_profile::Pallet::<T>::machines_info(machine_id);
                        if let Some(machine_info) = machine_info_result {
                            // must be rented
                            if machine_info.machine_status != MachineStatus::Rented {
                                should_retain = false
                            };

                            // must be rented by owner
                            if machine_info.renters.len() != 1 {
                                should_retain = false
                            }

                            let renter_result = machine_info.renters.last();
                            if renter_result.is_none() {
                                should_retain = false
                            }

                            let renter = renter_result.unwrap().clone();

                            if machine_info.controller != renter &&
                                machine_info.machine_stash != renter
                            {
                                should_retain = false
                            }
                        } else {
                            should_retain = false
                        }
                        if !should_retain {
                            frame_support::log::info!(
                                "remove machine_id : {}",
                                str::from_utf8(&machine_id).unwrap()
                            );
                        }
                        return should_retain
                    });
                });
            }
            Weight::zero()
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(10000, 0))]
        pub fn set_phase_one_start_threshold(
            origin: OriginFor<T>,
            value: u64,
        ) -> DispatchResultWithPostInfo {
            let _who = ensure_root(origin)?;
            ensure!(value > 0, Error::<T>::InvalidValue);
            PhaseOneStartThreshold::<T>::put(value);
            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(10000, 0))]
        pub fn reset_phase_one(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            // PhaseOneDLCMachineIdsInStaking::<T>::kill();
            PhaseOneRewardStartAt::<T>::kill();
            PhaseOneRewardPausedDetails::<T>::kill();
            let _ = PhaseOneGPUType2NumberInStaking::<T>::clear(100, None);
            Ok(().into())
        }
    }
}

impl<T: Config> DLCMachineReportStakingTrait for Pallet<T> {
    type BlockNumber = T::BlockNumber;
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
            return Err(Error::<T>::RenterNotOwner.as_str())
        }

        DLCMachineIdsInStaking::<T>::mutate(|ids| {
            if ids.contains(&machine_id) {
                return Err(Error::<T>::AlreadyInStaking.as_str())
            };
            ids.push(machine_id.clone());
            Ok(())
        })?;

        let stakeholder = account_id::<T>(from)?;
        Self::deposit_event(Event::ReportDLCStaking(stakeholder, machine_id));
        Ok(())
    }

    fn report_phase_one_dlc_nft_staking(
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
            return Err(Error::<T>::RenterNotOwner.as_str())
        }

        let machine_info_result = online_profile::Pallet::<T>::machines_info(machine_id.clone());
        if let Some(machine_info) = machine_info_result {
            let gpu_type = machine_info.machine_info_detail.committee_upload_info.gpu_type;
            if PhaseOneGPUType2NumberInStaking::<T>::contains_key(gpu_type.clone()) {
                let gpu_number = Self::pahse_one_gpu_type_2_number_in_staking(gpu_type.clone());
                PhaseOneGPUType2NumberInStaking::<T>::insert(
                    gpu_type,
                    gpu_number.saturating_add(1),
                );
            } else {
                PhaseOneGPUType2NumberInStaking::<T>::insert(gpu_type, 1);
            }
        }

        DLCMachineIdsInStaking::<T>::mutate(|ids| {
            if ids.contains(&machine_id) {
                return Err(Error::<T>::AlreadyInStaking.as_str())
            };
            ids.push(machine_id.clone());
            Ok(())
        })?;
        if PhaseOneGPUType2NumberInStaking::<T>::iter().count() as u64 >=
            Self::pahse_one_start_threshold() &&
            Self::get_phase_one_reward_start_at().saturated_into::<u64>() == 0
        {
            PhaseOneRewardStartAt::<T>::put(<frame_system::Pallet<T>>::block_number());
        }

        let details = Self::pahse_one_reward_paused_details();
        if details.len() > 0 {
            PhaseOneRewardPausedDetails::<T>::mutate(
                |paused_details: &mut Vec<(RewardPausedAt<T>, RewardRecoveredAt<T>)>| {
                    if let Some((_, recovered_at)) = paused_details.last_mut() {
                        if (*recovered_at).saturated_into::<u64>() == 0 {
                            *recovered_at = <frame_system::Pallet<T>>::block_number();
                        }
                    }
                },
            )
        }

        let stakeholder = account_id::<T>(from)?;
        Self::deposit_event(Event::ReportDLCStaking(stakeholder, machine_id));
        Ok(())
    }

    fn report_phase_one_dlc_nft_end_staking(
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
            return Err(Error::<T>::RenterNotOwner.as_str())
        }

        if !Self::dlc_machine_ids_in_staking().contains(&machine_id) {
            return Ok(())
        }

        let machine_info_result = online_profile::Pallet::<T>::machines_info(machine_id.clone());
        if let Some(machine_info) = machine_info_result {
            let gpu_type = machine_info.machine_info_detail.committee_upload_info.gpu_type;
            let gpu_number_before = Self::pahse_one_gpu_type_2_number_in_staking(gpu_type.clone());
            if gpu_number_before == 1 {
                PhaseOneGPUType2NumberInStaking::<T>::remove(gpu_type);
            } else {
                PhaseOneGPUType2NumberInStaking::<T>::insert(
                    gpu_type,
                    gpu_number_before.saturating_sub(1),
                );
            }

            if Self::get_phase_one_reward_start_at().saturated_into::<u64>() > 0 {
                let gpu_number = PhaseOneGPUType2NumberInStaking::<T>::iter().count() as u64;
                let phase_one_start_threshold = Self::pahse_one_start_threshold();
                if gpu_number < phase_one_start_threshold &&
                    gpu_number_before >= phase_one_start_threshold
                {
                    PhaseOneRewardPausedDetails::<T>::mutate(|details| {
                        details.push((
                            <frame_system::Pallet<T>>::block_number(),
                            T::BlockNumber::default(),
                        ));
                    })
                }
            }
        }

        DLCMachineIdsInStaking::<T>::mutate(|ids| ids.retain(|id| id != &machine_id));
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
            return Err(Error::<T>::RenterNotOwner.as_str())
        }

        if Self::dlc_machine_ids_in_staking().contains(&machine_id) {
            return Ok(())
        }

        DLCMachineIdsInStaking::<T>::mutate(|ids| ids.retain(|id| id != &machine_id));
        Ok(())
    }

    fn get_valid_reward_duration(
        last_claim_at: Self::BlockNumber,
        total_stake_duration: Self::BlockNumber,
        phase_number: u64,
    ) -> Self::BlockNumber {
        if Self::pahse_one_reward_start_at() == T::BlockNumber::default() {
            return T::BlockNumber::default()
        }

        let mut reward_paused_duration = T::BlockNumber::default();
        let mut reward_paused_details: Vec<(RewardPausedAt<T>, RewardRecoveredAt<T>)> = Vec::new();
        if phase_number == 1 {
            reward_paused_details = Self::pahse_one_reward_paused_details();
        }
        if reward_paused_details.len() > 0 {
            reward_paused_details.iter().for_each(
                |(paused_at_block_number, recovered_at_block_number)| {
                    if (*recovered_at_block_number).saturated_into::<u64>() > 0 {
                        if last_claim_at < *recovered_at_block_number &&
                            last_claim_at >= *paused_at_block_number
                        {
                            reward_paused_duration +=
                                recovered_at_block_number.saturating_sub(*paused_at_block_number);
                        }
                        if last_claim_at < *paused_at_block_number {
                            reward_paused_duration +=
                                recovered_at_block_number.saturating_sub(*paused_at_block_number);
                        }
                    } else {
                        if last_claim_at < *paused_at_block_number {
                            reward_paused_duration += <frame_system::Pallet<T>>::block_number()
                                .saturating_sub(*paused_at_block_number);
                        } else {
                            reward_paused_duration = total_stake_duration;
                        }
                    }
                },
            );
            return total_stake_duration - reward_paused_duration
        }
        total_stake_duration
    }

    fn get_phase_one_reward_start_at() -> Self::BlockNumber {
        Self::pahse_one_reward_start_at()
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
