#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]
#![warn(unused_crate_dependencies)]

use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
use sp_core::H160;
use sp_std::prelude::*;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type WhitelistLimit: Get<u32>;
    }

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    #[pallet::getter(fn destroy_hook)]
    pub type PrecompileWhitelist<T: Config> =
        StorageMap<_, Blake2_128Concat, H160, BoundedVec<H160, T::WhitelistLimit>, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub fn deposit_event)]
    pub enum Event<T: Config> {
        PrecompileWhitelistSet(H160, Vec<H160>),
    }

    #[pallet::error]
    pub enum Error<T> {
        WhitelistExceedsLimit,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn set_precompile_whitelist(
            origin: OriginFor<T>,
            precompile: H160,
            whitelist: Vec<H160>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            let bounded_whitelist: BoundedVec<H160, T::WhitelistLimit> =
                whitelist.clone().try_into().map_err(|_| Error::<T>::WhitelistExceedsLimit)?;

            PrecompileWhitelist::<T>::insert(precompile, bounded_whitelist);
            Self::deposit_event(Event::PrecompileWhitelistSet(precompile, whitelist));
            Ok(().into())
        }
    }
}
