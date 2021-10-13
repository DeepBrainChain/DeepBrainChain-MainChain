use codec::{Decode, Encode};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use generic_func::MachineId;
use online_profile::CommitteeUploadInfo;
use sp_runtime::RuntimeDebug;
use sp_std::vec::Vec;

/// 36 hours divide into 9 intervals for verification
pub const DISTRIBUTION: u32 = 9;
/// Each committee have 480 blocks (4 hours) to verify machine
pub const DURATIONPERCOMMITTEE: u32 = 480;
/// After order distribution 36 hours, allow committee submit raw info
pub const SUBMIT_RAW_START: u32 = 4320;
/// Summary committee's opinion after 48 hours
pub const SUBMIT_RAW_END: u32 = 5760;
pub const TWO_DAY: u32 = 5760;

/// Query distributed machines by committee address
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct OCCommitteeMachineList {
    /// machines, that distributed to committee, and should be verified
    pub booked_machine: Vec<MachineId>,
    /// machines, have submited machine info hash
    pub hashed_machine: Vec<MachineId>,
    /// machines, have submited raw machine info
    pub confirmed_machine: Vec<MachineId>,
    /// machines, online successfully
    pub online_machine: Vec<MachineId>,
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

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum OCVerifyStatus {
    SubmittingHash,
    SubmittingRaw,
    Summarizing,
    Finished,
}

impl Default for OCVerifyStatus {
    fn default() -> Self {
        OCVerifyStatus::SubmittingHash
    }
}

/// A record of committee’s operations when verifying machine info
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct OCCommitteeOps<BlockNumber, Balance> {
    pub staked_dbc: Balance,
    /// When one committee can start the virtual machine to verify machine info
    pub verify_time: Vec<BlockNumber>,
    pub confirm_hash: [u8; 16],
    pub hash_time: BlockNumber,
    /// When one committee submit raw machine info
    pub confirm_time: BlockNumber,
    pub machine_status: OCMachineStatus,
    pub machine_info: CommitteeUploadInfo,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum OCMachineStatus {
    Booked,
    Hashed,
    Confirmed,
}

impl Default for OCMachineStatus {
    fn default() -> Self {
        OCMachineStatus::Booked
    }
}

/// What will happen after all committee submit raw machine info
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum MachineConfirmStatus<AccountId> {
    /// Machine is confirmed by committee, so can be online later
    Confirmed(Summary<AccountId>),
    /// Machine is refused, will not online
    Refuse(Summary<AccountId>),
    /// No consensus, so machine will be redistributed and verified later
    NoConsensus(Summary<AccountId>),
}

impl<AccountId: Default> Default for MachineConfirmStatus<AccountId> {
    fn default() -> Self {
        Self::Confirmed(Summary { ..Default::default() })
    }
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct Summary<AccountId> {
    /// Machine will be online, and those committee will get reward
    pub valid_support: Vec<AccountId>,
    /// Machine will be online, and those committee cannot get reward
    /// for they submit different message from majority committee
    pub invalid_support: Vec<AccountId>,
    /// Committees, that not submit all message
    /// such as: not submit hash, not submit raw info before deadline
    pub unruly: Vec<AccountId>,
    /// Committees, refuse machine online
    pub against: Vec<AccountId>,
    /// Raw machine info, most majority committee submit
    pub info: Option<CommitteeUploadInfo>,
}

// NOTE: If slash is from maintain committee, and reporter is slashed, but when
// committee support the reporter's slash is canceled, reporter's slash is not canceled at the same time.
// Mainwhile, if reporter's slash is canceled..
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct OCPendingSlashInfo<AccountId, BlockNumber, Balance> {
    pub machine_id: MachineId,
    pub machine_stash: AccountId,
    pub stash_slash_amount: Balance,

    // TODO: maybe should record slash_reason: refuse online refused or change hardware
    // TODO: info refused, maybe slash amount is different
    pub inconsistent_committee: Vec<AccountId>,
    pub unruly_committee: Vec<AccountId>,
    pub reward_committee: Vec<AccountId>,
    pub committee_stake: Balance,

    pub slash_time: BlockNumber,
    pub slash_exec_time: BlockNumber,

    pub book_result: OCBookResultType,
    pub slash_result: OCSlashResult,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum OCBookResultType {
    OnlineSucceed,
    OnlineRefused,
    NoConsensus,
    // TODO: add if is reonline
}

impl Default for OCBookResultType {
    fn default() -> Self {
        Self::OnlineRefused
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
