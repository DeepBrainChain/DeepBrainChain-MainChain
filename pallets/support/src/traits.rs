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
        machine_id: Self::MachineId,
        fee_to_destroy: Self::Balance,
        fee_to_stash: Self::Balance,
    );
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

    fn get_dbc_amount_by_value(value: u64) -> Option<Self::Balance>;
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
