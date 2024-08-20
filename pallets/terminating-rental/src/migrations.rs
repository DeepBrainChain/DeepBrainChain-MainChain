use dbc_support::{
    verify_committee_slash::{OCPendingSlashInfo, OCSlashResult},
    verify_online::OCBookResultType,
    MachineId,
};
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_runtime::{RuntimeDebug};
use sp_std::vec::Vec;

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
struct OldOCPendingSlashInfo<AccountId, BlockNumber, Balance> {
    pub machine_id: MachineId,
    pub machine_stash: AccountId, // Changed to Option<AccountId>
    pub stash_slash_amount: Balance,
    // info refused, maybe slash amount is different
    pub inconsistent_committee: Vec<AccountId>,
    pub unruly_committee: Vec<AccountId>,
    pub reward_committee: Vec<AccountId>,
    pub committee_stake: Balance,
    pub slash_time: BlockNumber,
    pub slash_exec_time: BlockNumber,
    pub book_result: OCBookResultType,
    pub slash_result: OCSlashResult,
}
// A: AccountId, B: BlockNumber, C: Balance
impl<A, B, C> From<OldOCPendingSlashInfo<A, B, C>> for OCPendingSlashInfo<A, B, C> {
    fn from(info: OldOCPendingSlashInfo<A, B, C>) -> OCPendingSlashInfo<A, B, C> {
        OCPendingSlashInfo {
            machine_id: info.machine_id,
            machine_stash: None,
            stash_slash_amount: info.stash_slash_amount,
            inconsistent_committee: info.inconsistent_committee,
            unruly_committee: info.unruly_committee,
            reward_committee: info.reward_committee,
            committee_stake: info.committee_stake,
            slash_time: info.slash_time,
            slash_exec_time: info.slash_exec_time,
            book_result: info.book_result,
            slash_result: info.slash_result,
        }
    }
}

// pub fn migrate<T: Config>() {
//     <PendingOnlineSlash<T>>::translate(
//         |_key, old: OldOCPendingSlashInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>| {
//             Some(old.into())
//         },
//     );
// }

use frame_support::{pallet_prelude::*, storage_alias, traits::OnRuntimeUpgrade};
use Config;

use crate::*;
use sp_std::prelude::*;

const TARGET: &'static str = "terminating-rental-migration";

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

    pub struct Migration<T>(PhantomData<T>);
    impl<T: Config> OnRuntimeUpgrade for Migration<T> {
        fn on_runtime_upgrade() -> Weight {
            migrate::<T>()
        }

        #[cfg(feature = "try-runtime")]
        fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
            log::info!("pre_upgrade ok");
            let current_version = Pallet::<T>::current_storage_version();
            let on_chain_version = Pallet::<T>::on_chain_storage_version();

            log::info!("c : {:?} ", current_version);
            log::info!("o : {:?}", on_chain_version);

            ensure!(on_chain_version == 0, "this migration can be deleted");
            Ok(Vec::new())
        }

        #[cfg(feature = "try-runtime")]
        fn post_upgrade(_state: Vec<u8>) -> Result<(), TryRuntimeError> {
            let on_chain_version = Pallet::<T>::on_chain_storage_version();

            ensure!(on_chain_version == 1, "this migration needs to be removed");

            log::info!("post_upgrade ok");
            Ok(())
        }
    }

    pub fn migrate<T: Config>() -> Weight {
        let mut weight = T::DbWeight::get().reads(2);

        log::info!(target: TARGET, "migrate executing");

        MachinesInfo::<T>::translate(
            |_index, old: v0::MachineInfo<AccountIdOf<T>, BlockNumberOf<T>, BalanceOf<T>>| {
                weight.saturating_accrue(T::DbWeight::get().reads_writes(1, 1));

                let new_machine_info = MachineInfo {
                    controller: old.controller,
                    machine_stash: old.machine_stash,
                    renters: old.renters,
                    last_machine_restake: old.last_machine_restake,
                    bonding_height: old.bonding_height,
                    online_height: old.online_height,
                    last_online_height: old.last_online_height,
                    init_stake_per_gpu: old.init_stake_per_gpu,
                    stake_amount: old.stake_amount,
                    machine_status: old.machine_status,
                    total_rented_duration: old.total_rented_duration,
                    total_rented_times: old.total_rented_times,
                    total_rent_fee: old.total_rent_fee,
                    total_burn_fee: old.total_burn_fee,
                    machine_info_detail: MachineInfoDetail {
                        staker_customize_info: StakerCustomizeInfo {
                            server_room: old.machine_info_detail.staker_customize_info.server_room,
                            upload_net: old.machine_info_detail.staker_customize_info.upload_net,
                            download_net: old
                                .machine_info_detail
                                .staker_customize_info
                                .download_net,
                            longitude: old.machine_info_detail.staker_customize_info.longitude,
                            latitude: old.machine_info_detail.staker_customize_info.latitude,
                            telecom_operators: old
                                .machine_info_detail
                                .staker_customize_info
                                .telecom_operators,
                            is_bare_machine: false,
                        },
                        committee_upload_info: old.machine_info_detail.committee_upload_info,
                    },

                    reward_committee: old.reward_committee,
                    reward_deadline: old.reward_deadline,
                };
                Some(new_machine_info)
            },
        );

        log::info!("migrate ok");
        weight
    }
}
