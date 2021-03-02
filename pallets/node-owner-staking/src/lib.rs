#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::traits::Currency;
use frame_support::{
    debug, decl_error, decl_event, decl_module, decl_storage, dispatch::DispatchResult,
};
use frame_system::{self as system, ensure_signed};
use sp_runtime::{
    offchain,
    offchain::{
        storage::StorageValueRef,
        storage_lock::{BlockAndTime, StorageLock},
    },
};
use sp_std::{prelude::*, str};

mod machine_info;

type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as system::Config>::AccountId>>::Balance;

pub const HTTP_REMOTE_REQUEST: &str =
    "http://116.85.24.172:41107/api/v1/mining_nodes/2gfpp3MAB4Aq2ZPEU72neZTVcZkbzDzX96op9d3fvi3";
pub const HTTP_HEADER_USER_AGENT: &str = "jimmychu0807"; // TODO: remove this

pub const FETCH_TIMEOUT_PERIOD: u64 = 3000; // in milli-seconds
pub const LOCK_TIMEOUT_EXPIRATION: u64 = FETCH_TIMEOUT_PERIOD + 1000; // in milli-seconds
pub const LOCK_BLOCK_EXPIRATION: u32 = 3; // in block number

pub trait Config: system::Config {
    type Currency: Currency<Self::AccountId>;
    type Event: From<Event<Self>> + Into<<Self as system::Config>::Event>;
}

decl_event! {
    pub enum Event<T>
    where
        AccountId = <T as system::Config>::AccountId,
    {
        BondMachine(AccountId, u32),
    }
}

decl_error! {
    pub enum Error for Module<T: Config> {
        MachineIDNotBond,
        HttpFetchingError,
    }
}

decl_storage! {
    trait Store for Module<T: Config> as NodeOwnerStaking {
        pub Members get(fn members): map hasher(blake2_128_concat) T::AccountId => ();
        pub UserCurrProfile get(fn user_curr_profile): map hasher(blake2_128_concat) T::AccountId => u128;
        pub UserFutureProfile get(fn user_future_profile): map hasher(blake2_128_concat) T::AccountId => Vec<u64>;
    }
}

decl_module! {
    pub struct Module<T: Config> for enum Call where origin: T::Origin {
        fn deposit_event() = default;

        #[weight = 20_000]
        pub fn bond_machine(origin, machine_id: u64) -> DispatchResult{
            let _user = ensure_signed(origin)?;
            // TODO: call off-chain worker to bind machine
            //
            // http://116.85.24.172:41107/api/v1/mining_nodes/2gfpp3MAB4Aq2ZPEU72neZTVcZkbzDzX96op9d3fvi3
            Ok(())
        }

        #[weight = 10_000]
        pub fn add_bonded_token(origin, machine_id: u64, bond_amount: BalanceOf<T>) -> DispatchResult{
            let _user = ensure_signed(origin)?;
            Ok(())
        }

        #[weight = 10_000]
        pub fn reduce_bonded_token(origin, machine_id: u64, reduce_amount: BalanceOf<T>) -> DispatchResult{
            let _user = ensure_signed(origin)?;
            Ok(())
        }

        #[weight = 10_000]
        pub fn reduce_all_bonded_token(origin) -> DispatchResult{
            let _user = ensure_signed(origin)?;
            Ok(())
        }

        #[weight = 10_000]
        pub fn rm_bonded_machine(origin, machine_id: u64) -> DispatchResult{
            let _user = ensure_signed(origin)?;
            Ok(())
        }
    }
}

impl<T: Config> Module<T> {
    fn fetch_machine_info() -> Result<(), Error<T>> {
        let s_info = StorageValueRef::persistent(b"offchain-worker::?");

        if let Some(Some(gh_info)) = s_info.get::<machine_info::MachineInfo>() {
            debug::info!("cached gh-info: {:?}", gh_info);
            return Ok(());
        }

        let mut lock = StorageLock::<BlockAndTime<Self>>::with_block_and_time_deadline(
            b"offchain-demo::lock",
            LOCK_BLOCK_EXPIRATION,
            offchain::Duration::from_millis(LOCK_TIMEOUT_EXPIRATION),
        );

        if let Ok(_gurad) = lock.try_lock() {
            match Self::fetch_n_parse() {
                Ok(gh_info) => {
                    s_info.set(&gh_info);
                }
                Err(err) => {
                    return Err(err);
                }
            }
        }

        Ok(())
    }

    fn fetch_n_parse() -> Result<machine_info::MachineInfo, Error<T>> {
        let resp_bytes = Self::fetch_from_remote().map_err(|e| {
            debug::error!("fetch_from_remote error: {:?}", e);
            <Error<T>>::HttpFetchingError
        })?;

        let resp_str = str::from_utf8(&resp_bytes).map_err(|_| <Error<T>>::HttpFetchingError)?;
        debug::info!("{}", resp_str);

        let gh_info: machine_info::MachineInfo =
            serde_json::from_str(&resp_str).map_err(|_| <Error<T>>::HttpFetchingError)?;
        Ok(gh_info)
    }

    fn fetch_from_remote() -> Result<Vec<u8>, Error<T>> {
        debug::info!("sending request to: {}", HTTP_REMOTE_REQUEST);

        let request = offchain::http::Request::get(HTTP_REMOTE_REQUEST);

        let timeout =
            sp_io::offchain::timestamp().add(offchain::Duration::from_millis(FETCH_TIMEOUT_PERIOD));

        let pending = request
            .add_header("User-Agent", HTTP_HEADER_USER_AGENT)
            .deadline(timeout)
            .send()
            .map_err(|_| <Error<T>>::HttpFetchingError)?;

        let response = pending
            .try_wait(timeout)
            .map_err(|_| <Error<T>>::HttpFetchingError)?
            .map_err(|_| <Error<T>>::HttpFetchingError)?;

        if response.code != 200 {
            debug::error!("Unexpected http request status code: {}", response.code);
            return Err(<Error<T>>::HttpFetchingError);
        }

        Ok(response.body().collect::<Vec<u8>>())
    }
}

impl<T: Config> offchain::storage_lock::BlockNumberProvider for Module<T> {
    type BlockNumber = T::BlockNumber;
    fn current_block_number() -> Self::BlockNumber {
        <frame_system::Module<T>>::block_number()
    }
}

// impl<T: Trait> frame_support::unsigned::ValidateUnsigned for Module<T> {
//     type Call = Call<T>;

//     fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
//         let valid_tx = |provid| {
//             ValidTransaction::with_tag_prefix("node-owner-staking")
//                 .priority(T::UnsignedPriority::get())
//                 .and_provides([&provide])
//                 .longevity(3)
//                 .propagate(true)
//                 .build()
//         };
//         match all {
//             Call::submit_bond_machine_unsigned() => valid_tx(b"adf".to_vec()),
//             _ => InvalidTransaction::Call.into(),
//         }
//     }
// }
