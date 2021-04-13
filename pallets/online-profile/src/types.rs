use codec::{Decode, Encode, HasCompact};
use sp_runtime::{
    traits::{AtLeast32BitUnsigned, Saturating, Zero},
    RuntimeDebug,
};
use sp_std::{collections::btree_map::BTreeMap, collections::vec_deque::VecDeque, prelude::*, str};

pub type RewardPoint = u32;

pub type MachineId = Vec<u8>;
pub type EraIndex = u32;

pub const MAX_PENDING_BONDING: usize = 20;
// pub const HTTP_REMOTE_REQUEST: &str = "http://116.85.24.172:41107/api/v1/mining_nodes/";
pub const HTTP_HEADER_USER_AGENT: &str = "jimmychu0807"; // TODO: remove this

pub const FETCH_TIMEOUT_PERIOD: u64 = 3_000; // in milli-seconds
pub const LOCK_TIMEOUT_EXPIRATION: u64 = FETCH_TIMEOUT_PERIOD + 1_000; // in milli-seconds
pub const LOCK_BLOCK_EXPIRATION: u32 = 3; // in block number

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MachineGradeInfo<AccountId> {
    confirmed_machine_grade: Vec<ConfirmedMachineGrade<AccountId>>,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct ConfirmedMachineGrade<AccountId> {
    confirmed: bool,
    confirmed_by: AccountId,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct BondingPair<AccountId> {
    pub account_id: AccountId,
    pub machine_id: MachineId,
    pub request_count: u64,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct BookingItem<BlockNumber> {
    pub machine_id: MachineId,
    pub book_time: BlockNumber,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct ConfirmedMachine<AccountId, BlockNumber> {
    pub machine_grade: MachineGradeDetail,
    pub committee_info: Vec<CommitteeInfo<AccountId, BlockNumber>>,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug, Copy)]
pub struct MachineGradeDetail {
    pub cpu: u64,
    pub disk: u64,
    pub gpu: u64,
    pub mem: u64,
    pub net: u64,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug, Copy)]
pub struct CommitteeInfo<AccountId, BlockNumber> {
    pub account_id: AccountId,
    pub block_height: BlockNumber,
    pub confirm: bool,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MachineStakeInfo<AccountId, Balance, BlockNumber> {
    pub account_id: AccountId,
    pub machine_id: MachineId,

    pub current_fee_balance: Balance, // money use can gain now
    pub current_locked_balance: Balance,

    pub bond_era: u32,
    pub bond_time: BlockNumber, // block height
    pub release_start_height: BlockNumber,
    pub release_end_height: BlockNumber, // release start height + 180 day
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MachineMeta<AccountId> {
    pub machine_price: u64, // 单位： 美分
    pub machine_grade: u64,
    pub committee_confirm: Vec<CommitteeConfirm<AccountId>>,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct CommitteeConfirm<AccountId> {
    pub committee: AccountId,
    pub confirm: bool,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct StakingLedger<AccountId, Balance: HasCompact> {
    pub stash: AccountId,

    #[codec(compact)]
    pub total: Balance,

    #[codec(compact)]
    pub active: Balance,

    pub unlocking: Vec<UnlockChunk<Balance>>,
    pub claimed_rewards: Vec<EraIndex>,

    pub released_rewards: Balance,
    pub upcoming_rewards: VecDeque<Balance>, // 构建一个150长度的，
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct UnlockChunk<Balance: HasCompact> {
    #[codec(compact)]
    pub value: Balance,

    #[codec(compact)]
    pub era: EraIndex,
}

// TOOD: 这个用来记录每个Era的总分,
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug)]
pub struct EraRewardGrades<AccountId: Ord> {
    /// Total number of points. Equals the sum of reward points for each validator.
    pub total: RewardPoint,
    /// The reward points earned by a given validator.
    pub individual: BTreeMap<AccountId, RewardPoint>,
}

impl<AccountId, Balance: HasCompact + Copy + Saturating + AtLeast32BitUnsigned>
    StakingLedger<AccountId, Balance>
{
    // 筛选去掉已经过期的unlocking
    pub fn consolidate_unlock(self, current_era: EraIndex) -> Self {
        let mut total = self.total;
        let unlocking = self
            .unlocking
            .into_iter()
            .filter(|chunk| {
                if chunk.era > current_era {
                    true
                } else {
                    total = total.saturating_sub(chunk.value);
                    false
                }
            })
            .collect();

        Self {
            stash: self.stash,
            total,
            active: self.active,
            unlocking,
            claimed_rewards: self.claimed_rewards,
            released_rewards: self.released_rewards,
            upcoming_rewards: self.upcoming_rewards,
        }
    }
}

impl<AccountId, Balance> StakingLedger<AccountId, Balance>
where
    Balance: AtLeast32BitUnsigned + Saturating + Copy,
{
    pub fn slash(&mut self, mut value: Balance, minimum_balance: Balance) -> Balance {
        let pre_total = self.total;
        let total = &mut self.total;
        let active = &mut self.active;

        let slash_out_of =
            |total_remaining: &mut Balance, target: &mut Balance, value: &mut Balance| {
                let mut slash_from_target = (*value).min(*target);

                if !slash_from_target.is_zero() {
                    *target -= slash_from_target;

                    if *target <= minimum_balance {
                        slash_from_target += *target;
                        *value += sp_std::mem::replace(target, Zero::zero());
                    }

                    *total_remaining = total_remaining.saturating_sub(slash_from_target);
                    *value -= slash_from_target;
                }
            };

        slash_out_of(total, active, &mut value);

        let i = self
            .unlocking
            .iter_mut()
            .map(|chunk| {
                slash_out_of(total, &mut chunk.value, &mut value);
                chunk.value
            })
            .take_while(|value| value.is_zero())
            .count();

        let _ = self.unlocking.drain(..i);

        pre_total.saturating_sub(*total)
    }
}
