use crate::{ItemList, ReportId};
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_runtime::{traits::Saturating, RuntimeDebug};
use sp_std::{cmp::PartialEq, vec::Vec};

/// 报告人的报告记录
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug, TypeInfo)]
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

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug, TypeInfo)]
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
