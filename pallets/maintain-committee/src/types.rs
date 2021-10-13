use codec::{Decode, Encode};
use generic_func::MachineId;
use sp_runtime::{Perbill, RuntimeDebug};
use sp_std::{vec, vec::Vec};

pub const FIVE_MINUTE: u32 = 10;
pub const TEN_MINUTE: u32 = 20;
pub const HALF_HOUR: u32 = 60;
pub const ONE_HOUR: u32 = 120;
pub const THREE_HOUR: u32 = 360;
pub const FOUR_HOUR: u32 = 480;
pub const TWO_DAY: u32 = 5760;

pub type ReportId = u64;
pub type BoxPubkey = [u8; 32];
pub type ReportHash = [u8; 16];

/// 机器故障的报告
/// 记录该模块中所有活跃的报告, 根据ReportStatus来区分
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MTLiveReportList {
    /// 委员会可以抢单的报告
    pub bookable_report: Vec<ReportId>,
    /// 正在被验证的机器报告,验证完如能预定，转成上面状态，如不能则转成下面状态
    pub verifying_report: Vec<ReportId>,
    /// 等待提交原始值的报告, 所有委员会提交或时间截止，转为下面状态
    pub waiting_raw_report: Vec<ReportId>,
    /// 等待48小时后执行的报告, 此期间可以申述，由技术委员会审核
    pub finished_report: Vec<ReportId>,
}

/// 报告人的报告记录
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct ReporterReportList {
    pub processing_report: Vec<ReportId>,
    pub canceled_report: Vec<ReportId>,
    pub succeed_report: Vec<ReportId>,
    pub failed_report: Vec<ReportId>,
}

// 报告的详细信息
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MTReportInfoDetail<AccountId, BlockNumber, Balance> {
    ///报告人
    pub reporter: AccountId,
    /// 报告提交时间
    pub report_time: BlockNumber,
    /// 报告人质押数量
    pub reporter_stake: Balance,
    /// 第一个委员会抢单时间
    pub first_book_time: BlockNumber,
    /// 出问题的机器，只有委员会提交原始信息时才存入
    pub machine_id: MachineId,
    /// 机器的故障原因
    pub err_info: Vec<u8>,
    /// 当前正在验证机器的委员会
    pub verifying_committee: Option<AccountId>,
    /// 抢单的委员会
    pub booked_committee: Vec<AccountId>,
    /// 获得报告人提交了加密信息的委员会列表
    pub get_encrypted_info_committee: Vec<AccountId>,
    /// 提交了检查报告Hash的委员会
    pub hashed_committee: Vec<AccountId>,
    /// 开始提交raw信息的时间
    pub confirm_start: BlockNumber,
    /// 提交了Raw信息的委员会
    pub confirmed_committee: Vec<AccountId>,
    /// 支持报告人的委员会
    pub support_committee: Vec<AccountId>,
    /// 不支持报告人的委员会
    pub against_committee: Vec<AccountId>,
    /// 当前报告的状态
    pub report_status: ReportStatus,
    /// 机器的故障类型
    pub machine_fault_type: MachineFaultType,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum ReportStatus {
    /// 没有委员会预订过的报告, 允许报告人取消
    Reported,
    /// 前一个委员会的报告已经超过一个小时，自动改成可预订状态
    WaitingBook,
    /// 有委员会抢单，处于验证中
    Verifying,
    /// 距离第一个验证人抢单3个小时后，等待委员会上传原始信息
    SubmittingRaw,
    /// 委员会已经完成，等待第48小时, 检查报告结果
    CommitteeConfirmed,
}

impl Default for ReportStatus {
    fn default() -> Self {
        ReportStatus::Reported
    }
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum MachineFaultType {
    /// 机器被租用，但无法访问的故障 (机器离线)
    RentedInaccessible(MachineId),
    /// 机器被租用，但有硬件故障
    RentedHardwareMalfunction(ReportHash, BoxPubkey),
    /// 机器被租用，但硬件参数造假
    RentedHardwareCounterfeit(ReportHash, BoxPubkey),
    /// 机器是在线状态，但无法租用
    OnlineRentFailed(ReportHash, BoxPubkey),
}

// 默认硬件故障
impl Default for MachineFaultType {
    fn default() -> Self {
        Self::RentedInaccessible(vec![])
    }
}

/// Summary after all committee submit raw info
pub enum ReportConfirmStatus<AccountId> {
    Confirmed(Vec<AccountId>, Vec<AccountId>, Vec<u8>),
    Refuse(Vec<AccountId>, Vec<AccountId>),
    NoConsensus,
}

/// 委员会抢到的报告的列表
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MTCommitteeOrderList {
    /// 委员会预订的报告
    pub booked_report: Vec<ReportId>,
    /// 已经提交了Hash信息的报告
    pub hashed_report: Vec<ReportId>,
    /// 已经提交了原始确认数据的报告
    pub confirmed_report: Vec<ReportId>,
    /// 已经成功上线的机器
    pub finished_report: Vec<ReportId>,
}

/// 委员会对报告的操作信息
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MTCommitteeOpsDetail<BlockNumber, Balance> {
    pub booked_time: BlockNumber,
    /// reporter 提交的加密后的信息
    pub encrypted_err_info: Option<Vec<u8>>,
    pub encrypted_time: BlockNumber,
    pub confirm_hash: ReportHash,
    pub hash_time: BlockNumber,
    /// 委员会可以补充额外的信息
    pub extra_err_info: Vec<u8>,
    /// 委员会提交raw信息的时间
    pub confirm_time: BlockNumber,
    pub confirm_result: bool,
    pub staked_balance: Balance,
    pub order_status: MTOrderStatus,
}

/// 委员会抢单之后，对应订单的状态
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum MTOrderStatus {
    /// 预订报告，状态将等待加密信息
    WaitingEncrypt,
    /// 获得加密信息之后，状态将等待加密信息
    Verifying,
    /// 等待提交原始信息
    WaitingRaw,
    /// 委员会已经完成了全部操作
    Finished,
}

impl Default for MTOrderStatus {
    fn default() -> Self {
        MTOrderStatus::Verifying
    }
}

/// Reporter stake params
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct ReporterStakeParamsInfo<Balance> {
    /// First time when report
    pub stake_baseline: Balance,
    /// How much stake will be used each report & how much should stake in this
    /// module to apply for SlashReview(reporter, committee, stash stake the same)
    pub stake_per_report: Balance,
    /// 当剩余的质押数量到阈值时，需要补质押
    pub min_free_stake_percent: Perbill,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct ReporterStakeInfo<Balance> {
    pub staked_amount: Balance,
    pub used_stake: Balance,
    pub can_claim_reward: Balance,
    pub claimed_reward: Balance,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MTReportResultInfo<AccountId, BlockNumber, Balance> {
    pub report_id: ReportId,
    pub reporter: AccountId,
    pub reporter_stake: Balance,

    pub inconsistent_committee: Vec<AccountId>,
    pub unruly_committee: Vec<AccountId>,
    pub reward_committee: Vec<AccountId>,
    pub committee_stake: Balance,

    pub machine_stash: AccountId,
    pub machine_id: MachineId,

    pub slash_time: BlockNumber,
    pub slash_exec_time: BlockNumber,
    pub report_result: ReportResultType,
    pub slash_result: MCSlashResult,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum ReportResultType {
    ReportSucceed,
    ReportRefused,
    ReporterNotSubmitEncryptedInfo,
    NoConsensus,
}

impl Default for ReportResultType {
    fn default() -> Self {
        Self::ReportRefused
    }
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum MCSlashResult {
    Pending,
    Canceled,
    Executed,
}

impl Default for MCSlashResult {
    fn default() -> Self {
        Self::Pending
    }
}

impl<AccountId, BlockNumber, Balance> MTReportResultInfo<AccountId, BlockNumber, Balance>
where
    AccountId: Ord,
{
    pub fn is_slashed_reporter(&self, who: &AccountId) -> bool {
        match self.report_result {
            ReportResultType::ReportRefused | ReportResultType::ReporterNotSubmitEncryptedInfo => &self.reporter == who,
            _ => false,
        }
    }

    pub fn is_slashed_committee(&self, who: &AccountId) -> bool {
        self.inconsistent_committee.binary_search(who).is_ok() || self.unruly_committee.binary_search(who).is_ok()
    }

    pub fn is_slashed_stash(&self, who: &AccountId) -> bool {
        match self.report_result {
            ReportResultType::ReportSucceed => &self.machine_stash == who,
            _ => false,
        }
    }
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MTPendingSlashReviewInfo<AccountId, Balance, BlockNumber> {
    pub applicant: AccountId,
    pub staked_amount: Balance,
    pub apply_time: BlockNumber,
    pub expire_time: BlockNumber,
    pub reason: Vec<u8>,
}
