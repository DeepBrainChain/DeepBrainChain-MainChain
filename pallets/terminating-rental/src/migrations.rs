use crate::{
    BalanceOf, Config, OnlineDeposit, ReporterStakeParams, StakePerGPU, StandardGPUPointPrice,
    StorageVersion,
};
use dbc_support::report::ReporterStakeParamsInfo;
use frame_support::{debug::info, weights::Weight};
use sp_runtime::{
    traits::{SaturatedConversion, Saturating},
    Perbill,
};

pub fn apply<T: Config>() -> Weight {
    frame_support::debug::RuntimeLogger::init();
    info!(
        target: "runtime::rent_machine",
        "Running migration for rentMachine pallet"
    );

    let storage_version = StorageVersion::<T>::get();
    if storage_version <= 1 {
        StorageVersion::<T>::put(2);
        migrate_to_v2::<T>();
    }
    0
}

fn migrate_to_v2<T: Config>() -> Weight {
    StandardGPUPointPrice::<T>::put(dbc_support::machine_type::StandardGpuPointPrice {
        gpu_point: 100,
        gpu_price: 13_550, // 28229 * 0.6 * 0.8
    });

    let one_dbc: BalanceOf<T> = 1_000_000_000_000_000_u64.saturated_into();
    StakePerGPU::<T>::put(Into::<BalanceOf<T>>::into(100_000u32).saturating_mul(one_dbc));
    ReporterStakeParams::<T>::put(ReporterStakeParamsInfo {
        stake_baseline: Into::<BalanceOf<T>>::into(20_000u32).saturating_mul(one_dbc),
        stake_per_report: Into::<BalanceOf<T>>::into(1_000u32).saturating_mul(one_dbc),
        min_free_stake_percent: Perbill::from_percent(40),
    });
    OnlineDeposit::<T>::put(Into::<BalanceOf<T>>::into(10_000u32).saturating_mul(one_dbc));

    0
}
