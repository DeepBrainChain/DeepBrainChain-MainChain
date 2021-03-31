use codec::{Decode, Encode, HasCompact};
use sp_runtime::RuntimeDebug;
// use sp_std::{collections::btree_map::BTreeMap, prelude::*, str};

pub type MachineId = Vec<u8>;
pub type EraIndex = u32;

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct StakingLedger<AccountId, Balance: HasCompact> {
    pub stash: AccountId,

    #[codec(compact)]
    pub total: Balance,

    #[codec(compact)]
    pub active: Balance,

    pub unlocking: Vec<UnlockChunk<Balance>>,
    pub claimed_rewards: Vec<EraIndex>,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct UnlockChunk<Balance: HasCompact> {
    #[codec(compact)]
    pub value: Balance,

    #[codec(compact)]
    pub era: EraIndex,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct CommitteeConfirm<AccountId> {
    pub committee: AccountId,
    pub confirm: bool,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MachineMeta<AccountId> {
    pub machine_price: u64, // 单位： 美分
    pub machine_grade: u64,
    pub committee_confirm: Vec<CommitteeConfirm<AccountId>>,
}
