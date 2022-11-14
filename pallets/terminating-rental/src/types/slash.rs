use crate::IRBookResultType;
use codec::{Decode, Encode};
use generic_func::MachineId;
use sp_runtime::RuntimeDebug;
use sp_std::vec::Vec;

// NOTE: If slash is from maintain committee, and reporter is slashed, but when
// committee support the reporter's slash is canceled, reporter's slash is not canceled at the same
// time. Mainwhile, if reporter's slash is canceled..
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct IRPendingSlashInfo<AccountId, BlockNumber, Balance> {
    pub machine_id: MachineId,
    pub machine_stash: AccountId,
    pub stash_slash_amount: Balance,

    // TODO: maybe should record slash_reason: refuse online refused or change hardware
    // info refused, maybe slash amount is different
    pub inconsistent_committee: Vec<AccountId>,
    pub unruly_committee: Vec<AccountId>,
    pub reward_committee: Vec<AccountId>,
    pub committee_stake: Balance,

    pub slash_time: BlockNumber,
    pub slash_exec_time: BlockNumber,

    pub book_result: IRBookResultType,
    pub slash_result: IRSlashResult,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum IRSlashResult {
    Pending,
    Canceled,
    Executed,
}

impl Default for IRSlashResult {
    fn default() -> Self {
        Self::Pending
    }
}
