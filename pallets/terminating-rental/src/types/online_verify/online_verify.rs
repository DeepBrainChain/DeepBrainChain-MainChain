#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use codec::{Decode, Encode};
use frame_support::ensure;
use sp_runtime::RuntimeDebug;
use sp_std::{ops, vec::Vec};

use crate::{CustomErr, ReportId, SUBMIT_HASH_END, SUBMIT_RAW_END};
use dbc_support::{
    machine_type::CommitteeUploadInfo,
    verify_online::{MachineConfirmStatus, OCBookResultType, Summary},
    MachineId,
};
use generic_func::ItemList;

/// The reason why a stash account is punished
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum IRSlashReason<BlockNumber> {
    // Controller report rented machine offline
    // RentedReportOffline(BlockNumber),
    OnlineRentFailed(BlockNumber),
}

/// Query distributed machines by committee address
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct IRCommitteeMachineList {
    /// machines, that distributed to committee, and should be verified
    pub booked_machine: Vec<MachineId>,
    /// machines, have submited machine info hash
    pub hashed_machine: Vec<MachineId>,
    /// machines, have submited raw machine info
    pub confirmed_machine: Vec<MachineId>,
    /// machines, online successfully
    pub online_machine: Vec<MachineId>,
}

impl IRCommitteeMachineList {
    pub fn submit_hash(&mut self, machine_id: MachineId) {
        ItemList::rm_item(&mut self.booked_machine, &machine_id);
        ItemList::add_item(&mut self.hashed_machine, machine_id);
    }

    pub fn submit_raw(&mut self, machine_id: MachineId) -> Result<(), CustomErr> {
        ensure!(self.hashed_machine.binary_search(&machine_id).is_ok(), CustomErr::NotSubmitHash);
        ensure!(
            self.confirmed_machine.binary_search(&machine_id).is_err(),
            CustomErr::AlreadySubmitRaw
        );

        ItemList::rm_item(&mut self.hashed_machine, &machine_id);
        ItemList::add_item(&mut self.confirmed_machine, machine_id);
        Ok(())
    }

    // 将要重新派单的机器从订单里清除
    pub fn revert_book(&mut self, machine_id: &MachineId) {
        ItemList::rm_item(&mut self.booked_machine, &machine_id);
        ItemList::rm_item(&mut self.hashed_machine, &machine_id);
        ItemList::rm_item(&mut self.confirmed_machine, &machine_id);
    }

    // 机器成功上线后，从其他字段中清理掉机器记录
    // (如果未完成某一阶段的任务，机器ID将记录在那个阶段，需要进行清理)
    pub fn online_cleanup(&mut self, machine_id: &MachineId) {
        ItemList::rm_item(&mut self.booked_machine, &machine_id);
        ItemList::rm_item(&mut self.hashed_machine, &machine_id);
        ItemList::rm_item(&mut self.confirmed_machine, &machine_id);
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
    pub status: IRVerifyStatus,
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
            self.status = IRVerifyStatus::SubmittingRaw;
        }

        Ok(())
    }

    pub fn submit_raw(&mut self, time: BlockNumber, committee: AccountId) -> Result<(), CustomErr> {
        if self.status != IRVerifyStatus::SubmittingRaw {
            ensure!(time >= self.confirm_start_time, CustomErr::TimeNotAllow);
            ensure!(time <= self.book_time + SUBMIT_RAW_END.into(), CustomErr::TimeNotAllow);
        }
        ensure!(self.hashed_committee.binary_search(&committee).is_ok(), CustomErr::NotSubmitHash);

        ItemList::add_item(&mut self.confirmed_committee, committee);
        if self.confirmed_committee.len() == self.hashed_committee.len() {
            self.status = IRVerifyStatus::Summarizing;
        }
        Ok(())
    }

    // 是Summarizing的状态或 是SummitingRaw 且在有效时间内
    pub fn can_summary(&mut self, now: BlockNumber) -> bool {
        matches!(self.status, IRVerifyStatus::Summarizing) ||
            matches!(self.status, IRVerifyStatus::SubmittingRaw) &&
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
                self.status = IRVerifyStatus::Finished;
                self.onlined_committee = summary.valid_support;
            },
            MachineConfirmStatus::NoConsensus(_) => {},
            MachineConfirmStatus::Refuse(_) => {
                self.status = IRVerifyStatus::Finished;
            },
        }
    }
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum IRVerifyStatus {
    SubmittingHash,
    SubmittingRaw,
    Summarizing,
    Finished,
}

impl Default for IRVerifyStatus {
    fn default() -> Self {
        IRVerifyStatus::SubmittingHash
    }
}

/// A record of committee’s operations when verifying machine info
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct IRCommitteeOnlineOps<BlockNumber, Balance> {
    pub staked_dbc: Balance,
    /// When one committee can start the virtual machine to verify machine info
    pub verify_time: Vec<BlockNumber>,
    pub confirm_hash: [u8; 16],
    pub hash_time: BlockNumber,
    /// When one committee submit raw machine info
    pub confirm_time: BlockNumber,
    pub machine_status: IRVerifyMachineStatus,
    pub machine_info: CommitteeUploadInfo,
}

impl<BlockNumber, Balance> IRCommitteeOnlineOps<BlockNumber, Balance> {
    pub fn submit_hash(&mut self, time: BlockNumber, hash: [u8; 16]) {
        self.machine_status = IRVerifyMachineStatus::Hashed;
        self.confirm_hash = hash;
        self.hash_time = time;
    }

    // 添加用户对机器的操作记录
    pub fn submit_raw(&mut self, time: BlockNumber, machine_info: CommitteeUploadInfo) {
        self.confirm_time = time;
        self.machine_status = IRVerifyMachineStatus::Confirmed;
        self.machine_info = machine_info;
        self.machine_info.rand_str = Vec::new();
    }
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum IRVerifyMachineStatus {
    Booked,
    Hashed,
    Confirmed,
}

impl Default for IRVerifyMachineStatus {
    fn default() -> Self {
        IRVerifyMachineStatus::Booked
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
