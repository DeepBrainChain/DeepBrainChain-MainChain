use codec::{alloc::string::ToString, Decode, Encode, HasCompact};
use sp_runtime::{
    traits::{AtLeast32BitUnsigned, Saturating, Zero},
    RuntimeDebug,
};
use sp_std::{collections::btree_map::BTreeMap, collections::vec_deque::VecDeque, prelude::*, str};

pub type MachineId = Vec<u8>;
pub type EraIndex = u32;

pub const LOCK_BLOCK_EXPIRATION: u32 = 3; // in block number

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode, Default)]
pub struct MachineInfoByCommittee {
    pub machine_id: MachineId,
    pub gpu_type: Vec<u8>,
    pub gpu_num: u32,
    pub cuda_core: u32,
    pub gpu_mem: u64,
    pub calc_point: u64,
    pub hard_disk: u64,
    pub upload_net: u64,
    pub download_net: u64,
    pub cpu_type: Vec<u8>,
    pub cpu_core_num: u32,
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

    pub released_rewards: Balance, // 委员会和用户已经释放的奖励
    pub upcoming_rewards: VecDeque<Balance>, // 用户剩余未释放的奖励
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
pub struct EraRewardBalance<AccountId: Ord, Balance> {
    /// Total number of points. Equals the sum of reward points for each validator.
    pub total: Balance,
    /// The reward points earned by a given validator.
    pub individual: BTreeMap<AccountId, Balance>,
}

impl<AccountId, Balance: HasCompact + Copy + Saturating + AtLeast32BitUnsigned>
    StakingLedger<AccountId, Balance>
{
    // 筛选去掉已经到期的unlocking
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
        let total = &mut self.total; // total = active + releasing
        let active = &mut self.active;

        let slash_out_of =
            |total_remaining: &mut Balance, target: &mut Balance, value: &mut Balance| {
                let mut slash_from_target = (*value).min(*target); // 最小惩罚 = min(avtive, slash)

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

        slash_out_of(total, active, &mut value); // 扣除处罚的资金

        let i = self
            .unlocking
            .iter_mut()
            .map(|chunk| {
                slash_out_of(total, &mut chunk.value, &mut value); // 从正在解压的部分中，扣除剩下的罚款
                chunk.value
            })
            .take_while(|value| value.is_zero())
            .count();

        let _ = self.unlocking.drain(..i); // 删掉为0的chunk

        pre_total.saturating_sub(*total) // 返回一共惩罚成功的资金
    }
}
