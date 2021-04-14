#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
    pallet_prelude::*,
    traits::{Get, Randomness},
};
use frame_system::pallet_prelude::*;
use sp_core::H256;
use sp_runtime::traits::SaturatedConversion;
use sp_runtime::{traits::BlakeTwo256, RandomNumberGenerator};
use sp_std::prelude::*;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type BlockPerEra: Get<u32>;
        type RandomnessSource: Randomness<H256>;
    }
    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    // nonce to generate random number for selecting committee
    #[pallet::type_value]
    pub(super) fn RandNonceDefault<T: Config>() -> u64 {
        0
    }

    #[pallet::storage]
    #[pallet::getter(fn rand_nonce)]
    pub(super) type RandNonce<T: Config> = StorageValue<_, u64, ValueQuery, RandNonceDefault<T>>;

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {}
}

impl<T: Config> Pallet<T> {
    // Add randomness
    fn update_nonce() -> Vec<u8> {
        let nonce = RandNonce::<T>::get();
        let nonce: u64 = if nonce == u64::MAX {
            0
        } else {
            RandNonce::<T>::get() + 1
        };
        RandNonce::<T>::put(nonce);
        nonce.encode()
    }

    // Generate random num, range: [0, max]
    pub fn random_u32(max: u32) -> u32 {
        let subject = Self::update_nonce();
        let random_seed = T::RandomnessSource::random(&subject);
        let mut rng = <RandomNumberGenerator<BlakeTwo256>>::new(random_seed);
        rng.pick_u32(max)
    }

    // get current era
    pub fn current_era() -> u32 {
        let current_block_height =
            <frame_system::Module<T>>::block_number().saturated_into::<u32>();
        return current_block_height / T::BlockPerEra::get();
    }

    pub fn block_per_era() -> u32 {
        T::BlockPerEra::get()
    }
}
