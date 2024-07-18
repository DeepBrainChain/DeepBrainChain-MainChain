use frame_support::{
    pallet_prelude::*,
    storage_alias,
    traits::{OnRuntimeUpgrade, StorageInstance},
};
use Config;

use crate::*;
use sp_std::prelude::*;

const TARGET: &'static str = "online-profile-migration";

type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;

mod v0 {
    use dbc_support::{
        machine_type::{CommitteeUploadInfo, Latitude, Longitude, MachineStatus},
        EraIndex,
    };
    use frame_support::{
        dispatch::{Decode, Encode, TypeInfo},
        RuntimeDebug,
    };
    #[cfg(feature = "std")]
    use serde::{Deserialize, Serialize};
    use sp_core::H256;

    #[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
    #[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
    #[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
    pub struct MachineInfo<AccountId: Ord, BlockNumber, Balance> {
        /// Who can control this machine
        pub controller: AccountId,
        /// Who own this machine and will get machine's reward
        pub machine_stash: AccountId,
        /// Last machine renter
        pub renters: Vec<AccountId>,
        /// Every 365 days machine can restake(For token price maybe changed)
        pub last_machine_restake: BlockNumber,
        /// When controller bond this machine
        pub bonding_height: BlockNumber,
        /// When machine is passed verification and is online
        pub online_height: BlockNumber,
        /// Last time machine is online
        /// (When first online; Rented -> Online, Offline -> Online e.t.)
        pub last_online_height: BlockNumber,
        /// When first bond_machine, record how much should stake per GPU
        pub init_stake_per_gpu: Balance,
        /// How much machine staked
        pub stake_amount: Balance,
        /// Status of machine
        pub machine_status: MachineStatus<BlockNumber, AccountId>,
        /// How long machine has been rented(will be update after one rent is end)
        /// NOTE: 单位从天改为BlockNumber
        pub total_rented_duration: BlockNumber,
        /// How many times machine has been rented
        pub total_rented_times: u64,
        /// How much rent fee machine has earned for rented(before Galaxy is ON)
        pub total_rent_fee: Balance,
        /// How much rent fee is burn after Galaxy is ON
        pub total_burn_fee: Balance,
        /// Machine's hardware info
        pub machine_info_detail: MachineInfoDetail,
        /// Committees, verified machine and will be rewarded in the following days.
        /// (After machine is online, get 1% rent fee)
        pub reward_committee: Vec<AccountId>,
        /// When reward will be over for committees
        pub reward_deadline: EraIndex,
    }

    #[derive(Debug, PartialEq, Eq, Clone, Encode, Decode, Default, TypeInfo)]
    #[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
    pub struct MachineInfoDetail {
        pub committee_upload_info: CommitteeUploadInfo,
        pub staker_customize_info: StakerCustomizeInfo,
    }

    #[derive(Debug, PartialEq, Eq, Clone, Encode, Decode, Default, TypeInfo)]
    #[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
    pub struct StakerCustomizeInfo {
        pub server_room: H256,
        /// 上行带宽
        pub upload_net: u64,
        /// 下行带宽
        pub download_net: u64,
        /// 经度(+东经; -西经)
        pub longitude: Longitude,
        /// 纬度(+北纬； -南纬)
        pub latitude: Latitude,
        /// 网络运营商
        pub telecom_operators: Vec<Vec<u8>>,
    }

    use super::*;
    #[storage_alias]
    pub type OldMachinesInfo<T: Config> = StorageMap<
        Pallet<T>,
        Blake2_128Concat,
        MachineId,
        MachineInfo<AccountIdOf<T>, BlockNumberOf<T>, BalanceOf<T>>,
    >;
}
pub mod v1 {
    use super::*;
    use dbc_support::machine_type::MachineInfoDetail;
    use frame_support::{
        migration::{storage_iter, storage_key_iter},
    };

    pub struct Migration<T>(PhantomData<T>);
    impl<T: Config> OnRuntimeUpgrade for Migration<T> {
        fn on_runtime_upgrade() -> Weight {
            migrate::<T>()
        }

        #[cfg(feature = "try-runtime")]
        fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
            log::info!("pre_upgrade ok");
            let current_version = Pallet::<T>::current_storage_version();
            let on_chain_version = Pallet::<T>::on_chain_storage_version();

            log::info!("c : {:?} ", current_version);
            log::info!("o : {:?}", on_chain_version);

            // ensure!(on_chain_version == 0, "this migration can be deleted");
            Ok(Vec::new())
        }

        #[cfg(feature = "try-runtime")]
        fn post_upgrade(_state: Vec<u8>) -> Result<(), &'static str> {
            let on_chain_version = Pallet::<T>::on_chain_storage_version();

            // ensure!(on_chain_version == 1, "this migration needs to be removed");

            log::info!("post_upgrade ok");
            Ok(())
        }
    }

    pub fn migrate<T: Config>() -> Weight {
        let on_chain_version = Pallet::<T>::on_chain_storage_version();
        let current_version = Pallet::<T>::current_storage_version();
        let mut weight = T::DbWeight::get().reads(2);

        // if on_chain_version == 0 && current_version == 1 {
            log::info!(target: TARGET, "migrate executing");

            // MachinesInfo::<T>::translate(
            //     |index, old: v0::MachineInfo<AccountIdOf<T>, BlockNumberOf<T>, BalanceOf<T>>| {
            //         weight.saturating_accrue(T::DbWeight::get().reads_writes(1, 1));
            //
            //         let new_machine_info = MachineInfo {
            //             controller: old.controller,
            //             machine_stash: old.machine_stash,
            //             renters: old.renters,
            //             last_machine_restake: old.last_machine_restake,
            //             bonding_height: old.bonding_height,
            //             online_height: old.online_height,
            //             last_online_height: old.last_online_height,
            //             init_stake_per_gpu: old.init_stake_per_gpu,
            //             stake_amount: old.stake_amount,
            //             machine_status: old.machine_status,
            //             total_rented_duration: old.total_rented_duration,
            //             total_rented_times: old.total_rented_times,
            //             total_rent_fee: old.total_rent_fee,
            //             total_burn_fee: old.total_burn_fee,
            //             machine_info_detail: MachineInfoDetail {
            //                 staker_customize_info: StakerCustomizeInfo {
            //                     server_room: old
            //                         .machine_info_detail
            //                         .staker_customize_info
            //                         .server_room,
            //                     upload_net: old
            //                         .machine_info_detail
            //                         .staker_customize_info
            //                         .upload_net,
            //                     download_net: old
            //                         .machine_info_detail
            //                         .staker_customize_info
            //                         .download_net,
            //                     longitude: old.machine_info_detail.staker_customize_info.longitude,
            //                     latitude: old.machine_info_detail.staker_customize_info.latitude,
            //                     telecom_operators: old
            //                         .machine_info_detail
            //                         .staker_customize_info
            //                         .telecom_operators,
            //                     is_bare_machine: false,
            //                 },
            //                 committee_upload_info: old.machine_info_detail.committee_upload_info,
            //             },
            //
            //             reward_committee: old.reward_committee,
            //             reward_deadline: old.reward_deadline,
            //         };
            //         Some(new_machine_info)
            //     },
            // );

            StashMachines::<T>::translate(
                |stash, old: StashMachine<BalanceOf<T>>| {
                    weight.saturating_accrue(T::DbWeight::get().reads(1));

                    let mut total_online_gpu_num: u64 = 0;
                    old.online_machine.iter().for_each(|machine_id| {
                        weight.saturating_accrue(T::DbWeight::get().reads(1));
                        let  machine_info_result = MachinesInfo::<T>::get(machine_id);
                        if let Some(machine_info) = machine_info_result {
                                total_online_gpu_num += machine_info.gpu_num() as u64;
                        }
                    });
                    if total_online_gpu_num != old.total_gpu_num{
                        log::info!("old.total_gpu_num: {}, total_online_gpu_num: {}",old.total_gpu_num, total_online_gpu_num);
                        weight.saturating_accrue(T::DbWeight::get().writes( 1));
                        if old.online_machine.len() as u64 != old.total_gpu_num{
                            return Some(StashMachine {
                                total_gpu_num: total_online_gpu_num,
                                ..old
                            })
                        }
                    }
                    Some(old)
                }
            );

            let machine_id = "601d50086714f19a24ae378be63167e75ce8d22aa798548ee24b1c91c4609a61".as_bytes().to_vec();
            let result = MachinesInfo::<T>::get(&machine_id);
            // let result = MachinesInfo::<T>::get("c64f005ade44d989e067de03cf46aaa01fd71dbb717503a5e43ae588efb90065".as_bytes().to_vec());
            if let Some(mut machine_info) = result {
               if let MachineStatus::ReporterReportOffline(reason,last_status,a,b) = machine_info.machine_status{
                    if let MachineStatus::ReporterReportOffline(reason,inner_last_status,a,b) = *last_status{
                        machine_info.machine_status = MachineStatus::ReporterReportOffline(reason,inner_last_status,a,b);
                        log::info!("machine_info.machine_status: {:?}",machine_info.machine_status);
                        MachinesInfo::<T>::insert(machine_id, machine_info);
                    }
                }
            }
            // current_version.put::<Pallet<T>>()
        // }

        log::info!("migrate ok");
        weight
    }
}
