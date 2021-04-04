#![cfg_attr(not(feature = "std"), no_std)]

use codec::Encode;
use frame_support::{debug, traits::Randomness};
use frame_system::offchain::SubmitTransaction;
use lite_json::json::JsonValue;
use sp_core::H256;
use sp_runtime::{
    offchain::{http, Duration},
    traits::{BlakeTwo256, SaturatedConversion},
    RandomNumberGenerator,
};
use sp_std::str;
use sp_std::vec::Vec;

pub use pallet::*;

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
    pub trait Config: frame_system::Config + CreateSignedTransaction<Call<Self>> {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type RandomnessSource: Randomness<H256>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    #[pallet::getter(fn prices)]
    pub type Prices<T> = StorageValue<_, Vec<u64>, ValueQuery>;

    // #[pallet::storage]
    // #[pallet::getter(fn next_unsigned_at)]
    // pub type NextUnsignedAt<T: Config> = StorageValue<_, T::BlockNumber>;

    #[pallet::type_value]
    pub fn MyPriceURL() -> Vec<URL> {
        let mut init_url: Vec<URL> = Vec::new();
        init_url.push("https://min-api.cryptocompare.com/data/price?fsym=DBC&tsyms=USD".into());
        // vec!["https://min-api.cryptocompare.com/data/price?fsym=DBC&tsyms=USD".into()]
        init_url
    }

    #[pallet::storage]
    pub(super) type PriceURL<T> = StorageValue<_, Vec<URL>, ValueQuery, MyPriceURL>;

    #[pallet::type_value]
    pub(super) fn RandNonceDefault<T: Config>() -> u64 {
        0
    }

    #[pallet::storage]
    pub(super) type RandNonce<T: Config> = StorageValue<_, u64, ValueQuery, RandNonceDefault<T>>;

    #[pallet::event]
    #[pallet::metadata(T::AccountId = "AccountId")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        NewPrice(u64, Option<T::AccountId>),
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
            debug::native::info!(
                "Hello world from offchain worker at height: {:#?}!",
                block_number
            );

            let average: Option<u64> = Self::average_price();
            debug::debug!("Current price: {:?}", average);

            let result = Self::fetch_price_and_send_unsigned_tx();
            if let Err(e) = result {
                debug::error!("offchain_worker error: {:?}", e);
            }
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(0)]
        pub fn submit_price_unsigned(
            origin: OriginFor<T>,
            price: u64,
        ) -> DispatchResultWithPostInfo {
            ensure_none(origin)?;
            Self::add_price(None, price);
            Ok(().into())
        }

        // TODO: 改变成add price url 和 rm price url
        #[pallet::weight(0)]
        pub fn add_price_url(origin: OriginFor<T>, new_url: URL) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            let mut price_url = PriceURL::<T>::get();
            price_url.push(new_url);
            PriceURL::<T>::put(price_url);

            Ok(().into())
        }

        #[pallet::weight(0)]
        pub fn rm_price_url_by_index(
            origin: OriginFor<T>,
            index: u32,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            let mut price_url = PriceURL::<T>::get();
            ensure!(index > price_url.len() as u32, Error::<T>::IndexOutOfRange);

            price_url.remove(index as usize);
            PriceURL::<T>::put(price_url);

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
                Call::submit_price_unsigned(_price) => valid_tx(b"submit_price_unsigned".to_vec()),
                _ => InvalidTransaction::Call.into(),
            }
        }
    }
}

impl<T: Config> Module<T> {
    // TODO: 重构该函数与online-profile同名的函数
    // 增加随机性
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

    // 生成一个随机的u32
    fn random_num(max: u32) -> u32 {
        let subject = Self::update_nonce();
        let random_seed = T::RandomnessSource::random(&subject);
        let mut rng = <RandomNumberGenerator<BlakeTwo256>>::new(random_seed);
        rng.pick_u32(max)
    }

    fn gen_rand_url() -> u32 {
        let price_url = PriceURL::<T>::get();
        Self::random_num((price_url.len() - 1) as u32)
    }

    fn fetch_price_and_send_unsigned_tx() -> Result<(), Error<T>> {
        let price = Self::fetch_price().map_err(|_| <Error<T>>::FetchPriceFailed)?;

        let call = Call::submit_price_unsigned(price);
        SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into()).map_err(|_| {
            debug::error!("Failed in offchain_unsigned_tx"); // TODO: error here
            <Error<T>>::OffchainUnsignedTxError
        })
    }

    fn fetch_price() -> Result<u64, http::Error> {
        let deadline = sp_io::offchain::timestamp().add(Duration::from_millis(4_000));

        let price_url = PriceURL::<T>::get();
        let rand_price_url_index = Self::gen_rand_url();
        let price_url = str::from_utf8(&price_url[rand_price_url_index as usize])
            .map_err(|_| http::Error::Unknown)?;

        let request = http::Request::get(price_url);

        let pending = request
            .deadline(deadline)
            .send()
            .map_err(|_| http::Error::IoError)?;

        let response = pending
            .try_wait(deadline)
            .map_err(|_| http::Error::DeadlineReached)??;
        // Let's check the status code before we proceed to reading the response.
        if response.code != 200 {
            debug::warn!("Unexpected status code: {}", response.code);
            return Err(http::Error::Unknown);
        }
        let body = response.body().collect::<Vec<u8>>();

        // Create a str slice from the body.
        let body_str = sp_std::str::from_utf8(&body).map_err(|_| {
            debug::warn!("No UTF8 body");
            http::Error::Unknown
        })?;

        let price = match Self::parse_price(body_str) {
            Some(price) => Ok(price),
            None => {
                debug::warn!("Unable to extract price from the response: {:?}", body_str);
                Err(http::Error::Unknown)
            }
        }?;

        debug::warn!("Got price: {} cents", price);

        Ok(price)
    }

    fn parse_price(price_str: &str) -> Option<u64> {
        let val = lite_json::parse_json(price_str);
        let price = val.ok().and_then(|v| match v {
            JsonValue::Object(obj) => {
                let mut chars = "USD".chars();
                obj.into_iter()
                    .find(|(k, _)| k.iter().all(|k| Some(*k) == chars.next()))
                    .and_then(|v| match v.1 {
                        JsonValue::Number(number) => Some(number),
                        _ => None,
                    })
            }
            _ => None,
        })?;

        // out = price.integer * 10**6 + price.fraction / 10**fraction_length * 10**6
        let fraction = price.fraction * 10_u64.pow(6) / 10_u64.pow(price.fraction_length);
        Some(price.integer as u64 * 1000_000 + fraction)
    }

    fn add_price(who: Option<T::AccountId>, price: u64) {
        debug::info!("Adding to the average: {}", price);
        let mut prices = Prices::<T>::get();
        if prices.len() < MAX_LEN {
            prices.push(price);
        } else {
            prices[price as usize % MAX_LEN] = price;
        }

        Prices::<T>::put(prices);

        let average = Self::average_price()
            .expect("The average is not empty, because it was just mutated; qed");

        debug::info!("Current average price is: {}", average);

        Self::deposit_event(Event::NewPrice(price, who));
    }

    fn average_price() -> Option<u64> {
        let prices = Prices::<T>::get();
        Some(prices.iter().fold(0_u64, |a, b| a.saturating_add(*b)) / prices.len() as u64)
    }
}
