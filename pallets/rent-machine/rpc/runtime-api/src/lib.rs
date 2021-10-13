#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::unnecessary_mut_passed)]

use codec::Codec;
use generic_func::MachineId;
use rent_machine::RpcRentOrderDetail;
use sp_runtime::traits::MaybeDisplay;
use sp_std::prelude::Vec;

// Here we declare the runtime API. It is implemented it the `impl` block in
// runtime amalgamator file (the `runtime/src/lib.rs`)
sp_api::decl_runtime_apis! {
    pub trait RmRpcApi<AccountId, BlockNumber, Balance> where
        AccountId: codec::Codec + Ord,
        BlockNumber: Codec + MaybeDisplay,
        Balance: Codec + MaybeDisplay,
    {
        fn get_rent_order(renter: AccountId, machine_id: MachineId) -> RpcRentOrderDetail<AccountId, BlockNumber, Balance>;
        fn get_rent_list(renter: AccountId) -> Vec<MachineId>;
        fn get_machine_renter(machine_id: MachineId) -> Option<AccountId>;
    }
}
