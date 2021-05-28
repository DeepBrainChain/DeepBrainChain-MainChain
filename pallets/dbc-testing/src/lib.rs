#![recursion_limit = "128"]
#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::debug;
use frame_support::traits::{Currency, Imbalance, OnUnbalanced};
use frame_system::{self as system, ensure_root, ensure_signed};
use phase_reward::PhaseReward;
use sp_arithmetic::{traits::Saturating, Permill};
use sp_io::hashing::blake2_128;
use sp_std::{convert::TryInto, str};

type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as system::Config>::AccountId>>::Balance;
type PositiveImbalanceOf<T> = <<T as Config>::Currency as Currency<
    <T as frame_system::Config>::AccountId,
>>::PositiveImbalance;
type NegativeImbalanceOf<T> = <<T as Config>::Currency as Currency<
    <T as frame_system::Config>::AccountId,
>>::NegativeImbalance;

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode, Default)]
pub struct TestMachineInfo<AccountId, BlockNumber> {
    pub machine_owner: AccountId,
    pub bonding_height: BlockNumber,
    pub machine_grade: u64,
    pub machine_price: u64,
    pub reward_deadline: BlockNumber,
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_timestamp::Config {
        type Currency: Currency<Self::AccountId>;
        type PhaseReward: PhaseReward<Balance = BalanceOf<Self>>;
        type Slash: OnUnbalanced<NegativeImbalanceOf<Self>>;
        type Reward: OnUnbalanced<PositiveImbalanceOf<Self>>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    #[pallet::getter(fn things1)]
    pub(super) type Things1<T: Config> = StorageValue<_, u64>;

    #[pallet::storage]
    #[pallet::getter(fn things2)]
    pub(super) type Things2<T: Config> = StorageValue<_, u64>;

    #[pallet::storage]
    #[pallet::getter(fn things3)]
    pub(super) type Things3<T: Config> = StorageValue<_, u64>;

    #[pallet::storage]
    #[pallet::getter(fn things4)]
    pub(super) type Things4<T: Config> = StorageValue<_, Permill>;

    #[pallet::storage]
    #[pallet::getter(fn things5)]
    pub(super) type Things5<T: Config> =
        StorageValue<_, TestMachineInfo<T::AccountId, T::BlockNumber>, ValueQuery>;

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(0)]
        pub fn return_err_test(origin: OriginFor<T>, in_num: u32) -> DispatchResultWithPostInfo {
            ensure_signed(origin)?;
            if in_num == 0 {
                return Err(Error::<T>::TestError.into());
            }
            Ok(().into())
        }

        #[pallet::weight(0)]
        pub fn set_things5(
            origin: OriginFor<T>,
            new_data: TestMachineInfo<T::AccountId, T::BlockNumber>,
        ) -> DispatchResultWithPostInfo {
            ensure_signed(origin)?;

            Things5::<T>::put(new_data);

            Ok(().into())
        }

        /// Slashes the specified amount of funds from the specified account
        #[pallet::weight(0)]
        pub fn slash_funds(
            origin: OriginFor<T>,
            _to_punish: T::AccountId,
            collateral: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            let _ = ensure_signed(origin)?;

            T::Slash::on_unbalanced(T::Currency::issue(collateral));

            let _now = <frame_system::Module<T>>::block_number();
            Ok(().into())
        }

        /// Awards the specified amount of funds to the specified account
        #[pallet::weight(0)]
        pub fn reward_funds(
            origin: OriginFor<T>,
            to_reward: T::AccountId,
            reward: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            let _ = ensure_signed(origin)?;

            let mut total_imbalance = <PositiveImbalanceOf<T>>::zero();

            let r = T::Currency::deposit_into_existing(&to_reward, reward).ok();
            total_imbalance.maybe_subsume(r);
            T::Reward::on_unbalanced(total_imbalance);

            let _now = <frame_system::Module<T>>::block_number();
            Ok(().into())
        }

        #[pallet::weight(0)]
        fn say_hello(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            // let secs_per_block = babe::Module::<T>::slot_duration();
            // let secs_per_block2 = <babe::Module<T>>::slot_duration();

            let secs_per_block =
                <T as pallet_timestamp::Config>::MinimumPeriod::get().saturating_mul(2u32.into());

            // let secs_per_block =<T as babe::Config>::slot_duration();
            // let secs_per_block = <T as timestamp::Config>::MinimumPeriod::get();
            let caller = ensure_signed(origin)?;

            let mut output: [u8; 35] = [0; 35];
            let decoded =
                bs58::decode("5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY").into(&mut output);

            let account_id_32: [u8; 32] = output[1..33].try_into().unwrap();
            debug::info!("########## decoded2 Alice: {:?}, {:?}", decoded, output);

            let b = T::AccountId::decode(&mut &account_id_32[..]).unwrap_or_default();

            if caller == b {
                debug::info!("########## true");
            }

            debug::info!(
                "######### Request sent by: {:?}, {:?} #########",
                caller,
                secs_per_block,
                // secs_per_block2
            );
            Ok(().into())
        }

        #[pallet::weight(0)]
        fn set_phase0_reward(
            origin: OriginFor<T>,
            reward_balance: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            T::PhaseReward::set_phase0_reward(reward_balance);
            Ok(().into())
        }

        #[pallet::weight(0)]
        fn set_phase1_reward(
            origin: OriginFor<T>,
            reward_balance: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            T::PhaseReward::set_phase1_reward(reward_balance);
            Ok(().into())
        }

        #[pallet::weight(0)]
        fn set_phase2_reward(
            origin: OriginFor<T>,
            reward_balance: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            T::PhaseReward::set_phase2_reward(reward_balance);
            Ok(().into())
        }

        #[pallet::weight(0)]
        fn set_fix_point(origin: OriginFor<T>, new_factor: Permill) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            Things4::<T>::put(new_factor);

            Ok(().into())
        }

        #[pallet::weight(0)]
        fn test_blake2_128(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let encode_data: [u8; 16] = blake2_128(&b"Hello world!"[..]); // .to_vec().encode();
            debug::info!(
                "###### blake2_128 Hash of Hello world! is: {:?}",
                encode_data
            );
            Ok(().into())
        }

        #[pallet::weight(0)]
        fn test_submit_hash(origin: OriginFor<T>, hash: [u8; 16]) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let encode_data: [u8; 16] = blake2_128(&b"Hello world!"[..]); // .to_vec().encode();
            if encode_data == hash {
                debug::info!("##### good hash {:?}", hash);
            } else {
                debug::info!("##### bad hash {:?}", hash);
            }

            Ok(().into())
        }
    }

    #[pallet::error]
    pub enum Error<T> {
        TestError,
    }
}

impl<T: Config> Pallet<T> {
    // TODO: why cannot run here?
    fn _test() {
        let mut output: [u8; 35] = [0; 35];
        let decoded =
            bs58::decode("5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY").into(&mut output);

        let account_id_32: [u8; 32] = output[1..33].try_into().unwrap();
        debug::info!("########## decoded2 Alice: {:?}, {:?}", decoded, output);

        let _b = T::AccountId::decode(&mut &account_id_32[..]).unwrap_or_default();
    }
}
