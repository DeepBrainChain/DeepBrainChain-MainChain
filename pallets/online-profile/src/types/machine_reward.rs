use parity_scale_codec::{Decode, Encode};
use dbc_support::{EraIndex, MachineId};
use scale_info::TypeInfo;
use sp_runtime::{Perbill, RuntimeDebug};
use sp_std::{
    collections::{btree_map::BTreeMap, vec_deque::VecDeque},
    ops::{Add, Sub},
    vec::Vec,
};

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug, TypeInfo)]
pub struct MachineRecentRewardInfo<AccountId, Balance> {
    // machine total reward(committee reward included)
    pub machine_stash: AccountId,
    pub recent_machine_reward: VecDeque<Balance>,
    pub recent_reward_sum: Balance,
    // the days which committee can get reward
    pub reward_committee_deadline: EraIndex,
    pub reward_committee: Vec<AccountId>,
}

// NOTE: Call order of add_new_reward and get_..released is very important
// Add new reward first, then calc committee/stash released reward
impl<AccountId, Balance> MachineRecentRewardInfo<AccountId, Balance>
where
    Balance: Default
        + Clone
        + Add<Output = Balance>
        + Sub<Output = Balance>
        + Copy
        + sp_runtime::traits::Saturating,
{
    pub fn add_new_reward(&mut self, reward_amount: Balance) {
        let mut reduce = Balance::default();

        if self.recent_machine_reward.len() == 150 {
            reduce = self.recent_machine_reward.pop_front().unwrap();
            self.recent_machine_reward.push_back(reward_amount);
        } else {
            self.recent_machine_reward.push_back(reward_amount);
        }

        self.recent_reward_sum =
            self.recent_reward_sum.saturating_add(reward_amount).saturating_sub(reduce);
    }
}

/// 记录每个Era的机器的总分
/// NOTE: 这个账户应该是stash账户，而不是controller账户
#[derive(PartialEq, Encode, Decode, RuntimeDebug, Clone, TypeInfo)]
pub struct EraStashPoints<AccountId: Ord> {
    /// Total grade of the system (inflation grades from onlineStatus or multipGPU is counted)
    pub total: u64,
    /// Some Era，grade snap of stash account
    pub staker_statistic: BTreeMap<AccountId, StashMachineStatistics>,
}

impl<AccountId: Ord> Default for EraStashPoints<AccountId> {
    fn default() -> Self {
        Self { total: 0, staker_statistic: BTreeMap::new() }
    }
}

/// Stash账户的统计
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone, TypeInfo)]
pub struct StashMachineStatistics {
    /// 用户在线的GPU数量
    pub online_gpu_num: u64,
    /// 用户对应的膨胀系数，由在线GPU数量决定
    pub inflation: Perbill,
    /// 用户的机器的总计算点数得分(不考虑膨胀)
    pub machine_total_calc_point: u64,
    /// 用户机器因被租用获得的额外得分
    pub rent_extra_grade: u64,
}

// 每台机器的基础得分与租用情况
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone, TypeInfo)]
pub struct MachineGradeStatus {
    /// 机器的基础得分
    pub basic_grade: u64,
    /// 机器的租用状态
    pub is_rented: bool,
}

impl<AccountId: Ord + Clone> EraStashPoints<AccountId> {
    /// 增加一台在线的机器，gpu数量 + gpu的总得分
    /// NOTE: 只修改当前Era，调用下线逻辑前应检查机器存在
    pub fn on_online_changed(
        &mut self,
        stash: AccountId,
        gpu_num: u64,
        basic_grade: u64,
        is_online: bool,
    ) {
        let mut staker_statistic = self
            .staker_statistic
            .entry(stash.clone())
            .or_insert(StashMachineStatistics { ..Default::default() });

        let pre_grade = staker_statistic.total_grades().unwrap_or_default();

        staker_statistic.online_gpu_num = if is_online {
            staker_statistic.online_gpu_num.saturating_add(gpu_num)
        } else {
            // 避免上线24小时即下线时，当前Era还没有初始化该值
            staker_statistic.online_gpu_num.saturating_sub(gpu_num)
        };
        // 根据显卡数量n更新inflation系数: inflation = min(10%, n/10000)
        // 当stash账户显卡数量n=1000时，inflation最大为10%
        staker_statistic.inflation =
            Perbill::from_rational(staker_statistic.online_gpu_num.min(1000), 10_000);
        // 根据在线情况更改stash的基础分
        staker_statistic.machine_total_calc_point = if is_online {
            staker_statistic.machine_total_calc_point.saturating_add(basic_grade)
        } else {
            staker_statistic.machine_total_calc_point.saturating_sub(basic_grade)
        };

        // 更新系统分数记录
        let new_grade = staker_statistic.total_grades().unwrap_or_default();

        self.total = self.total.saturating_add(new_grade).saturating_sub(pre_grade);

        // 更新该stash账户的记录
        if staker_statistic.online_gpu_num == 0 {
            self.staker_statistic.remove(&stash);
        } else {
            let staker_statistic = (*staker_statistic).clone();
            self.staker_statistic.insert(stash, staker_statistic);
        }
    }

    /// 因机器租用状态改变，而影响得分
    pub fn on_rent_changed(&mut self, stash: AccountId, basic_grade: u64, is_rented: bool) {
        let mut staker_statistic = self
            .staker_statistic
            .entry(stash.clone())
            .or_insert(StashMachineStatistics { ..Default::default() });

        // 因租用而产生的分数
        let grade_on_rent = Perbill::from_rational(30u64, 100u64) * basic_grade;

        // 更新 rent_extra_grade
        self.total = if is_rented {
            self.total.saturating_add(grade_on_rent)
        } else {
            self.total.saturating_sub(grade_on_rent)
        };
        staker_statistic.rent_extra_grade = if is_rented {
            staker_statistic.rent_extra_grade.saturating_add(grade_on_rent)
        } else {
            staker_statistic.rent_extra_grade.saturating_sub(grade_on_rent)
        };

        let staker_statistic = (*staker_statistic).clone();
        self.staker_statistic.insert(stash, staker_statistic);
    }
}

impl StashMachineStatistics {
    /// 该Stash账户对应的总得分
    /// total_grades = inflation * total_calc_point + total_calc_point + rent_grade
    pub fn total_grades(&self) -> Option<u64> {
        (self.inflation * self.machine_total_calc_point)
            .checked_add(self.machine_total_calc_point)?
            .checked_add(self.rent_extra_grade)
    }
}

impl MachineGradeStatus {
    pub fn machine_actual_grade(&self, inflation: Perbill) -> u64 {
        let rent_extra_grade = if self.is_rented {
            Perbill::from_rational(30u32, 100u32) * self.basic_grade
        } else {
            0
        };
        let inflation_extra_grade = inflation * self.basic_grade;
        self.basic_grade
            .saturating_add(rent_extra_grade)
            .saturating_add(inflation_extra_grade)
    }
}

// 奖励发放前，对所有machine_id进行备份
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone, TypeInfo)]
pub struct AllMachineIdSnapDetail {
    pub all_machine_id: VecDeque<MachineId>,
    pub snap_len: u64,
}
