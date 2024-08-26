use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use parity_scale_codec::{Decode, Encode};
use sp_runtime::{Perbill, RuntimeDebug};
use sp_std::{vec, vec::Vec};

// 处于不同状态的委员会的列表
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct CommitteeList<AccountId: Ord> {
    /// 质押并通过社区选举的委员会，正常状态
    pub normal: Vec<AccountId>,
    /// 委员会，但不想被派单
    pub chill_list: Vec<AccountId>,
    /// 等待提交box pubkey的委员会
    pub waiting_box_pubkey: Vec<AccountId>,
    /// 等待补充质押的委员会
    pub fulfilling_list: Vec<AccountId>,
}

impl<AccountId: Ord> Default for CommitteeList<AccountId> {
    fn default() -> Self {
        Self {
            normal: vec![],
            chill_list: vec![],
            waiting_box_pubkey: vec![],
            fulfilling_list: vec![],
        }
    }
}

impl<AccountId: Ord> CommitteeList<AccountId> {
    pub fn is_normal(&self, who: &AccountId) -> bool {
        self.normal.binary_search(who).is_ok()
    }
    pub fn is_chill(&self, who: &AccountId) -> bool {
        self.chill_list.binary_search(who).is_ok()
    }
    pub fn is_waiting_puk(&self, who: &AccountId) -> bool {
        self.waiting_box_pubkey.binary_search(who).is_ok()
    }
    pub fn is_fulfilling(&self, who: &AccountId) -> bool {
        self.fulfilling_list.binary_search(who).is_ok()
    }
    pub fn is_committee(&self, who: &AccountId) -> bool {
        self.is_normal(who) ||
            self.is_chill(who) ||
            self.is_waiting_puk(who) ||
            self.is_fulfilling(who)
    }
}

/// 与委员会质押基本参数
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug, TypeInfo)]
pub struct CommitteeStakeParamsInfo<Balance> {
    /// 第一次委员会质押的基准数值
    pub stake_baseline: Balance,
    /// 每次订单使用的质押数量 & apply_slash_review stake amount
    pub stake_per_order: Balance,
    /// 当剩余的质押数量到阈值时，需要补质押
    pub min_free_stake_percent: Perbill,
}

/// 委员会质押的状况
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug, TypeInfo)]
pub struct CommitteeStakeInfo<Balance> {
    pub box_pubkey: [u8; 32],
    pub staked_amount: Balance,
    pub used_stake: Balance,
    pub can_claim_reward: Balance,
    pub claimed_reward: Balance,
}
