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

use phase_reward::PhaseReward;

// pub trait Config: frame_system::Config + timestamp::Config {}
pub trait Config: frame_system::Config + babe::Config {
    type PhaseReward: PhaseReward;
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

        // #[weight = 0]
        // pub fn set_phase0_reward() -> DispatchResult {
        //     let out = T::PhaseReward::set_phase0_reward();
        //     <Things1>::put(out);
        //     Ok(())
        // }
    }
}
