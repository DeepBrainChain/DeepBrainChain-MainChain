use crate::{BalanceOf, Config, MachineId, MachinesInfo, PendingSlash, StorageVersion};
use codec::{Decode, Encode};
use dbc_support::{
    machine_info::MachineInfo,
    machine_type::{MachineInfoDetail, MachineStatus},
    verify_slash::{OPPendingSlashInfo, OPSlashReason},
    EraIndex,
};
use frame_support::{debug::info, traits::Get, weights::Weight, RuntimeDebug};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::SaturatedConversion;
use sp_std::{vec, vec::Vec};

// machine_info:
//    .machine_status: creating -> online (creating状态被弃用)
//    .total_rented_duration: 单位从天 -> BlockNumber
//    .last_machine_renter: Option<AccountId> -> .renters: Vec<AccountId>,

/// All details of a machine
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct OldMachineInfo<AccountId: Ord, BlockNumber, Balance> {
    pub controller: AccountId,
    pub machine_stash: AccountId,
    // NOTE: V2变更为 pub renters: Vec<AccountId>
    pub last_machine_renter: Option<AccountId>,
    pub last_machine_restake: BlockNumber,
    pub bonding_height: BlockNumber,
    pub online_height: BlockNumber,
    pub last_online_height: BlockNumber,
    pub init_stake_per_gpu: Balance,
    pub stake_amount: Balance,
    pub machine_status: MachineStatus<BlockNumber, AccountId>,
    /// NOTE: V2单位从天改为BlockNumber
    pub total_rented_duration: u64,
    pub total_rented_times: u64,
    pub total_rent_fee: Balance,
    pub total_burn_fee: Balance,
    pub machine_info_detail: MachineInfoDetail,
    pub reward_committee: Vec<AccountId>,
    pub reward_deadline: EraIndex,
}

impl<AccountId, BlockNumber, Balance> From<OldMachineInfo<AccountId, BlockNumber, Balance>>
    for MachineInfo<AccountId, BlockNumber, Balance>
where
    AccountId: Ord,
    BlockNumber: From<u32> + sp_runtime::traits::Bounded,
{
    fn from(
        info: OldMachineInfo<AccountId, BlockNumber, Balance>,
    ) -> MachineInfo<AccountId, BlockNumber, Balance> {
        let renters = if info.last_machine_renter.is_some() {
            vec![info.last_machine_renter.unwrap()]
        } else {
            vec![]
        };
        let total_rented_duration =
            ((info.total_rented_duration as u32).saturating_mul(2880)).saturated_into();
        let machine_status = match info.machine_status {
            MachineStatus::Creating => MachineStatus::Rented,
            _ => info.machine_status,
        };
        MachineInfo {
            controller: info.controller,
            machine_stash: info.machine_stash,
            renters,
            last_machine_restake: info.last_machine_restake,
            bonding_height: info.bonding_height,
            online_height: info.online_height,
            last_online_height: info.last_online_height,
            init_stake_per_gpu: info.init_stake_per_gpu,
            stake_amount: info.stake_amount,
            machine_status,
            total_rented_duration,
            total_rented_times: info.total_rented_times,
            total_rent_fee: info.total_rent_fee,
            total_burn_fee: info.total_burn_fee,
            machine_info_detail: info.machine_info_detail,
            reward_committee: info.reward_committee,
            reward_deadline: info.reward_deadline,
        }
    }
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct OldOPPendingSlashInfo<AccountId, BlockNumber, Balance> {
    pub slash_who: AccountId,
    pub machine_id: MachineId,
    pub slash_time: BlockNumber,
    pub slash_amount: Balance,
    pub slash_exec_time: BlockNumber,

    // NOTE: V2变更为 reporter: Option<AccountId>,
    pub reward_to_reporter: Option<AccountId>,
    // NOTE: V2新增 renters: Vec<AccountId>,
    pub reward_to_committee: Option<Vec<AccountId>>,
    pub slash_reason: OPSlashReason<BlockNumber>,
}

impl<AccountId, BlockNumber, Balance> From<OldOPPendingSlashInfo<AccountId, BlockNumber, Balance>>
    for OPPendingSlashInfo<AccountId, BlockNumber, Balance>
where
    AccountId: Ord,
    BlockNumber: From<u32> + sp_runtime::traits::Bounded,
{
    fn from(
        info: OldOPPendingSlashInfo<AccountId, BlockNumber, Balance>,
    ) -> OPPendingSlashInfo<AccountId, BlockNumber, Balance> {
        OPPendingSlashInfo {
            slash_who: info.slash_who,
            machine_id: info.machine_id,
            slash_time: info.slash_time,
            slash_amount: info.slash_amount,
            slash_exec_time: info.slash_exec_time,
            reporter: info.reward_to_reporter,
            renters: vec![],
            reward_to_committee: info.reward_to_committee,
            slash_reason: info.slash_reason,
        }
    }
}

/// Apply all of the migrations due to taproot.
///
/// ### Warning
///
/// Use with care and run at your own risk.
pub fn apply<T: Config>() -> Weight {
    frame_support::debug::RuntimeLogger::init();

    info!(
        target: "runtime::online_profile",
        "Running migration for onlineProfile pallet"
    );

    if StorageVersion::<T>::get() <= 1 {
        // NOTE: Update storage version.
        StorageVersion::<T>::put(2);

        migrate_machine_info_to_v2::<T>().saturating_add(migrate_pending_slash_to_v2::<T>())
    } else {
        frame_support::debug::info!(" >>> Unused migration!");
        0
    }
}

fn migrate_machine_info_to_v2<T: Config>() -> Weight {
    MachinesInfo::<T>::translate::<OldMachineInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>, _>(
        |_, machine_info| Some(machine_info.into()),
    );
    let count = MachinesInfo::<T>::iter_values().count();

    info!(
        target: "runtime::onlineProfile",
        "migrated {} onlineProfile machineInfo.",
        count,
    );

    <T as frame_system::Config>::DbWeight::get()
        .reads_writes(count as Weight + 1, count as Weight + 1)
}

fn migrate_pending_slash_to_v2<T: Config>() -> Weight {
    PendingSlash::<T>::translate::<
        OldOPPendingSlashInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>,
        _,
    >(|_, slash_info| Some(slash_info.into()));
    let count = PendingSlash::<T>::iter_values().count();

    info!(
        target: "runtime::onlineProfile",
        "migrated {} onlineProfile PendingSlash.",
        count,
    );

    <T as frame_system::Config>::DbWeight::get()
        .reads_writes(count as Weight + 1, count as Weight + 1)
}
