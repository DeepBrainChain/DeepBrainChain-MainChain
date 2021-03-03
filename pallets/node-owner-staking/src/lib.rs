#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
    debug, decl_error, decl_event, decl_module, decl_storage,
    dispatch::{DispatchError, DispatchResult},
    ensure,
    traits::{Currency, ExistenceRequirement::AllowDeath},
};
use frame_system::{self as system, ensure_signed};
use sp_runtime::{
    offchain,
    offchain::{
        storage::StorageValueRef,
        storage_lock::{BlockAndTime, StorageLock},
    },
    traits::AccountIdConversion,
    ModuleId,
};
use sp_std::{collections::vec_deque::VecDeque, prelude::*, str};

mod machine_info;

type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as system::Config>::AccountId>>::Balance;

// PALLET_ID must be exactly eight characters long.
const PALLET_ID: ModuleId = ModuleId(*b"MCStake!");

pub const NUM_VEC_LEN: usize = 10;
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
        Balance = BalanceOf<T>,
    {
        BondMachine(AccountId, u32),
        AddBondReceive(AccountId, Balance, AccountId),
        ReduceBonded(AccountId, Balance, AccountId),
    }
}

decl_error! {
    pub enum Error for Module<T: Config> {
        MachineIDNotBond,
        HttpFetchingError,
        BalanceNotEnough,
    }
}

decl_storage! {
    trait Store for Module<T: Config> as NodeOwnerStaking {
        BindingQueue get(fn binding_queue): VecDeque<u64>;

        pub Members get(fn members): map hasher(blake2_128_concat) T::AccountId => ();
        pub UserCurrProfile get(fn user_curr_profile): map hasher(blake2_128_concat) T::AccountId => u128;
        pub UserFutureProfile get(fn user_future_profile): map hasher(blake2_128_concat) T::AccountId => Vec<u64>;
    }
    add_extra_genesis {
        build(|_config| {
            // Create the charity's pot of funds, and ensure it has the minimum required deposit
            let _ = T::Currency::make_free_balance_be(
                &<Module<T>>::account_id(),
                T::Currency::minimum_balance(),
            );
        });
    }
}

decl_module! {
    pub struct Module<T: Config> for enum Call where origin: T::Origin {
        fn deposit_event() = default;

        #[weight = 20_000]
        pub fn bond_machine(origin, machine_id: u64) -> DispatchResult{
            let _user = ensure_signed(origin)?;
            Self::append_or_relpace_binding_machine(machine_id);
            Ok(())
        }

        #[weight = 10_000]
        pub fn add_bonded_token(origin, machine_id: u64, bond_amount: BalanceOf<T>) -> DispatchResult{
            let user = ensure_signed(origin)?;

            // TODO: 1. check balance of user
            ensure!(T::Currency::free_balance(&user) > bond_amount,Error::<T>::BalanceNotEnough );

            let _ = T::Currency::transfer(&user, &Self::account_id(), bond_amount, AllowDeath);

            Self::deposit_event(RawEvent::AddBondReceive(user, bond_amount, Self::account_id()));

            // TODO: 3. record user's stake history to calc block info

            Ok(())
        }

        #[weight = 10_000]
        fn reduce_bonded_token(origin, machine_id: u64, amount: BalanceOf<T>) -> DispatchResult {
            let user = ensure_signed(origin)?;
            // TODO: check if machine belong to this user.
            // TODO: check bond amount bigger than user's

            // TODO: cannot transfer to user directly, but lock some time instead

            // Make the transfer requested
            T::Currency::transfer(&Self::account_id(), &user, amount, AllowDeath)
                .map_err(|_| DispatchError::Other("Can't make allocation"))?;

            // TODO what about errors here??

            Self::deposit_event(RawEvent::ReduceBonded(user, amount, Self::account_id()));
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

        fn offchain_worker(block_number: T::BlockNumber) {
            debug::info!("Entering off-chain worker");

            // TODO: run multiple query
            BindingQueue::mutate(|binding_queue| {
                if binding_queue.len() == 0 {
                    return
                }
                let a_machine_info = binding_queue.pop_front().unwrap();

                Self::fetch_machine_info(a_machine_info);
            })
        }


    }
}

impl<T: Config> Module<T> {
    fn append_or_relpace_binding_machine(machine_id: u64) {
        BindingQueue::mutate(|binding_queue| {
            if binding_queue.len() == NUM_VEC_LEN {
                let _ = binding_queue.pop_front();
            }
            binding_queue.push_back(machine_id);
            debug::info!("Machine info: {:?}", binding_queue);
        })
    }

    pub fn account_id() -> T::AccountId {
        PALLET_ID.into_account()
    }

    fn pot() -> BalanceOf<T> {
        T::Currency::free_balance(&Self::account_id())
    }

    // TODO: fetch machine info and compare with user's addr, if it's same, store it else return
    fn fetch_machine_info(machine_id: u64) -> Result<(), Error<T>> {
        let s_info = StorageValueRef::persistent(b"offchain-worker::mc-info");

        if let Some(Some(mc_info)) = s_info.get::<machine_info::MachineInfo>() {
            debug::info!("cached gh-info: {:?}", mc_info);
            return Ok(());
        }

        let mut lock = StorageLock::<BlockAndTime<Self>>::with_block_and_time_deadline(
            b"offchain-demo::lock",
            LOCK_BLOCK_EXPIRATION,
            offchain::Duration::from_millis(LOCK_TIMEOUT_EXPIRATION),
        );

        if let Ok(_gurad) = lock.try_lock() {
            match Self::fetch_n_parse() {
                Ok(mc_info) => {
                    s_info.set(&mc_info);
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
