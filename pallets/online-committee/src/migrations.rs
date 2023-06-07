
// TODO: 1. 迁移这个存储
// TODO: 2. 迁移 terminating_rental 存储
// #[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
// pub struct OldOCPendingSlashInfo<AccountId, BlockNumber, Balance> {
//     pub machine_id: MachineId,
//     pub machine_stash: AccountId,
//     pub stash_slash_amount: Balance,

//     // info refused, maybe slash amount is different
//     pub inconsistent_committee: Vec<AccountId>,
//     pub unruly_committee: Vec<AccountId>,
//     pub reward_committee: Vec<AccountId>,
//     pub committee_stake: Balance,

//     // TODO: maybe should record slash_reason: refuse online refused or change hardware
//     pub slash_time: BlockNumber,
//     pub slash_exec_time: BlockNumber,

//     pub book_result: OCBookResultType,
//     pub slash_result: OCSlashResult,
// }