use crate::{Config, Error};
use codec::{Decode, Encode};
use dbc_support::{
    verify_online::{CustomErr, OCBookResultType},
    MachineId,
};
use sp_runtime::RuntimeDebug;
use sp_std::{cmp, vec::Vec};

/// 36 hours divide into 9 intervals for verification
pub const DISTRIBUTION: u32 = 9;
/// Each committee have 480 blocks (4 hours) to verify machine
pub const DURATIONPERCOMMITTEE: u32 = 480;
/// After order distribution 36 hours, allow committee submit raw info
pub const SUBMIT_RAW_START: u32 = 4320;
/// Summary committee's opinion after 48 hours
pub const SUBMIT_RAW_END: u32 = 5760;
pub const TWO_DAY: u32 = 5760;

#[derive(Clone, Debug)]
pub struct VerifySequence<AccountId> {
    pub who: AccountId,
    pub index: Vec<usize>,
}

impl<T: Config> From<CustomErr> for Error<T> {
    fn from(err: CustomErr) -> Self {
        match err {
            CustomErr::NotInBookList => Error::NotInBookList,
            CustomErr::TimeNotAllow => Error::TimeNotAllow,
            CustomErr::AlreadySubmitHash => Error::AlreadySubmitHash,
            CustomErr::AlreadySubmitRaw => Error::AlreadySubmitRaw,
            CustomErr::NotSubmitHash => Error::NotSubmitHash,
            CustomErr::Overflow => Error::Overflow,
        }
    }
}

// NOTE: If slash is from maintain committee, and reporter is slashed, but when
// committee support the reporter's slash is canceled, reporter's slash is not canceled at the same
// time. Mainwhile, if reporter's slash is canceled..
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct OCPendingSlashInfo<AccountId, BlockNumber, Balance> {
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

    pub book_result: OCBookResultType,
    pub slash_result: OCSlashResult,
}

impl<AccountId: PartialEq + cmp::Ord, BlockNumber, Balance>
    OCPendingSlashInfo<AccountId, BlockNumber, Balance>
{
    pub fn applicant_is_stash(&self, stash: AccountId) -> bool {
        self.book_result == OCBookResultType::OnlineRefused && self.machine_stash == stash
    }

    pub fn applicant_is_committee(&self, applicant: &AccountId) -> bool {
        self.inconsistent_committee.binary_search(applicant).is_ok()
    }
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
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

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct OCPendingSlashReviewInfo<AccountId, Balance, BlockNumber> {
    pub applicant: AccountId,
    pub staked_amount: Balance,
    pub apply_time: BlockNumber,
    pub expire_time: BlockNumber,
    pub reason: Vec<u8>,
}
