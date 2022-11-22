use crate::{MTReportInfoDetail, ReportConfirmStatus, ReportId, TWO_DAY};
use codec::{Decode, Encode};
use generic_func::{ItemList, MachineId};
use sp_runtime::{traits::Zero, RuntimeDebug};
use sp_std::{
    cmp::PartialEq,
    ops::{Add, Sub},
    vec::Vec,
};

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

impl<AccountId, BlockNumber, Balance> MTReportResultInfo<AccountId, BlockNumber, Balance>
where
    AccountId: Default + Clone + Ord,
    BlockNumber: From<u32> + Add<Output = BlockNumber> + Default + Copy,
    Balance: Default + Copy,
{
    pub fn new_inaccessible_result(
        now: BlockNumber,
        report_id: ReportId,
        report_info: &MTReportInfoDetail<AccountId, BlockNumber, Balance>,
        machine_stash: AccountId,
    ) -> Self {
        Self {
            report_id,
            reporter: report_info.reporter.clone(),
            slash_time: now,
            slash_exec_time: now + TWO_DAY.into(),
            slash_result: MCSlashResult::Pending,
            machine_stash,
            machine_id: report_info.machine_id.clone(),
            reporter_stake: report_info.reporter_stake,

            ..Default::default()
        }
    }

    pub fn add_unruly(&mut self, who: AccountId) {
        ItemList::add_item(&mut self.unruly_committee, who);
    }
}

impl<Account, Block, Balance> MTReportResultInfo<Account, Block, Balance>
where
    Account: Ord + Clone,
    Balance: Zero,
{
    pub fn is_slashed_reporter(&self, who: &Account) -> bool {
        matches!(
            self.report_result,
            ReportResultType::ReportRefused | ReportResultType::ReporterNotSubmitEncryptedInfo
        ) && &self.reporter == who
    }

    pub fn is_slashed_committee(&self, who: &Account) -> bool {
        self.inconsistent_committee.binary_search(who).is_ok() ||
            self.unruly_committee.binary_search(who).is_ok()
    }

    pub fn is_slashed_stash(&self, who: &Account) -> bool {
        matches!(self.report_result, ReportResultType::ReportSucceed) && &self.machine_stash == who
    }

    pub fn i_exten_sorted(&mut self, a_list: Vec<Account>) {
        for a_item in a_list {
            ItemList::add_item(&mut self.inconsistent_committee, a_item);
        }
    }

    pub fn r_exten_sorted(&mut self, a_list: Vec<Account>) {
        for a_item in a_list {
            ItemList::add_item(&mut self.reward_committee, a_item);
        }
    }

    // 接收report_info.summary结果，修改自身
    // 仅仅在summary_waiting_raw中使用
    pub fn get_verify_result(
        &mut self,
        now: Block,
        report_id: ReportId,
        committee_order_stake: Balance,
        report_info: &MTReportInfoDetail<Account, Block, Balance>,
    ) where
        Account: Default + Clone + Ord,
        Block: Default + PartialEq + Zero + From<u32> + Copy + Sub<Output = Block> + PartialOrd,
        Balance: Default + Copy,
    {
        self.report_id = report_id;
        self.slash_result = MCSlashResult::Pending;
        self.slash_time = now;
        self.slash_exec_time = now + TWO_DAY.into();
        self.reporter = report_info.reporter.clone();
        self.committee_stake = committee_order_stake;

        let verify_summary = report_info.summary();
        match verify_summary {
            // 报告成功
            ReportConfirmStatus::Confirmed(support, against, _) => {
                self.report_result = ReportResultType::ReportSucceed;
                self.reporter_stake = report_info.reporter_stake;

                for a_committee in against {
                    ItemList::add_item(&mut self.inconsistent_committee, a_committee.clone());
                }

                for a_committee in support.clone() {
                    ItemList::add_item(&mut self.reward_committee, a_committee.clone());
                }
            },
            // 报告失败
            ReportConfirmStatus::Refuse(support_committee, against_committee) => {
                self.report_result = ReportResultType::ReportRefused;
                self.reporter_stake = report_info.reporter_stake;

                // Slash support committee and release against committee stake
                self.i_exten_sorted(support_committee);
                self.r_exten_sorted(against_committee);
            },
            // 如果没有人提交，会出现NoConsensus的情况，并重新派单
            ReportConfirmStatus::NoConsensus => {
                self.report_result = ReportResultType::NoConsensus;

                // 记录unruly的委员会，两天后进行惩罚
                ItemList::expand_to_order(
                    &mut self.unruly_committee,
                    report_info.booked_committee.clone(),
                );

                // 重新举报时，记录报告人的质押将被重新使用，因此不再退还。
                self.reporter_stake = Zero::zero();
            },
        }
    }
}
