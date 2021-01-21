#![recursion_limit = "128"]
#![cfg_attr(not(feature = "std"), no_std)]

#[allow(unused_imports)]
use frame_support::{
    debug, decl_error, decl_event, decl_module, decl_storage, dispatch::DispatchResult,
};

#[allow(unused_imports)]
use frame_system::{
    self as system, ensure_none, ensure_root, ensure_signed, offchain::SendTransactionTypes,
};

use frame_support::traits::Currency;
use phase_reward::PhaseReward;

type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as system::Config>::AccountId>>::Balance;

// pub trait Config: frame_system::Config + timestamp::Config {}
pub trait Config: frame_system::Config + babe::Config {
    type Currency: Currency<Self::AccountId>;
    type PhaseReward: PhaseReward<Balance = BalanceOf<Self>>;
}

decl_storage! {
    trait Store for Module<T: Config> as DBCTesting {
        pub Thing1 get(fn thing1): u64;
        pub Thing2 get(fn thing2): u64;
        pub Thing3 get(fn thing3): u64;
    }
}

decl_module! {
    pub struct Module<T: Config> for enum Call where origin: T::Origin {

        #[weight = 0]
        pub fn say_hello(origin) -> DispatchResult{
            let secs_per_block = babe::Module::<T>::slot_duration();
            let secs_per_block2 = <babe::Module::<T>>::slot_duration();

            // let secs_per_block =<T as babe::Config>::slot_duration();
            // let secs_per_block = <T as timestamp::Config>::MinimumPeriod::get();
            let caller = ensure_signed(origin)?;

            debug::info!("######### Request sent by: {:?}, {:?}, {:?} #########", caller, secs_per_block, secs_per_block2);
            Ok(())
        }

        #[weight = 0]
        pub fn set_phase0_reward(origin, reward_balance: BalanceOf<T>) -> DispatchResult {
            ensure_root(origin)?;
            T::PhaseReward::set_phase0_reward(reward_balance);
            Ok(())
        }

        #[weight = 0]
        pub fn set_phase1_reward(origin, reward_balance :BalanceOf<T>) -> DispatchResult{
            ensure_root(origin)?;
            T::PhaseReward::set_phase1_reward(reward_balance);
            Ok(())
        }

        #[weight = 0]
         pub fn set_phase2_reward(origin,reward_balance:BalanceOf<T>) -> DispatchResult{
            ensure_root(origin)?;
            T::PhaseReward::set_phase2_reward(reward_balance);
            Ok(())
        }
    }
}
