use codec::{Decode, Encode};
use dbc_support::{
    verify_committee_slash::{OCPendingSlashInfo, OCSlashResult},
    verify_online::OCBookResultType,
    MachineId,
};
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;
use sp_std::vec::Vec;

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
struct OldOCPendingSlashInfo<AccountId, BlockNumber, Balance> {
    pub machine_id: MachineId,
    pub machine_stash: AccountId, // Changed to Option<AccountId>
    pub stash_slash_amount: Balance,
    // info refused, maybe slash amount is different
    pub inconsistent_committee: Vec<AccountId>,
    pub unruly_committee: Vec<AccountId>,
    pub reward_committee: Vec<AccountId>,
    pub committee_stake: Balance,
    pub slash_time: BlockNumber,
    pub slash_exec_time: BlockNumber,
    pub book_result: OCBookResultType,
    pub slash_result: OCSlashResult,
}
// A: AccountId, B: BlockNumber, C: Balance
impl<A, B, C> From<OldOCPendingSlashInfo<A, B, C>> for OCPendingSlashInfo<A, B, C> {
    fn from(info: OldOCPendingSlashInfo<A, B, C>) -> OCPendingSlashInfo<A, B, C> {
        OCPendingSlashInfo {
            machine_id: info.machine_id,
            machine_stash: None,
            stash_slash_amount: info.stash_slash_amount,
            inconsistent_committee: info.inconsistent_committee,
            unruly_committee: info.unruly_committee,
            reward_committee: info.reward_committee,
            committee_stake: info.committee_stake,
            slash_time: info.slash_time,
            slash_exec_time: info.slash_exec_time,
            book_result: info.book_result,
            slash_result: info.slash_result,
        }
    }
}

// pub fn migrate<T: Config>() {
//     <PendingOnlineSlash<T>>::translate(
//         |_key, old: OldOCPendingSlashInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>| {
//             Some(old.into())
//         },
//     );
// }
