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

// pub trait Config: frame_system::Config + timestamp::Config {}
pub trait Config: frame_system::Config + babe::Config {}

decl_module! {
    pub struct Module<T: Config> for enum Call where origin: T::Origin {
        // pub

        #[weight = 0]
        pub fn say_hello(origin) -> DispatchResult{
            let secs_per_block = babe::Module::<T>::slot_duration();

            let secs_per_block2 = <babe::Module::<T>>::slot_duration();

            // let secs_per_block =<T as babe::Config>::slot_duration();

            // let secs_per_block = <T as timestamp::Config>::MinimumPeriod::get();
            let caller = ensure_signed(origin)?;

            debug::info!("###################### Request sent by: {:?},,, {:?},,, {:?}", caller, secs_per_block, secs_per_block2);
            Ok(())
        }
    }
}
