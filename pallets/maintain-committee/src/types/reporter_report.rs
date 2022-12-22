use codec::{Decode, Encode};
use dbc_support::{ItemList, ReportId};
use sp_runtime::{traits::Saturating, Perbill, RuntimeDebug};
use sp_std::{cmp::PartialEq, vec::Vec};

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

// 处理除了inaccessible错误之外的错误
impl ReporterReportList {
    // 机器正在被该委员会验证，但该报告人超时未提交加密信息
    pub fn clean_not_submit_encrypted_report(&mut self, report_id: ReportId) {
        ItemList::rm_item(&mut self.processing_report, &report_id);
        ItemList::add_item(&mut self.failed_report, report_id);
    }
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
