use crate::{BalanceOf, Committee, CommitteeStake, Config, Pallet, ReportId};
use frame_support::{ensure, traits::ReservableCurrency};
use online_profile_machine::ManageCommittee;
use sp_runtime::traits::{CheckedAdd, CheckedSub};
use sp_std::vec::Vec;

impl<T: Config> ManageCommittee for Pallet<T> {
    type AccountId = T::AccountId;
    type Balance = BalanceOf<T>;
    type ReportId = ReportId;

    // 检查是否为状态正常的委员会
    fn is_valid_committee(who: &T::AccountId) -> bool {
        Self::committee().normal.binary_search(&who).is_ok()
    }

    // 检查委员会是否有足够的质押,返回有可以抢单的机器列表
    // 在每个区块以及每次分配一个机器之后，都需要检查
    fn available_committee() -> Option<Vec<T::AccountId>> {
        let committee_list = Self::committee();
        (committee_list.normal.len() > 0).then(|| committee_list.normal)
    }

    fn change_stake_for_slash_review(committee: T::AccountId, amount: BalanceOf<T>, is_add: bool) -> Result<(), ()> {
        let mut committee_stake = Self::committee_stake(&committee);

        if is_add {
            ensure!(<T as Config>::Currency::can_reserve(&committee, amount), ());
            <T as Config>::Currency::reserve(&committee, amount).map_err(|_| ())?;
            committee_stake.staked_amount = committee_stake.staked_amount.checked_add(&amount).ok_or(())?;
        } else {
            committee_stake.staked_amount = committee_stake.staked_amount.checked_sub(&amount).ok_or(())?;
            let _ = <T as Config>::Currency::unreserve(&committee, amount);
        }

        CommitteeStake::<T>::insert(&committee, committee_stake);
        Ok(())
    }

    // 改变委员会使用的质押数量
    // - Writes: CommitteeStake, Committee
    fn change_used_stake(committee: T::AccountId, amount: BalanceOf<T>, is_add: bool) -> Result<(), ()> {
        let mut committee_stake = Self::committee_stake(&committee);
        let mut committee_list = Self::committee();

        // 计算下一阶段需要的质押数量
        committee_stake.used_stake = if is_add {
            committee_stake.used_stake.checked_add(&amount).ok_or(())?
        } else {
            committee_stake.used_stake.checked_sub(&amount).ok_or(())?
        };

        let is_committee_list_changed =
            Self::change_committee_status_when_stake_changed(committee.clone(), &mut committee_list, &committee_stake);

        if is_committee_list_changed {
            Committee::<T>::put(committee_list);
        }
        CommitteeStake::<T>::insert(&committee, committee_stake);

        Ok(())
    }

    fn change_total_stake(committee: T::AccountId, amount: BalanceOf<T>, is_add: bool) -> Result<(), ()> {
        let mut committee_stake = Self::committee_stake(&committee);
        let mut committee_list = Self::committee();

        committee_stake.staked_amount = if is_add {
            committee_stake.staked_amount.checked_add(&amount).ok_or(())?
        } else {
            committee_stake.staked_amount.checked_sub(&amount).ok_or(())?
        };

        let is_committee_list_changed =
            Self::change_committee_status_when_stake_changed(committee.clone(), &mut committee_list, &committee_stake);

        if is_committee_list_changed {
            Committee::<T>::put(committee_list);
        }

        CommitteeStake::<T>::insert(&committee, committee_stake);
        Ok(())
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
