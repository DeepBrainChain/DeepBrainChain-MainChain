#![recursion_limit = "128"]
#![cfg_attr(not(feature = "std"), no_std)]

use codec::Decode;
use frame_support::debug;
use frame_support::traits::{Currency, ExistenceRequirement::KeepAlive, Imbalance, OnUnbalanced};
use frame_system::{self as system, ensure_root, ensure_signed};
use phase_reward::PhaseReward;
use sp_arithmetic::{traits::Saturating, Permill};
//use sp_core::Public;
use sp_core::crypto::Public;
use sp_io::hashing::blake2_128;
use sp_runtime::traits::{Verify, Zero};
use sp_std::{
    convert::{TryFrom, TryInto},
    prelude::*,
    str,
};

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

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config:
        frame_system::Config + pallet_timestamp::Config + pallet_treasury::Config
    {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
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

        /// Slashes the specified amount of funds from the specified account
        #[pallet::weight(0)]
        pub fn slash_funds(
            origin: OriginFor<T>,
            _to_punish: T::AccountId,
            collateral: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            let _ = ensure_signed(origin)?;

            T::Slash::on_unbalanced(<T as pallet::Config>::Currency::issue(collateral));

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

            let r = <T as pallet::Config>::Currency::deposit_into_existing(&to_reward, reward).ok();
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
            debug::info!("Dcoded2 Alice: {:?}, {:?}", decoded, output);

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
            debug::info!("###### blake2_128 Hash of Hello world! is: {:?}", encode_data);
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

        #[pallet::weight(0)]
        fn test_slash(
            origin: OriginFor<T>,
            value: BalanceOf<T>,
            slash_who: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            Self::do_slash(value, slash_who);
            Ok(().into())
        }

        #[pallet::weight(0)]
        fn verify_sig(
            origin: OriginFor<T>,
            msg: Vec<u8>,
            sig: Vec<u8>,
            account: Vec<u8>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            debug::error!(
                "#### msg: {:?}, sig: {:?}, sign_by: {:?}",
                msg.clone(),
                sig.clone(),
                account.clone()
            );

            let out = match sp_core::sr25519::Signature::try_from(&sig[..]) {
                Ok(signature) => {
                    // 获取帐号的方法
                    let public = sp_core::sr25519::Public::from_slice(account.as_ref());
                    signature.verify(&msg[..], &public)
                }
                _ => false,
            };
            debug::error!("##### verify result: {}", out);
            Ok(().into())
        }

        #[pallet::weight(0)]
        fn send_to_treasury(
            origin: OriginFor<T>,
            from: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            let treasury_account = <pallet_treasury::Module<T>>::account_id();
            <T as pallet::Config>::Currency::transfer(&from, &treasury_account, amount, KeepAlive)
                .map_err(|_| DispatchError::Other("Can't make tx payment"))?;

            Ok(().into())
        }

        #[pallet::weight(0)]
        fn decode_hex_pubkey(
            origin: OriginFor<T>,
            hex_account: Vec<u8>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            if hex_account.len() != 122 {
                debug::error!("Length not equal 122: {:?}", hex_account);
            }

            let pubkey_u8 = hex_account[..64].to_vec();
            debug::error!("Pubkey_u8 is: {:?}", pubkey_u8);

            // pubkey_u8 to str
            let pubkey_str = str::from_utf8(&pubkey_u8).unwrap();
            debug::error!("Pubkey_str is: {:?}", pubkey_str);

            // pubkey_str to hex array
            let t: Result<Vec<u8>, _> = (0..pubkey_str.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&pubkey_str[i..i + 2], 16))
                .collect();
            if let Ok(t) = t {
                debug::error!("hex pubkey is: {:?}", t);
            }
            Ok(().into())
        }
    }

    #[pallet::error]
    pub enum Error<T> {
        TestError,
    }

    #[pallet::event]
    #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        Slash(T::AccountId, BalanceOf<T>),
    }
}

impl<T: Config> Pallet<T> {
    // TODO: why cannot run here?
    fn _test() {
        let mut output: [u8; 35] = [0; 35];
        let decoded =
            bs58::decode("5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY").into(&mut output);

        let account_id_32: [u8; 32] = output[1..33].try_into().unwrap();
        debug::info!("##### decoded2 Alice: {:?}, {:?}", decoded, output);

        let _b = T::AccountId::decode(&mut &account_id_32[..]).unwrap_or_default();
    }

    fn do_slash(value: BalanceOf<T>, who: T::AccountId) {
        // let mut slashed_imbalance = NegativeImbalanceOf::<T>::zero();
        if !value.is_zero() {
            if <T as pallet::Config>::Currency::can_slash(&who, value) {
                let (imbalance, missing) = <T as pallet::Config>::Currency::slash(&who, value);
                Self::deposit_event(Event::Slash(who, missing.clone()));
                // slashed_imbalance.subsume(imbalance);
                T::Slash::on_unbalanced(imbalance);
            }
        }
    }
}
