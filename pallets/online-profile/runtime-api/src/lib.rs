#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::unnecessary_mut_passed)]

use codec::Codec;
use sp_std::prelude::Vec;
// use frame_support::pallet_prelude::Vec;
pub use online_profile::{StakerInfo, SysInfo};
use sp_runtime::traits::MaybeDisplay;

// Here we declare the runtime API. It is implemented it the `impl` block in
// runtime amalgamator file (the `runtime/src/lib.rs`)
sp_api::decl_runtime_apis! {
    pub trait SumStorageApi<AccountId, Balance> where
        AccountId: codec::Codec,
        Balance: Codec + MaybeDisplay,
    {
        fn get_sum() -> u32;
        fn get_op_info() -> SysInfo<Balance>;
        fn get_staker_info(account: AccountId) -> StakerInfo<Balance>;
        fn get_staker_list(start: u64, end: u64) -> Vec<AccountId>;
        fn get_staker_identity(account: AccountId) -> Vec<u8>;
    }
}
