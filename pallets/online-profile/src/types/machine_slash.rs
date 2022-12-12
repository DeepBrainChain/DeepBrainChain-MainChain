use crate::OPSlashReason;
use codec::{Decode, Encode};
use dbc_support::MachineId;
use sp_runtime::RuntimeDebug;
use sp_std::vec::Vec;

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct OPPendingSlashInfo<AccountId, BlockNumber, Balance> {
    /// Who will be slashed
    pub slash_who: AccountId,
    /// Which machine will be slashed
    pub machine_id: MachineId,
    /// When slash action is created(not exec time)
    pub slash_time: BlockNumber,
    /// How much slash will be
    pub slash_amount: Balance,
    /// When slash will be exec
    pub slash_exec_time: BlockNumber,
    /// If reporter is some, will be rewarded when slash is executed
    pub reporter: Option<AccountId>,
    /// 机器当前的租用人
    pub renters: Vec<AccountId>,
    /// If committee is some, will be rewarded when slash is executed
    pub reward_to_committee: Option<Vec<AccountId>>,
    /// Why one is slashed
    pub slash_reason: OPSlashReason<BlockNumber>,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct OPPendingSlashReviewInfo<AccountId, Balance, BlockNumber> {
    pub applicant: AccountId,
    pub staked_amount: Balance,
    pub apply_time: BlockNumber,
    pub expire_time: BlockNumber,
    pub reason: Vec<u8>,
}
