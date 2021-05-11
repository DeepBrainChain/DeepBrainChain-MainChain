// 委员会不设置个数限制，满足质押，并且通过议案选举即可。
// 每次机器加入，系统随机选择9组委员会，对应到0~36h，每个委员会分配4个小时。
// 当委员会人数n < 9 时，每n个时间段委员会将都被选上。剩余9%n则随机从n中选择。
// 当委员会n >= 9 时，随机从n中选择9个，且委员会不重复。
// 下一轮选择，与上一轮委员会是否被选择的状态无关。
// 委员会确认机器，会提供三个字段组成的 Hash1 = Hash(机器原始信息, 委员会随机字符串, bool(机器正常与否))
// 最后12个小时，统计委员会结果，多数结果为最终结果。第二次提交信息为： 机器原始信息，委员会随机字符串，bool.
// 验证：1. Hash(机器原始信息) == OCW获取到的机器Hash
//      2. Hash(机器原始信息，委员会随机字符串, bool) == Hash1
// 如果没有人提交信息，则进行新一轮随机派发。

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode, HasCompact};
use frame_support::{
    dispatch::DispatchResult,
    ensure,
    pallet_prelude::*,
    traits::{Currency, Get, LockIdentifier, LockableCurrency, WithdrawReasons},
};
use frame_system::{ensure_root, ensure_signed, pallet_prelude::*};
// use online_profile::types::*;
use online_profile_machine::LCOps;
use sp_io::hashing::blake2_128;
use sp_runtime::{traits::SaturatedConversion, RuntimeDebug};
use sp_std::{collections::vec_deque::VecDeque, prelude::*, str, vec::Vec};

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
pub const Distribution: u32 = 9; // 订单分发9次，在36个小时内

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

// 记录机器从派单到确认的信息
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MachineInfo<AccountId, BlockNumber> {
    pub joined_height: BlockNumber,
    pub booked_committee: Vec<(AccountId, BlockNumber)>, // 记录订阅的用户及验证开始时间
}

// 记录处于不同状态的委员会的列表，方便派单
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct StakerList<AccountId: Ord> {
    pub candidacy: Vec<AccountId>,    // 质押但还未通过社区选举的委员会
    pub committee: Vec<AccountId>,    // 质押并通过社区选举的委员会
    pub chill_list: Vec<AccountId>,   // 委员会，但不想被派单
    pub fulfill_list: Vec<AccountId>, // 委员会, 但需要补交质押
}

impl<AccountId: Ord> StakerList<AccountId> {
    fn staker_exist(&self, who: &AccountId) -> bool {
        if let Ok(_) = self.candidacy.binary_search(who) {
            return true;
        }
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
pub struct CommitteeMachineList {
    pub booked_machine: Vec<MachineId>,
    pub hashed_machine: Vec<MachineId>, // 存储已经提交了Hash信息的机器
    pub confirmed_machine: Vec<MachineId>, // 存储已经提交了原始确认数据的机器
    pub online_machine: Vec<MachineId>, // 存储已经成功上线的机器
}

// 一台机器对应的委员会
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MachineCommitteeList<AccountId> {
    pub booked_committee: Vec<AccountId>,
    pub hashed_committee: Vec<AccountId>,
    pub confirmed_committee: Vec<AccountId>,
    pub onlined_committee: Vec<AccountId>,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct CommitteeMachineOps<BlockNumber> {
    pub booked_time: BlockNumber,
    pub confirm_hash: [u8; 16],
    pub hash_time: BlockNumber,
    pub confirm_raw: Vec<u8>,
    pub confirm_time: BlockNumber,
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
    Candidacy,             // 候补委员会，用户质押可以成为该状态。等待社区投票
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
    pub trait Config: frame_system::Config + online_profile::Config + random_num::Config {
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
        fn on_finalize(block_number: T::BlockNumber) {
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

    // 最小质押默认10wDBC
    /// Minmum stake amount to become candidacy
    #[pallet::storage]
    #[pallet::getter(fn committee_min_stake)]
    pub(super) type CommitteeMinStake<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

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
    pub(super) type CommitteeMachine<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, CommitteeMachineList, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn machine_committee)]
    pub(super) type MachineCommittee<T: Config> = StorageMap<_, Blake2_128Concat, MachineId, MachineCommitteeList<T::AccountId>, ValueQuery>;

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
        /// set min stake to become candidacy
        #[pallet::weight(0)]
        pub fn set_min_stake(origin: OriginFor<T>, value: BalanceOf<T>) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            CommitteeMinStake::<T>::put(value);
            Ok(().into())
        }

        /// user can be candidacy by staking
        #[pallet::weight(10000)]
        pub fn stake_for_candidacy(origin: OriginFor<T>, value: BalanceOf<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            // 质押数量应该不小于最小质押要求
            ensure!(value >= Self::committee_min_stake(), Error::<T>::StakeNotEnough);
            // 检查用户余额
            ensure!(value < <T as Config>::Currency::free_balance(&who), Error::<T>::FreeBalanceNotEnough);

            let mut staker = Self::staker();
            // 确保用户还未加入到本模块
            ensure!(!staker.staker_exist(&who), Error::<T>::AccountAlreadyExist);

            let item = StakingLedger {
                total: value,
                active: value,
            };
            Self::update_ledger(&who, &item);
            StakerList::add_staker(&mut staker.candidacy, who.clone());
            Staker::<T>::put(staker);

            Self::deposit_event(Event::StakeToBeCandidacy(who, value));
            Ok(().into())
        }

        // 该操作由社区决定
        // Root权限，将candidacy中的成员，添加到委员会
        #[pallet::weight(0)]
        pub fn add_committee(origin: OriginFor<T>, member: T::AccountId) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            // Committee::<T>::insert(&member, CommitteeStatus::Health);
            let mut staker = Self::staker();
            if let Ok(index) = staker.candidacy.binary_search(&member) {
                staker.candidacy.remove(index);
            } else {
                return Err(Error::<T>::NotCandidacy.into());
            }

            if let Err(index) = staker.committee.binary_search(&member) {
                staker.committee.insert(index, member.clone());
            } else {
                return Err(Error::<T>::AlreadyCommittee.into());
            }

            Staker::<T>::put(staker);
            Self::deposit_event(Event::CommitteeAdded(member));
            Ok(().into())
        }

        // 用户停止作为委员会，此时，不再进行派单
        #[pallet::weight(10000)]
        pub fn chill(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let mut staker = Self::staker();
            ensure!(staker.staker_exist(&who), Error::<T>::AccountNotExist);

            // 只有committee状态才允许进行chill
            if let Ok(index) = staker.committee.binary_search(&who) {
                staker.committee.remove(index);
            } else {
                return Err(Error::<T>::NotCommittee.into());
            }

            if let Err(index) = staker.chill_list.binary_search(&who) {
                staker.chill_list.insert(index, who.clone());
            }
            Staker::<T>::put(staker);
            Self::deposit_event(Event::Chill(who));

            Ok(().into())
        }

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
            StakerList::rm_staker(&mut staker.candidacy, &who);
            StakerList::rm_staker(&mut staker.committee, &who);
            StakerList::rm_staker(&mut staker.fulfill_list, &who);
            StakerList::rm_staker(&mut staker.chill_list, &who);

            Staker::<T>::put(staker);
            let mut ledger = Self::committee_ledger(&who);
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

        #[pallet::weight(10000)]
        pub fn fill_pledge(origin: OriginFor<T>) -> DispatchResultWithPostInfo{
            let who = ensure_signed(origin)?;

            let mut staker = Self::staker();
            if let Err(_) = staker.fulfill_list.binary_search(&who) {
                return Err(Error::<T>::NoNeedFulfill.into());
            }

            let mut ledger = Self::committee_ledger(&who);
            let committee_stake_need = Self::committee_min_stake();

            if let None = ledger {
                return Err(Error::<T>::NoLedgerFound.into());
            }

            let mut ledger = ledger.unwrap();

            // 检查用户余额
            let needed = committee_stake_need - ledger.total;
            ensure!(needed < <T as Config>::Currency::free_balance(&who), Error::<T>::FreeBalanceNotEnough);

            ledger.total = committee_stake_need;
            Self::update_ledger(&who, &ledger);

            // 从fulfill 移出来，并放到正常委员会列表
            if let Ok(index) = staker.fulfill_list.binary_search(&who) {
                staker.fulfill_list.remove(index);
            }

            if let Err(index) = staker.committee.binary_search(&who) {
                staker.committee.insert(index, who.clone());
            }

            Ok(().into())
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
        #[pallet::weight(10000)]
        fn add_confirm_hash(origin: OriginFor<T>, machine_id: MachineId, hash: [u8; 16]) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            // ensure!(Committee::<T>::contains_key(&who), Error::<T>::NotCommittee);
            // // 2. 没有提交过信息
            // let mut committee_ops_detail = Self::ops_detail(&who, &machine_id);
            // ensure!(
            //     committee_ops_detail.machine_status == MachineStatus::Booked,
            //     Error::<T>::AlreadySubmitHash
            // );

            // // 更改CommitteeMachine, MachineCommittee两个变量
            // let mut committee_machine = Self::committee_machine(&who);
            // if let Ok(index) = committee_machine.booked_machine.binary_search(&machine_id) {
            //     committee_machine.booked_machine.remove(index);

            //     if let Err(index) = committee_machine.hashed_machine.binary_search(&machine_id) {
            //         committee_machine
            //             .hashed_machine
            //             .insert(index, machine_id.clone());
            //     }
            // } else {
            //     return Ok(().into());
            // }
            // CommitteeMachine::<T>::insert(&who, committee_machine);

            // let mut machine_committee = Self::machine_committee(&machine_id);
            // if let Ok(index) = machine_committee.booked_committee.binary_search(&who) {
            //     machine_committee.booked_committee.remove(index);

            //     if let Err(index) = machine_committee.hashed_committee.binary_search(&who) {
            //         machine_committee
            //             .hashed_committee
            //             .insert(index, who.clone());
            //     }
            // } else {
            //     return Ok(().into());
            // }
            // MachineCommittee::<T>::insert(&machine_id, machine_committee);

            // committee_ops_detail.machine_status = MachineStatus::Hashed;
            // committee_ops_detail.hash_time = <frame_system::Module<T>>::block_number();
            // committee_ops_detail.confirm_hash = hash;

            // OpsDetail::<T>::insert(&who, &machine_id, committee_ops_detail);

            Ok(().into())
        }

        #[pallet::weight(10000)]
        fn submit_confirm_raw(origin: OriginFor<T>, machine_id: MachineId, confirm_raw: Vec<u8>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

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
        Withdrawn(T::AccountId, BalanceOf<T>),
        AddToWhiteList(T::AccountId),
        AddToBlackList(T::AccountId),
        ExitFromCandidacy(T::AccountId),
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
    }
}

// #[rustfmt::skip]
impl<T: Config> Pallet<T> {
    fn hash_is_identical(raw_input: &Vec<u8>, hash: [u8; 16]) -> bool {
        let raw_hash: [u8; 16] = blake2_128(raw_input);
        return raw_hash == hash;
    }

    // FIXME: 分派一个machineId给随机的委员会
    fn book_one(machineId: MachineId) {
        let mut staker = Self::staker();
        let mut new_committee = Vec::new();

        if staker.committee.len() < 9 {
            for i in 0..(9 / staker.committee.len()) {
                let mut committee = staker.committee.clone();
                for i in 0..committee.len() {
                    let lucky =
                        <random_num::Module<T>>::random_u32(staker.committee.len() as u32 - 1u32)
                            as usize;
                    new_committee.push(committee[lucky].clone());
                    committee.remove(lucky);
                }
            }

            for i in 0..(9 % staker.committee.len()) {
                let mut committee = staker.committee.clone();
                for i in 0..committee.len() {
                    let lucky =
                        <random_num::Module<T>>::random_u32(staker.committee.len() as u32 - 1u32)
                            as usize;
                    new_committee.push(staker.committee[lucky].clone());
                    staker.committee.remove(lucky);
                }
            }

            return;
        } else {
            for i in 0..9 {
                let lucky_committee =
                    <random_num::Module<T>>::random_u32(staker.committee.len() as u32 - 1u32)
                        as usize;
                new_committee.push(staker.committee[lucky_committee].clone());
                staker.committee.remove(lucky_committee);
            }
        }
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
