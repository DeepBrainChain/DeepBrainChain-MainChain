use crate::{MachineId, RentOrderId};
use frame_support::{
    dispatch::{Decode, Encode, TypeInfo},
    RuntimeDebug,
};
use sp_core::H160;
use sp_std::vec::Vec;

pub trait PhaseReward {
    type Balance;

    fn set_phase0_reward(balance: Self::Balance);
    fn set_phase1_reward(balance: Self::Balance);
    fn set_phase2_reward(balance: Self::Balance);
}

// online-committee_ops
pub trait OCOps {
    type AccountId;
    type MachineId;
    type CommitteeUploadInfo;
    type Balance;

    fn booked_machine(id: Self::MachineId) -> Result<(), ()>;
    fn revert_booked_machine(id: Self::MachineId) -> Result<(), ()>;

    fn confirm_machine(
        who: Vec<Self::AccountId>,
        machine_info: Self::CommitteeUploadInfo,
    ) -> Result<(), ()>;
    fn refuse_machine(
        committees: Vec<Self::AccountId>,
        machine_id: Self::MachineId,
    ) -> Option<(Self::AccountId, Self::Balance)>;
    fn change_staked_balance(
        stash: Self::AccountId,
        amount: Self::Balance,
        is_add: bool,
    ) -> Result<(), ()>;
    fn exec_slash(stash: Self::AccountId, amount: Self::Balance) -> Result<(), ()>;
}

pub trait RTOps {
    type AccountId;
    type MachineId;
    type MachineStatus;
    type Balance;
    type BlockNumber;

    fn get_machine_price(machine_point: u64, need_gpu: u32, total_gpu: u32) -> Option<u64>;

    fn change_machine_status_on_rent_start(
        machine_id: &Self::MachineId,
        gpu_num: u32,
    ) -> Result<(), ()>;
    fn change_machine_status_on_confirmed(
        machine_id: &Self::MachineId,
        renter: Self::AccountId,
    ) -> Result<(), ()>;
    fn change_machine_status_on_rent_end(
        machine_id: &Self::MachineId,
        gpu_num: u32,
        rent_duration: Self::BlockNumber,
        is_machine_last_rent: bool,
        is_user_last_rent: bool,
        renter: Self::AccountId,
    ) -> Result<(), ()>;
    fn change_machine_status_on_confirm_expired(
        machine_id: &Self::MachineId,
        gpu_num: u32,
    ) -> Result<(), ()>;
    fn change_machine_rent_fee(
        machine_stash: Self::AccountId,
        machine_id: Self::MachineId,
        fee_to_destroy: Self::Balance,
        fee_to_stash: Self::Balance,
    ) -> Result<(), ()>;
    fn reset_machine_renters(
        machine_id: Self::MachineId,
        renters: Vec<Self::AccountId>,
    ) -> Result<(), ()>;
}

pub trait OPRPCQuery {
    type AccountId;
    type StashMachine;

    fn get_all_stash() -> Vec<Self::AccountId>;
    fn get_stash_machine(stash: Self::AccountId) -> Self::StashMachine;
}

pub trait ManageCommittee {
    type AccountId;
    type Balance;
    type ReportId;

    fn is_valid_committee(who: &Self::AccountId) -> bool;
    fn available_committee() -> Option<Vec<Self::AccountId>>;
    // Only change stake record, not influence actual stake
    fn change_used_stake(
        committee: Self::AccountId,
        amount: Self::Balance,
        is_add: bool,
    ) -> Result<(), ()>;
    // Only change stake record, not influence actual stake
    fn change_total_stake(
        committee: Self::AccountId,
        amount: Self::Balance,
        is_add: bool,
        change_reserve: bool,
    ) -> Result<(), ()>;
    fn stake_per_order() -> Option<Self::Balance>;
    fn add_reward(committee: Self::AccountId, reward: Self::Balance);
}

pub trait DbcPrice {
    type Balance;

    fn get_dbc_price() -> Option<Self::Balance>;
    fn get_dbc_amount_by_value(value: u64) -> Option<Self::Balance>;

    fn get_dlc_amount_by_value(value: u64) -> Option<Self::Balance>;
}

pub trait ProjectRegister {
    // type BlockNumber;
    fn is_registered(machine_id: MachineId, project_name: Vec<u8>) -> bool;

    fn add_machine_registered_project(
        data: Vec<u8>,
        sig: sp_core::sr25519::Signature,
        from: sp_core::sr25519::Public,
        machine_id: MachineId,
        project_name: Vec<u8>,
    ) -> Result<(), &'static str>;

    fn remove_machine_registered_project(
        data: Vec<u8>,
        sig: sp_core::sr25519::Signature,
        from: sp_core::sr25519::Public,
        machine_id: MachineId,
        project_name: Vec<u8>,
    ) -> Result<(), &'static str>;

    fn is_registered_machine_owner(
        data: Vec<u8>,
        sig: sp_core::sr25519::Signature,
        from: sp_core::sr25519::Public,
        machine_id: MachineId,
        project_name: Vec<u8>,
    ) -> Result<bool, &'static str>;
}

pub trait MachineInfoTrait {
    type BlockNumber;
    fn get_machine_calc_point(machine_id: MachineId) -> u64;
    fn get_machine_cpu_rate(machine_id: MachineId) -> u64;
    fn get_machine_gpu_num(machine_id: MachineId) -> u64;
    fn get_rent_end_at(
        machine_id: MachineId,
        rent_id: RentOrderId,
    ) -> Result<Self::BlockNumber, &'static str>;

    fn is_machine_owner(machine_id: MachineId, evm_address: H160) -> Result<bool, &'static str>;

    fn get_dlc_machine_rent_fee(
        machine_id: MachineId,
        duration: Self::BlockNumber,
        rent_gpu_num: u32,
    ) -> Result<u64, &'static str>;

    fn get_dlc_rent_fee_by_calc_point(
        calc_point: u64,
        duration: Self::BlockNumber,
        rent_gpu_num: u32,
        total_gpu_num: u32,
    ) -> Result<u64, &'static str>;

    fn get_dbc_machine_rent_fee(
        machine_id: MachineId,
        duration: Self::BlockNumber,
        rent_gpu_num: u32,
    ) -> Result<u64, &'static str>;

    fn get_usdt_machine_rent_fee(
        machine_id: MachineId,
        duration: Self::BlockNumber,
        rent_gpu_num: u32,
    ) -> Result<u64, &'static str>;
}
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
pub enum PhaseLevel {
    PhaseOne,
    PhaseTwo,
    PhaseThree,
    PhaseNone,
}

impl From<u64> for PhaseLevel {
    fn from(value: u64) -> Self {
        if value == 1 {
            return PhaseLevel::PhaseOne
        };
        if value == 2 {
            return PhaseLevel::PhaseTwo
        }
        if value == 3 {
            return PhaseLevel::PhaseThree
        }
        return PhaseLevel::PhaseNone
    }
}

pub trait MTOps {
    type AccountId;
    type MachineId;
    type FaultType;
    type Balance;

    fn mt_machine_offline(
        reporter: Self::AccountId,
        committee: Vec<Self::AccountId>,
        machine_id: Self::MachineId,
        fault_type: Self::FaultType,
    ) -> Result<(), ()>;
    fn mt_change_staked_balance(
        stash: Self::AccountId,
        amount: Self::Balance,
        is_add: bool,
    ) -> Result<(), ()>;
    fn mt_rm_stash_total_stake(stash: Self::AccountId, amount: Self::Balance) -> Result<(), ()>;
}

pub trait GNOps {
    type AccountId;
    type Balance;

    fn slash_and_reward(
        slash_who: Vec<Self::AccountId>,
        each_slash: Self::Balance,
        reward_who: Vec<Self::AccountId>,
    ) -> Result<(), ()>;
}
