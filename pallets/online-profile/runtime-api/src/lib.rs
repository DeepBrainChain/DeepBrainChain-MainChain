#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::unnecessary_mut_passed)]

use codec::Codec;
pub use online_profile::SysInfo;
use sp_runtime::traits::MaybeDisplay;

// Here we declare the runtime API. It is implemented it the `impl` block in
// runtime amalgamator file (the `runtime/src/lib.rs`)
sp_api::decl_runtime_apis! {
    pub trait SumStorageApi<Balance> where Balance: Codec + MaybeDisplay {
        fn get_sum() -> u32;
        fn get_op_info() -> SysInfo<Balance>;
    }
}
