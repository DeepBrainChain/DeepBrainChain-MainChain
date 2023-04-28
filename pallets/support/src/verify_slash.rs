use super::MachineId;
use codec::{Decode, Encode};
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::RuntimeDebug;
use sp_std::vec::Vec;

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
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

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
pub struct OPPendingSlashReviewInfo<AccountId, Balance, BlockNumber> {
    pub applicant: AccountId,
    pub staked_amount: Balance,
    pub apply_time: BlockNumber,
    pub expire_time: BlockNumber,
    pub reason: Vec<u8>,
}

/// The reason why a stash account is punished
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum OPSlashReason<BlockNumber> {
    /// Controller report rented machine offline
    RentedReportOffline(BlockNumber),
    /// Controller report online machine offline
    OnlineReportOffline(BlockNumber),
    /// Reporter report rented machine is offline
    RentedInaccessible(BlockNumber),
    /// Reporter report rented machine hardware fault
    RentedHardwareMalfunction(BlockNumber),
    /// Reporter report rented machine is fake
    RentedHardwareCounterfeit(BlockNumber),
    /// Machine is online, but rent failed
    OnlineRentFailed(BlockNumber),
    /// Committee refuse machine online
    CommitteeRefusedOnline,
    /// Committee refuse changed hardware info machine reonline
    CommitteeRefusedMutHardware,
    /// Machine change hardware is passed, so should reward committee
    ReonlineShouldReward,
}

impl<BlockNumber> Default for OPSlashReason<BlockNumber> {
    fn default() -> Self {
        Self::CommitteeRefusedOnline
    }
}
