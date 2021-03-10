#![cfg_attr(not(feature = "std"), no_std)]

use alt_serde::Deserialize;
use codec::{Decode, Encode};
use frame_support::{
    debug, decl_error, decl_event, decl_module, decl_storage,
    dispatch::{DispatchError, DispatchResult},
    ensure,
    traits::{Currency, ExistenceRequirement::AllowDeath, Randomness},
};
use frame_system::{self as system, ensure_root, ensure_signed};
use sp_core::H256;
use sp_runtime::{
    offchain,
    offchain::http,
    traits::{AccountIdConversion, Saturating},
    ModuleId,
};
use sp_std::{collections::vec_deque::VecDeque, prelude::*, str};

mod machine_info;

type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as system::Config>::AccountId>>::Balance;
type MachineId = Vec<u8>;

// PALLET_ID must be exactly eight characters long.
const PALLET_ID: ModuleId = ModuleId(*b"MCStake!");

pub const NUM_VEC_LEN: usize = 10;
pub const HTTP_REMOTE_REQUEST: &str = "http://116.85.24.172:41107/api/v1/mining_nodes/";
pub const HTTP_HEADER_USER_AGENT: &str = "jimmychu0807"; // TODO: remove this

pub const FETCH_TIMEOUT_PERIOD: u64 = 3_000; // in milli-seconds
pub const LOCK_TIMEOUT_EXPIRATION: u64 = FETCH_TIMEOUT_PERIOD + 1_000; // in milli-seconds
pub const LOCK_BLOCK_EXPIRATION: u32 = 3; // in block number

pub trait Config: system::Config {
    type Currency: Currency<Self::AccountId>;
    type Event: From<Event<Self>> + Into<<Self as system::Config>::Event>;
    type RandomnessSource: Randomness<H256>;
}

decl_event! {
    pub enum Event<T>
    where
        AccountId = <T as system::Config>::AccountId,
        Balance = BalanceOf<T>,
    {
        BondMachine(AccountId, MachineId),
        AddBondReceive(AccountId, Balance, AccountId),
        RemoveBonded(AccountId, MachineId, Balance),

        CommitteeAdded(AccountId),
        CommitteeRemoved(AccountId),

        AlternateCommitteeAdded(AccountId),
        AlternateCommitteeRemoved(AccountId),

        DonationReceived(AccountId, Balance, Balance),
        FundsAllocated(AccountId, Balance, Balance),
    }
}

decl_error! {
    pub enum Error for Module<T: Config> {
        MachineIDNotBonded,
        MachineHasBonded,
        MachineInBondingQueue,
        TokenNotBonded,
        BondedNotEnough,
        HttpFetchingError,
        HttpDecodeError,
        BalanceNotEnough,
        NotMachineOwner,
        AlreadyAddedMachine,

        AlternateCommitteeLimitReached,
        AlreadyAlternateCommittee,
        NotAlternateCommittee,

        CommitteeLimitReached,
        AlreadyCommittee,
        NotCommittee,
    }
}

#[serde(crate = "alt_serde")]
#[derive(Deserialize, Encode, Decode, Default, Debug)]
pub struct BondingPair<AccountId> {
    account_id: AccountId,
    machine_id: MachineId,
}

decl_storage! {
    trait Store for Module<T: Config> as NodeOwnerStaking {
        /// balance that can be draw now
        pub UserCurrentProfile get(fn user_current_profile): map hasher(blake2_128_concat) T::AccountId => BalanceOf<T>;

        /// balance that linear release
        pub UserPendingProfile get(fn user_pending_profile): map hasher(blake2_128_concat) T::AccountId => BalanceOf<T>;

        /// store user's machine
        pub UserBondedMachine get(fn user_bonded_machine): map hasher(blake2_128_concat) T::AccountId => Vec<MachineId>;

        /// store how much user has bonded
        pub UserBondedMoney get(fn user_bonded_token): double_map hasher(blake2_128_concat) T::AccountId, hasher(blake2_128_concat) MachineId => BalanceOf<T>;

        /// used for OCW to store pending binding pair
        pub BondingQueue get(fn bonding_queue): VecDeque<BondingPair<T::AccountId>>;

        /// BondingQueue machine for quick search if machine_id is pending
        pub BondingQueueMachine get(fn bonding_queue_machine): map hasher(blake2_128_concat) MachineId => ();

        /// Machine has been bonded
        pub BondedMachine get(fn bonded_machine): map hasher(blake2_128_concat) MachineId => ();

        /// MachineInfo
        pub MachineInfo get(fn machine_info): map hasher(blake2_128_concat) MachineId => ();

        /// Alternate Committee
        pub AlternateCommittee get(fn alternate_committee): Vec<T::AccountId>;

        /// ALternate Committee Num
        pub AlternateCommitteeNum get(fn alternate_committee_num) config(): u32 = 10;

        /// committee
        pub Committee get(fn committee): Vec<T::AccountId>;

        /// max committee num
        pub CommitteeNum get(fn max_committee_num) config(): u32 = 3;

        /// nonce to generate random number for selecting committee
        Nonce get(fn nonce) config(): u32;

        /// machine info url
        pub MachineInfoUrl get(fn machine_info_url) config(): MachineId = "http://116.85.24.172:41107/api/v1/mining_nodes/".as_bytes().to_vec();
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

        /// Bonding machine only remember caller-machine_id pair.
        /// OCW will check it and record machine info.
        #[weight = 20_000]
        pub fn bond_machine(origin, machine_id: MachineId) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            debug::info!("############ Callse is: {:#?}", caller);

            // BondingQueue not have this machine_id
            ensure!(!<BondingQueueMachine>::contains_key(&machine_id), Error::<T>::MachineInBondingQueue);
            // machine must not be bonded yet
            ensure!(!<BondedMachine>::contains_key(&machine_id), Error::<T>::MachineHasBonded);

            // append it to BondingQueue
            Self::append_or_relpace_bonding_machine(
                BondingPair{
                    account_id: caller,
                    machine_id: machine_id.clone(),
                });

            BondingQueueMachine::insert(&machine_id, ());

            Ok(())
        }

        #[weight = 10_000]
        pub fn rm_bonded_machine(origin, machine_id: MachineId) -> DispatchResult {
            let user = ensure_signed(origin)?;
            let mut user_bonded_machine = UserBondedMachine::<T>::get(&user);

            match user_bonded_machine.binary_search(&machine_id) {
                Ok(index) => {
                    user_bonded_machine.remove(index);
                    UserBondedMachine::<T>::insert(user.clone(), user_bonded_machine);
                    let user_bonded_money = <UserBondedMoney<T>>::get(&user, &machine_id);

                    // TODO: Lock user balanced money
                    T::Currency::transfer(&Self::account_id(), &user, user_bonded_money,AllowDeath)
                        .map_err(|_| DispatchError::Other("Can't make allocation"))?;

                    BondedMachine::remove(&machine_id);

                    Self::deposit_event(RawEvent::RemoveBonded(user, machine_id.clone(), user_bonded_money));
                    return Ok(())
                },
                Err(_) => return Err(Error::<T>::MachineIDNotBonded.into()),
            }
        }

        #[weight = 10_000]
        pub fn add_bonded_token(origin, machine_id: MachineId, bond_amount: BalanceOf<T>) -> DispatchResult {
            let user = ensure_signed(origin)?;

            // Check free balance of user
            ensure!(T::Currency::free_balance(&user) > bond_amount, Error::<T>::BalanceNotEnough);
            // ensure machine_id is bonded, UserBondedMachine must contain this pair
            let user_bonded_machine = <UserBondedMachine<T>>::get(&user);
            if let Err(_) = user_bonded_machine.binary_search(&machine_id){
                return Err(Error::<T>::MachineIDNotBonded.into())
            };

            let _ = T::Currency::transfer(&user, &Self::account_id(), bond_amount, AllowDeath);

            if <UserBondedMoney<T>>::contains_key(&user, &machine_id) {
                let user_bonded_money = <UserBondedMoney<T>>::get(&user, &machine_id);
                <UserBondedMoney<T>>::insert(&user, &machine_id, user_bonded_money.saturating_add(bond_amount));
            } else {
                <UserBondedMoney<T>>::insert(&user, &machine_id, bond_amount);
            }

            Self::deposit_event(RawEvent::AddBondReceive(user, bond_amount, Self::account_id()));

            Ok(())
        }

        #[weight = 10_000]
        fn reduce_bonded_token(origin, machine_id: MachineId, amount: BalanceOf<T>) -> DispatchResult {
            let user = ensure_signed(origin)?;

            ensure!(<UserBondedMachine<T>>::contains_key(&user), Error::<T>::MachineIDNotBonded);
            ensure!(<UserBondedMoney<T>>::contains_key(&user, &machine_id), Error::<T>::TokenNotBonded);

            let bonded_money_left = <UserBondedMoney<T>>::get(&user, &machine_id);
            ensure!(bonded_money_left >= amount, Error::<T>::BondedNotEnough);

            // TODO: Lock some time instead of transfer to user directly,

            // Make the transfer requested
            T::Currency::transfer(&Self::account_id(), &user, amount, AllowDeath)
                .map_err(|_| DispatchError::Other("Can't make allocation"))?;
            // TODO what about errors here??

            <UserBondedMoney<T>>::insert(&user, &machine_id, bonded_money_left.saturating_sub(amount));

            Self::deposit_event(RawEvent::RemoveBonded(user, machine_id, amount));
            Ok(())
        }

        #[weight = 10_000]
        pub fn reduce_all_bonded_token(origin, machine_id: MachineId) -> DispatchResult {
            let user = ensure_signed(origin)?;

            ensure!(<UserBondedMachine<T>>::contains_key(&user), Error::<T>::MachineIDNotBonded);
            ensure!(<UserBondedMoney<T>>::contains_key(&user, &machine_id), Error::<T>::TokenNotBonded);

            let bonded_money_left = <UserBondedMoney<T>>::get(&user, &machine_id);
            ensure!(bonded_money_left > 0.into(), Error::<T>::BondedNotEnough);

            T::Currency::transfer(&Self::account_id(), &user, bonded_money_left, AllowDeath)
                .map_err(|_| DispatchError::Other("Can't make allocation"))?;
            // TODO what about errors here??

            <UserBondedMoney<T>>::insert(&user, &machine_id, bonded_money_left.saturating_sub(bonded_money_left));
            Self::deposit_event(RawEvent::RemoveBonded(user, machine_id, bonded_money_left));
            Ok(())
        }

        #[weight = 0]
        pub fn add_alternate_committee(origin, new_member: T::AccountId) -> DispatchResult {
            ensure_root(origin)?;

            let mut members = AlternateCommittee::<T>::get();
            ensure!(members.len() < AlternateCommitteeNum::get() as usize, Error::<T>::AlternateCommitteeLimitReached);

            match members.binary_search(&new_member) {
                Ok(_) => Err(Error::<T>::AlreadyAlternateCommittee.into()),
                Err(index) => {
                    members.insert(index, new_member.clone());
                    Committee::<T>::put(members);
                    Self::deposit_event(RawEvent::AlternateCommitteeAdded(new_member));
                    Ok(())
                }
            }
        }

        #[weight = 0]
        pub fn remove_alternate_committee(origin, old_member: T::AccountId) -> DispatchResult {
            ensure_root(origin)?;

            let mut members = AlternateCommittee::<T>::get();

            match members.binary_search(&old_member) {
                Ok(index) => {
                    members.remove(index);
                    AlternateCommittee::<T>::put(members);
                    Self::deposit_event(RawEvent::AlternateCommitteeRemoved(old_member));
                    Ok(())
                },
                Err(_) => Err(Error::<T>::NotAlternateCommittee.into()),
            }
        }

        #[weight = 0]
        pub fn add_committee(origin, new_member: T::AccountId) -> DispatchResult {
            ensure_root(origin)?;

            let mut members = Committee::<T>::get();
            ensure!(members.len() < CommitteeNum::get() as usize, Error::<T>::CommitteeLimitReached);

            match members.binary_search(&new_member) {
                Ok(_) => Err(Error::<T>::AlreadyCommittee.into()),
                Err(index) => {
                    members.insert(index, new_member.clone());
                    Committee::<T>::put(members);
                    Self::deposit_event(RawEvent::CommitteeAdded(new_member));
                    Ok(())
                }
            }
        }

        /// adf
        #[weight = 0]
        pub fn remove_committee(origin, old_member: T::AccountId) -> DispatchResult {
            ensure_root(origin)?;

            let mut members = Committee::<T>::get();

            match members.binary_search(&old_member) {
                Ok(index) => {
                    members.remove(index);
                    Committee::<T>::put(members);
                    Self::deposit_event(RawEvent::CommitteeRemoved(old_member));
                    Ok(())
                },
                Err(_) => Err(Error::<T>::NotCommittee.into()),
            }
        }

        #[weight = 0]
        pub fn manual_select_committee(origin) -> DispatchResult {
            ensure_root(origin)?;

            Self::select_committee();
            Ok(())
        }

        #[weight = 0]
        pub fn donate_money(origin, amount: BalanceOf<T>) -> DispatchResult {
            let donor = ensure_signed(origin)?;

            T::Currency::transfer(&donor, &Self::account_id(), amount, AllowDeath)
                .map_err(|_| DispatchError::Other("Can't make donation"))?;
            Self::deposit_event(RawEvent::DonationReceived(donor, amount, Self::pot()));
            Ok(())
        }

        #[weight = 0]
        pub fn allocate(origin, dest: T::AccountId, amount: BalanceOf<T>,) -> DispatchResult {
            ensure_root(origin)?;

            T::Currency::transfer(&Self::account_id(), &dest, amount, AllowDeath,)
                .map_err(|_| DispatchError::Other("Can't make allocation"))?;

            Self::deposit_event(RawEvent::FundsAllocated(dest, amount, Self::pot()));
            Ok(())
        }

        fn offchain_worker(block_number: T::BlockNumber) {
            debug::info!("Entering off-chain worker");

            BondingQueue::<T>::mutate(|bonding_queue| {
                while bonding_queue.len() > 0 {
                    let bond_pair = bonding_queue.pop_front().unwrap();
                    let machine_id =  str::from_utf8(&bond_pair.machine_id).map_err(|_| http::Error::Unknown).unwrap();

                    let machine_info = Self::fetch_machine_info(&machine_id);
                    if let Err(e) = machine_info {
                        debug::error!("Offchain worker error: {:?}", e);
                        return
                    }

                    // if bond_pair.account_id == machine_info.unwrap().data.wallet[1].0 {
                        let user_bonded_machine = UserBondedMachine::<T>::get(bond_pair.account_id.clone());
                        UserBondedMachine::<T>::insert(bond_pair.account_id, user_bonded_machine);
                        BondedMachine::insert(bond_pair.machine_id, ());
                    // }
                }
            });

        }
    }
}

impl<T: Config> Module<T> {
    fn append_or_relpace_bonding_machine(machine_pair: BondingPair<T::AccountId>) {
        BondingQueue::<T>::mutate(|bonding_queue| {
            if bonding_queue.len() == NUM_VEC_LEN {
                let _ = bonding_queue.pop_front();
            }
            bonding_queue.push_back(machine_pair);
        })
    }

    pub fn account_id() -> T::AccountId {
        PALLET_ID.into_account()
    }

    fn select_committee() {
        // H256
        let subject = Self::encode_and_update_nonce();
        let _random_seed = T::RandomnessSource::random(&subject);
    }

    fn encode_and_update_nonce() -> Vec<u8> {
        let nonce = Nonce::get();
        Nonce::put(nonce.wrapping_add(1));
        nonce.encode()
    }

    fn pot() -> BalanceOf<T> {
        T::Currency::free_balance(&Self::account_id())
    }

    fn fetch_machine_info(machine_id: &str) -> Result<machine_info::MachineInfo, Error<T>> {
        let mut url = HTTP_REMOTE_REQUEST.as_bytes().to_vec();
        url.extend(&machine_id.as_bytes().to_vec());

        let url = str::from_utf8(&url)
            .map_err(|_| http::Error::Unknown)
            .unwrap();
        debug::info!("sending request to: {}", &url);

        let request = offchain::http::Request::get(&url);

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

        let body = response.body().collect::<Vec<u8>>();
        let body_str = sp_std::str::from_utf8(&body)
            .map_err(|_| {
                debug::warn!("No UTF8 body");
                http::Error::Unknown
            })
            .unwrap(); // TODO: handle error here

        let machine_info: machine_info::MachineInfo = serde_json::from_str(&body_str).unwrap(); // TODO: handler error here

        debug::info!("#### MachineInfo str: {}", &body_str);
        debug::info!("############ Machine_info is: {:?}", machine_info);

        Ok(machine_info)
    }
}

impl<T: Config> offchain::storage_lock::BlockNumberProvider for Module<T> {
    type BlockNumber = T::BlockNumber;
    fn current_block_number() -> Self::BlockNumber {
        <frame_system::Module<T>>::block_number()
    }
}
