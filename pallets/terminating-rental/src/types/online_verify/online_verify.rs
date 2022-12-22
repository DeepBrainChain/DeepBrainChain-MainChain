use crate::{Config, Error};
use codec::{Decode, Encode};
use dbc_support::{verify_online::CustomErr, ItemList, ReportId};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::RuntimeDebug;
use sp_std::vec::Vec;

/// The reason why a stash account is punished
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum IRSlashReason<BlockNumber> {
    // Controller report rented machine offline
    // RentedReportOffline(BlockNumber),
    OnlineRentFailed(BlockNumber),
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

/// 委员会抢到的报告的列表
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct IRCommitteeReportOrderList {
    /// 委员会预订的报告
    pub booked_report: Vec<ReportId>,
    /// 已经提交了Hash信息的报告
    pub hashed_report: Vec<ReportId>,
    /// 已经提交了原始确认数据的报告
    pub confirmed_report: Vec<ReportId>,
    /// 已经成功上线的机器
    pub finished_report: Vec<ReportId>,
}

impl IRCommitteeReportOrderList {
    pub fn clean_unfinished_order(&mut self, report_id: &ReportId) {
        ItemList::rm_item(&mut self.booked_report, report_id);
        ItemList::rm_item(&mut self.hashed_report, report_id);
        ItemList::rm_item(&mut self.confirmed_report, report_id);
    }

    // pub fn can_submit_hash(&self, report_id: ReportId) -> Result<(), CustomErr> {
    //     ensure!(self.booked_report.binary_search(&report_id).is_ok(),
    // CustomErr::NotInBookedList);     Ok(())
    // }

    // pub fn add_hash(&mut self, report_id: ReportId) {
    //     // 将订单从委员会已预订移动到已Hash
    //     ItemList::rm_item(&mut self.booked_report, &report_id);
    //     ItemList::add_item(&mut self.hashed_report, report_id);
    // }

    // pub fn add_raw(&mut self, report_id: ReportId) {
    //     ItemList::rm_item(&mut self.hashed_report, &report_id);
    //     ItemList::add_item(&mut self.confirmed_report, report_id);
    // }

    // pub fn clean_when_summary(&mut self, report_id: ReportId, is_confirmed_committee: bool) {
    //     ItemList::rm_item(&mut self.hashed_report, &report_id);
    //     if is_confirmed_committee {
    //         ItemList::rm_item(&mut self.confirmed_report, &report_id);
    //         ItemList::add_item(&mut self.finished_report, report_id);
    //     } else {
    //         ItemList::rm_item(&mut self.booked_report, &report_id);
    //     }
    // }
}
