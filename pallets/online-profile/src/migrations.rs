use crate::{
    BalanceOf, Config, ErasStashPoints, LiveMachines, MachineId, MachinesInfo, Pallet,
    PendingSlash, Phase1Destruction, Phase2Destruction, StandardGPUPointPrice, StashMachines,
    StashStake, StorageVersion, SysInfo,
};
use frame_support::{pallet_prelude::*, storage_alias, traits::OnRuntimeUpgrade};
use frame_support::pallet_prelude::StorageValue;
use codec::{Decode, Encode};
use dbc_support::{
    machine_info::MachineInfo,
    machine_type::{MachineInfoDetail, MachineStatus},
    verify_slash::{OPPendingSlashInfo, OPSlashReason},
    EraIndex,
};
use frame_support::{debug::info, traits::Get, weights::Weight, IterableStorageMap, RuntimeDebug};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{
    traits::{Saturating, Zero},
    Perbill, SaturatedConversion,
};
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
            ((info.total_rented_duration as u32).saturating_mul(ONE_DAY)).saturated_into();
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

    let storage_version = StorageVersion::<T>::get();

    if storage_version <= 1 {
        // NOTE: Update storage version.
        StorageVersion::<T>::put(2);
        migrate_machine_info_to_v2::<T>().saturating_add(migrate_pending_slash_to_v2::<T>())
    } else if storage_version == 2 {
        StorageVersion::<T>::put(3);
        fix_slashed_online_machine::<T>() +
            fix_online_rent_orders::<T>() +
            regenerate_sys_info::<T>() +
            reset_params::<T>()
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

// 需要变更：LiveMachine, MachinesInfo, StashMachine,
// 需要重新生成: PosGPUInfo, SysInfo

// NOTE: 这个应该要首先触发迁移，以在最后修复SysInfo等总计信息
// 修复机器因未确认租用就举报成功时，机器从stash_machine.online_machine移除
// 但仍存在于LiveMachine.online_machine及机器状态是Online的问题
fn fix_slashed_online_machine<T: Config>() -> Weight {
    let all_stash = <StashMachines<T> as IterableStorageMap<T::AccountId, _>>::iter()
        .map(|(stash, _)| stash)
        .collect::<Vec<_>>();

    for stash in all_stash {
        let stash_machines = Pallet::<T>::stash_machines(&stash);
        // 只处理没有任何Online的机器的情况
        if !stash_machines.online_machine.is_empty() {
            continue
        }

        let stash_reserved = Pallet::<T>::stash_stake(&stash);
        // 将所有的机器ID信息移除
        if stash_reserved == Zero::zero() {
            for machine_id in stash_machines.total_machine {
                MachinesInfo::<T>::remove(&machine_id);
                LiveMachines::<T>::mutate(|live_machine| {
                    live_machine.clean(&machine_id);
                });
            }
        } else {
            // NOTE: 判断是否是机器主动下线的情况
            let mut is_all_slashed = true;
            for machine_id in stash_machines.total_machine {
                let machine_info = Pallet::<T>::machines_info(&machine_id);
                if !matches!(
                    machine_info.machine_status,
                    MachineStatus::ReporterReportOffline(..) | MachineStatus::Online
                ) {
                    is_all_slashed = false;
                }

                MachinesInfo::<T>::remove(&machine_id);
                LiveMachines::<T>::mutate(|live_machine| {
                    live_machine.clean(&machine_id);
                });
            }

            if is_all_slashed {
                let stash_stake = Pallet::<T>::stash_stake(&stash);
                // 惩罚到国库
                let _ = Pallet::<T>::slash_and_reward(stash.clone(), stash_stake, vec![]);
                StashStake::<T>::remove(&stash);
            } else {
                // 有主动下线的机器，进行退还质押
                // 对Stash解质押
                let stash_stake = Pallet::<T>::stash_stake(&stash);
                let _ = Pallet::<T>::change_stake(&stash, stash_stake, false);
                StashStake::<T>::remove(&stash);
            }
        }
    }

    0
}

fn fix_online_rent_orders<T: Config>() -> Weight {
    let all_machine_id = <MachinesInfo<T> as IterableStorageMap<MachineId, _>>::iter()
        .map(|(machine_id, _)| machine_id)
        .collect::<Vec<_>>();
    for machine_id in all_machine_id {
        MachinesInfo::<T>::mutate(&machine_id, |machine_info| {
            // NOTE: 将不是Rented状态的机器的租用人都重置为默认值
            if !matches!(machine_info.machine_status, MachineStatus::Rented) {
                machine_info.renters = vec![];
            }
        });
    }

    0
}

fn regenerate_sys_info<T: Config>() -> Weight {
    let all_stash = <StashMachines<T> as IterableStorageMap<T::AccountId, _>>::iter()
        .map(|(stash, _)| stash)
        .collect::<Vec<_>>();

    let mut total_staker: u64 = 0;
    let mut total_stake: BalanceOf<T> = Zero::zero();
    let mut total_calc_points: u64 = 0;
    let mut total_gpu_num: u64 = 0;
    let mut total_rented_gpu: u64 = 0;
    for stash in all_stash {
        let stash_machine = Pallet::<T>::stash_machines(&stash);
        if !stash_machine.online_machine.is_empty() {
            total_staker = total_staker.saturating_add(1);
            let stash_stake = Pallet::<T>::stash_stake(&stash);
            total_stake = total_stake.saturating_add(stash_stake);
            total_calc_points = total_calc_points.saturating_add(stash_machine.total_calc_points);
            total_gpu_num = total_gpu_num.saturating_add(stash_machine.total_gpu_num);
            total_rented_gpu = total_rented_gpu.saturating_add(stash_machine.total_rented_gpu);
        }
    }

    SysInfo::<T>::mutate(|sys_info| {
        sys_info.total_staker = total_staker;
        sys_info.total_stake = total_stake;
        sys_info.total_rented_gpu = total_rented_gpu;
        sys_info.total_calc_points = total_calc_points;
        sys_info.total_gpu_num = total_gpu_num;
    });

    // NOTE: 要重新计算 EraStashPoints.total 以修复多次退出未成功造成的该值小于实际值
    let current_era = Pallet::<T>::current_era();
    let next_era = current_era.saturating_add(1);
    ErasStashPoints::<T>::mutate(current_era, |era_stash_points| {
        era_stash_points.total = total_calc_points;
    });
    ErasStashPoints::<T>::mutate(next_era, |era_stash_points| {
        era_stash_points.total = total_calc_points;
    });

    0
}

// 1.销毁达到2500卡启动
// 2.单位算力值价格变更为60％
fn reset_params<T: Config>() -> Weight {
    let percent_50 = Perbill::from_rational(50u32, 100u32);
    let percent_100 = Perbill::from_rational(100u32, 100u32);

    Phase1Destruction::<T>::put((2500, percent_50, false));
    Phase2Destruction::<T>::put((5000, percent_100, false));

    let mut standard_gpu_point_price = Pallet::<T>::standard_gpu_point_price().unwrap_or_default();
    standard_gpu_point_price.gpu_price =
        Perbill::from_rational(60u32, 100u32) * standard_gpu_point_price.gpu_price;
    StandardGPUPointPrice::<T>::put(standard_gpu_point_price);

    0
}

