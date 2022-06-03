use crate::{BalanceOf, CommitteeStake, Config, Pallet, ReportId};
use online_profile_machine::ManageCommittee;
use sp_std::vec::Vec;

impl<T: Config> ManageCommittee for Pallet<T> {
    type AccountId = T::AccountId;
    type Balance = BalanceOf<T>;
    type ReportId = ReportId;

    // 检查是否为状态正常的委员会
    fn is_valid_committee(who: &T::AccountId) -> bool {
        Self::committee().is_normal(who)
    }

    // 检查委员会是否有足够的质押,返回有可以抢单的机器列表
    // 在每个区块以及每次分配一个机器之后，都需要检查
    fn available_committee() -> Option<Vec<T::AccountId>> {
        let committee_list = Self::committee();
        (!committee_list.normal.is_empty()).then(|| committee_list.normal)
    }

    // 改变委员会使用的质押数量
    // - Writes: CommitteeStake.used_stake(Add or Sub), Committee
    fn change_used_stake(committee: T::AccountId, amount: BalanceOf<T>, is_add: bool) -> Result<(), ()> {
        Self::do_change_used_stake(committee, amount, is_add)
    }

    // 改变Reserved的金额
    // - Writes: CommitteeStake.staked_amount(Add or Sub), Committee
    fn change_total_stake(
        committee: T::AccountId,
        amount: BalanceOf<T>,
        is_add: bool,
        change_reserve: bool,
    ) -> Result<(), ()> {
        Self::do_change_reserved(committee, amount, is_add, change_reserve)
    }

    fn stake_per_order() -> Option<BalanceOf<T>> {
        Some(Self::committee_stake_params()?.stake_per_order)
    }

    fn add_reward(committee: T::AccountId, reward: BalanceOf<T>) {
        let mut committee_stake = Self::committee_stake(&committee);
        committee_stake.can_claim_reward += reward;
        CommitteeStake::<T>::insert(&committee, committee_stake);
    }
}
