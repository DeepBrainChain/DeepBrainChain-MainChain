use crate::{custom_err::ReportErr, report::MachineFaultType, ItemList, ReportHash, ReportId};
use codec::{Decode, Encode};
use frame_support::ensure;
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;
use sp_std::{cmp::PartialEq, vec::Vec};

/// 委员会抢到的报告的列表
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug, TypeInfo)]
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

    pub fn can_submit_hash(&self, report_id: ReportId) -> Result<(), ReportErr> {
        ensure!(self.booked_report.binary_search(&report_id).is_ok(), ReportErr::NotInBookedList);
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

    pub fn clean_when_summary(&mut self, report_id: ReportId, is_confirmed_committee: bool) {
        ItemList::rm_item(&mut self.hashed_report, &report_id);
        if is_confirmed_committee {
            ItemList::rm_item(&mut self.confirmed_report, &report_id);
            ItemList::add_item(&mut self.finished_report, report_id);
        } else {
            ItemList::rm_item(&mut self.booked_report, &report_id);
        }
    }
}

/// 委员会抢单之后，对应订单的状态
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
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

/// 委员会对报告的操作信息
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug, TypeInfo)]
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

    pub fn can_submit_hash(&self) -> Result<(), ReportErr> {
        ensure!(self.order_status == MTOrderStatus::Verifying, ReportErr::OrderStatusNotFeat);
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
