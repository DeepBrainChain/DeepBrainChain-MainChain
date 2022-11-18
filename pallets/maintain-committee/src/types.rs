#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use crate::{Config, Error};
use codec::{Decode, Encode};
use frame_support::ensure;
use generic_func::{ItemList, MachineId};
use rent_machine::RentOrderId;
use sp_runtime::{
    traits::{Saturating, Zero},
    Perbill, RuntimeDebug,
};
use sp_std::{cmp::PartialEq, vec, vec::Vec};

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

#[derive(PartialEq, Eq, Clone, Copy, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CustomErr {
    OrderNotAllowBook,
    AlreadyBooked,
    NotNeedEncryptedInfo,
    NotOrderReporter,
    OrderStatusNotFeat,
    NotOrderCommittee,
    NotInBookedList,
}

impl<T: Config> From<CustomErr> for Error<T> {
    fn from(err: CustomErr) -> Self {
        match err {
            CustomErr::OrderNotAllowBook => Error::OrderNotAllowBook,
            CustomErr::AlreadyBooked => Error::AlreadyBooked,
            CustomErr::NotNeedEncryptedInfo => Error::NotNeedEncryptedInfo,
            CustomErr::NotOrderReporter => Error::NotOrderReporter,
            CustomErr::OrderStatusNotFeat => Error::OrderStatusNotFeat,
            CustomErr::NotOrderCommittee => Error::NotOrderCommittee,
            CustomErr::NotInBookedList => Error::NotInBookedList,
        }
    }
}

/// 机器故障的报告列表
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

impl MTLiveReportList {
    pub fn new_report(&mut self, report_id: ReportId) {
        ItemList::add_item(&mut self.bookable_report, report_id);
    }

    pub fn cancel_report(&mut self, report_id: &ReportId) {
        ItemList::rm_item(&mut self.bookable_report, report_id);
    }

    pub fn book_report(
        &mut self,
        report_id: ReportId,
        report_type: MachineFaultType,
        booked_committee_count: usize,
    ) {
        if booked_committee_count == 3 ||
            !matches!(report_type, MachineFaultType::RentedInaccessible(..))
        {
            ItemList::rm_item(&mut self.bookable_report, &report_id);
            ItemList::add_item(&mut self.verifying_report, report_id);
        }
    }

    pub fn submit_hash(
        &mut self,
        report_id: ReportId,
        report_type: MachineFaultType,
        hashed_committee_count: usize,
    ) {
        if hashed_committee_count == 3 {
            // 全都提交了hash后，进入提交raw的阶段
            ItemList::rm_item(&mut self.verifying_report, &report_id);
            ItemList::add_item(&mut self.waiting_raw_report, report_id);
        } else if !matches!(report_type, MachineFaultType::RentedInaccessible(..)) {
            // 否则，是普通错误时，继续允许预订
            ItemList::rm_item(&mut self.verifying_report, &report_id);
            ItemList::add_item(&mut self.bookable_report, report_id);
        }
    }

    pub fn clean_unfinished_report(&mut self, report_id: &ReportId) {
        ItemList::rm_item(&mut self.bookable_report, report_id);
        ItemList::rm_item(&mut self.verifying_report, report_id);
        ItemList::rm_item(&mut self.waiting_raw_report, report_id);
    }
}

/// 报告人的报告记录
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct ReporterReportList {
    pub processing_report: Vec<ReportId>,
    pub canceled_report: Vec<ReportId>,
    pub succeed_report: Vec<ReportId>,
    pub failed_report: Vec<ReportId>,
}

impl ReporterReportList {
    pub fn new_report(&mut self, report_id: ReportId) {
        ItemList::add_item(&mut self.processing_report, report_id);
    }

    pub fn cancel_report(&mut self, report_id: ReportId) {
        ItemList::rm_item(&mut self.processing_report, &report_id);
        ItemList::add_item(&mut self.canceled_report, report_id);
    }
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
    /// 出问题的机器的租用ID
    pub rent_order_id: RentOrderId,
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

impl<Account, BlockNumber, Balance> MTReportInfoDetail<Account, BlockNumber, Balance>
where
    Account: Default + Clone + Ord,
    BlockNumber: Default + PartialEq + Zero + From<u32> + Copy,
    Balance: Default,
{
    pub fn new(
        reporter: Account,
        report_time: BlockNumber,
        machine_fault_type: MachineFaultType,
        reporter_stake: Balance,
    ) -> Self {
        let mut report_info = MTReportInfoDetail {
            reporter,
            report_time,
            machine_fault_type: machine_fault_type.clone(),
            reporter_stake,
            ..Default::default()
        };

        // 该类型错误可以由程序快速完成检测，因此可以提交并需记录machine_id
        if let MachineFaultType::RentedInaccessible(machine_id, rent_order_id) =
            machine_fault_type.clone()
        {
            report_info.machine_id = machine_id;
            report_info.rent_order_id = rent_order_id;
        }

        report_info
    }

    pub fn can_book(&self, committee: &Account) -> Result<(), CustomErr> {
        // 检查订单是否可以抢定
        ensure!(self.report_time != Zero::zero(), CustomErr::OrderNotAllowBook);
        ensure!(
            matches!(self.report_status, ReportStatus::Reported | ReportStatus::WaitingBook),
            CustomErr::OrderNotAllowBook
        );
        ensure!(self.booked_committee.len() < 3, CustomErr::OrderNotAllowBook);
        ensure!(self.booked_committee.binary_search(committee).is_err(), CustomErr::AlreadyBooked);
        Ok(())
    }

    pub fn can_submit_encrypted_info(&self, from: &Account, to: &Account) -> Result<(), CustomErr> {
        ensure!(
            !matches!(self.machine_fault_type, MachineFaultType::RentedInaccessible(..)),
            CustomErr::NotNeedEncryptedInfo
        );
        ensure!(&self.reporter == from, CustomErr::NotOrderReporter);
        ensure!(self.report_status == ReportStatus::Verifying, CustomErr::OrderStatusNotFeat);
        ensure!(self.booked_committee.binary_search(to).is_ok(), CustomErr::NotOrderCommittee);
        Ok(())
    }

    pub fn can_submit_hash(&self) -> Result<(), CustomErr> {
        if matches!(self.machine_fault_type, MachineFaultType::RentedInaccessible(..)) {
            ensure!(
                matches!(self.report_status, ReportStatus::WaitingBook | ReportStatus::Verifying),
                CustomErr::OrderStatusNotFeat
            );
        } else {
            ensure!(self.report_status == ReportStatus::Verifying, CustomErr::OrderStatusNotFeat);
        }

        Ok(())
    }

    pub fn book_report(&mut self, committee: Account, now: BlockNumber) {
        ItemList::add_item(&mut self.booked_committee, committee.clone());

        if self.report_status == ReportStatus::Reported {
            // 是第一个预订的委员会时:
            self.first_book_time = now;
            self.confirm_start = match self.machine_fault_type {
                // 将在5分钟后开始提交委员会的验证结果
                MachineFaultType::RentedInaccessible(..) => now + 10u32.into(),
                // 将在三个小时之后开始提交委员会的验证结果
                _ => now + THREE_HOUR.into(),
            };
        }

        self.report_status = match self.machine_fault_type {
            MachineFaultType::RentedInaccessible(..) =>
                if self.booked_committee.len() == 3 {
                    ReportStatus::Verifying
                } else {
                    ReportStatus::WaitingBook
                },
            _ => {
                // 仅在不是RentedInaccessible时进行记录，因为这些情况只能一次有一个验证委员会
                self.verifying_committee = Some(committee);
                // 改变report状态为正在验证中，此时禁止其他委员会预订
                ReportStatus::Verifying
            },
        };
    }

    pub fn add_hash(&mut self, who: Account) {
        // 添加到report的已提交Hash的委员会列表
        ItemList::add_item(&mut self.hashed_committee, who.clone());
        self.verifying_committee = None;

        // 达到book_limit，则允许提交Raw
        if self.hashed_committee.len() == 3 {
            self.report_status = ReportStatus::SubmittingRaw;
        } else if !matches!(self.machine_fault_type, MachineFaultType::RentedInaccessible(..)) {
            // 否则，是普通错误时，继续允许预订
            self.report_status = ReportStatus::WaitingBook;
        }
    }

    pub fn add_raw(
        &mut self,
        who: Account,
        is_support: bool,
        machine_id: Option<MachineId>,
        err_reason: Vec<u8>,
    ) {
        // 添加到Report的已提交Raw的列表
        ItemList::add_item(&mut self.confirmed_committee, who.clone());

        // 将委员会插入到是否支持的委员会列表
        if is_support {
            ItemList::add_item(&mut self.support_committee, who);
        } else {
            ItemList::add_item(&mut self.against_committee, who);
        }

        // 一般错误报告
        if machine_id.is_some() {
            self.err_info = err_reason;
            self.machine_id = machine_id.unwrap();
        }
    }
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
    RentedInaccessible(MachineId, RentOrderId),
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
        Self::RentedInaccessible(vec![], 0)
    }
}
impl MachineFaultType {
    pub fn get_hash(self) -> Option<ReportHash> {
        match self {
            MachineFaultType::RentedHardwareMalfunction(hash, ..) |
            MachineFaultType::RentedHardwareCounterfeit(hash, ..) |
            MachineFaultType::OnlineRentFailed(hash, ..) => Some(hash),
            MachineFaultType::RentedInaccessible(..) => None,
        }
    }
}

/// Summary after all committee submit raw info
#[derive(Clone)]
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

impl MTCommitteeOrderList {
    pub fn clean_unfinished_order(&mut self, report_id: &ReportId) {
        ItemList::rm_item(&mut self.booked_report, report_id);
        ItemList::rm_item(&mut self.hashed_report, report_id);
        ItemList::rm_item(&mut self.confirmed_report, report_id);
    }

    pub fn can_submit_hash(&self, report_id: ReportId) -> Result<(), CustomErr> {
        ensure!(self.booked_report.binary_search(&report_id).is_ok(), CustomErr::NotInBookedList);
        Ok(())
    }

    pub fn add_hash(&mut self, report_id: ReportId) {
        // 将订单从委员会已预订移动到已Hash
        ItemList::rm_item(&mut self.booked_report, &report_id);
        ItemList::add_item(&mut self.hashed_report, report_id);
    }

    pub fn add_raw(&mut self, report_id: ReportId) {
        ItemList::rm_item(&mut self.hashed_report, &report_id);
        ItemList::add_item(&mut self.confirmed_report, report_id);
    }
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

impl<BlockNumber, Balance> MTCommitteeOpsDetail<BlockNumber, Balance> {
    pub fn add_encry_info(&mut self, info: Vec<u8>, time: BlockNumber) {
        self.encrypted_err_info = Some(info);
        self.encrypted_time = time;
        self.order_status = MTOrderStatus::Verifying;
    }

    pub fn book_report(
        &mut self,
        fault_type: MachineFaultType,
        now: BlockNumber,
        order_stake: Balance,
    ) {
        // 更改committee_ps
        self.booked_time = now;
        self.order_status = match fault_type {
            MachineFaultType::RentedInaccessible(..) => MTOrderStatus::Verifying,
            _ => {
                self.staked_balance = order_stake;
                MTOrderStatus::WaitingEncrypt
            },
        };
    }

    pub fn can_submit_hash(&self) -> Result<(), CustomErr> {
        ensure!(self.order_status == MTOrderStatus::Verifying, CustomErr::OrderStatusNotFeat);
        Ok(())
    }

    pub fn add_hash(&mut self, hash: ReportHash, time: BlockNumber) {
        self.confirm_hash = hash;
        self.hash_time = time;
        self.order_status = MTOrderStatus::WaitingRaw;
    }
    pub fn add_raw(&mut self, time: BlockNumber, is_support: bool, extra_err_info: Vec<u8>) {
        self.confirm_time = time;
        self.extra_err_info = extra_err_info;
        self.confirm_result = is_support;
        self.order_status = MTOrderStatus::Finished;
    }
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

impl<Balance: Saturating + Copy> ReporterStakeInfo<Balance> {
    pub fn change_stake_on_report_close(&mut self, amount: Balance, is_slashed: bool) {
        self.used_stake = self.used_stake.saturating_sub(amount);
        if is_slashed {
            self.staked_amount = self.staked_amount.saturating_sub(amount);
        }
    }
}

/// 报告的处理结果
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

// A: Account, B: Block, C: Balance
impl<A, B, C> MTReportResultInfo<A, B, C>
where
    A: Ord,
{
    pub fn is_slashed_reporter(&self, who: &A) -> bool {
        match self.report_result {
            ReportResultType::ReportRefused | ReportResultType::ReporterNotSubmitEncryptedInfo =>
                &self.reporter == who,
            _ => false,
        }
    }

    pub fn is_slashed_committee(&self, who: &A) -> bool {
        self.inconsistent_committee.binary_search(who).is_ok() ||
            self.unruly_committee.binary_search(who).is_ok()
    }

    pub fn is_slashed_stash(&self, who: &A) -> bool {
        match self.report_result {
            ReportResultType::ReportSucceed => &self.machine_stash == who,
            _ => false,
        }
    }

    pub fn i_exten_sorted(&mut self, a_list: Vec<A>) {
        for a_item in a_list {
            ItemList::add_item(&mut self.inconsistent_committee, a_item);
        }
    }

    pub fn r_exten_sorted(&mut self, a_list: Vec<A>) {
        for a_item in a_list {
            ItemList::add_item(&mut self.reward_committee, a_item);
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
