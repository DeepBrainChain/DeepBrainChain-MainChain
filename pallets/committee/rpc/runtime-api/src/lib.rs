#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::unnecessary_mut_passed)]

use committee::CommitteeList;

// Here we declare the runtime API. It is implemented it the `impl` block in
// runtime amalgamator file (the `runtime/src/lib.rs`)
sp_api::decl_runtime_apis! {
    pub trait CmRpcApi<AccountId> where
        AccountId: codec::Codec + Ord,
    {
        fn get_committee_list() -> CommitteeList<AccountId>;
    }
}
