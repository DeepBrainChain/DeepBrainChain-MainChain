#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::unnecessary_mut_passed)]

use codec::Codec;
use sp_runtime::traits::MaybeDisplay;
use sp_std::prelude::Vec;

use dbc_support::{
    rental_type::{MachineGPUOrder, RentOrderDetail},
    verify_online::{OCCommitteeMachineList, OCMachineCommitteeList},
    MachineId, RentOrderId,
};
use terminating_rental::{
    rpc_types::{RpcIRCommitteeOps, StakerInfo},
    IRLiveMachine, IRMachineInfo,
};

// Here we declare the runtime API. It is implemented it the `impl` block in
// runtime amalgamator file (the `runtime/src/lib.rs`)
sp_api::decl_runtime_apis! {
    pub trait IrRpcApi<AccountId, Balance, BlockNumber> where
        AccountId: codec::Codec + Ord,
        Balance: Codec + MaybeDisplay,
        BlockNumber: Codec + MaybeDisplay,
    {
        fn get_total_staker_num() -> u64;
        fn get_staker_info(account: AccountId) -> StakerInfo<Balance, BlockNumber, AccountId>;
        fn get_machine_list() -> IRLiveMachine;
        fn get_machine_info(machine_id: MachineId) -> IRMachineInfo<AccountId, BlockNumber, Balance>;

        fn get_machine_committee_list(machine_id: MachineId) -> OCMachineCommitteeList<AccountId, BlockNumber>;
        fn get_committee_machine_list(committee: AccountId) -> OCCommitteeMachineList;
        fn get_committee_ops(committee: AccountId, machine_id: MachineId) -> RpcIRCommitteeOps<BlockNumber, Balance>;


        fn get_rent_order(rent_id: RentOrderId) -> RentOrderDetail<AccountId, BlockNumber, Balance>;
        fn get_rent_list(renter: AccountId) -> Vec<RentOrderId>;
        fn is_machine_renter(machine_id: MachineId, renter: AccountId) -> bool;
        fn get_machine_rent_id(machine_id: MachineId) -> MachineGPUOrder;
    }
}