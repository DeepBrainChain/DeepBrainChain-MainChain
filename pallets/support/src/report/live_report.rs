use crate::{report::MachineFaultType, ItemList, ReportId};
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;
use sp_std::{cmp::PartialEq, vec::Vec};

/// 机器故障的报告列表
/// 记录该模块中所有活跃的报告, 根据ReportStatus来区分
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug, TypeInfo)]
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

    pub fn time_to_submit_raw(&mut self, report_id: ReportId) {
        ItemList::rm_item(&mut self.bookable_report, &report_id); // 小于3人时处于bookable
        ItemList::rm_item(&mut self.verifying_report, &report_id); // 等于3人时处于verifying
        ItemList::add_item(&mut self.waiting_raw_report, report_id);
    }

    // pub fn summary(&mut self, report_id: ReportId) {
    //     ItemList::rm_item(&mut self.waiting_raw_report, &report_id);
    // }

    pub fn clean_unfinished_report(&mut self, report_id: &ReportId) {
        ItemList::rm_item(&mut self.bookable_report, report_id);
        ItemList::rm_item(&mut self.verifying_report, report_id);
        ItemList::rm_item(&mut self.waiting_raw_report, report_id);
    }

    // TODO: func rename
    pub fn get_verify_result<Account>(
        &mut self,
        report_id: ReportId,
        summary_result: ReportConfirmStatus<Account>,
    ) {
        match summary_result {
            ReportConfirmStatus::Confirmed(..) | ReportConfirmStatus::Refuse(..) => {
                ItemList::rm_item(&mut self.waiting_raw_report, &report_id);
                ItemList::add_item(&mut self.finished_report, report_id);
            },
            ReportConfirmStatus::NoConsensus => {
                ItemList::rm_item(&mut self.waiting_raw_report, &report_id);
            },
        }
    }

    // NOTE: 处理除了inaccessible错误之外的错误
    // 机器正在被该委员会验证，但该委员会超时未提交验证hash
    pub fn clean_not_submit_hash_report(&mut self, report_id: ReportId) {
        ItemList::rm_item(&mut self.verifying_report, &report_id);
        ItemList::add_item(&mut self.bookable_report, report_id);
    }
}

// NOTE: 移动到其他路径
/// Summary after all committee submit raw info
#[derive(Clone)]
pub enum ReportConfirmStatus<AccountId> {
    Confirmed(Vec<AccountId>, Vec<AccountId>, Vec<u8>),
    Refuse(Vec<AccountId>, Vec<AccountId>),
    NoConsensus,
}
