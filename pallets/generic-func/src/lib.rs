#![cfg_attr(not(feature = "std"), no_std)]

pub mod rpc_types;
mod traits;

use frame_support::{
    pallet_prelude::*,
    traits::{Currency, OnUnbalanced, Randomness, ReservableCurrency},
};
use frame_system::pallet_prelude::*;
use sp_core::H256;
use sp_runtime::{
    traits::{BlakeTwo256, Saturating},
    RandomNumberGenerator,
};
use sp_std::{convert::TryInto, prelude::*};

pub use pallet::*;
pub use rpc_types::*;

pub struct ItemList;
impl ItemList {
    pub fn add_item<T>(a_field: &mut Vec<T>, a_item: T)
    where
        T: Ord,
    {
        if let Err(index) = a_field.binary_search(&a_item) {
            a_field.insert(index, a_item);
        }
    }

    pub fn rm_item<T>(a_field: &mut Vec<T>, a_item: &T)
    where
        T: Ord,
    {
        if let Ok(index) = a_field.binary_search(a_item) {
            a_field.remove(index);
        }
    }

    pub fn expand_to_order<T>(raw_items: &mut Vec<T>, new_items: Vec<T>)
    where
        T: Ord,
    {
        for a_item in new_items {
            Self::add_item(raw_items, a_item);
        }
    }
}

pub type SlashId = u64;
pub type MachineId = Vec<u8>;
pub type EraIndex = u32;

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
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type BlockPerEra: Get<u32>;
        type RandomnessSource: Randomness<H256>;
        type FixedTxFee: OnUnbalanced<NegativeImbalanceOf<Self>>;
        type Slash: OnUnbalanced<NegativeImbalanceOf<Self>>;
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
            let frequency = Self::destroy_hook();

            match frequency {
                Some(frequency) => {
                    if frequency.1 == 0u32.into() {
                        return 0
                    }
                    if block_number % frequency.1 == 0u32.into() {
                        Self::auto_destroy(frequency.0);
                    }
                },
                None => return 0,
            }
            0
        }

        fn on_runtime_upgrade() -> Weight {
            let rent_fee_pot: Vec<u8> =
                b"5GR31fgcHdrJ14eFW1xJmHhZJ56eQS7KynLKeXmDtERZTiw2".to_vec();

            let account_id32: [u8; 32] = Self::get_accountid32(&rent_fee_pot).unwrap_or_default();
            let account = T::AccountId::decode(&mut &account_id32[..]).ok().unwrap_or_default();

            let destroy_frequency: T::BlockNumber = (2880 * 7u32).into();
            DestroyHook::<T>::put((account, destroy_frequency));

            0
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // 设置租用机器手续费：10 DBC
        #[pallet::weight(0)]
        pub fn set_fixed_tx_fee(
            origin: OriginFor<T>,
            tx_fee: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            FixedTxFee::<T>::put(tx_fee);
            Ok(().into())
        }

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
        #[pallet::weight(0)]
        pub fn destroy_free_dbc(
            origin: OriginFor<T>,
            amount: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::do_destroy_dbc(who, amount);
            Ok(().into())
        }

        // 强制销毁DBC
        #[pallet::weight(0)]
        pub fn force_destroy_free_dbc(
            origin: OriginFor<T>,
            who: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            Self::do_destroy_dbc(who, amount);
            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        DonateToTreasury(T::AccountId, BalanceOf<T>),
        TxFeeToTreasury(T::AccountId, BalanceOf<T>),
        DestroyDBC(T::AccountId, BalanceOf<T>),
    }

    #[pallet::error]
    pub enum Error<T> {
        FreeBalanceNotEnough,
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

    // Generate random num, range: [0, max]
    pub fn random_u32(max: u32) -> u32 {
        let subject = Self::update_nonce();
        let random_seed = T::RandomnessSource::random(&subject);
        let mut rng = <RandomNumberGenerator<BlakeTwo256>>::new(random_seed);
        rng.pick_u32(max)
    }

    /// 产生随机的ServerRoomId
    pub fn random_server_room() -> H256 {
        let subject = Self::update_nonce();
        T::RandomnessSource::random(&subject)
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
        Self::do_destroy_dbc(who, free_balance);
    }

    pub fn do_destroy_dbc(who: T::AccountId, burn_amount: BalanceOf<T>) {
        let free_balance = T::Currency::free_balance(&who);
        let burn_amount = if free_balance >= burn_amount { burn_amount } else { free_balance };

        T::Currency::make_free_balance_be(&who, free_balance - burn_amount);
        // ensure T::CurrencyToVote will work correctly.
        T::Currency::burn(burn_amount);

        // 记录总burn数量
        let mut total_destroy = Self::total_destroy(&who);
        total_destroy = total_destroy.saturating_add(burn_amount);
        TotalDestroy::<T>::insert(&who, total_destroy);
        Self::deposit_event(Event::DestroyDBC(who, burn_amount));
    }

    pub fn get_accountid32(addr: &[u8]) -> Option<[u8; 32]> {
        let mut data: [u8; 35] = [0; 35];

        let length = bs58::decode(addr).into(&mut data).ok()?;
        if length != 35 {
            return None
        }

        let (_prefix_len, _ident) = match data[0] {
            0..=63 => (1, data[0] as u16),
            _ => return None,
        };

        let account_id32: [u8; 32] = data[1..33].try_into().ok()?;
        Some(account_id32)
    }
}
