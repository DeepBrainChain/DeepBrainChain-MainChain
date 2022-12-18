use codec::{Decode, Encode};
use dbc_support::MachineId;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{traits::UniqueSaturatedInto, RuntimeDebug, SaturatedConversion};
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

/// The reason why a stash account is punished
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
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

impl<BlockNumber> OPSlashReason<BlockNumber>
where
    BlockNumber: Clone + SaturatedConversion + UniqueSaturatedInto<u64>,
{
    // 根据下线时长确定 slash 比例.
    pub fn slash_percent(&self, duration: u64) -> u32 {
        match self {
            Self::RentedReportOffline(_) => match duration {
                0 => 0,
                1..=14 => 2,        // <=7M扣除2%质押币。100%进入国库
                15..=5760 => 4,     // <=48H扣除4%质押币。100%进入国库
                5761..=14400 => 30, // <=120H扣30%质押币，10%给用户，90%进入国库
                _ => 50,            // >120H扣除50%质押币。10%给用户，90%进入国库
            },
            Self::OnlineReportOffline(_) => match duration {
                // FIXME: 处理这里 ，因为涉及到了now的判断
                // TODO: 如果机器从首次上线时间起超过365天，剩下20%押金可以申请退回。扣除80%质押币。
                // 质押币全部进入国库。
                0 => 0,
                1..=14 => 2,        /* <=7M扣除2%质押币，全部进入国库。 */
                15..=5760 => 4,     /* <=48H扣除4%质押币，全部进入国库 */
                5761..=28800 => 30, /* <=240H扣除30%质押币，全部进入国库 */
                _ => 80,
            },
            Self::RentedInaccessible(_) => match duration {
                0 => 0,
                1..=14 => 4,        // <=7M扣除4%质押币。10%给验证人，90%进入国库
                15..=5760 => 8,     // <=48H扣除8%质押币。10%给验证人，90%进入国库
                5761..=14400 => 60, /* <=120H扣除60%质押币。10%给用户，20%给验证人，70%进入国库 */
                _ => 100,           /* >120H扣除100%押金。10%给用户，20%给验证人，70%进入国库 */
            },
            Self::RentedHardwareMalfunction(_) => match duration {
                0 => 0,
                1..=480 => 6,       // <=4H扣除6%质押币
                481..=2880 => 12,   // <=24H扣除12%质押币
                2881..=5760 => 16,  // <=48H扣除16%质押币
                5761..=14400 => 60, // <=120H扣除60%质押币
                _ => 100,           // >120H扣除100%质押币
            },
            Self::RentedHardwareCounterfeit(_) => match duration {
                0 => 0,
                1..=480 => 12,      // <=4H扣12%质押币
                481..=2880 => 24,   // <=24H扣24%质押币
                2881..=5760 => 32,  // <=48H扣32%质押币
                5761..=14400 => 60, // <=120H扣60%质押币
                _ => 100,           // >120H扣100%押金
            },
            Self::OnlineRentFailed(_) => match duration {
                0 => 0,
                1..=480 => 6,       // <=4H扣6%质押币
                481..=2880 => 12,   // <=24H扣12%质押币
                2881..=5760 => 16,  // <=48H扣16%质押币
                5761..=14400 => 60, // <=120H扣60%质押币
                _ => 100,           // >120H扣100%押金
            },
            _ => 0,
        }
    }
}
