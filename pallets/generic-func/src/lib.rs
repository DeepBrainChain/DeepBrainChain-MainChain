#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]
#![warn(unused_crate_dependencies)]

mod traits;

pub use dbc_support::ItemList;
use frame_support::{
    pallet_prelude::*,
    traits::{Currency, OnUnbalanced, Randomness, ReservableCurrency},
    weights::Weight,
};
use frame_system::pallet_prelude::*;
use rand_chacha::{
    rand_core::{RngCore, SeedableRng},
    ChaChaRng,
};
use sp_core::H256;
use sp_runtime::traits::Saturating;
use sp_std::prelude::*;

pub use pallet::*;

type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type NegativeImbalanceOf<T> = <<T as Config>::Currency as Currency<
    <T as frame_system::Config>::AccountId,
>>::NegativeImbalance;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Currency: ReservableCurrency<Self::AccountId>;
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type BlockPerEra: Get<u32>;
        type RandomnessSource: Randomness<H256, Self::BlockNumber>;
        type FixedTxFee: OnUnbalanced<NegativeImbalanceOf<Self>>;
        type Slash: OnUnbalanced<NegativeImbalanceOf<Self>>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    // nonce to generate random number for selecting committee
    #[pallet::type_value]
    pub(super) fn RandNonceDefault<T: Config>() -> u64 {
        0
    }

    #[pallet::storage]
    #[pallet::getter(fn rand_nonce)]
    pub(super) type RandNonce<T: Config> = StorageValue<_, u64, ValueQuery, RandNonceDefault<T>>;

    // 控制全局交易费用
    #[pallet::storage]
    #[pallet::getter(fn fixed_tx_fee)]
    pub type FixedTxFee<T: Config> = StorageValue<_, BalanceOf<T>>;

    #[pallet::storage]
    #[pallet::getter(fn destroy_hook)]
    pub(super) type DestroyHook<T: Config> = StorageValue<_, (T::AccountId, T::BlockNumber)>;

    #[pallet::storage]
    #[pallet::getter(fn total_destroy)]
    pub(super) type TotalDestroy<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(block_number: T::BlockNumber) -> Weight {
            let weight = Weight::default();

            let frequency = Self::destroy_hook();

            match frequency {
                Some(frequency) => {
                    if frequency.1 == 0u32.into() {
                        return weight
                    }
                    if block_number % frequency.1 == 0u32.into() {
                        Self::auto_destroy(frequency.0);
                    }
                },
                None => return weight,
            }
            weight
        }

        // // NOTE: only be used when upgrade from runtime 273.
        // fn on_runtime_upgrade() -> frame_support::weights::Weight {
        //     frame_support::log::info!("🔍 GenericFunc Storage Migration start");

        //     let account: Vec<u8> = b"5GR31fgcHdrJ14eFW1xJmHhZJ56eQS7KynLKeXmDtERZTiw2".to_vec();
        //     let account_id32: [u8; 32] =
        //         dbc_support::utils::get_accountid32(&account).unwrap_or_default();
        //     if let Some(account) = T::AccountId::decode(&mut &account_id32[..]).ok() {
        //         TotalDestroy::<T>::mutate(&account, |total_destroy| {
        //             *total_destroy = 13247230286575760612585u128.saturated_into();
        //         });
        //     }

        //     frame_support::log::info!("🚀 GenericFunc Storage Migration end");
        //     Weight::zero()
        // }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // 设置租用机器手续费：10 DBC
        #[pallet::call_index(0)]
        #[pallet::weight(0)]
        pub fn set_fixed_tx_fee(
            origin: OriginFor<T>,
            tx_fee: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            FixedTxFee::<T>::put(tx_fee);
            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(0)]
        pub fn deposit_into_treasury(
            origin: OriginFor<T>,
            amount: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            ensure!(
                <T as Config>::Currency::can_slash(&who, amount),
                Error::<T>::FreeBalanceNotEnough
            );

            let (imbalance, _) = <T as Config>::Currency::slash(&who, amount);
            T::FixedTxFee::on_unbalanced(imbalance);
            Self::deposit_event(Event::DonateToTreasury(who, amount));
            Ok(().into())
        }

        /// fre == 0 将销毁DestroyHook
        #[pallet::call_index(2)]
        #[pallet::weight(0)]
        pub fn set_auto_destroy(
            origin: OriginFor<T>,
            who: T::AccountId,
            frequency: T::BlockNumber,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            if frequency == 0u32.into() {
                DestroyHook::<T>::kill();
            } else {
                DestroyHook::<T>::put((who, frequency));
            }
            Ok(().into())
        }

        // 将DBC销毁
        #[pallet::call_index(3)]
        #[pallet::weight(0)]
        pub fn destroy_free_dbc(
            origin: OriginFor<T>,
            amount: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::do_destroy_dbc(who, amount)
        }

        // 强制销毁DBC
        #[pallet::call_index(4)]
        #[pallet::weight(0)]
        pub fn force_destroy_free_dbc(
            origin: OriginFor<T>,
            who: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            Self::do_destroy_dbc(who, amount)
        }
    }

    // #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        DonateToTreasury(T::AccountId, BalanceOf<T>),
        TxFeeToTreasury(T::AccountId, BalanceOf<T>),
        DestroyDBC(T::AccountId, BalanceOf<T>),
    }

    #[pallet::error]
    pub enum Error<T> {
        FreeBalanceNotEnough,
        UnknownFixedTxFee,
    }
}

impl<T: Config> Pallet<T> {
    // Add randomness
    fn update_nonce() -> Vec<u8> {
        let nonce = RandNonce::<T>::get();
        let nonce: u64 = if nonce == u64::MAX { 0 } else { RandNonce::<T>::get() + 1 };
        RandNonce::<T>::put(nonce);
        nonce.encode()
    }

    /// Pick a new PRN, in the range [0, `max`) (exclusive).
    fn pick_u32<R: RngCore>(rng: &mut R, max: u32) -> u32 {
        rng.next_u32() % max
    }

    // Generate random num, range: [0, `max`)(exclusive)
    pub fn random_u32(max: u32) -> u32 {
        let subject = Self::update_nonce();
        let (random_seed, _) = T::RandomnessSource::random(&subject);
        // let random_seed = sp_io::offchain::random_seed();
        let mut rng = ChaChaRng::from_seed(random_seed.into());
        Self::pick_u32(&mut rng, max)
    }

    /// 产生随机的ServerRoomId
    pub fn random_server_room() -> H256 {
        let subject = Self::update_nonce();
        T::RandomnessSource::random(&subject).0
    }

    // 每次交易消耗一些交易费: 10DBC
    // 交易费直接转给国库
    pub fn pay_fixed_tx_fee(who: T::AccountId) -> Result<(), ()> {
        let fixed_tx_fee = Self::fixed_tx_fee().ok_or(())?;
        ensure!(<T as Config>::Currency::can_slash(&who, fixed_tx_fee), ());

        let (imbalance, _) = <T as Config>::Currency::slash(&who, fixed_tx_fee);
        T::FixedTxFee::on_unbalanced(imbalance);

        Self::deposit_event(Event::TxFeeToTreasury(who, fixed_tx_fee));
        Ok(())
    }

    pub fn auto_destroy(who: T::AccountId) {
        let free_balance = T::Currency::free_balance(&who);
        let _ = Self::do_destroy_dbc(who, free_balance);
    }

    pub fn do_destroy_dbc(
        who: T::AccountId,
        burn_amount: BalanceOf<T>,
    ) -> DispatchResultWithPostInfo {
        let free_balance = T::Currency::free_balance(&who);
        let fixed_tx_fee = Self::fixed_tx_fee().ok_or(Error::<T>::UnknownFixedTxFee)?;
        if free_balance <= fixed_tx_fee {
            return Ok(().into())
        }

        let max_burn_amount = free_balance.saturating_sub(fixed_tx_fee);
        let burn_amount = if max_burn_amount > burn_amount { burn_amount } else { max_burn_amount };

        T::Currency::make_free_balance_be(&who, free_balance.saturating_sub(burn_amount));
        // ensure T::CurrencyToVote will work correctly.
        T::Currency::burn(burn_amount);

        // 记录总burn数量
        TotalDestroy::<T>::mutate(&who, |total_destroy| {
            *total_destroy = total_destroy.saturating_add(burn_amount);
        });
        Self::deposit_event(Event::DestroyDBC(who, burn_amount));
        Ok(().into())
    }
}
