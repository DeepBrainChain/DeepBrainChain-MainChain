#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::unnecessary_mut_passed)]
#![warn(unused_crate_dependencies)]

use codec::Codec;
use dbc_support::{
    verify_online::{OCCommitteeMachineList, OCMachineCommitteeList},
    MachineId,
};
use online_committee::rpc::RpcOCCommitteeOps;
use sp_runtime::traits::MaybeDisplay;

// Here we declare the runtime API. It is implemented it the `impl` block in
// runtime amalgamator file (the `runtime/src/lib.rs`)

sp_api::decl_runtime_apis! {
    pub trait OcRpcApi<AccountId, BlockNumber, Balance> where
        AccountId: codec::Codec + Ord,
        BlockNumber: Codec + MaybeDisplay,
        Balance: Codec + MaybeDisplay,
    {
        fn get_machine_committee_list(machine_id: MachineId) -> OCMachineCommitteeList<AccountId, BlockNumber>;
        fn get_committee_machine_list(committee: AccountId) -> OCCommitteeMachineList;
        fn get_committee_ops(committee: AccountId, machine_id: MachineId) -> Option<RpcOCCommitteeOps<BlockNumber, Balance>>;
    }
}
