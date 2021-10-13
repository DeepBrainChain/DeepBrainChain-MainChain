use crate::{BalanceOf, Config, Pallet};
use frame_support::traits::{BalanceStatus, OnUnbalanced, ReservableCurrency};
use online_profile_machine::GNOps;
use sp_runtime::{
    traits::{CheckedSub, Zero},
    Perbill,
};
use sp_std::prelude::Vec;

impl<T: Config> GNOps for Pallet<T> {
    type AccountId = T::AccountId;
    type Balance = BalanceOf<T>;

    fn slash_and_reward(
        slash_who: Vec<T::AccountId>,
        each_slash: BalanceOf<T>,
        reward_who: Vec<T::AccountId>,
    ) -> Result<(), ()> {
        // 如果reward_to为0，则将币转到国库
        let reward_to_num = reward_who.len() as u32;

        if slash_who.len() == 0 || each_slash == Zero::zero() {
            return Ok(())
        }

        if reward_to_num == 0 {
            // Slash to Treasury
            for a_slash_person in slash_who {
                if T::Currency::reserved_balance(&a_slash_person) >= each_slash {
                    let (imbalance, _missing) = T::Currency::slash_reserved(&a_slash_person, each_slash);
                    T::Slash::on_unbalanced(imbalance);
                }
            }
            return Ok(())
        }

        for a_slash_person in slash_who {
            let reward_each_get = Perbill::from_rational_approximation(1u32, reward_to_num) * each_slash;
            let mut left_reward = each_slash;

            for a_committee in &reward_who {
                if T::Currency::reserved_balance(&a_slash_person) >= left_reward {
                    if left_reward >= reward_each_get {
                        let _ = T::Currency::repatriate_reserved(
                            &a_slash_person,
                            a_committee,
                            reward_each_get,
                            BalanceStatus::Free,
                        );
                        left_reward = left_reward.checked_sub(&reward_each_get).ok_or(())?;
                    } else {
                        let _ = T::Currency::repatriate_reserved(
                            &a_slash_person,
                            a_committee,
                            left_reward,
                            BalanceStatus::Free,
                        );
                    }
                }
            }
        }

        Ok(())
    }
}
