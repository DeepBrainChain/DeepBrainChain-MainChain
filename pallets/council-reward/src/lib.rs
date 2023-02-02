#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
    pallet_prelude::*,
    traits::{Currency, ExistenceRequirement::KeepAlive},
};
use frame_system::pallet_prelude::*;
use pallet_collective::Instance1;
use sp_runtime::{
    traits::{SaturatedConversion, Saturating, Zero},
    Perbill,
};

use sp_std::{vec, vec::Vec};

use dbc_support::traits::DbcPrice;

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
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type DbcPrice: DbcPrice<Balance = BalanceOf<Self>>;
        type Currency: Currency<Self::AccountId>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        // 当升级时设置国库地址
        fn on_runtime_upgrade() -> Weight {
            Treasury::<T>::mutate(|treasury| {
                let account: Vec<u8> = b"5GR31fgcHdrJ14eFW1xJmHhZJ56eQS7KynLKeXmDtERZTiw2".to_vec();
                let account_id32: [u8; 32] =
                    dbc_support::utils::get_accountid32(&account).unwrap_or_default();
                *treasury = T::AccountId::decode(&mut &account_id32[..]).ok().unwrap_or_default()
            });

            0
        }
    }

    #[pallet::storage]
    #[pallet::getter(fn treasury)]
    pub(super) type Treasury<T: Config> = StorageValue<_, T::AccountId, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn reward_params)]
    pub(super) type RewardParams<T: Config> = StorageValue<_, Vec<()>, ValueQuery>;

    // #[pallet::storage]
    // #[pallet::getter(fn council_prime)]
    // pub(super) type CouncilPrime<T: Config> = StorageValue<_, Option<T::AccountId>, ValueQuery>;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(0)]
        pub fn set_reward_params(_origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    // #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
    pub enum Event<T: Config> {
        Hello(T::AccountId),
    }

    #[pallet::error]
    pub enum Error<T> {
        Overflow,
    }
}

impl<T: Config> Pallet<T> {
    // TODO: handle error
    fn get_rewards() -> Vec<BalanceOf<T>> {
        let reward_amount = <T as Config>::DbcPrice::get_dbc_amount_by_value(5000_000_000).unwrap();
        let one_dbc: BalanceOf<T> = 1_000_000_000_000_000_u64.saturated_into();
        let reward_amount = reward_amount
            .min(1000_000_u64.saturated_into::<BalanceOf<T>>().saturating_mul(one_dbc));

        let first_reward = Perbill::from_rational_approximation(60u32, 100u32) * reward_amount;
        let second_reward = Perbill::from_rational_approximation(20u32, 100u32) * reward_amount;
        let third_reward = reward_amount.saturating_sub(first_reward).saturating_sub(second_reward);
        vec![first_reward, second_reward, third_reward]
    }

    fn get_primes_reward() -> Vec<(T::AccountId, BalanceOf<T>)> {
        let prime = pallet_collective::Module::<T, Instance1>::prime();
        let mut members = pallet_elections_phragmen::Module::<T>::members();
        members.sort_by(|a, b| b.stake.cmp(&a.stake));

        let rewards = Self::get_rewards();

        let mut out = vec![];
        let mut reward_index = 0;
        let prime = if let Some(prime) = prime {
            out.push((prime.clone(), rewards[reward_index]));
            reward_index += 1;
            prime
        } else {
            Default::default()
        };

        for member in members {
            if out.len() == 3 || reward_index >= 3 {
                break
            }
            if member.who != prime {
                out.push((member.who, rewards[reward_index]));
                reward_index += 1;
            }
        }
        out
    }

    pub fn reward_council() {
        let treasury = Self::treasury();
        let primes_reward = Self::get_primes_reward();
        for (reward_who, amount) in primes_reward {
            let _ = <T as Config>::Currency::transfer(&treasury, &reward_who, amount, KeepAlive);
        }
    }
}
