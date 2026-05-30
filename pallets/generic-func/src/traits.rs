use crate::{BalanceOf, Config, Pallet};
use dbc_support::traits::GNOps;
use frame_support::traits::{BalanceStatus, Imbalance, OnUnbalanced, ReservableCurrency};
use sp_runtime::{
    traits::{Saturating, Zero},
    Perbill,
};
use sp_std::prelude::Vec;

impl<T: Config> GNOps for Pallet<T> {
    type AccountId = T::AccountId;
    type Balance = BalanceOf<T>;

    fn slash_and_reward(
        slash_who: Vec<T::AccountId>,
        each_slash: BalanceOf<T>, // 每个人惩罚的金额
        reward_who: Vec<T::AccountId>,
    ) -> Result<BalanceOf<T>, ()> {
        // 如果reward_to为0，则将币转到国库
        let reward_to_num = reward_who.len() as u32;
        // Accumulate how much actually left the slashed accounts' reserve, so the
        // caller can keep its staking aggregates consistent with reality.
        let mut total_moved: BalanceOf<T> = Zero::zero();

        if slash_who.is_empty() || each_slash == Zero::zero() {
            return Ok(Zero::zero())
        }

        if reward_to_num == 0 {
            // Slash to Treasury
            for a_slash_person in slash_who {
                if T::Currency::reserved_balance(&a_slash_person) >= each_slash {
                    let (imbalance, _missing) =
                        T::Currency::slash_reserved(&a_slash_person, each_slash);
                    total_moved = total_moved.saturating_add(imbalance.peek());
                    T::Slash::on_unbalanced(imbalance);
                }
            }
            return Ok(total_moved)
        }

        for a_slash_person in slash_who {
            let reward_each_get = Perbill::from_rational(1u32, reward_to_num) * each_slash;
            let mut left_reward = each_slash;

            for a_committee in &reward_who {
                if T::Currency::reserved_balance(&a_slash_person) >= left_reward {
                    let to_move =
                        if left_reward >= reward_each_get { reward_each_get } else { left_reward };
                    // repatriate_reserved returns Ok(not_moved); moved = to_move - not_moved.
                    if let Ok(not_moved) = T::Currency::repatriate_reserved(
                        &a_slash_person,
                        a_committee,
                        to_move,
                        BalanceStatus::Free,
                    ) {
                        total_moved = total_moved.saturating_add(to_move.saturating_sub(not_moved));
                    }
                    left_reward = left_reward.saturating_sub(to_move);
                }
            }
            if left_reward > Zero::zero() {
                let (imbalance, _missing) =
                    T::Currency::slash_reserved(&a_slash_person, left_reward);
                total_moved = total_moved.saturating_add(imbalance.peek());
                T::Slash::on_unbalanced(imbalance);
            }
        }

        Ok(total_moved)
    }
}
