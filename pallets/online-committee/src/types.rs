use codec::{Decode, Encode};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use crate::{Config, Error};
use dbc_support::{
    verify_online::{CustomErr, MachineConfirmStatus, OCBookResultType, OCVerifyStatus},
    MachineId,
};
use frame_support::ensure;
use generic_func::ItemList;
use sp_runtime::RuntimeDebug;
use sp_std::{cmp, ops, vec::Vec};

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

/// Machines' verifying committee
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct OCMachineCommitteeList<AccountId, BlockNumber> {
    /// When order distribution happened
    pub book_time: BlockNumber,
    /// Committees, get the job to verify machine info
    pub booked_committee: Vec<AccountId>,
    /// Committees, have submited machine info hash
    pub hashed_committee: Vec<AccountId>,
    /// When committee can submit raw machine info, submit machine info can
    /// immediately start after all booked_committee submit hash
    pub confirm_start_time: BlockNumber,
    /// Committees, have submit raw machine info
    pub confirmed_committee: Vec<AccountId>,
    /// Committees, get a consensus, so can get rewards after machine online
    pub onlined_committee: Vec<AccountId>,
    /// Current order status
    pub status: OCVerifyStatus,
}

impl<AccountId, BlockNumber> OCMachineCommitteeList<AccountId, BlockNumber>
where
    AccountId: Clone + Ord,
    BlockNumber: Copy + PartialOrd + ops::Add<Output = BlockNumber> + From<u32>,
{
    pub fn submit_hash(&mut self, committee: AccountId) -> Result<(), CustomErr> {
        ensure!(self.booked_committee.binary_search(&committee).is_ok(), CustomErr::NotInBookList);
        ensure!(
            self.hashed_committee.binary_search(&committee).is_err(),
            CustomErr::AlreadySubmitHash
        );

        ItemList::add_item(&mut self.hashed_committee, committee);
        // 如果委员会都提交了Hash,则直接进入提交原始信息的阶段
        if self.booked_committee.len() == self.hashed_committee.len() {
            self.status = OCVerifyStatus::SubmittingRaw;
        }

        Ok(())
    }

    pub fn submit_raw(&mut self, time: BlockNumber, committee: AccountId) -> Result<(), CustomErr> {
        if self.status != OCVerifyStatus::SubmittingRaw {
            ensure!(time >= self.confirm_start_time, CustomErr::TimeNotAllow);
            ensure!(time <= self.book_time + SUBMIT_RAW_END.into(), CustomErr::TimeNotAllow);
        }
        ensure!(self.hashed_committee.binary_search(&committee).is_ok(), CustomErr::NotSubmitHash);

        ItemList::add_item(&mut self.confirmed_committee, committee);
        if self.confirmed_committee.len() == self.hashed_committee.len() {
            self.status = OCVerifyStatus::Summarizing;
        }
        Ok(())
    }

    // 是Summarizing的状态或 是SummitingRaw 且在有效时间内
    pub fn can_summary(&mut self, now: BlockNumber) -> bool {
        matches!(self.status, OCVerifyStatus::Summarizing) ||
            matches!(self.status, OCVerifyStatus::SubmittingRaw) &&
                now >= self.book_time + SUBMIT_RAW_END.into()
    }

    // 记录没有提交原始信息的委员会
    pub fn summary_unruly(&self) -> Vec<AccountId> {
        let mut unruly = Vec::new();
        for a_committee in self.booked_committee.clone() {
            if self.confirmed_committee.binary_search(&a_committee).is_err() {
                ItemList::add_item(&mut unruly, a_committee);
            }
        }
        unruly
    }

    pub fn after_summary(&mut self, summary_result: MachineConfirmStatus<AccountId>) {
        match summary_result {
            MachineConfirmStatus::Confirmed(summary) => {
                self.status = OCVerifyStatus::Finished;
                self.onlined_committee = summary.valid_support;
            },
            MachineConfirmStatus::NoConsensus(_) => {},
            MachineConfirmStatus::Refuse(_) => {
                self.status = OCVerifyStatus::Finished;
            },
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
