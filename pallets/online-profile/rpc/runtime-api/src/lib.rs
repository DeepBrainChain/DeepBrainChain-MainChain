#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::unnecessary_mut_passed)]
#![warn(unused_crate_dependencies)]

use dbc_support::{
    live_machine::LiveMachine,
    machine_info::MachineInfo,
    machine_type::{Latitude, Longitude},
    EraIndex, MachineId,
};
pub use online_profile::{rpc_types::StakerInfo, PosInfo, SysInfoDetail};
use parity_scale_codec::Codec;
use sp_runtime::traits::MaybeDisplay;
use sp_std::prelude::Vec;

// Here we declare the runtime API. It is implemented it the `impl` block in
// runtime amalgamator file (the `runtime/src/lib.rs`)
sp_api::decl_runtime_apis! {
    pub trait OpRpcApi<AccountId, Balance, BlockNumber> where
        AccountId: Codec + Ord,
        Balance: Codec + MaybeDisplay,
        BlockNumber: Codec + MaybeDisplay,
    {
        fn get_total_staker_num() -> u64;
        fn get_op_info() -> SysInfoDetail<Balance>;
        fn get_staker_info(account: AccountId) -> StakerInfo<Balance, BlockNumber, AccountId>;
        fn get_machine_list() -> LiveMachine;
        fn get_machine_info(machine_id: MachineId) -> Option<MachineInfo<AccountId, BlockNumber, Balance>>;
        fn get_pos_gpu_info() -> Vec<(Longitude, Latitude, PosInfo)>;
        fn get_machine_era_reward(machine_id: MachineId, era_index: EraIndex) -> Balance;
        fn get_machine_era_released_reward(machine_id: MachineId, era_index: EraIndex) -> Balance;
        fn get_stash_era_reward(stash: AccountId, era_index: EraIndex) -> Balance;
        fn get_stash_era_released_reward(stash: AccountId, era_index: EraIndex) -> Balance;
    }
}
