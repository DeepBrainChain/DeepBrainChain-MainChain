#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]
#![warn(unused_crate_dependencies)]

// use alt_serde::{Deserialize, Deserializer};
use dbc_support::traits::DbcPrice;
use frame_support::traits::{Currency, Randomness, ReservableCurrency};
use frame_system::offchain::SubmitTransaction;
use sp_core::H256;
use sp_runtime::{
    offchain::{http, Duration},
    traits::{CheckedDiv, CheckedMul, SaturatedConversion},
};
use sp_std::{collections::vec_deque::VecDeque, str, vec::Vec};

pub use pallet::*;
pub mod parse_price;

type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{dispatch::DispatchResultWithPostInfo, pallet_prelude::*};
    use frame_system::{offchain::CreateSignedTransaction, pallet_prelude::*};
    use sp_std::vec::Vec;

    /// The type to sign and send transactions.
    pub const UNSIGNED_TXS_PRIORITY: u64 = 100;
    pub const MAX_LEN: usize = 64;
    type URL = Vec<u8>;

    #[pallet::config]
    pub trait Config:
        frame_system::Config + CreateSignedTransaction<Call<Self>> + generic_func::Config
    {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type RandomnessSource: Randomness<H256, Self::BlockNumber>;
        type Currency: ReservableCurrency<Self::AccountId>;
    }

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::type_value]
    pub(super) fn PriceUpdateFrequencyDefault<T: Config>() -> u32 {
        10
    }

    #[pallet::storage]
    #[pallet::getter(fn price_update_frequency)]
    pub(super) type PriceUpdateFrequency<T: Config> =
        StorageValue<_, u32, ValueQuery, PriceUpdateFrequencyDefault<T>>;

    #[pallet::storage]
    #[pallet::getter(fn prices)]
    pub type Prices<T> = StorageValue<_, VecDeque<u64>, ValueQuery>;

    // https://min-api.cryptocompare.com/data/price?fsym=DBC&tsyms=USD
    #[pallet::storage]
    #[pallet::getter(fn price_url)]
    pub(super) type PriceURL<T> = StorageValue<_, Vec<URL>>;

    /// avgPrice = price * 10**6 usd
    #[pallet::storage]
    #[pallet::getter(fn avg_price)]
    pub(super) type AvgPrice<T> = StorageValue<_, u64>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        AddNewPrice(u64),
        AddAvgPrice(u64),
    }

    #[pallet::error]
    pub enum Error<T> {
        NoLocalAcctForSigning,
        FetchPriceFailed,
        OffchainUnsignedTxSignedPayloadError,
        OffchainUnsignedTxError,
        NoneValue,
        IndexOutOfRange,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn offchain_worker(block_number: T::BlockNumber) {
            if Self::price_url().is_none() {
                return;
            };
            let price_update_frequency = Self::price_update_frequency();
            if price_update_frequency == 0 {
                return;
            }

            if block_number % price_update_frequency.into() == 0u32.into() {
                let _ = Self::fetch_price_and_send_unsigned_tx();
            }
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn submit_price_unsigned(
            origin: OriginFor<T>,
            price: u64,
        ) -> DispatchResultWithPostInfo {
            ensure_none(origin)?;
            Self::add_price(price);
            Self::add_avg_price();
            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn submit_price_by_root(
            origin: OriginFor<T>,
            price: u64,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            Self::add_price(price);
            Self::add_avg_price();
            Ok(().into())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn add_price_url(origin: OriginFor<T>, new_url: URL) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let mut price_url = Self::price_url().unwrap_or_default();
            price_url.push(new_url);
            PriceURL::<T>::put(price_url);
            Ok(().into())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn set_price_update_frequency(
            origin: OriginFor<T>,
            frequency: u32,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            if frequency == 0 {
                return Ok(().into());
            }
            PriceUpdateFrequency::<T>::put(frequency);
            Ok(().into())
        }

        #[pallet::call_index(4)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn rm_price_url_by_index(
            origin: OriginFor<T>,
            index: u32,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let mut price_url = Self::price_url().unwrap_or_default();
            ensure!(index < price_url.len() as u32, Error::<T>::IndexOutOfRange);
            price_url.remove(index as usize);
            if price_url.is_empty() {
                PriceURL::<T>::kill();
            } else {
                PriceURL::<T>::put(price_url);
            }
            Ok(().into())
        }
    }

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;

        fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            let valid_tx = |provide| {
                ValidTransaction::with_tag_prefix("dbc-price")
                    .priority(UNSIGNED_TXS_PRIORITY)
                    .and_provides([&provide])
                    .longevity(3)
                    .propagate(true)
                    .build()
            };

            match call {
                Call::submit_price_unsigned { price: _ } => {
                    valid_tx(b"submit_price_unsigned".to_vec())
                },
                _ => InvalidTransaction::Call.into(),
            }
        }
    }
}

impl<T: Config> Pallet<T> {
    fn gen_rand_url() -> Option<u32> {
        let price_url = Self::price_url()?;
        Some(<generic_func::Pallet<T>>::random_u32(price_url.len() as u32))
    }

    fn fetch_price_and_send_unsigned_tx() -> Result<(), Error<T>> {
        let price = Self::fetch_price().map_err(|_| <Error<T>>::FetchPriceFailed)?;

        let call = Call::submit_price_unsigned { price };
        SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into())
            .map_err(|_| <Error<T>>::OffchainUnsignedTxError)
    }

    // 获取并返回当前价格
    fn fetch_price() -> Result<u64, http::Error> {
        let timeout = sp_io::offchain::timestamp().add(Duration::from_millis(4_000));

        let price_url = Self::price_url().ok_or(http::Error::Unknown)?;

        let rand_price_url_index = Self::gen_rand_url().ok_or(http::Error::Unknown)?;

        let price_url = str::from_utf8(&price_url[rand_price_url_index as usize])
            .map_err(|_| http::Error::Unknown)?;

        let request = http::Request::get(price_url);

        let pending = request.deadline(timeout).send().map_err(|_| http::Error::IoError)?;

        let response = pending.try_wait(timeout).map_err(|_| http::Error::DeadlineReached)??;
        // Let's check the status code before we proceed to reading the response.
        if response.code != 200 {
            return Err(http::Error::Unknown);
        }
        let body = response.body().collect::<Vec<u8>>();

        // Create a str slice from the body.
        let body_str = sp_std::str::from_utf8(&body).map_err(|_| http::Error::Unknown)?;

        parse_price::parse_price(body_str).ok_or(http::Error::Unknown)
    }

    // 存储获取到的价格
    pub fn add_price(price: u64) {
        let mut prices = Prices::<T>::get();
        if prices.len() >= MAX_LEN {
            prices.pop_front();
        }
        prices.push_back(price);

        Prices::<T>::put(prices);
        Self::deposit_event(Event::AddNewPrice(price));
    }

    pub fn add_avg_price() {
        let prices = Prices::<T>::get();
        if prices.len() != MAX_LEN {
            return;
        }
        let avg_price = prices
            .iter()
            .fold(0_u64, |a, b| a.saturating_add(*b))
            .saturating_div(prices.len() as u64);

        AvgPrice::<T>::put(avg_price);
        Self::deposit_event(Event::AddAvgPrice(avg_price));
    }
}

impl<T: Config> DbcPrice for Pallet<T> {
    type Balance = BalanceOf<T>;

    fn get_dbc_price() -> Option<Self::Balance> {
        Some(Self::avg_price()?.saturated_into::<Self::Balance>())
    }

    // avgPrice = price * 10**6 usd
    // usd = avgPrice * dbc_num => dbc_amount = usd * 10**6 / avgPrice
    fn get_dbc_amount_by_value(value: u64) -> Option<Self::Balance> {
        let one_dbc: Self::Balance = 1_000_000_000_000_000_u64.saturated_into();
        let dbc_price: Self::Balance = Self::avg_price()?.saturated_into();
        value
            .saturated_into::<Self::Balance>()
            .checked_mul(&one_dbc)?
            .checked_div(&dbc_price)
    }
}
