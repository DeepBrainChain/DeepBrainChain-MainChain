use crate::{
    custom_err::ReportErr, report::ReportConfirmStatus, BoxPubkey, ItemList, MachineId,
    RentOrderId, ReportHash, FOUR_HOUR, TEN_MINUTE, THREE_HOUR,
};
use codec::{Decode, Encode};
use frame_support::ensure;
use sp_runtime::{traits::Zero, Perbill, RuntimeDebug};
use sp_std::{cmp::PartialEq, ops::Sub, vec, vec::Vec};

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
    BlockNumber:
        Default + PartialEq + Zero + From<u32> + Copy + Sub<Output = BlockNumber> + PartialOrd,
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

    pub fn can_book(&self, committee: &Account) -> Result<(), ReportErr> {
        // 检查订单是否可以抢定
        ensure!(self.report_time != Zero::zero(), ReportErr::OrderNotAllowBook);
        ensure!(
            matches!(self.report_status, ReportStatus::Reported | ReportStatus::WaitingBook),
            ReportErr::OrderNotAllowBook
        );
        ensure!(self.booked_committee.len() < 3, ReportErr::OrderNotAllowBook);
        ensure!(self.booked_committee.binary_search(committee).is_err(), ReportErr::AlreadyBooked);
        Ok(())
    }

    pub fn can_submit_encrypted_info(&self, from: &Account, to: &Account) -> Result<(), ReportErr> {
        ensure!(
            !matches!(self.machine_fault_type, MachineFaultType::RentedInaccessible(..)),
            ReportErr::NotNeedEncryptedInfo
        );
        ensure!(&self.reporter == from, ReportErr::NotOrderReporter);
        ensure!(self.report_status == ReportStatus::Verifying, ReportErr::OrderStatusNotFeat);
        ensure!(self.booked_committee.binary_search(to).is_ok(), ReportErr::NotOrderCommittee);
        Ok(())
    }

    pub fn can_submit_hash(&self) -> Result<(), ReportErr> {
        if matches!(self.machine_fault_type, MachineFaultType::RentedInaccessible(..)) {
            ensure!(
                matches!(self.report_status, ReportStatus::WaitingBook | ReportStatus::Verifying),
                ReportErr::OrderStatusNotFeat
            );
        } else {
            ensure!(self.report_status == ReportStatus::Verifying, ReportErr::OrderStatusNotFeat);
        }

        Ok(())
    }

    pub fn can_submit_raw(&self, who: &Account) -> Result<(), ReportErr> {
        ensure!(self.report_status == ReportStatus::SubmittingRaw, ReportErr::OrderStatusNotFeat);
        // 检查是否提交了该订单的hash
        ensure!(self.hashed_committee.binary_search(who).is_ok(), ReportErr::NotProperCommittee);
        Ok(())
    }

    // 获取链上已经记录的报告人提交的Hash
    pub fn get_reporter_hash(&self) -> Result<ReportHash, ReportErr> {
        self.machine_fault_type.clone().get_hash().ok_or(ReportErr::OrderStatusNotFeat)
    }

    pub fn can_submit_inaccessible_raw(&self, who: &Account) -> Result<(), ReportErr> {
        ensure!(self.report_status == ReportStatus::SubmittingRaw, ReportErr::OrderStatusNotFeat);
        ensure!(
            matches!(self.machine_fault_type, MachineFaultType::RentedInaccessible(..)),
            ReportErr::OrderStatusNotFeat
        );

        // 检查是否提交了该订单的hash
        ensure!(self.hashed_committee.binary_search(&who).is_ok(), ReportErr::NotProperCommittee);
        Ok(())
    }

    pub fn can_summary_fault(&self) -> Result<(), ()> {
        // 忽略掉线的类型
        if self.first_book_time == Zero::zero() ||
            matches!(self.machine_fault_type, MachineFaultType::RentedInaccessible(..))
        {
            return Err(())
        }

        Ok(())
    }

    // Other fault type
    pub fn can_summary(&self, now: BlockNumber) -> bool {
        if self.first_book_time == Zero::zero() {
            return false
        }

        // 禁止对快速报告进行检查，快速报告会处理这种情况
        if matches!(self.machine_fault_type, MachineFaultType::RentedInaccessible(..)) {
            return false
        }

        // 未全部提交了原始信息且未达到了四个小时，需要继续等待
        if now - self.first_book_time < FOUR_HOUR.into() &&
            self.hashed_committee.len() != self.confirmed_committee.len()
        {
            return false
        }

        true
    }

    // Summary committee's handle result depend on support & against votes
    pub fn summary(&self) -> ReportConfirmStatus<Account> {
        if self.confirmed_committee.is_empty() {
            return ReportConfirmStatus::NoConsensus
        }

        if self.support_committee.len() >= self.against_committee.len() {
            return ReportConfirmStatus::Confirmed(
                self.support_committee.clone(),
                self.against_committee.clone(),
                self.err_info.clone(),
            )
        }
        ReportConfirmStatus::Refuse(self.support_committee.clone(), self.against_committee.clone())
    }

    pub fn is_confirmed_committee(&self, who: &Account) -> bool {
        self.confirmed_committee.binary_search(who).is_ok()
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

// 处理inaccessible类型的报告
impl<Account, BlockNumber, Balance> MTReportInfoDetail<Account, BlockNumber, Balance>
where
    Account: Default + Clone + Ord,
    BlockNumber:
        Default + PartialEq + Zero + From<u32> + Copy + Sub<Output = BlockNumber> + PartialOrd,
    Balance: Default,
{
    pub fn can_summary_inaccessible(&self, now: BlockNumber) -> Result<(), ()> {
        // 仅处理被抢单的报告
        if self.first_book_time == Zero::zero() {
            return Err(())
        }
        // 仅处理Inaccessible的情况
        if !matches!(self.machine_fault_type, MachineFaultType::RentedInaccessible(..)) {
            return Err(())
        }

        // 忽略未被抢单的报告或已完成的报告
        if matches!(self.report_status, ReportStatus::Reported | ReportStatus::CommitteeConfirmed) {
            return Err(())
        }

        if matches!(self.report_status, ReportStatus::SubmittingRaw) {
            // 不到10分钟，且没全部提交确认，允许继续提交
            if now - self.first_book_time < TEN_MINUTE.into() &&
                self.confirmed_committee.len() < self.hashed_committee.len()
            {
                return Err(())
            }
        }

        Ok(())
    }
}

// 处理除了inaccessible错误之外的错误
impl<Account, BlockNumber, Balance> MTReportInfoDetail<Account, BlockNumber, Balance>
where
    Account: Default + Clone + Ord,
    BlockNumber:
        Default + PartialEq + Zero + From<u32> + Copy + Sub<Output = BlockNumber> + PartialOrd,
    Balance: Default,
{
    // 机器正在被该委员会验证，但该委员会超时未提交验证hash
    pub fn clean_not_submit_hash_committee(&mut self, verifying_committee: &Account) {
        self.verifying_committee = None;
        // 删除，以允许其他委员会进行抢单
        ItemList::rm_item(&mut self.booked_committee, verifying_committee);
        ItemList::rm_item(&mut self.get_encrypted_info_committee, verifying_committee);

        // 如果此时booked_committee.len() == 0；返回到最初始的状态，并允许取消报告
        if self.booked_committee.is_empty() {
            self.first_book_time = Zero::zero();
            self.confirm_start = Zero::zero();
            self.report_status = ReportStatus::Reported;
        } else {
            self.report_status = ReportStatus::WaitingBook
        };
    }

    // 当块高从抢单验证变为提交原始值时，移除最后一个正在验证的委员会
    pub fn clean_not_submit_raw_committee(&mut self, verifying_committee: &Account) {
        // 将最后一个委员会移除，不惩罚
        self.verifying_committee = None;
        ItemList::rm_item(&mut self.booked_committee, &verifying_committee);
        ItemList::rm_item(&mut self.get_encrypted_info_committee, &verifying_committee);
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
    /// 机器是在线状态，但无法租用(创建虚拟机失败)，举报时同样需要先租下来
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

    // pub fn into_op_err<BlockNumber>(&self, report_time: BlockNumber) ->
    // OPSlashReason<BlockNumber> {     match self {
    //         Self::RentedInaccessible(..) => OPSlashReason::RentedInaccessible(report_time),
    //         Self::RentedHardwareMalfunction(..) =>
    //             OPSlashReason::RentedHardwareMalfunction(report_time),
    //         Self::RentedHardwareCounterfeit(..) =>
    //             OPSlashReason::RentedHardwareCounterfeit(report_time),
    //         Self::OnlineRentFailed(..) => OPSlashReason::OnlineRentFailed(report_time),
    //     }
    // }
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
