// 委员会不设置个数限制，满足质押，并且通过议案选举即可。
// 每次机器加入，系统随机选择3个委员会，对应到0~36h。每次验证区间为4个小时,共有9个验证区间。
// 每个委员随机分得3个验证区间，进行验证。
// 下一轮选择，与上一轮委员会是否被选择的状态无关。
// 委员会确认机器，会提供三个字段组成的 Hash1 = Hash(机器原始信息, 委员会随机字符串, bool(机器正常与否))
// 最后12个小时，统计委员会结果，多数结果为最终结果。第二次提交信息为： 机器原始信息，委员会随机字符串，bool.
// 验证：1. Hash(机器原始信息) == OCW获取到的机器Hash
//      2. Hash(机器原始信息，委员会随机字符串, bool) == Hash1
// 如果没有人提交信息，则进行新一轮随机派发。

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode, HasCompact};
use frame_support::{
    ensure,
    pallet_prelude::*,
    traits::{Currency, Get, LockIdentifier, LockableCurrency, WithdrawReasons},
};
use frame_system::{ensure_root, ensure_signed, pallet_prelude::*};
// use online_profile::types::*;
use online_profile_machine::LCOps;
use sp_io::hashing::blake2_128;
use sp_runtime::{traits::SaturatedConversion, RuntimeDebug};
use sp_std::{prelude::*, str, vec::Vec};

pub type MachineId = Vec<u8>;
pub type EraIndex = u32;
type BalanceOf<T> =
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct PendingVerify<BlockNumber> {
    pub machine_id: MachineId,
    pub add_height: BlockNumber,
}

pub const PALLET_LOCK_ID: LockIdentifier = *b"leasecom";
pub const DISTRIBUTION: u32 = 9; // 分成9个区间进行验证
                                 // pub const DURATIONPERCOMMITTEE: u32 = 480; // 每个用户有480个块的时间验证机器: 480 * 30 / 3600 = 4 hours

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

// 记录处于不同状态的委员会的列表，方便派单
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct StakerList<AccountId: Ord> {
    pub committee: Vec<AccountId>,    // 质押并通过社区选举的委员会
    pub chill_list: Vec<AccountId>,   // 委员会，但不想被派单
    pub fulfill_list: Vec<AccountId>, // 委员会, 但需要补交质押
}

impl<AccountId: Ord> StakerList<AccountId> {
    fn staker_exist(&self, who: &AccountId) -> bool {
        if let Ok(_) = self.committee.binary_search(who) {
            return true;
        }
        if let Ok(_) = self.chill_list.binary_search(who) {
            return true;
        }
        if let Ok(_) = self.fulfill_list.binary_search(who) {
            return true;
        }
        false
    }

    fn add_staker(a_field: &mut Vec<AccountId>, new_staker: AccountId) {
        if let Err(index) = a_field.binary_search(&new_staker) {
            a_field.insert(index, new_staker);
        }
    }

    fn rm_staker(a_field: &mut Vec<AccountId>, drop_staker: &AccountId) {
        if let Ok(index) = a_field.binary_search(drop_staker) {
            a_field.remove(index);
        }
    }
}

// 记录用户的质押及罚款
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, Default)]
pub struct StakingLedger<Balance: HasCompact> {
    #[codec(compact)]
    pub total: Balance,
    #[codec(compact)]
    pub active: Balance,
}

// 从用户地址查询绑定的机器列表
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct CommitteeMachineList<BlockNumber> {
    pub booked_machine: Vec<(MachineId, BlockNumber)>, // 记录分配给用户的机器ID及开始验证时间
    pub hashed_machine: Vec<MachineId>,                // 存储已经提交了Hash信息的机器
    pub confirmed_machine: Vec<MachineId>,             // 存储已经提交了原始确认数据的机器
    pub online_machine: Vec<MachineId>,                // 存储已经成功上线的机器
}

// 一台机器对应的委员会
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MachineCommitteeList<AccountId, BlockNumber> {
    pub book_time: BlockNumber,
    pub booked_committee: Vec<(AccountId, BlockNumber)>, // 记录分配给机器的委员会及验证开始时间
    pub hashed_committee: Vec<AccountId>,
    pub confirm_start: BlockNumber, // 开始提交raw信息的时间
    pub confirmed_committee: Vec<AccountId>,
    pub onlined_committee: Vec<AccountId>,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct CommitteeMachineOps<BlockNumber> {
    pub booked_time: BlockNumber,
    pub confirm_hash: [u8; 16],
    pub hash_time: BlockNumber,
    pub confirm_raw: Vec<u8>,
    pub confirm_time: BlockNumber, // 委员会提交raw信息的时间
    pub machine_status: MachineStatus,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum MachineStatus {
    Booked,
    Hashed,
    Confirmed,
}

impl Default for MachineStatus {
    fn default() -> Self {
        MachineStatus::Booked
    }
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum CommitteeStatus<BlockNumber> {
    NotCommittee,          // 非委员会，默认状态
    Health,                // 正常的委员会状态
    FillingPledge,         // 需要等待补充押金
    Chilling(BlockNumber), // 正在退出的状态, 记录Chill时的高度，当达到质押限制时，则可以退出
}

impl<BlockNumber> Default for CommitteeStatus<BlockNumber> {
    fn default() -> Self {
        CommitteeStatus::NotCommittee
    }
}

#[rustfmt::skip]
#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + online_profile::Config + random_num::Config + dbc_price_ocw::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Currency: LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;
        type LCOperations: LCOps<AccountId = Self::AccountId, MachineId = MachineId>;
        type BondingDuration: Get<EraIndex>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_finalize(_block_number: T::BlockNumber) {
            // 分派机器
            Self::distribute_machines();

            // TODO: 轮训是否已经有委员会的订单到期，如果到期
            // 则对该成员进行惩罚。并从该委员会的列表中移除该任务

            // if Candidacy::<T>::get().len() > 0 && Committee::<T>::get().len() == 0 {
            //     Self::update_committee();
            //     return;
            // }

            // let committee_duration = T::CommitteeDuration::get();
            // let block_per_era = <random_num::Module<T>>::block_per_era();

            // if block_number.saturated_into::<u64>() / (block_per_era * committee_duration) as u64
            //     == 0
            // {
            //     Self::update_committee()
            // }
        }
    }

    // 最小质押默认100RMB等价DBC
    /// Minmum stake amount to become candidacy (usd * 10**6)
    #[pallet::storage]
    #[pallet::getter(fn committee_min_stake)]
    pub(super) type CommitteeMinStake<T: Config> = StorageValue<_, u64, ValueQuery>;

    // 设定委员会审查所需时间，单位：块高
    #[pallet::type_value]
    pub fn ConfirmTimeLimitDefault<T: Config>() -> T::BlockNumber {
        480u32.into()
    }

    // 记录用户需要在book之后，多少个高度内完成确认
    #[pallet::storage]
    #[pallet::getter(fn confirm_time_limit)]
    pub(super) type ConfirmTimeLimit<T: Config> = StorageValue<_, T::BlockNumber, ValueQuery, ConfirmTimeLimitDefault<T>>;

    #[pallet::storage]
    #[pallet::getter(fn staker)]
    pub(super) type Staker<T: Config> = StorageValue<_, StakerList<T::AccountId>, ValueQuery>;

    // 存储用户订阅的不同确认阶段的机器
    #[pallet::storage]
    #[pallet::getter(fn committee_machine)]
    pub(super) type CommitteeMachine<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, CommitteeMachineList<T::BlockNumber>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn machine_committee)]
    pub(super) type MachineCommittee<T: Config> = StorageMap<_, Blake2_128Concat, MachineId, MachineCommitteeList<T::AccountId, T::BlockNumber>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn ops_detail)]
    pub(super) type OpsDetail<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        MachineId,
        CommitteeMachineOps<T::BlockNumber>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn black_list)]
    pub(super) type BlackList<T: Config> = StorageValue<_, Vec<T::AccountId>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn committee_ledger)]
    pub(super) type CommitteeLedger<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Option<StakingLedger<BalanceOf<T>>>,
        ValueQuery,
    >;

    #[pallet::call]
    #[rustfmt::skip]
    impl<T: Config> Pallet<T> {
        // 设置committee的最小质押，一般等于两天的奖励
        /// set min stake to become committee, value: usd * 10^6
        #[pallet::weight(0)]
        pub fn set_min_stake(origin: OriginFor<T>, value: u64) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            CommitteeMinStake::<T>::put(value);
            Ok(().into())
        }

        // 该操作由社区决定
        // Root权限，添加到委员会，直接添加到fulfill列表中。当竞选成功后，需要操作以从fulfill_list到committee
        #[pallet::weight(0)]
        pub fn add_committee(origin: OriginFor<T>, member: T::AccountId) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let mut staker = Self::staker();

            // 确保用户还未加入到本模块
            ensure!(!staker.staker_exist(&member), Error::<T>::AccountAlreadyExist);

            // 将用户添加到fulfill列表中
            StakerList::add_staker(&mut staker.fulfill_list, member.clone());
            Self::deposit_event(Event::CommitteeAdded(member));
            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn fill_pledge(origin: OriginFor<T>) -> DispatchResultWithPostInfo{
            let who = ensure_signed(origin)?;

            // 检查是否在fulfill列表中
            let mut staker = Self::staker();
            if let Err(_) = staker.fulfill_list.binary_search(&who) {
                return Err(Error::<T>::NoNeedFulfill.into());
            }

            // 获取需要质押的数量
            let min_stake = Self::get_min_stake_amount();
            if let None = min_stake {
                return Err(Error::<T>::MinStakeNotFound.into());
            }
            let min_stake = min_stake.unwrap();

            let mut ledger = Self::committee_ledger(&who).unwrap_or(StakingLedger {
                ..Default::default()
            });

            // 检查用户余额，更新质押
            let needed = min_stake - ledger.total;
            ensure!(needed < <T as Config>::Currency::free_balance(&who), Error::<T>::FreeBalanceNotEnough);

            ledger.active += min_stake - ledger.total;
            ledger.total = min_stake;
            Self::update_ledger(&who, &ledger);

            // 从fulfill 移出来，并放到正常委员会列表
            StakerList::rm_staker(&mut staker.fulfill_list, &who);
            StakerList::add_staker(&mut staker.committee, who.clone());

            Self::deposit_event(Event::CommitteeFulfill(needed));

            Ok(().into())
        }

        // 委员会停止接单
        #[pallet::weight(10000)]
        pub fn chill(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let mut staker = Self::staker();
            ensure!(staker.staker_exist(&who), Error::<T>::AccountNotExist);

            // 只有committee状态才允许进行chill
            if let Err(_) = staker.committee.binary_search(&who) {
                return Err(Error::<T>::NotCommittee.into());
            }

            StakerList::rm_staker(&mut staker.committee, &who);
            StakerList::add_staker(&mut staker.chill_list, who.clone());

            Staker::<T>::put(staker);
            Self::deposit_event(Event::Chill(who));

            Ok(().into())
        }

        // 委员会可以接单
        #[pallet::weight(10000)]
        pub fn undo_chill(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let mut staker = Self::staker();
            if let Err(_) = staker.chill_list.binary_search(&who) {
                return Err(Error::<T>::NotInChillList.into());
            }

            StakerList::rm_staker(&mut staker.chill_list, &who);
            StakerList::add_staker(&mut staker.committee, who.clone());
            Staker::<T>::put(staker);

            Self::deposit_event(Event::UndoChill(who));
            Ok(().into())
        }

        // 委员会可以退出, 从chill_list中退出
        #[pallet::weight(10000)]
        pub fn exit_staker(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let mut staker = Self::staker();
            ensure!(staker.staker_exist(&who), Error::<T>::AccountNotExist);

            // 如果有未完成的工作，则不允许退出
            let committee_machines = Self::committee_machine(&who);
            if committee_machines.booked_machine.len() > 0 ||
                committee_machines.hashed_machine.len() > 0 ||
                committee_machines.confirmed_machine.len() > 0 {
                    return Err(Error::<T>::JobNotDone.into());
            }

            // 如果是candidacy，则可以直接退出, 从staker中删除
            // 如果是fulfill_list则可以直接退出(低于5wDBC的将进入fulfill_list，无法抢单,每次惩罚1w)
            StakerList::rm_staker(&mut staker.committee, &who);
            StakerList::rm_staker(&mut staker.fulfill_list, &who);
            StakerList::rm_staker(&mut staker.chill_list, &who);

            Staker::<T>::put(staker);
            let ledger = Self::committee_ledger(&who);
            if let Some(mut ledger) = ledger {
                ledger.total = 0u32.into();
                Self::update_ledger(&who, &ledger);
            }

            CommitteeLedger::<T>::remove(&who);
            Self::deposit_event(Event::ExitFromCandidacy(who));

            return Ok(().into());
        }

        #[pallet::weight(0)]
        pub fn add_black_list(origin: OriginFor<T>, member: T::AccountId) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let mut members = BlackList::<T>::get();

            match members.binary_search(&member) {
                Ok(_) => Err(Error::<T>::AlreadyInBlackList.into()),
                Err(index) => {
                    members.insert(index, member.clone());
                    BlackList::<T>::put(members);
                    Self::deposit_event(Event::AddToBlackList(member));
                    Ok(().into())
                }
            }
        }

        #[pallet::weight(0)]
        pub fn rm_black_list(origin: OriginFor<T>, member: T::AccountId) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let mut members = BlackList::<T>::get();
            match members.binary_search(&member) {
                Ok(index) => {
                    members.remove(index);
                    BlackList::<T>::put(members);
                    Ok(().into())
                }
                Err(_) => Err(Error::<T>::NotInBlackList.into()),
            }
        }


        // // 成为委员会之后，将可以预订订单
        // #[pallet::weight(10000)]
        // pub fn book_one(origin: OriginFor<T>, machine_id: Option<MachineId>) -> DispatchResultWithPostInfo {
        //     let who = ensure_signed(origin)?;

        //     ensure!(Committee::<T>::contains_key(&who), Error::<T>::NotCommittee);
        //     ensure!(Self::committee(&who) == CommitteeStatus::Health, Error::<T>::NotHealthStatus);

        //     let live_machines = <online_profile::Pallet<T>>::live_machines();
        //     if live_machines.bonding_machine.len() == 0 {
        //         return Err(Error::<T>::NoMachineCanBook.into());
        //     }

        //     let machine_id = if let Some(id) = machine_id {
        //         if let Err(_) = live_machines.bonding_machine.binary_search(&id){
        //             return Err(Error::<T>::NoMachineIdFound.into())
        //         }
        //         id
        //     } else {
        //         live_machines.bonding_machine[0].clone()
        //     };

        //     // 将状态设置为已被订阅状态
        //     T::LCOperations::lc_add_booked_machine(machine_id.clone());

        //     let mut user_machines = Self::committee_machine(&who);
        //     if let Err(index) = user_machines.booked_machine.binary_search(&machine_id) {
        //         user_machines.booked_machine.insert(index, machine_id.clone());
        //     }
        //     CommitteeMachine::<T>::insert(&who, user_machines);

        //     let mut machine_users = Self::machine_committee(&machine_id);
        //     if let Err(index) = machine_users.booked_committee.binary_search(&who) {
        //         machine_users.booked_committee.insert(index, who.clone());
        //     }
        //     MachineCommittee::<T>::insert(&machine_id, machine_users);

        //     let mut user_ops = Self::ops_detail(&who, &machine_id);
        //     user_ops.booked_time = <frame_system::Module<T>>::block_number();
        //     OpsDetail::<T>::insert(&who, &machine_id, user_ops);

        //     Ok(().into())
        // }

        // 添加确认hash
        // FIXME: 注意，提交Hash需要检查，不与其他人的/已存在的Hash相同, 否则, 是很严重的作弊行为
        #[pallet::weight(10000)]
        fn add_confirm_hash(origin: OriginFor<T>, machine_id: MachineId, hash: [u8; 16]) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let now = <frame_system::Module<T>>::block_number();

            let mut machine_committee = Self::machine_committee(&machine_id);
            let left_schedule = machine_committee.booked_committee.len();

            // 从机器信息列表中移除所有该委员的任务
            machine_committee.booked_committee = machine_committee.booked_committee.into_iter().filter(|x| x.0 == who.clone()).collect::<Vec<_>>();
            if left_schedule == machine_committee.booked_committee.len() {
                // 不是分配给该机器的用户
                return Err(Error::<T>::NotInBookList.into());
            }

            // 该机器信息中，记录上委员会已经进行了Hash
            if let Err(index) = machine_committee.hashed_committee.binary_search(&who) {
                machine_committee.hashed_committee.insert(index, who.clone());
            }

            // 从委员的任务中，删除该机器的任务
            let mut committee_machine = Self::committee_machine(&who);
            committee_machine.booked_machine = committee_machine.booked_machine.into_iter().filter(|x| &x.0 == &machine_id).collect::<Vec<_>>();

            // 委员会hashedmachine添加上该机器
            if let Err(index) = committee_machine.hashed_machine.binary_search(&machine_id) {
                committee_machine.hashed_machine.insert(index, machine_id.clone());
            }

            // 添加用户对机器的操作记录
            let mut committee_ops = Self::ops_detail(&who, &machine_id);
            committee_ops.machine_status = MachineStatus::Hashed;
            committee_ops.confirm_hash = hash.clone();
            committee_ops.hash_time = now;

            // 更新存储
            MachineCommittee::<T>::insert(&machine_id, machine_committee);
            CommitteeMachine::<T>::insert(&who, committee_machine);
            OpsDetail::<T>::insert(&who, &machine_id, committee_ops);

            Self::deposit_event(Event::AddConfirmHash(who, hash));

            Ok(().into())
        }

        #[pallet::weight(10000)]
        fn submit_confirm_raw(origin: OriginFor<T>, machine_id: MachineId, confirm_raw: Vec<u8>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            // 查询是否已经到了提交hash的时间
            let now = <frame_system::Module<T>>::block_number();
            let mut machine_committee = Self::machine_committee(&machine_id);
            ensure!(now >= machine_committee.confirm_start, Error::<T>::TimeNotAllow);
            ensure!(now <= machine_committee.book_time + (3600u32 / 30 * 48).into(), Error::<T>::TimeNotAllow);

            // // 用户为委员会
            // ensure!(Committee::<T>::contains_key(&who), Error::<T>::NotCommittee);
            // // 需要在committee的booking list中
            // ensure!(
            //     Self::ops_detail(&who, &machine_id).machine_status
            //         == MachineStatus::Hashed,
            //     Error::<T>::NotInBookList
            // );

            // // 确保所有的委员会已经提交了hash, 修改委员会参数
            // // 检查，必须在三个确认Hash都完成之后，才能进行
            // let machine_committee = Self::machine_committee(&machine_id);
            // ensure!(
            //     machine_committee.hashed_committee.len() == 3,
            //     Error::<T>::NotAllHashSubmited
            // );

            // // 保存用户的raw confirm
            // let mut committee_ops = Self::ops_detail(&who, &machine_id);

            // // TODO: 确保还没有提交过raw
            // // 确保raw的hash与原始hash一致
            // ensure!(
            //     Self::hash_is_identical(&confirm_raw, committee_ops.confirm_hash),
            //     Error::<T>::HashIsNotIdentical
            // );

            // committee_ops.machine_status = MachineStatus::Confirmed;
            // committee_ops.confirm_raw = confirm_raw;
            // committee_ops.confirm_time = <frame_system::Module<T>>::block_number();

            // OpsDetail::<T>::insert(&who, &machine_id, committee_ops);

            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        StakeToBeCandidacy(T::AccountId, BalanceOf<T>),
        CommitteeAdded(T::AccountId),
        CommitteeRemoved(T::AccountId),
        CandidacyAdded(T::AccountId),
        CandidacyRemoved(T::AccountId),
        Chill(T::AccountId),
        UndoChill(T::AccountId),
        Withdrawn(T::AccountId, BalanceOf<T>),
        AddToWhiteList(T::AccountId),
        AddToBlackList(T::AccountId),
        ExitFromCandidacy(T::AccountId),
        CommitteeFulfill(BalanceOf<T>),
        AddConfirmHash(T::AccountId, [u8; 16]),
    }

    #[pallet::error]
    pub enum Error<T> {
        AccountAlreadyExist,
        AccountNotExist,
        CandidacyLimitReached,
        AlreadyCandidacy,
        NotCandidacy,
        CommitteeLimitReached,
        AlreadyCommittee,
        NotCommittee,
        MachineGradePriceNotSet,
        CommitteeConfirmedYet,
        StakeNotEnough,
        FreeBalanceNotEnough,
        AlreadyInChillList,
        UserInBlackList,
        AlreadyInWhiteList,
        NotInWhiteList,
        UserInWhiteList,
        AlreadyInBlackList,
        NotInBlackList,
        NotInBookList,
        BookFailed,
        AlreadySubmitHash,
        StatusNotAllowedSubmitHash,
        NotAllHashSubmited,
        HashIsNotIdentical,
        NoMachineCanBook,
        NoMachineIdFound,
        JobNotDone,
        NotHealthStatus,
        NoNeedFulfill,
        NoLedgerFound,
        MinStakeNotFound,
        NotInChillList,
        TimeNotAllow,
    }
}

// #[rustfmt::skip]
impl<T: Config> Pallet<T> {
    fn _hash_is_identical(raw_input: &Vec<u8>, hash: [u8; 16]) -> bool {
        let raw_hash: [u8; 16] = blake2_128(raw_input);
        return raw_hash == hash;
    }

    // 根据DBC价格获得最小质押数量
    fn get_min_stake_amount() -> Option<BalanceOf<T>> {
        let dbc_price = <dbc_price_ocw::Module<T>>::avg_price();
        if let None = dbc_price {
            return None;
        }
        let dbc_price = dbc_price.unwrap();

        let committee_min_stake = Self::committee_min_stake();

        return Some((committee_min_stake / dbc_price).saturated_into());
    }

    // 获取所有新加入的机器，并进行分派给委员会
    fn distribute_machines() {
        let live_machines = <online_profile::Pallet<T>>::live_machines();
        for a_machine_id in live_machines.bonding_machine {
            Self::distribute_one_machine(&a_machine_id);
        }
    }

    fn distribute_one_machine(machine_id: &MachineId) {
        let lucky_committee = Self::lucky_committee();
        if lucky_committee.len() == 0 {
            return;
        }

        let now = <frame_system::Module<T>>::block_number();
        let start_time: Vec<_> = (0..9)
            .map(|x| now + (x * 3600u32 / 30 * 4).into())
            .collect();

        // 给机器信息记录上分配的委员会
        let mut machine_committee = Self::machine_committee(machine_id);
        for (a_committee, &start_time) in lucky_committee.iter().zip(start_time.iter()) {
            machine_committee
                .booked_committee
                .push(((*a_committee).clone(), start_time));
        }

        // 记录开始填写原始确认信息的时间
        let now = <frame_system::Module<T>>::block_number();
        machine_committee.book_time = now;
        machine_committee.confirm_start = now + (3600u32 / 30 * 36).into(); // 添加确认信息时间为分发之后的36小时

        // machine_committee.booked_committee.append(schedule);
        MachineCommittee::<T>::insert(machine_id, machine_committee);

        // 给委员会记录上分配的机器
        for (a_committee, &start_time) in lucky_committee.iter().zip(start_time.iter()) {
            let mut committee_machines = Self::committee_machine(a_committee);

            committee_machines
                .booked_machine
                .push((machine_id.to_vec(), start_time));
            CommitteeMachine::<T>::insert(a_committee, committee_machines);
        }

        T::LCOperations::lc_add_booked_machine(machine_id.clone()); // 最后一步执行这个
    }

    // 分派一个machineId给随机的委员会
    // 返回Distribution个随机顺序的账户列表
    fn lucky_committee() -> Vec<T::AccountId> {
        let staker = Self::staker();
        let mut verify_schedule = Vec::new();

        // 如果委员会数量为0，直接返回空列表
        if staker.committee.len() == 0 {
            return verify_schedule;
        }

        // 每n个区间，委员会循环一次, 这个区间n最大为3，最小为committee.len()
        let lucky_committee_len = if staker.committee.len() < 3 {
            staker.committee.len()
        } else {
            3
        };

        // 选出lucky_committee_len个委员会
        let mut committee = staker.committee.clone();
        let mut lucky_committee = Vec::new();
        for _ in 0..lucky_committee_len {
            let lucky_index =
                <random_num::Module<T>>::random_u32(committee.len() as u32 - 1u32) as usize;
            lucky_committee.push(committee[lucky_index].clone());
            committee.remove(lucky_index);
        }

        let repeat_slot = DISTRIBUTION as usize / lucky_committee_len;
        let extra_slot = DISTRIBUTION as usize % lucky_committee_len;

        for _ in 0..repeat_slot {
            let mut committee = lucky_committee.clone();
            for _ in 0..committee.len() {
                let lucky =
                    <random_num::Module<T>>::random_u32(committee.len() as u32 - 1u32) as usize;
                verify_schedule.push(committee[lucky].clone());
                committee.remove(lucky);
            }
        }

        for _ in 0..extra_slot {
            let mut committee = lucky_committee.clone();
            let lucky = <random_num::Module<T>>::random_u32(committee.len() as u32 - 1u32) as usize;
            verify_schedule.push(committee[lucky].clone());
            committee.remove(lucky);
        }

        verify_schedule
    }

    fn update_ledger(controller: &T::AccountId, ledger: &StakingLedger<BalanceOf<T>>) {
        <T as Config>::Currency::set_lock(
            PALLET_LOCK_ID,
            controller,
            ledger.total,
            WithdrawReasons::all(),
        );
        <CommitteeLedger<T>>::insert(controller, Some(ledger));
    }
}
