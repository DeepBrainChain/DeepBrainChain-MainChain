use crate::{
    report::{MTReportInfoDetail, ReportConfirmStatus},
    ItemList, MachineId, ReportId, TWO_DAY,
};
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_runtime::{
    traits::{Saturating, Zero},
    RuntimeDebug,
};
use sp_std::{
    cmp::PartialEq,
    ops::{Add, Sub},
    vec,
    vec::Vec,
};

/// 报告的处理结果
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
pub struct MTReportResultInfo<AccountId, BlockNumber, Balance> {
    pub report_id: ReportId,
    pub reporter: AccountId,
    pub reporter_stake: Balance,

    pub inconsistent_committee: Vec<AccountId>,
    pub unruly_committee: Vec<AccountId>,
    pub reward_committee: Vec<AccountId>,
    pub committee_stake: Balance,

    pub machine_stash: Option<AccountId>,
    pub machine_id: MachineId,

    pub slash_time: BlockNumber,
    pub slash_exec_time: BlockNumber,
    pub report_result: ReportResultType,
    pub slash_result: MCSlashResult,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
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

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
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

impl<AccountId, BlockNumber, Balance> MTReportResultInfo<AccountId, BlockNumber, Balance>
where
    AccountId: Clone + Ord,
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
            machine_stash: Some(machine_stash),
            machine_id: report_info.machine_id.clone(),
            reporter_stake: report_info.reporter_stake,
            inconsistent_committee: vec![],
            unruly_committee: vec![],
            reward_committee: vec![],
            committee_stake: Balance::default(),
            report_result: ReportResultType::default(),
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

    pub fn is_slashed_stash(&self, who: Account) -> bool {
        matches!(self.report_result, ReportResultType::ReportSucceed) &&
            self.machine_stash.clone() == Some(who)
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
        current_result: Option<Self>,
        now: Block,
        report_id: ReportId,
        committee_order_stake: Balance,
        report_info: &MTReportInfoDetail<Account, Block, Balance>,
    ) -> Self
    where
        Account: Clone + Ord,
        Block: Default
            + PartialEq
            + Zero
            + From<u32>
            + Copy
            + Sub<Output = Block>
            + PartialOrd
            + Saturating,
        Balance: Default + Copy,
    {
        let mut out = match current_result {
            Some(current_result) => Self {
                report_id,
                slash_result: MCSlashResult::Pending,
                slash_time: now,
                slash_exec_time: now + TWO_DAY.into(),
                reporter: report_info.reporter.clone(),
                committee_stake: committee_order_stake,
                ..current_result
            },
            None => Self {
                report_id,
                slash_result: MCSlashResult::Pending,
                slash_time: now,
                slash_exec_time: now + TWO_DAY.into(),
                reporter: report_info.reporter.clone(),
                committee_stake: committee_order_stake,

                reporter_stake: Default::default(),

                inconsistent_committee: vec![],
                unruly_committee: vec![],
                reward_committee: vec![],

                machine_stash: None,
                machine_id: vec![],

                report_result: ReportResultType::default(),
            },
        };
        let verify_summary = report_info.summary();
        match verify_summary {
            // 报告成功
            ReportConfirmStatus::Confirmed(support, against, _) => {
                out.report_result = ReportResultType::ReportSucceed;
                out.reporter_stake = report_info.reporter_stake;

                for a_committee in against {
                    ItemList::add_item(&mut out.inconsistent_committee, a_committee.clone());
                }

                for a_committee in support.clone() {
                    ItemList::add_item(&mut out.reward_committee, a_committee.clone());
                }
            },
            // 报告失败
            ReportConfirmStatus::Refuse(support_committee, against_committee) => {
                out.report_result = ReportResultType::ReportRefused;
                out.reporter_stake = report_info.reporter_stake;

                // Slash support committee and release against committee stake
                out.i_exten_sorted(support_committee);
                out.r_exten_sorted(against_committee);
            },
            // 如果没有人提交，会出现NoConsensus的情况，并重新派单
            ReportConfirmStatus::NoConsensus => {
                out.report_result = ReportResultType::NoConsensus;

                // 记录unruly的委员会，两天后进行惩罚
                ItemList::expand_to_order(
                    &mut out.unruly_committee,
                    report_info.booked_committee.clone(),
                );

                // 重新举报时，记录报告人的质押将被重新使用，因此不再退还。
                out.reporter_stake = Zero::zero();
            },
        }
        return out;
    }
}
