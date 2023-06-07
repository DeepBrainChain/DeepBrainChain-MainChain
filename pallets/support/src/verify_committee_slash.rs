use crate::{verify_online::OCBookResultType, MachineId};
use codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;
use sp_std::{cmp, vec::Vec};

// NOTE: If slash is from maintain committee, and reporter is slashed, but when
// committee support the reporter's slash is canceled, reporter's slash is not canceled at the same
// time. Mainwhile, if reporter's slash is canceled..
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
pub struct OCPendingSlashInfo<AccountId, BlockNumber, Balance> {
    pub machine_id: MachineId,
    pub machine_stash: Option<AccountId>,
    pub stash_slash_amount: Balance,

    // info refused, maybe slash amount is different
    pub inconsistent_committee: Vec<AccountId>,
    pub unruly_committee: Vec<AccountId>,
    pub reward_committee: Vec<AccountId>,
    pub committee_stake: Balance,

    // TODO: maybe should record slash_reason: refuse online refused or change hardware
    pub slash_time: BlockNumber,
    pub slash_exec_time: BlockNumber,

    pub book_result: OCBookResultType,
    pub slash_result: OCSlashResult,
}

impl<AccountId: PartialEq + cmp::Ord, BlockNumber, Balance>
    OCPendingSlashInfo<AccountId, BlockNumber, Balance>
{
    pub fn applicant_is_stash(&self, stash: AccountId) -> bool {
        self.machine_stash.is_some() &&
            self.book_result == OCBookResultType::OnlineRefused &&
            self.machine_stash == Some(stash)
    }

    pub fn applicant_is_committee(&self, applicant: &AccountId) -> bool {
        self.inconsistent_committee.binary_search(applicant).is_ok()
    }
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
pub enum OCSlashResult {
    Pending,
    Canceled,
    Executed,
}

impl Default for OCSlashResult {
    fn default() -> Self {
        Self::Pending
    }
}
