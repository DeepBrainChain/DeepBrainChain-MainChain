#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use codec::{Decode, Encode};
use frame_support::ensure;
use sp_runtime::RuntimeDebug;
use sp_std::{ops, vec::Vec};

use crate::{Config, Error, ReportId, SUBMIT_HASH_END, SUBMIT_RAW_END};
use dbc_support::{
    verify_online::{CustomErr, MachineConfirmStatus, OCVerifyStatus},
    ItemList,
};

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

/// Machines' verifying committee
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct IRMachineCommitteeList<AccountId, BlockNumber> {
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

impl<AccountId, BlockNumber> IRMachineCommitteeList<AccountId, BlockNumber>
where
    AccountId: Clone + Ord,
    BlockNumber: Copy + PartialOrd + ops::Add<Output = BlockNumber> + From<u32>,
{
    pub fn submit_hash_end(&self, now: BlockNumber) -> bool {
        now >= self.book_time + SUBMIT_HASH_END.into()
    }

    pub fn submit_raw_end(&self, now: BlockNumber) -> bool {
        now >= self.book_time + SUBMIT_RAW_END.into()
    }

    pub fn submit_hash(&mut self, committee: AccountId) -> Result<(), CustomErr> {
        ensure!(self.booked_committee.binary_search(&committee).is_ok(), CustomErr::NotInBookList);
        ensure!(
            self.hashed_committee.binary_search(&committee).is_err(),
            CustomErr::AlreadySubmitHash
        );

        ItemList::add_item(&mut self.hashed_committee, committee);
        // 如果委员会都提交了Hash,则直接进入提交原始信息的阶段
        if self.booked_committee.len() == self.hashed_committee.len() {
            self.status = OCVerifyStatus::SubmittingRaw;
        }

        Ok(())
    }

    pub fn submit_raw(&mut self, time: BlockNumber, committee: AccountId) -> Result<(), CustomErr> {
        if self.status != OCVerifyStatus::SubmittingRaw {
            ensure!(time >= self.confirm_start_time, CustomErr::TimeNotAllow);
            ensure!(time <= self.book_time + SUBMIT_RAW_END.into(), CustomErr::TimeNotAllow);
        }
        ensure!(self.hashed_committee.binary_search(&committee).is_ok(), CustomErr::NotSubmitHash);

        ItemList::add_item(&mut self.confirmed_committee, committee);
        if self.confirmed_committee.len() == self.hashed_committee.len() {
            self.status = OCVerifyStatus::Summarizing;
        }
        Ok(())
    }

    // 是Summarizing的状态或 是SummitingRaw 且在有效时间内
    pub fn can_summary(&mut self, now: BlockNumber) -> bool {
        matches!(self.status, OCVerifyStatus::Summarizing) ||
            matches!(self.status, OCVerifyStatus::SubmittingRaw) &&
                now >= self.book_time + SUBMIT_RAW_END.into()
    }

    // 没有提交原始信息的委员会
    pub fn unruly_committee(&self) -> Vec<AccountId> {
        let mut unruly = Vec::new();
        for a_committee in self.booked_committee.clone() {
            if self.confirmed_committee.binary_search(&a_committee).is_err() {
                ItemList::add_item(&mut unruly, a_committee);
            }
        }
        unruly
    }

    pub fn after_summary(&mut self, summary_result: MachineConfirmStatus<AccountId>) {
        match summary_result {
            MachineConfirmStatus::Confirmed(summary) => {
                self.status = OCVerifyStatus::Finished;
                self.onlined_committee = summary.valid_support;
            },
            MachineConfirmStatus::NoConsensus(_) => {},
            MachineConfirmStatus::Refuse(_) => {
                self.status = OCVerifyStatus::Finished;
            },
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
