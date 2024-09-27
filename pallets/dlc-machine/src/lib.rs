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
    traits::{DLCMachineReportStakingTrait, MachineInfoTrait, PhaseLevel},
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
    #[pallet::getter(fn dlc_machines_in_staking)]
    pub type DLCMachinesInStaking<T: Config> =
        StorageMap<_, Twox64Concat, MachineId, (), ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn dlc_machines_owner_rent_ended)]
    pub type DLCMachinesOwnerRentEnded<T: Config> =
        StorageMap<_, Twox64Concat, MachineId, (), ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn nft_staking_reward_start_threshold)]
    pub type NFTStakingRewardStartThreshold<T: Config> =
        StorageMap<_, Blake2_128Concat, PhaseLevel, u64, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn nft_staking_reward_start_at)]
    pub type NFTStakingRewardStartAt<T: Config> =
        StorageMap<_, Blake2_128Concat, PhaseLevel, T::BlockNumber, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn nft_staking_paused_details)]
    pub type NFTStakingPausedDetails<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        PhaseLevel,
        Vec<(RewardPausedAt<T>, RewardRecoveredAt<T>)>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn machine_id_2_gpu_count_of_idc_machine_nft_staking)]
    pub type MachineId2GPUCountInStakingOfIDCMachineNFTStaking<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        PhaseLevel,
        Blake2_128Concat,
        MachineId,
        u32,
        ValueQuery,
    >;

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        ReportDLCStaking(T::AccountId, MachineId),
        DLCMachinesOwnerRentEnded(MachineId),
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
                frame_support::log::info!("🔍 start clean up machine_ids_in_staking");
                let machine_id_to_remove: Vec<MachineId> = DLCMachinesInStaking::<T>::iter_keys()
                    .filter(|machine_id| {
                        let machine_info_result =
                            online_profile::Pallet::<T>::machines_info(machine_id);
                        if let Some(machine_info) = machine_info_result {
                            // must be rented
                            if machine_info.machine_status != MachineStatus::Rented {
                                return true
                            };

                            // must be rented by owner
                            if machine_info.renters.len() != 1 {
                                return true
                            }

                            let renter_result = machine_info.renters.last();
                            if renter_result.is_none() {
                                return true
                            }

                            let renter = renter_result.unwrap().clone();
                            if machine_info.controller != renter &&
                                machine_info.machine_stash != renter
                            {
                                return true
                            }
                        } else {
                            return true
                        }

                        return false
                    })
                    .collect();

                for machine_id in machine_id_to_remove {
                    frame_support::log::info!(
                        "remove machine_id : {}",
                        str::from_utf8(&machine_id).unwrap()
                    );
                    DLCMachinesOwnerRentEnded::<T>::insert(&machine_id, ());
                    Self::deposit_event(Event::DLCMachinesOwnerRentEnded(machine_id))
                    // DLCMachinesInStaking::<T>::remove(machine_id);
                    // MachineId2GPUCountInStakingOfIDCMachineNFTStaking::<T>::remove(&PhaseLevel::PhaseOne, &machine_id);
                    // MachineId2GPUCountInStakingOfIDCMachineNFTStaking::<T>::remove(&PhaseLevel::PhaseTwo, &machine_id);
                    // MachineId2GPUCountInStakingOfIDCMachineNFTStaking::<T>::remove(&PhaseLevel::PhaseThree, &machine_id);
                }
            }
            Weight::zero()
        }

        fn on_runtime_upgrade() -> frame_support::weights::Weight {
            if NFTStakingRewardStartThreshold::<T>::iter().count() as u32 == 0 {
                frame_support::log::info!("🔍 start set NFTStakingRewardStartThreshold");

                NFTStakingRewardStartThreshold::<T>::insert(PhaseLevel::PhaseOne, 500);
                NFTStakingRewardStartThreshold::<T>::insert(PhaseLevel::PhaseTwo, 1000);
                NFTStakingRewardStartThreshold::<T>::insert(PhaseLevel::PhaseThree, 2000);
            }

            Weight::zero()
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(10000, 0))]
        pub fn set_nft_staking_start_threshold(
            origin: OriginFor<T>,
            phase_level: PhaseLevel,
            value: u64,
        ) -> DispatchResultWithPostInfo {
            let _who = ensure_root(origin)?;
            ensure!(value > 0, Error::<T>::InvalidValue);
            NFTStakingRewardStartThreshold::<T>::insert(phase_level, value);
            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(10000, 0))]
        pub fn reset_nft_staking(origin: OriginFor<T>, limit: u32) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let _ = NFTStakingRewardStartAt::<T>::clear(limit, None);
            let _ = NFTStakingPausedDetails::<T>::clear(limit, None);
            let _ = MachineId2GPUCountInStakingOfIDCMachineNFTStaking::<T>::clear(limit, None);
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
        if DLCMachinesInStaking::<T>::contains_key(&machine_id) {
            return Err(Error::<T>::AlreadyInStaking.as_str())
        }

        DLCMachinesInStaking::<T>::insert(machine_id.clone(), ());

        let stakeholder = account_id::<T>(from)?;
        Self::deposit_event(Event::ReportDLCStaking(stakeholder, machine_id));
        Ok(())
    }

    fn report_dlc_nft_staking(
        data: Vec<u8>,
        sig: sp_core::sr25519::Signature,
        from: sp_core::sr25519::Public,
        machine_id: MachineId,
        phase_level: PhaseLevel,
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

        if DLCMachinesInStaking::<T>::contains_key(&machine_id) {
            return Err(Error::<T>::AlreadyInStaking.as_str())
        }

        DLCMachinesInStaking::<T>::insert(machine_id.clone(), ());

        let machine_info_result = online_profile::Pallet::<T>::machines_info(machine_id.clone());
        if let Some(machine_info) = machine_info_result {
            {
                MachineId2GPUCountInStakingOfIDCMachineNFTStaking::<T>::insert(
                    phase_level.clone(),
                    &machine_id,
                    machine_info.machine_info_detail.committee_upload_info.gpu_num,
                );
            }
        }

        let mut gpu_count = 0u32;
        for (_, gpu_count_of_one_machine) in
            MachineId2GPUCountInStakingOfIDCMachineNFTStaking::<T>::iter_prefix(&phase_level)
        {
            gpu_count = gpu_count.saturating_add(gpu_count_of_one_machine);
        }

        if gpu_count as u64 >= Self::nft_staking_reward_start_threshold(&phase_level) &&
            Self::get_nft_staking_reward_start_at(&phase_level).saturated_into::<u64>() == 0
        {
            NFTStakingRewardStartAt::<T>::insert(
                &phase_level,
                <frame_system::Pallet<T>>::block_number(),
            );
        }

        let details = Self::nft_staking_paused_details(&phase_level);
        if details.len() > 0 {
            NFTStakingPausedDetails::<T>::mutate(
                &phase_level,
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

    fn report_dlc_nft_end_staking(
        data: Vec<u8>,
        sig: sp_core::sr25519::Signature,
        from: sp_core::sr25519::Public,
        machine_id: MachineId,
        phase_level: PhaseLevel,
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

        if !DLCMachinesInStaking::<T>::contains_key(&machine_id) {
            return Ok(())
        }

        let gpu_count_of_the_machine =
            Self::machine_id_2_gpu_count_of_idc_machine_nft_staking(&phase_level, &machine_id);

        MachineId2GPUCountInStakingOfIDCMachineNFTStaking::<T>::remove(&phase_level, &machine_id);

        let mut gpu_count = 0u32;
        for (_, gpu_count_of_one_machine) in
            MachineId2GPUCountInStakingOfIDCMachineNFTStaking::<T>::iter_prefix(&phase_level)
        {
            gpu_count = gpu_count.saturating_add(gpu_count_of_one_machine);
        }

        let gpu_count_before = gpu_count.saturating_add(gpu_count_of_the_machine) as u64;

        if Self::get_nft_staking_reward_start_at(&phase_level).saturated_into::<u64>() > 0 {
            let start_threshold = Self::nft_staking_reward_start_threshold(&phase_level);
            if ((gpu_count as u64) < start_threshold) && (gpu_count_before >= start_threshold) {
                NFTStakingPausedDetails::<T>::mutate(&phase_level, |details| {
                    details.push((
                        <frame_system::Pallet<T>>::block_number(),
                        T::BlockNumber::default(),
                    ));
                })
            }
        }

        DLCMachinesInStaking::<T>::remove(&machine_id);
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

        if DLCMachinesInStaking::<T>::contains_key(&machine_id) {
            return Ok(())
        }

        DLCMachinesInStaking::<T>::remove(&machine_id);
        Ok(())
    }

    fn get_nft_staking_valid_reward_duration(
        last_claim_at: Self::BlockNumber,
        total_stake_duration: Self::BlockNumber,
        phase_level: PhaseLevel,
    ) -> Self::BlockNumber {
        if Self::nft_staking_reward_start_at(&phase_level) == T::BlockNumber::default() {
            return T::BlockNumber::default()
        }

        let mut reward_paused_duration = T::BlockNumber::default();
        let reward_paused_details = Self::nft_staking_paused_details(&phase_level);
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

    fn get_nft_staking_reward_start_at(phase_level: &PhaseLevel) -> Self::BlockNumber {
        Self::nft_staking_reward_start_at(phase_level)
    }
}

impl<T: Config> Pallet<T> {
    pub fn dlc_machine_in_staking(machine_id: MachineId) -> bool {
        DLCMachinesInStaking::<T>::contains_key(&machine_id)
    }

    pub fn report_dlc_machine_slashed(machine_id: MachineId) {
        DLCMachinesInStaking::<T>::remove(&machine_id);
    }
}
