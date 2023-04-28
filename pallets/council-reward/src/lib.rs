#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

use dbc_support::traits::DbcPrice;
use frame_support::{
    pallet_prelude::*,
    traits::{Currency, ExistenceRequirement::KeepAlive},
};
use frame_system::pallet_prelude::*;
use pallet_collective::Instance1;
use pallet_elections_phragmen::SeatHolder;
use sp_runtime::traits::Zero;
use sp_std::{vec, vec::Vec};

pub use pallet::*;

type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + pallet_collective::Config<Instance1>
        + pallet_elections_phragmen::Config
    {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type DbcPrice: DbcPrice<Balance = BalanceOf<Self>>;
        type Currency: Currency<Self::AccountId>;

        /// How long each seat is kept. This defines the next block number at which an election
        /// round will happen. If set to zero, no elections are ever triggered and the module will
        /// be in passive mode.
        type RewardFrequency: Get<Self::BlockNumber>;

        // 奖励特定(USD or DBC)
        type PrimerReward: Get<(u64, BalanceOf<Self>)>;
        type SecondReward: Get<(u64, BalanceOf<Self>)>;
        type ThirdReward: Get<(u64, BalanceOf<Self>)>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_finalize(n: T::BlockNumber) {
            let reward_frequency = <T as pallet::Config>::RewardFrequency::get();
            // NOTE: 议会当选后顺延15天(43200 blocks)发放奖励
            if !reward_frequency.is_zero() && n % reward_frequency == 43200u32.into() {
                let prime = pallet_collective::Pallet::<T, Instance1>::prime();
                let mut members = pallet_elections_phragmen::Pallet::<T>::members();

                Self::reward_council(prime, &mut members);
            }
        }

        // 当升级时设置国库地址
        fn on_runtime_upgrade() -> Weight {
            let weight = Weight::default();

            Treasury::<T>::mutate(|treasury| {
                let account: Vec<u8> = b"5GR31fgcHdrJ14eFW1xJmHhZJ56eQS7KynLKeXmDtERZTiw2".to_vec();
                let account_id32: [u8; 32] =
                    dbc_support::utils::get_accountid32(&account).unwrap_or_default();
                // AccountId default bound is removed: https://github.com/paritytech/substrate/pull/10403
                *treasury = T::AccountId::decode(&mut &account_id32[..]).ok()
            });

            weight
        }
    }

    #[pallet::storage]
    #[pallet::getter(fn treasury)]
    pub(super) type Treasury<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

    #[pallet::call]
    impl<T: Config> Pallet<T> {}

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    // #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
    pub enum Event<T: Config> {
        RewardCouncil(T::AccountId, BalanceOf<T>),
    }

    #[pallet::error]
    pub enum Error<T> {
        Overflow,
    }
}

impl<T: Config> Pallet<T> {
    pub fn get_rewards() -> Vec<BalanceOf<T>> {
        let primer_reward = <T as pallet::Config>::PrimerReward::get();
        let second_reward = <T as pallet::Config>::SecondReward::get();
        let third_reward = <T as pallet::Config>::ThirdReward::get();

        vec![primer_reward, second_reward, third_reward]
            .into_iter()
            .map(|reward| {
                <T as Config>::DbcPrice::get_dbc_amount_by_value(reward.0)
                    .unwrap_or_default()
                    .min(reward.1)
            })
            .collect()
    }

    pub fn get_council_reward<U: Ord>(
        prime: Option<T::AccountId>,
        members: &mut Vec<SeatHolder<T::AccountId, U>>,
    ) -> Vec<(T::AccountId, BalanceOf<T>)> {
        members.sort_by(|a, b| b.stake.cmp(&a.stake));

        let rewards = Self::get_rewards();

        let mut out = vec![];
        let mut reward_index = 0;
        let prime = if let Some(prime) = prime {
            out.push((prime.clone(), rewards[reward_index]));
            reward_index += 1;
            Some(prime)
        } else {
            None
        };

        for member in members {
            if out.len() == 3 || reward_index >= 3 {
                break
            }
            if Some(member.who.clone()) != prime {
                out.push((member.who.clone(), rewards[reward_index]));
                reward_index += 1;
            }
        }
        out
    }

    pub fn reward_council<U: Ord>(
        prime: Option<T::AccountId>,
        members: &mut Vec<SeatHolder<T::AccountId, U>>,
    ) {
        let treasury = Self::treasury();
        let council_reward = Self::get_council_reward(prime, members);

        if let Some(treasury) = treasury {
            for (reward_who, amount) in council_reward {
                if <T as Config>::Currency::transfer(&treasury, &reward_who, amount, KeepAlive)
                    .is_ok()
                {
                    Self::deposit_event(Event::RewardCouncil(reward_who, amount));
                }
            }
        }
    }
}
