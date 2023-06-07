use codec::{Decode, Encode};
use dbc_support::{
    report::{MCSlashResult, MTReportResultInfo, ReportResultType},
    MachineId, ReportId,
};
use frame_support::{traits::Get, weights::Weight, IterableStorageMap, RuntimeDebug};
use scale_info::TypeInfo;

use crate::Config;

// pub fn apply<T: Config>() -> Weight {
//     frame_support::debug::RuntimeLogger::init();
//     info!(
//         target: "runtime::maintain_committee",
//         "Running migration for maintainCommittee pallet"
//     );
// }

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
pub struct OldMTReportResultInfo<AccountId, BlockNumber, Balance> {
    pub report_id: ReportId,
    // 变更为Option<AccountId>
    pub reporter: AccountId,
    pub reporter_stake: Balance,

    pub inconsistent_committee: Vec<AccountId>,
    pub unruly_committee: Vec<AccountId>,
    pub reward_committee: Vec<AccountId>,
    pub committee_stake: Balance,

    // 变更为Option<AccountId>
    pub machine_stash: AccountId,
    pub machine_id: MachineId,

    pub slash_time: BlockNumber,
    pub slash_exec_time: BlockNumber,
    pub report_result: ReportResultType,
    pub slash_result: MCSlashResult,
}

impl<AccountId, BlockNumber, Balance> From<OldMTReportResultInfo<AccountId, BlockNumber, Balance>>
    for MTReportResultInfo<AccountId, BlockNumber, Balance>
{
    fn from(
        info: OldMTReportResultInfo<AccountId, BlockNumber, Balance>,
    ) -> MTReportResultInfo<AccountId, BlockNumber, Balance> {
        MTReportResultInfo {
            report_id: info.report_id,
            reporter: info.reporter,
            reporter_stake: info.reporter_stake,
            inconsistent_committee: info.inconsistent_committee,
            unruly_committee: info.unruly_committee,
            reward_committee: info.reward_committee,
            committee_stake: info.committee_stake,
            machine_stash: Some(info.machine_stash),
            machine_id: info.machine_id,
            slash_time: info.slash_time,
            slash_exec_time: info.slash_exec_time,
            report_result: info.report_result,
            slash_result: info.slash_result,
        }
    }
}

pub fn migrate_storage() -> Weight {
    todo!();
    Weight::zero()
}
