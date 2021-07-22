#![cfg_attr(not(feature = "std"), no_std)]

use sp_std::vec::Vec;

// lease-committee_ops
pub trait LCOps {
    type AccountId;
    type MachineId;
    type CommitteeUploadInfo;

    fn lc_booked_machine(id: Self::MachineId);
    fn lc_revert_booked_machine(id: Self::MachineId);

    fn lc_confirm_machine(
        who: Vec<Self::AccountId>,
        machine_info: Self::CommitteeUploadInfo,
    ) -> Result<(), ()>;
    fn lc_refuse_machine(machien_id: Self::MachineId) -> Result<(), ()>;
}

pub trait RTOps {
    type AccountId;
    type MachineId;
    type MachineStatus;
    type BalanceOf;

    fn change_machine_status(
        machine_id: &Self::MachineId,
        new_status: Self::MachineStatus,
        renter: Option<Self::AccountId>,
        rent_duration: Option<u64>, // 不为None时，表示租用结束
    );

    fn change_machine_rent_fee(amount: Self::BalanceOf, machine_id: Self::MachineId, is_burn: bool);
}

pub trait OPRPCQuery {
    type AccountId;
    type StashMachine;

    fn get_all_stash() -> Vec<Self::AccountId>;
    fn get_stash_machine(stash: Self::AccountId) -> Self::StashMachine;
}

pub trait ManageCommittee {
    type AccountId;
    type BalanceOf;

    fn is_valid_committee(who: &Self::AccountId) -> bool;

    fn available_committee() -> Result<Vec<Self::AccountId>, ()>;
    fn change_stake(
        controller: &Self::AccountId,
        amount: Self::BalanceOf,
        is_add: bool,
    ) -> Result<(), ()>;
    fn stake_per_order() -> Option<Self::BalanceOf>;
    fn add_reward(committee: Self::AccountId, reward: Self::BalanceOf);
    fn add_slash(who: Self::AccountId, amount: Self::BalanceOf, reward_to: Vec<Self::AccountId>);
}

pub trait DbcPrice {
    type BalanceOf;

    fn get_dbc_amount_by_value(value: u64) -> Option<Self::BalanceOf>;
}

pub trait MTOps {
    type AccountId;
    type MachineId;

    fn machine_offline(
        machine_id: Self::MachineId,
        committee: Vec<Self::AccountId>,
        reporter: Self::AccountId,
    );
}
